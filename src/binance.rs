use std::io::Error;

use bigdecimal::BigDecimal;
use challenge::orderbook::OrderBookDepth;
use futures_util::StreamExt;
use tokio::{sync::{mpsc, oneshot}, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

mod parsers;

use crate::orderbook::{start_orderbook_manager, OrderBook, OrderbookMessage, Pair};

type WsError = tokio_tungstenite::tungstenite::Error;

pub struct BinanceClient {
    tx: mpsc::UnboundedSender<OrderbookMessage>,
}

const BINANCE_WS: &str = "wss://stream.binance.com:9443/stream?streams=btcusdt@depth/ethusdt@depth";

struct BinancePair {
    symbol: &'static str,
    pair: Pair,
}

const PAIRS: [BinancePair; 2] = [
    BinancePair {
        symbol: "BTCUSDT",
        pair: Pair::BTCUSDT,
    },
    BinancePair {
        symbol: "ETHUSDT",
        pair: Pair::ETHUSDT,
    },
];

impl BinanceClient {
    pub fn new() -> (BinanceClient, JoinHandle<()>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let client = BinanceClient {
            tx: tx.clone(),
        };

        let handle = tokio::spawn(async move {
            BinanceClient::start_orderbook_stream(rx, tx).await.unwrap();
        });

        (client, handle)
    }

    pub async fn get_tips(&self, pair: Pair) -> Result<((BigDecimal, BigDecimal), (BigDecimal, BigDecimal)), Error> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx.send(OrderbookMessage::Tips(pair, resp_tx)).or_else(|_| {
            Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to send message to orderbook manager",
            ))
        })?;

        Ok(resp_rx.await.unwrap().unwrap())
    }

    pub async fn get_bids(&self, pair: Pair) -> Result<OrderBookDepth, Error> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx.send(OrderbookMessage::Bids(pair, resp_tx)).or_else(|_| {
            Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to send message to orderbook manager",
            ))
        })?;

        Ok(resp_rx.await.unwrap())
    }

    pub async fn get_asks(&self, pair: Pair) -> Result<OrderBookDepth, Error> {
      let (resp_tx, resp_rx) = oneshot::channel();
      self.tx.send(OrderbookMessage::Asks(pair, resp_tx)).or_else(|_| {
          Err(Error::new(
              std::io::ErrorKind::Other,
              "Failed to send message to orderbook manager",
          ))
      })?;

      Ok(resp_rx.await.unwrap())
  }

    async fn get_orderbook_snapshot(pair: Pair) -> Result<OrderBook, Error> {
        let binance_pair: &str = PAIRS
            .iter()
            .find(|p| p.pair == pair)
            .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Unknown pair"))?
            .symbol;
        
        let btc_res = reqwest::Client::new()
            .get("https://api.binance.com/api/v3/depth")
            .query(&[("symbol", binance_pair), ("limit", "1000")])
            .send()
            .await
            .or_else(|_| {
                Err(Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to get orderbook",
                ))
            })?;

        let body = btc_res.text().await.or_else(|_| {
            Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to read response body",
            ))
        })?;

        let orderbook = parsers::orderbook_from_binance_json(pair, &body).or_else(|_| {
            Err(Error::new(
                std::io::ErrorKind::Other,
                "Failed to parse orderbook",
            ))
        })?;

        Ok(orderbook)
    }

    async fn start_orderbook_stream(rx: mpsc::UnboundedReceiver<OrderbookMessage>, tx: mpsc::UnboundedSender<OrderbookMessage>) -> Result<(), Error> {
        let (ws_stream, _) = connect_async(BINANCE_WS).await.or_else(|err| {
            let msg = format!("Failed to connect to websocket: {:?}", err.to_string());
            Err(Error::new(std::io::ErrorKind::Other, msg))
        })?;
        let (_, read) = ws_stream.split();

        let handle = tokio::spawn(async move {
            read.for_each(|msg| async {
                handle_ws_message(msg, tx.clone()).await;
            })
            .await;
        });

        let btc_orderbook = BinanceClient::get_orderbook_snapshot(Pair::BTCUSDT).await?;
        println!("BTC orderbook: {:?}", btc_orderbook);
        
        let eth_orderbook = BinanceClient::get_orderbook_snapshot(Pair::ETHUSDT).await?;

        let manager_handle = start_orderbook_manager(btc_orderbook, eth_orderbook, rx);

        handle.await.or_else(|err| {
            println!("Error in websocket stream: {:?}", err);
            Ok::<(), Error>(())
        })?;
        manager_handle.await.or_else(|err| {
            println!("Error in orderbook manager: {:?}", err);
            Ok::<(), Error>(())
        })?;
        Ok(())
    }
}

async fn handle_ws_message(
    msg: Result<Message, WsError>,
    ws_tx: mpsc::UnboundedSender<OrderbookMessage>,
) {
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            println!("Error receiving message: {:?}", e);
            return;
        }
    };

    match msg {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).or_else(|err| {
                Err(Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to parse JSON: {:?}", err)
                ))
            }).unwrap();

            let stream_data = data["data"]
                .as_object()
                .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Invalid stream Data")).unwrap();

            if let Ok((pair, diff)) = parsers::orderbook_diff_from_binance_json(stream_data) {
                ws_tx
                    .send(OrderbookMessage::OrderbookDiff(pair, diff))
                    .unwrap();
            } else {
                println!("Failed to parse orderbook diff: {:?}", text);
            }
        }
        Message::Binary(bin) => {
            println!("Dropping unexpected binary message: {:?}", bin);
        }
        Message::Ping(ping) => {
            println!("Ping: {:?}", ping);
        }
        Message::Pong(pong) => {
            println!("Pong: {:?}", pong);
        }
        Message::Close(close) => {
            println!("Close: {:?}", close);
        }
    }
}
