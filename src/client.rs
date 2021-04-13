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

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use orderbook::orderbook_aggregator_client::*;
use orderbook::{Empty, Level, Summary};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = OrderbookAggregatorClient::connect("http://[::1]:10000").await?;

     Ok(())
}
