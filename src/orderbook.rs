use bigdecimal::{BigDecimal, Zero};
use tokio::{sync::{mpsc, oneshot}, task::JoinHandle};

type Responder<T> = oneshot::Sender<T>;
pub type OrderBookDepth = Vec<(BigDecimal, BigDecimal)>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pair {
    BTCUSDT,
    ETHUSDT,
}

#[derive(Debug)]
pub struct OrderBook {
    symbol: Pair,
    bids: OrderBookDepth,
    asks: OrderBookDepth,
    last_update_id: i64,
}

impl OrderBook {
    pub fn new(symbol: Pair, bids: OrderBookDepth, asks: OrderBookDepth, last_update_id: i64) -> OrderBook {
        OrderBook {
            symbol,
            bids,
            asks,
            last_update_id,
        }
    }

    pub fn get_tips(&self) -> Result<((BigDecimal, BigDecimal), (BigDecimal, BigDecimal)), std::io::Error> {
        let bid = self
            .bids
            .first()
            .map_or_else(
                || Err(std::io::Error::new(std::io::ErrorKind::Other, "No bids")),
                |(price, amount)| Ok((price.clone(), amount.clone()))
            )?;
        let ask = self
            .asks
            .first()
            .map_or_else(
                || Err(std::io::Error::new(std::io::ErrorKind::Other, "No asks")),
                |(price, amount)| Ok((price.clone(), amount.clone()))
            )?;
        Ok((bid, ask))
    }

    pub fn handle_diff(&mut self, diff: OrderBookDiff) {
        if diff.last_update_id <= self.last_update_id {
            println!("Ignoring diff with last_update_id {} <= {}", diff.last_update_id, self.last_update_id);
            return;
        }

        if diff.first_update_id > self.last_update_id + 1 || diff.last_update_id <= self.last_update_id + 1 {
            panic!("Diff is too far ahead or too far behind: {} -> {} vs {}", diff.first_update_id, diff.last_update_id, self.last_update_id);
        }

        if diff.first_update_id > self.last_update_id + 1 {
            println!("Orderbook might be out of sync, TODO: fetching full orderbook");
        }

        for (price, quantity) in diff.bids.into_iter() {
            let element_pos = self.bids.iter().position(|(p, _)| *p == price);
            if quantity.is_zero() {
                if let Some(pos) = element_pos {
                    self.bids.remove(pos);
                }
            } else {
                if let Some(pos) = element_pos {
                    self.bids[pos] = (price, quantity);
                } else {
                    if price < self.bids.last().map(|(p, _)| p.clone()).unwrap_or_else(|| BigDecimal::zero()) {
                        self.bids.push((price, quantity));
                        continue;
                    } else if price > self.bids.first().map(|(p, _)| p.clone()).unwrap_or_else(|| BigDecimal::zero()) {
                        self.bids.insert(0, (price, quantity));
                        continue;
                    } else {
                        for (i, (p, _)) in self.bids.iter().enumerate() {
                            if *p < price {
                                self.bids.insert(i, (price, quantity));
                                break;
                            }
                        }
                    }
                }
            }
        }

        for (price, quantity) in diff.asks.into_iter() {
            let element_pos = self.asks.iter().position(|(p, _)| *p == price);
            if quantity.is_zero() {
                if let Some(pos) = element_pos {
                    self.asks.remove(pos);
                }
            } else {
                if let Some(pos) = element_pos {
                    self.asks[pos] = (price, quantity);
                } else {
                    if price > self.asks.last().map(|(p, _)| p.clone()).unwrap_or_else(|| BigDecimal::zero()) {
                        self.asks.push((price, quantity));
                        continue;
                    } else if price < self.asks.first().map(|(p, _)| p.clone()).unwrap_or_else(|| BigDecimal::zero()) {
                        self.asks.insert(0, (price, quantity));
                        continue;
                    } else {
                        for (i, (p, _)) in self.asks.iter_mut().enumerate() {
                            if *p > price {
                                self.asks.insert(i, (price, quantity));
                                break;
                            }
                        }
                    }
                }
            }
        }

        self.last_update_id = diff.last_update_id;
        // println!("Updated orderbook for {:?} {}", self.symbol, self.last_update_id);
        // println!("Bids size {}", self.bids.len());
        // println!("Asks size {}", self.asks.len());
    }
}

#[derive(Debug)]
pub enum OrderbookMessage {
    OrderbookDiff(Pair, OrderBookDiff),
    Tips(Pair, Responder<Result<((BigDecimal, BigDecimal), (BigDecimal, BigDecimal)), std::io::Error>>),
    Bids(Pair, Responder<OrderBookDepth>),
    Asks(Pair, Responder<OrderBookDepth>),
}

pub struct OrderbookManager {
    orderbooks: [Option<OrderBook>; 2],
}

#[derive(Debug)]
pub struct OrderBookDiff {
    pub bids: OrderBookDepth,
    pub asks: OrderBookDepth,
    pub first_update_id: i64,
    pub last_update_id: i64,
}

