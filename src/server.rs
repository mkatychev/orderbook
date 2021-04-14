use std::collections::HashMap;
use std::error::Error;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use async_std::sync::RwLock;
use futures::{Stream, StreamExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use crate::ws::*;

mod ws;

pub mod orderbook {
    tonic::include_proto!("orderbook");
    impl Summary {}
}

use orderbook::orderbook_aggregator_server::*;
use orderbook::{Empty, Level, Summary};

#[derive(Debug, Default)]
pub struct AggregatorService {
    summary: Arc<RwLock<Summary>>,
}

#[tonic::async_trait]
impl OrderbookAggregator for AggregatorService {
    type BookSummaryStream = ReceiverStream<Result<Summary, Status>>;

    async fn book_summary(
        &self,
        _: Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let (mut tx, rx) = mpsc::channel(20);

        let summary_state = Arc::clone(&self.summary);
        tokio::spawn(async move {
            loop {
                let summary = summary_state.read().await;
                tx.send(Ok(summary.clone())).await.unwrap();
                thread::sleep(std::time::Duration::from_secs(2));
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

impl AggregatorService {
    pub async fn process_batch(
        summary: Arc<RwLock<Summary>>,
        mut stream: ReceiverStream<OrderBatch>,
    ) -> Result<(), Box<dyn Error + 'static + Send + Sync>> {
        while let Some(batch) = stream.next().await {
            let mut summary = summary.write().await;
            for bid in batch.bids {
                summary.bids.push(Level {
                    exchange: "default".to_string(),
                    price: bid.price,
                    amount: bid.quantity,
                });
            }
            for ask in batch.asks {
                summary.asks.push(Level {
                    exchange: "default".to_string(),
                    price: ask.price,
                    amount: ask.quantity,
                });
            }
            summary // sort in ascending order for bids
                .bids
                .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
            summary // sort in inverse order for asks
                .asks
                .sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());

            // retain the ten best ones
            summary.asks.truncate(10);
            summary.bids.truncate(10);

            // batch could contain only asks or only bids, bound check here
            if summary.bids.len() >= 1 && summary.asks.len() >= 1 {
                summary.spread = summary.asks[0].price - summary.bids[0].price;
            }
            println!(
                "processed batch with id: {}, current spread: {}",
                batch.last_updated_id, summary.spread,
            );
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "[::1]:10000".parse().unwrap();

    let (tx, rx) = mpsc::channel::<OrderBatch>(20);
    let aggregator = AggregatorService::default();
    let summary_state = Arc::clone(&aggregator.summary);

    let svc = OrderbookAggregatorServer::new(aggregator);

    tokio::spawn(async move {
        if let Err(e) = ws::client(tx.clone(), "etcbtc").await {
            return Err(e);
        }
        Ok(())
    });

    tokio::spawn(async move {
        if let Err(e) =
            AggregatorService::process_batch(summary_state.clone(), ReceiverStream::new(rx)).await
        {
            return Err(e);
        }
        Ok(())
    });

    Server::builder().add_service(svc).serve(addr).await?;
    Ok(())
}
