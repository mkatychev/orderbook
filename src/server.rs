use std::collections::HashMap;
use std::error::Error;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use futures::{Stream, StreamExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use orderbook::orderbook_aggregator_server::*;
use orderbook::{Empty, Level, Summary};

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

#[derive(Debug, Default)]
pub struct AggregatorService {}

#[tonic::async_trait]
impl OrderbookAggregator for AggregatorService {
    type BookSummaryStream = ReceiverStream<Result<Summary, Status>>;

    async fn book_summary(
        &self,
        _: Request<Empty>,
    ) -> Result<Response<Self::BookSummaryStream>, Status> {
        let (mut tx, rx) = mpsc::channel(20);

        tokio::spawn(async move {
            for n in 0i32..20i32 {
                let summary = Summary {
                    spread: n as f64,
                    bids: vec![],
                    asks: vec![],
                };
                tx.send(Ok(summary)).await.unwrap();
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "[::1]:10000".parse().unwrap();

    let aggregator = AggregatorService {};

    let svc = OrderbookAggregatorServer::new(aggregator);

    Server::builder().add_service(svc).serve(addr).await?;

    Ok(())
}