pub fn start_orderbook_manager(orderbook_btc: OrderBook, orderbook_eth: OrderBook, mut rx: mpsc::UnboundedReceiver<OrderbookMessage>) -> JoinHandle<()> {
    return tokio::spawn(async move {
        let mut state =     OrderbookManager {
            orderbooks: [Some(orderbook_btc), Some(orderbook_eth)],
        };

        while let Some(msg) = rx.recv().await {
            match msg {
                OrderbookMessage::OrderbookDiff(pair, diff) => {
                    match pair {
                        Pair::BTCUSDT => {
                            if let Some(orderbook) = &mut state.orderbooks[0] {
                                orderbook.handle_diff(diff);
                            }
                        },
                        Pair::ETHUSDT => {
                            if let Some(orderbook) = &mut state.orderbooks[1] {
                                orderbook.handle_diff(diff);
                            }
                        },
                    }
                },
                OrderbookMessage::Tips(pair, resp) => {
                    match pair {
                        Pair::BTCUSDT => {
                            if let Some(orderbook) = &state.orderbooks[0] {
                                let _ = resp.send(orderbook.get_tips());
                            }
                        },
                        Pair::ETHUSDT => {
                            if let Some(orderbook) = &state.orderbooks[1] {
                                let _ = resp.send(orderbook.get_tips());
                            }
                        },
                    }
                },
                OrderbookMessage::Bids(pair, resp) => {
                    match pair {
                        Pair::BTCUSDT => {
                            if let Some(orderbook) = &state.orderbooks[0] {
                                let bids = orderbook.bids.clone();
                                let _ = resp.send(bids);
                            }
                        },
                        Pair::ETHUSDT => {
                            if let Some(orderbook) = &state.orderbooks[1] {
                                let bids = orderbook.bids.clone();
                                let _ = resp.send(bids);
                            }
                        },
                    }
                },
                OrderbookMessage::Asks(pair, resp) => {
                    match pair {
                        Pair::BTCUSDT => {
                            if let Some(orderbook) = &state.orderbooks[0] {
                                let asks = orderbook.asks.clone();
                                let _ = resp.send(asks);
                            }
                        },
                        Pair::ETHUSDT => {
                            if let Some(orderbook) = &state.orderbooks[1] {
                                let asks = orderbook.asks.clone();
                                let _ = resp.send(asks);
                            }
                        },
                    }
                },
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use bigdecimal::BigDecimal;
    use crate::orderbook::OrderBookDepth;

    use super::{OrderBook, OrderBookDiff, Pair};

    #[test]
    fn test_bulk_values() {
        let bids = (0..2000).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();
        let asks = (0..2000).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();

        let mut orderbook = OrderBook::new(Pair::BTCUSDT, bids, asks, 2);
        assert_eq!(orderbook.bids.len(), 1000);
        assert_eq!(orderbook.asks.len(), 1000);
        // Should be ordered correctly
        assert_eq!(orderbook.bids.first().unwrap().0, BigDecimal::from(1999));
        assert_eq!(orderbook.bids.last().unwrap().0, BigDecimal::from(1));

        // Should not update zero diffs that do not exist
        let bids_zero_in_between = (1..2001).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
        let asks_zero_in_between = (1..2001).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
        orderbook.handle_diff(OrderBookDiff {
            bids: bids_zero_in_between,
            asks: asks_zero_in_between,
            first_update_id: 3,
            last_update_id: 4,
        });
        assert_eq!(orderbook.bids.len(), 1000);
        assert_eq!(orderbook.asks.len(), 1000);

        // Should remove half the values
        let bids_zero_half = (0..1000).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
        let asks_zero_half = (0..1000).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
        orderbook.handle_diff(OrderBookDiff {
            bids: bids_zero_half,
            asks: asks_zero_half,
            first_update_id: 5,
            last_update_id: 6,
        });
        assert_eq!(orderbook.bids.len(), 500);
        assert_eq!(orderbook.asks.len(), 500);

        // Should add 1500 values, replacing half the existing ones
        let bids_add = (1501..3001).rev().step_by(1).map(|i| (BigDecimal::from(i), BigDecimal::from(2))).collect::<OrderBookDepth>();
        let asks_add = (1500..3000).step_by(1).map(|i| (BigDecimal::from(i), BigDecimal::from(2))).collect::<OrderBookDepth>();
        orderbook.handle_diff(OrderBookDiff {
            bids: bids_add,
            asks: asks_add,
            first_update_id: 7,
            last_update_id: 8,
        });
        assert_eq!(orderbook.bids.len(), 1750);
        assert_eq!(orderbook.asks.len(), 1750);

        // Amounts should sum 3250
        let total_bids_amount: BigDecimal = orderbook.bids.iter().map(|(_, amount)| amount).sum();
        assert_eq!(total_bids_amount, BigDecimal::from(3250));
        let total_asks_amount: BigDecimal = orderbook.asks.iter().map(|(_, amount)| amount).sum();
        assert_eq!(total_asks_amount, BigDecimal::from(3250));

        // should be correctly ordered
        let bids_prices: Vec<BigDecimal> = orderbook.bids.iter().map(|(price, _)| price.clone()).collect();
        let mut bids_prices_sorted: Vec<BigDecimal> = bids_prices.clone();
        bids_prices_sorted.sort();
        bids_prices_sorted.reverse();
        assert_eq!(bids_prices, bids_prices_sorted);

        let asks_prices: Vec<BigDecimal> = orderbook.asks.iter().map(|(price, _)| price.clone()).collect();
        let mut asks_prices_sorted: Vec<BigDecimal> = asks_prices.clone();
        asks_prices_sorted.sort();
        assert_eq!(asks_prices, asks_prices_sorted);

    }

    #[test]
    fn correct_diff_handling() {
        let bids = vec![(BigDecimal::from(5), BigDecimal::from(5)), (BigDecimal::from(4), BigDecimal::from(4))];
        let asks = vec![(BigDecimal::from(1), BigDecimal::from(1)), (BigDecimal::from(2), BigDecimal::from(2))];

        let mut orderbook = OrderBook::new(Pair::BTCUSDT, bids, asks, 2);

        assert_eq!(orderbook.bids, vec![(BigDecimal::from(5), BigDecimal::from(5)), (BigDecimal::from(4), BigDecimal::from(4))]);
        assert_eq!(orderbook.asks, vec![(BigDecimal::from(1), BigDecimal::from(1)), (BigDecimal::from(2), BigDecimal::from(2))]);
        assert_eq!(orderbook.last_update_id, 2);

        orderbook.handle_diff(OrderBookDiff {
            bids: vec![(BigDecimal::from(5), BigDecimal::from(0)), (BigDecimal::from(4), BigDecimal::from(5))],
            asks: vec![(BigDecimal::from(1), BigDecimal::from(2)), (BigDecimal::from(2), BigDecimal::from(0))],
            first_update_id: 3,
            last_update_id: 7,
        });

        assert_eq!(orderbook.bids, vec![(BigDecimal::from(4), BigDecimal::from(5))]);
        assert_eq!(orderbook.asks, vec![(BigDecimal::from(1), BigDecimal::from(2))]);
        assert_eq!(orderbook.last_update_id, 7);

        orderbook.handle_diff(OrderBookDiff {
            bids: vec![(BigDecimal::from(6), BigDecimal::from(6)), (BigDecimal::from(5), BigDecimal::from(6)), (BigDecimal::from(3), BigDecimal::from(4))],
            asks: vec![(BigDecimal::from(1), BigDecimal::from(3)), (BigDecimal::from(2), BigDecimal::from(3)), (BigDecimal::from(3), BigDecimal::from(4))],
            first_update_id: 8,
            last_update_id: 10,
        });
        
        assert_eq!(orderbook.bids, vec![(BigDecimal::from(6), BigDecimal::from(6)), (BigDecimal::from(5), BigDecimal::from(6)), (BigDecimal::from(4), BigDecimal::from(5)), (BigDecimal::from(3), BigDecimal::from(4))]);
        assert_eq!(orderbook.asks, vec![(BigDecimal::from(1), BigDecimal::from(3)), (BigDecimal::from(2), BigDecimal::from(3)), (BigDecimal::from(3), BigDecimal::from(4))]);
        assert_eq!(orderbook.last_update_id, 10);
    }

    #[test]
    #[should_panic(expected = "Diff is too far ahead or too far behind")]
    fn panic_not_consecutive_ids() {
        let bids = vec![(BigDecimal::from(5), BigDecimal::from(5)), (BigDecimal::from(4), BigDecimal::from(4))];
        let asks = vec![(BigDecimal::from(1), BigDecimal::from(1)), (BigDecimal::from(2), BigDecimal::from(2))];

        let mut orderbook = OrderBook::new(Pair::BTCUSDT, bids, asks, 2);

        assert_eq!(orderbook.bids, vec![(BigDecimal::from(5), BigDecimal::from(5)), (BigDecimal::from(4), BigDecimal::from(4))]);
        assert_eq!(orderbook.asks, vec![(BigDecimal::from(1), BigDecimal::from(1)), (BigDecimal::from(2), BigDecimal::from(2))]);
        assert_eq!(orderbook.last_update_id, 2);

        orderbook.handle_diff(OrderBookDiff {
            bids: vec![(BigDecimal::from(5), BigDecimal::from(0)), (BigDecimal::from(4), BigDecimal::from(5))],
            asks: vec![(BigDecimal::from(1), BigDecimal::from(2)), (BigDecimal::from(2), BigDecimal::from(0))],
            first_update_id: 4,
            last_update_id: 7,
        });
    }
}
