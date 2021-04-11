use tonic::{transport::Server, Request, Response, Status};

use hello_world::{Level, Summary};
use orderbook::{OrderbookAggregatror, OrderbookAggregatrorServer};

pub mod hello_world {
    tonic::include_proto!("orderbook"); // The string specified here must match the proto package name
}
