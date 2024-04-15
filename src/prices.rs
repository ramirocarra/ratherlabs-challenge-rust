use std::str::FromStr;

use actix_web::{error, get, web, Responder, Result};
use bigdecimal::BigDecimal;
use serde::{Deserialize, Deserializer, Serialize};
use crate::{orderbook::Pair, AppState};

#[derive(Serialize)]
struct TipsResponse {
    bid: [String; 2],
    ask: [String; 2],
}

#[get("/price-tips/{pair}")]
async fn get_price_tips(path: web::Path<String>, data: web::Data<AppState>) -> Result<impl Responder> {
    let pair = match path.into_inner() {
        pair if pair == "BTCUSDT" => Pair::BTCUSDT,
        pair if pair == "ETHUSDT" => Pair::ETHUSDT,
        _ => return Err(error::ErrorBadRequest("Invalid pair")),
    };

    let (bid, ask) = data.binance_client.get_tips(pair).await.unwrap();

    Ok(web::Json(TipsResponse {
        bid: [bid.0.to_string(), bid.1.to_string()],
        ask: [ask.0.to_string(), ask.1.to_string()],
    }))
}

impl<'de> Deserialize<'de> for Pair {
    fn deserialize<D>(deserializer: D) -> Result<Pair, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "BTCUSDT" => Ok(Pair::BTCUSDT),
            "ETHUSDT" => Ok(Pair::ETHUSDT),
            _ => Err(serde::de::Error::custom("invalid pair")),
        }
    }
}

enum Operation {
  Buy,
  Sell,
}

impl<'de> Deserialize<'de> for Operation {
  fn deserialize<D>(deserializer: D) -> Result<Operation, D::Error>
  where
      D: Deserializer<'de>,
  {
      let s = String::deserialize(deserializer)?;
      match s.as_str() {
          "buy" => Ok(Operation::Buy),
          "sell" => Ok(Operation::Sell),
          _ => Err(serde::de::Error::custom("invalid operation")),
      }
  }
}

#[derive(Deserialize)]
struct ExecutionParams {
    pair: Pair,
    operation: Operation,
    amount: String,
}

#[get("/execution-price")]
async fn get_execution_price(info: web::Query<ExecutionParams>, data: web::Data<AppState>) -> Result<String> {
    let depth = match info.operation {
        Operation::Buy => data.binance_client.get_asks(info.pair).await.unwrap(),
        Operation::Sell => data.binance_client.get_bids(info.pair).await.unwrap(),
    };

    let target_amount = BigDecimal::from_str(&info.amount).or_else(|_| Err(error::ErrorBadRequest("Invalid amount")))?;
    let mut remaining = target_amount.clone();
    let mut total_cost = BigDecimal::from(0);
    for (price, amount) in depth.into_iter() {
        if amount < remaining {
            total_cost += price * &amount;
            remaining -= amount;
        } else {
            total_cost += price * remaining;
            break;
        }
    }

    let avg_price = total_cost / target_amount;

    Ok(format!("Average Price: {}", avg_price))
}

pub fn price_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_price_tips).service(get_execution_price);
}
