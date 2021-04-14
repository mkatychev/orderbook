use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Write};
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
    let mut stdout = io::stdout();

    let mut stream = client
        .book_summary(Request::new(Empty {}))
        .await?
        .into_inner();

    while let Some(summary) = stream.message().await? {
        print!("\x1B[2J");
        stdout.write_all(format!("SUMMARY = {:#?}", summary).as_bytes())?;
    }
    Ok(())
}
