use criterion::{criterion_group, criterion_main, Criterion};
use challenge::orderbook::{OrderBook, OrderBookDepth, OrderBookDiff, Pair};
use bigdecimal::BigDecimal;

pub fn criterion_benchmark(c: &mut Criterion) {
    let bids = (0..2000).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();
    let asks = (0..2000).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();

    let mut orderbook = OrderBook::new(Pair::BTCUSDT, bids, asks, 1);

    // Should not update zero diffs that do not exist
    let bids_zero_in_between = (1..2001).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
    let asks_zero_in_between = (1..2001).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
    
    // Should remove half the values
    let bids_zero_half = (0..1000).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();
    let asks_zero_half = (0..1000).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(0))).collect::<OrderBookDepth>();

    // Should add 1500 values, replacing half the existing ones
    let bids_add = (1501..3001).rev().step_by(1).map(|i| (BigDecimal::from(i), BigDecimal::from(2))).collect::<OrderBookDepth>();
    let asks_add = (1500..3000).step_by(1).map(|i| (BigDecimal::from(i), BigDecimal::from(2))).collect::<OrderBookDepth>();

    let mut update_id = 2;
    c.bench_function("diff handler", |b| b.iter(|| {
        orderbook.handle_diff(OrderBookDiff {
            bids: bids_zero_in_between.clone(),
            asks: asks_zero_in_between.clone(),
            first_update_id: update_id,
            last_update_id: update_id+1,
        });
        update_id += 2;

        orderbook.handle_diff(OrderBookDiff {
            bids: bids_zero_half.clone(),
            asks: asks_zero_half.clone(),
            first_update_id: update_id,
            last_update_id: update_id+1,
        });
        update_id += 2;

        orderbook.handle_diff(OrderBookDiff {
            bids: bids_add.clone(),
            asks: asks_add.clone(),
            first_update_id: update_id,
            last_update_id: update_id+1,
        });
        update_id += 2;
    }));
}

pub fn get_tips_benchmark(c: &mut Criterion) {
    let bids = (0..2000).rev().step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();
    let asks = (0..2000).step_by(2).map(|i| (BigDecimal::from(i), BigDecimal::from(1))).collect::<OrderBookDepth>();

    let orderbook = OrderBook::new(Pair::BTCUSDT, bids, asks, 3);

    c.bench_function("get tips", |b| b.iter(|| {
        orderbook.get_tips().unwrap();
    }));
}

criterion_group!(benches, criterion_benchmark, get_tips_benchmark);
criterion_main!(benches);
