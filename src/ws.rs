use futures_util::{SinkExt, StreamExt};
use serde::{de, Deserialize, Deserializer};
use serde_json::from_str;
use std::error::Error;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

// OrderBatch represents the normalised struct meant to be sent for processing by the
// AggregatorSerivce
#[derive(Debug)]
pub struct OrderBatch {
    pub exchange: String,
    pub bids:     Vec<Order>,
    pub asks:     Vec<Order>,
}

#[derive(Deserialize, Debug)]
pub struct BinanceOrderBatch {
    #[serde(skip)]
    pub last_updated_id: u32,
    pub bids:            Vec<Order>,
    pub asks:            Vec<Order>,
}

#[derive(Debug)]
pub struct Order {
    pub price:    f64,
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
            price:    helper.0.parse().map_err(de::Error::custom)?,
            quantity: helper.1.parse().map_err(de::Error::custom)?,
        })
    }
}

pub async fn binance_client(tx: Sender<OrderBatch>, symbol: &str) -> Result<(), Box<dyn Error>> {
    // Use BINANCE_WS_URL env var if set, otherwise default to testnet (main API is geo-restricted)
    let base_url = std::env::var("BINANCE_WS_URL")
        .unwrap_or_else(|_| "wss://stream.testnet.binance.vision/ws".to_string());
    let url = format!("{}/{}@depth20@100ms", base_url, symbol);
    println!("[BINANCE] Connecting to: {}", url);

    let (ws_stream, response) = connect_async(&url).await?;

    println!("[BINANCE] Connected to the server");
    println!("[BINANCE] Response HTTP code: {}", response.status());
    println!("[BINANCE] Response contains the following headers:");
    for (header, _value) in response.headers() {
        println!("* {}", &header);
    }

    let (_write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg?.into_text()?;
        let batch = match from_str::<BinanceOrderBatch>(&msg) {
            Ok(m) => m,
            Err(_) => {
                // sometimes the binance ws sends out the timestamp?
                println!("[BINANCE] non batch message: {}", &msg);
                continue;
            }
        };
        tx.send(OrderBatch {
            exchange: "binance".into(),
            bids:     batch.bids,
            asks:     batch.asks,
        })
        .await?;
    }

    Ok(())
}

#[derive(Deserialize, Debug)]
pub struct BitstampOrderBatch {
    data:    Data,
    #[serde(skip)]
    channel: String,
    #[serde(skip)]
    event:   String,
}

#[derive(Deserialize, Debug)]
pub struct Data {
    #[serde(skip)]
    timestamp:      String,
    #[serde(skip)]
    microtimestamp: String,
    pub bids:       Vec<Order>,
    pub asks:       Vec<Order>,
}

pub async fn bitstamp_client(tx: Sender<OrderBatch>, symbol: &str) -> Result<(), Box<dyn Error>> {
    let (ws_stream, response) = connect_async("wss://ws.bitstamp.net/").await?;

    println!("[BITSTAMP] Connected to the server");
    println!("[BITSTAMP] Response HTTP code: {}", response.status());
    println!("[BITSTAMP] Response contains the following headers:");
    for (header, _value) in response.headers() {
        println!("* {}", &header);
    }

    let (mut write, mut read) = ws_stream.split();

    // https://www.bitstamp.net/websocket/v2/
    let subscribe_msg = format!(
        r#"{{"event":"bts:subscribe","data":{{"channel":"order_book_{}"}}}}"#,
        symbol
    );
    write.send(Message::Text(subscribe_msg.into())).await?;

    while let Some(msg) = read.next().await {
        let msg = msg?.into_text()?;
        let mut batch = match from_str::<BitstampOrderBatch>(&msg) {
            Ok(m) => m,
            Err(_) => {
                // sometimes bitstamp sends subscription confirmations
                println!("[BITSTAMP] non batch message: {}", &msg);
                continue;
            }
        };
        // verified that api sorts bids and asks inversely, cannot limit depth request
        // smallest indices have greatest value for asks and bids
        batch.data.bids.truncate(10);
        batch.data.asks.truncate(10);
        tx.send(OrderBatch {
            exchange: "bitstamp".into(),
            bids:     batch.data.bids,
            asks:     batch.data.asks,
        })
        .await?;
    }

    Ok(())
}
