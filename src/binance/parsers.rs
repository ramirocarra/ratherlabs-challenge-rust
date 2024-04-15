use std::io::Error;

use bigdecimal::BigDecimal;
use serde_json::{Map, Value};
use std::str::FromStr;

use crate::orderbook::{OrderBook, OrderBookDepth, OrderBookDiff, Pair};

use super::PAIRS;

pub fn orderbook_diff_from_binance_json(data: &Map<String, Value>) -> Result<(Pair, OrderBookDiff), Error> {
  let pair = data["s"]
      .as_str()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing pair"))?;
  let pair = PAIRS
      .iter()
      .find(|p| p.symbol == pair)
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Unknown pair"))?
      .pair;

  let first_update_id = data["U"]
      .as_i64()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing firstUpdateId"))?;
  let last_update_id = data["u"]
      .as_i64()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing lastUpdateId"))?;

  let bids = data["b"]
      .as_array()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bids"))?
      .iter()
      .map(|bid| {
          let price = bid[0]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bid price"))?;
          let price = BigDecimal::from_str(price).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse bid price",
              ))
          })?;
          let quantity = bid[1]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bid quantity"))?;
          let quantity = BigDecimal::from_str(quantity).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse bid quantity",
              ))
          })?;
          Ok((price, quantity))
      })
      .collect::<Result<OrderBookDepth, Error>>()?;

  let asks = data["a"]
      .as_array()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing asks"))?
      .iter()
      .map(|ask| {
          let price = ask[0]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing ask price"))?;
          let price = BigDecimal::from_str(price).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse ask price",
              ))
          })?;
          let quantity = ask[1]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing ask quantity"))?;
          let quantity = BigDecimal::from_str(quantity).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse ask quantity",
              ))
          })?;
          Ok((price, quantity))
      })
      .collect::<Result<OrderBookDepth, Error>>()?;

  Ok((
      pair,
      OrderBookDiff {
          bids,
          asks,
          first_update_id,
          last_update_id,
      },
  ))
}

pub fn orderbook_from_binance_json(pair: Pair, json: &str) -> Result<OrderBook, Error> {
  let data: serde_json::Value = serde_json::from_str(json).or_else(|_| {
      Err(Error::new(
          std::io::ErrorKind::Other,
          "Failed to parse JSON",
      ))
  })?;

  let last_update_id = data["lastUpdateId"]
      .as_i64()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing lastUpdateId"))?;

  let bids = data["bids"]
      .as_array()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bids"))?
      .iter()
      .map(|bid| {
          let price = bid[0]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bid price"))?;
          let price = BigDecimal::from_str(price).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse bid price",
              ))
          })?;
          let quantity = bid[1]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing bid quantity"))?;
          let quantity = BigDecimal::from_str(quantity).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse bid quantity",
              ))
          })?;
          Ok((price, quantity))
      })
      .collect::<Result<OrderBookDepth, Error>>()?;

  let asks = data["asks"]
      .as_array()
      .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing asks"))?
      .iter()
      .map(|ask| {
          let price = ask[0]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing ask price"))?;
          let price = BigDecimal::from_str(price).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse ask price",
              ))
          })?;
          let quantity = ask[1]
              .as_str()
              .ok_or_else(|| Error::new(std::io::ErrorKind::Other, "Missing ask quantity"))?;
          let quantity = BigDecimal::from_str(quantity).or_else(|_| {
              Err(Error::new(
                  std::io::ErrorKind::Other,
                  "Failed to parse ask quantity",
              ))
          })?;
          Ok((price, quantity))
      })
      .collect::<Result<OrderBookDepth, Error>>()?;

  Ok(OrderBook::new(pair, bids, asks, last_update_id))
}
