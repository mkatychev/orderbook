use std::{collections::HashMap, error::Error, pin::Pin, sync::Arc};

use futures::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, RwLock},
    time,
};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use tonic::{transport::Server, Request, Response, Status};

use crate::ws::*;

mod ws;

pub mod orderbook {
    tonic::include_proto!("orderbook");
    impl Summary {}
}

use orderbook::{orderbook_aggregator_server::*, Empty, Level, Summary};

#[derive(Debug, Default)]
pub struct AggregatorService {
    summary:   Arc<RwLock<Summary>>,
    exchanges: Arc<HashMap<String, RwLock<Summary>>>,
}

#[tonic::async_trait]
impl OrderbookAggregator for AggregatorService {
    type BookSummaryStream =
        Pin<Box<dyn Stream<Item = Result<Summary, Status>> + Unpin + Send + Sync>>;

    async fn book_summary(
        &self,
        _: Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let interval = IntervalStream::new(time::interval(time::Duration::from_millis(400)));

        let stream = interval.map(move |_| Ok::<_, Status>(self.arc_summary()));

        Ok(Response::new(Pin::new(Box::new::(stream.into()))))
        // Ok(Response::new(Box::pin(stream.next().await.ok_or(|x| x).map_err(|e| Status::unknown(e.into()))?))
    }
}

impl AggregatorService {
    pub async fn arc_summary(&self) -> Summary {
        Arc::clone(&self.summary).read().await.clone()
    }
    pub async fn process_batch(
        stale_summary: Arc<RwLock<Summary>>,
        exchanges: Arc<HashMap<String, RwLock<Summary>>>,
        mut stream: ReceiverStream<OrderBatch>,
    ) -> Result<(), Box<dyn Error>> {
        let mut batches = 0u64;
        while let Some(batch) = stream.next().await {
            let bids = batch
                .bids
                .iter()
                .map(|bid| Level {
                    exchange: batch.exchange.clone(),
                    price:    bid.price,
                    amount:   bid.quantity,
                })
                .collect::<Vec<Level>>();

            let asks = batch
                .asks
                .iter()
                .map(|ask| Level {
                    exchange: batch.exchange.clone(),
                    price:    ask.price,
                    amount:   ask.quantity,
                })
                .collect::<Vec<Level>>();

            // batch could contain only asks or only bids, bound check here
            let spread = if !bids.is_empty() && !asks.is_empty() {
                asks[0].price - bids[0].price
            } else {
                0.0
            };

            println!(
                "[{}] batch spread: {}",
                batch.exchange.clone().to_uppercase(),
                spread
            );
            // scope so rwlock can be implicitly dropped
            {
                let mut exchange = exchanges
                    .get(&batch.exchange)
                    .ok_or_else(|| format!("missing exchange name: {}", &batch.exchange))?
                    .write()
                    .await;
                exchange.bids = bids;
                exchange.asks = asks;
            }

            batches += 1;
            // update summary every five batches
            if batches.rem_euclid(5) == 0 {
                Self::update_summary(stale_summary.clone(), exchanges.clone()).await;
                batches = 0;
            }
        }

        Ok(())
    }

    // do not think this needs to be async ¯\_(ツ)_/¯
    pub async fn update_summary(
        stale_summary: Arc<RwLock<Summary>>,
        exchanges: Arc<HashMap<String, RwLock<Summary>>>,
    ) {
        let mut summary = Summary::default();

        // pool all exchange orderbooks into one bloated summary
        for (_k, v) in exchanges.iter() {
            let exchange = v.read().await;
            for bid in exchange.bids.iter() {
                summary.bids.push(bid.clone());
            }
            for ask in exchange.asks.iter() {
                summary.asks.push(ask.clone());
            }
        }

        summary // sort in descending order for bids, we want the highest bids
            .bids
            .sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
        summary // sort in descending order for bids, we want the highest bids
            .asks
            .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

        // // retain the ten best ones
        summary.asks.truncate(10);
        summary.bids.truncate(10);

        // batch could contain only asks or only bids, bound check here
        if !summary.bids.is_empty() && !summary.asks.is_empty() {
            summary.spread = summary.asks[0].price - summary.bids[0].price;
        }

        let mut swap_summary = stale_summary.write().await;
        *swap_summary = summary;
    }
}

const SERVER_ARGS_ERR: &str =
    "error: server must run with these args - 'server --pair <currency-pair>'";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "[::1]:10000".parse().unwrap();

    let args: Vec<String> = std::env::args().collect();
    let curreny_pair = match args.len() {
        3 => {
            if args[1] != "--pair" {
                eprintln!("{}", SERVER_ARGS_ERR);
                std::process::exit(1);
            }
            args[2].to_owned()
        }
        _ => {
            eprintln!("{}", SERVER_ARGS_ERR);
            std::process::exit(1);
        }
    };

    let mut exchanges: HashMap<String, RwLock<Summary>> = HashMap::new();
    exchanges.insert("binance".into(), RwLock::new(Summary::default()));
    exchanges.insert("bitstamp".into(), RwLock::new(Summary::default()));

    let (tx, rx) = mpsc::channel::<OrderBatch>(10);
    let aggregator = AggregatorService {
        exchanges: exchanges.into(),
        ..Default::default()
    };
    let summary_state = Arc::clone(&aggregator.summary);
    let exchange_state = Arc::clone(&aggregator.exchanges);

    let svc = OrderbookAggregatorServer::new(aggregator);

    tokio::select! {
        // exchange clients
        res = async {
            ws::binance_client(tx.clone(),&curreny_pair).await?;
            Ok::<_, Box<dyn Error>>(())
        } => {
            println!("done!");
            res?;
        }

        res = async {
            ws::bitstamp_client(tx.clone(),&curreny_pair).await?;
            Ok::<_, Box<dyn Error>>(())
        } => {
            println!("done!");
            res?;
        }

        // process exchanes
        res = async {
            AggregatorService::process_batch(summary_state.clone(), exchange_state.clone(), ReceiverStream::new(rx)).await?;
            Ok::<_, Box<dyn Error>>(())
        } => {
            res?;
        }

        // gRPC server
        res = async move {
            Server::builder().add_service(svc).serve(addr).await?;
            Ok::<_, Box<dyn Error>>(())
        } => {
            res?;
        }

    }

    // tokio::spawn(async move {
    //     if let Err(e) =
    //         AggregatorService::process_batch(summary_state.clone(), ReceiverStream::new(rx)).await
    //     {
    //         return Err(e);
    //     }
    //     Ok(())
    // });

    // Server::builder().add_service(svc).serve(addr).await?;
    Ok(())
}
