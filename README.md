
## Binance
* Docs for Websocket connection: https://github.com/binance/binance-spot-api-docs/blob/master/web-socket-streams.md
* Example API feed: https://api.binance.com/api/v3/depth?symbol=ETHBTC
* Websocket connection URL for Binance: wss://stream.binance.com:9443/ws/ethbtc@depth20@100ms

## Bitstamp
* Docs: https://www.bitstamp.net/websocket/v2/
* Example API feed: https://www.bitstamp.net/api/v2/order_book/ethbtc/
* Example Websocket usage: https://www.bitstamp.net/s/webapp/examples/order_book_v2.html

```json
{
  "spread": 2.72,
  "asks": [
    { "exchange": "binance", "price": 8491.25, "amount": 0.008 },
    { "exchange": "coinbase", "price": 8496.37, "amount": 0.0303 }
  ],
  "bids": [
    { "exchange": "binance", "price": 8488.53, "amount": 0.002 },
    { "exchange": "kraken", "price": 8484.71, "amount": 1.0959 }
  ]
}
```

* Running server and client:

1. Server: `cargo run --bin orderbook-server`
1. Client: `cargo run --bin orderbook-client`

