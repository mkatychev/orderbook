use crate::orderbook;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use serde_json::from_str;
use std::error::Error;
use tokio::sync::mpsc::Sender;
use tungstenite::{connect, Message};
use url::Url;

#[derive(Deserialize, Debug)]
pub struct OrderBatch {
    #[serde(rename(deserialize = "lastUpdateId"))]
    pub last_updated_id: u32,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

#[derive(Debug)]
pub struct Order {
    pub price: f64,
    pub quantity: f64,
}

impl<'de> Deserialize<'de> for Order {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // private
        #[derive(Deserialize)]
        struct OrderHelper(String, String);

        let helper: OrderHelper = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            price: helper.0.parse().map_err(de::Error::custom)?,
            quantity: helper.0.parse().map_err(de::Error::custom)?,
        })
    }
}

pub async fn client(
    tx: Sender<OrderBatch>,
    symbol: &str,
) -> Result<(), Box<dyn Error + 'static + Send + Sync>> {
    dbg!("connecting");
    let (mut socket, response) = connect(Url::parse(
        format!("wss://stream.binance.com:9443/ws/{}@depth20@100ms", symbol).as_ref(),
    )?)?;

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (ref header, _value) in response.headers() {
        println!("* {}", header);
    }

    loop {
        let msg = socket.read_message()?.into_text()?;
        tx.send(from_str::<OrderBatch>(&msg)?).await?;
    }
}
