use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use hft_engine::OrderBook;
use hft_engine::core::spsc::SpscQueue;
use hft_engine::core::thread::pin_to_cpu;
use hft_engine::core::types::{Price, Quantity, Timestamp};
use hft_engine::messages::{
    MAX_LEVELS, MarketEvent, Order, PriceLevel, RiskDecision, Side, SignalEvent,
};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

fn copy_levels(levels: &[PriceLevel]) -> [PriceLevel; MAX_LEVELS] {
    let mut result = [PriceLevel::empty(); MAX_LEVELS];
    let count = levels.len().min(MAX_LEVELS);
    result[..count].copy_from_slice(&levels[..count]);
    result
}

fn bench_pipeline_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_single_thread");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("market_to_strategy", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();
            let mut signals = 0;

            for i in 0..1000 {
                let bid = Price::new(10000 + (i % 10) as i64, 0);
                let ask = Price::new(10001 + (i % 10) as i64, 0);

                book.update_level(Side::Buy, bid, Quantity::new(100, 0));
                book.update_level(Side::Sell, ask, Quantity::new(100, 0));

                if let Some(spread) = book.spread() {
                    if spread.raw() < 5000 {
                        signals += 1;
                    }
                }
            }

            black_box(signals);
        });
    });

    group.finish();
}

fn bench_pipeline_two_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_two_threads");
    group.throughput(Throughput::Elements(10000));
    group.sample_size(20);

    group.bench_function("market_strategy_10k", |b| {
        b.iter(|| {
            let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(1024));
            let running = Arc::new(AtomicBool::new(true));

            let m2s = market_to_strategy.clone();
            let run1 = running.clone();

            let strategy_handle = thread::spawn(move || {
                let _ = pin_to_cpu(1);
                let mut count = 0;

                while run1.load(Ordering::Relaxed) {
                    if let Some(event) = m2s.pop() {
                        black_box(event);
                        count += 1;
                        if count >= 10000 {
                            break;
                        }
                    }
                }

                count
            });

            let m2s = market_to_strategy.clone();
            let market_handle = thread::spawn(move || {
                let _ = pin_to_cpu(0);
                let mut book = OrderBook::new();

                for i in 0..10000 {
                    let bid = Price::new(10000 + (i % 10) as i64, 0);
                    let ask = Price::new(10001 + (i % 10) as i64, 0);

                    book.update_level(Side::Buy, bid, Quantity::new(100, 0));
                    book.update_level(Side::Sell, ask, Quantity::new(100, 0));

                    let event = MarketEvent::BookUpdate {
                        symbol: 1,
                        timestamp: Timestamp::from_cycles(unsafe { core::arch::x86_64::_rdtsc() }),
                        bids: copy_levels(book.bids()),
                        asks: copy_levels(book.asks()),
                    };

                    while m2s.push(event).is_err() {
                        std::hint::spin_loop();
                    }
                }
            });

            market_handle.join().unwrap();
            let count = strategy_handle.join().unwrap();
            running.store(false, Ordering::Relaxed);

            black_box(count);
        });
    });

    group.finish();
}

fn bench_pipeline_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_full_chain");
    group.sample_size(10);

    for num_events in [1000, 10000] {
        group.throughput(Throughput::Elements(num_events as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_events),
            &num_events,
            |b, &num_events| {
                b.iter(|| {
                    let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(1024));
                    let strategy_to_risk = Arc::new(SpscQueue::<SignalEvent>::new(256));
                    let risk_to_gateway = Arc::new(SpscQueue::<RiskDecision>::new(256));
                    let running = Arc::new(AtomicBool::new(true));

                    let r2g = risk_to_gateway.clone();
                    let run3 = running.clone();
                    let gateway_handle = thread::spawn(move || {
                        let _ = pin_to_cpu(3);
                        let mut count = 0;

                        while run3.load(Ordering::Relaxed) || r2g.pop().is_some() {
                            if let Some(decision) = r2g.pop() {
                                black_box(decision);
                                count += 1;
                                if count >= num_events {
                                    break;
                                }
                            }
                        }
                        count
                    });

                    let s2r = strategy_to_risk.clone();
                    let r2g = risk_to_gateway.clone();
                    let run2 = running.clone();
                    let risk_handle = thread::spawn(move || {
                        let _ = pin_to_cpu(2);
                        let mut count = 0;

                        while run2.load(Ordering::Relaxed) || s2r.pop().is_some() {
                            if let Some(signal) = s2r.pop() {
                                if let SignalEvent::Cancel { .. } = signal {
                                    continue;
                                }

                                let (price, qty, side, timestamp) = match signal {
                                    SignalEvent::Buy {
                                        price,
                                        qty,
                                        timestamp,
                                        ..
                                    } => (price, qty, Side::Buy, timestamp),
                                    SignalEvent::Sell {
                                        price,
                                        qty,
                                        timestamp,
                                        ..
                                    } => (price, qty, Side::Sell, timestamp),
                                    SignalEvent::Cancel { .. } => unreachable!(),
                                };

                                let decision = RiskDecision::Approve(Order::new(
                                    count as u64,
                                    1,
                                    price,
                                    qty,
                                    side,
                                    timestamp,
                                ));

                                while r2g.push(decision).is_err() {
                                    std::hint::spin_loop();
                                }
                                count += 1;
                            }
                        }
                    });

                    let m2s = market_to_strategy.clone();
                    let s2r = strategy_to_risk.clone();
                    let run1 = running.clone();
                    let strategy_handle = thread::spawn(move || {
                        let _ = pin_to_cpu(1);
                        let mut count = 0;

                        while run1.load(Ordering::Relaxed) || m2s.pop().is_some() {
                            if let Some(event) = m2s.pop() {
                                if let MarketEvent::BookUpdate {
                                    bids, timestamp, ..
                                } = event
                                {
                                    if let Some(bid) = bids.first() {
                                        if bid.qty.raw() > 0 {
                                            let signal = SignalEvent::Buy {
                                                symbol: 1,
                                                timestamp,
                                                price: bid.price,
                                                qty: Quantity::new(10, 0),
                                            };

                                            while s2r.push(signal).is_err() {
                                                std::hint::spin_loop();
                                            }
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    });

                    let m2s = market_to_strategy.clone();
                    let market_handle = thread::spawn(move || {
                        let _ = pin_to_cpu(0);
                        let mut book = OrderBook::new();

                        for i in 0..num_events {
                            let bid = Price::new(10000 + (i % 10) as i64, 0);
                            let ask = Price::new(10001 + (i % 10) as i64, 0);

                            book.update_level(Side::Buy, bid, Quantity::new(100, 0));
                            book.update_level(Side::Sell, ask, Quantity::new(100, 0));

                            let event = MarketEvent::BookUpdate {
                                symbol: 1,
                                timestamp: Timestamp::from_cycles(unsafe {
                                    core::arch::x86_64::_rdtsc()
                                }),
                                bids: copy_levels(book.bids()),
                                asks: copy_levels(book.asks()),
                            };

                            while m2s.push(event).is_err() {
                                std::hint::spin_loop();
                            }
                        }
                    });

                    market_handle.join().unwrap();
                    thread::sleep(Duration::from_millis(10));
                    running.store(false, Ordering::Relaxed);

                    strategy_handle.join().unwrap();
                    risk_handle.join().unwrap();
                    let count = gateway_handle.join().unwrap();

                    black_box(count);
                });
            },
        );
    }

    group.finish();
}

fn bench_pipeline_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_throughput");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);

    group.bench_function("max_throughput_10s", |b| {
        b.iter_custom(|iters| {
            let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(4096));
            let running = Arc::new(AtomicBool::new(true));

            let m2s = market_to_strategy.clone();
            let run1 = running.clone();

            let consumer_handle = thread::spawn(move || {
                let _ = pin_to_cpu(1);
                let mut count = 0u64;

                while run1.load(Ordering::Relaxed) {
                    if m2s.pop().is_some() {
                        count += 1;
                    }
                }
                count
            });

            let m2s = market_to_strategy.clone();
            let run0 = running.clone();
            let start = Instant::now();

            thread::spawn(move || {
                let _ = pin_to_cpu(0);
                let mut book = OrderBook::new();
                let mut i = 0u64;

                while run0.load(Ordering::Relaxed) {
                    let bid = Price::new(10000 + (i % 10) as i64, 0);
                    book.update_level(Side::Buy, bid, Quantity::new(100, 0));

                    let event = MarketEvent::BookUpdate {
                        symbol: 1,
                        timestamp: Timestamp::from_cycles(unsafe { core::arch::x86_64::_rdtsc() }),
                        bids: copy_levels(book.bids()),
                        asks: copy_levels(book.asks()),
                    };

                    if m2s.push(event).is_ok() {
                        i += 1;
                    }
                }
            });

            thread::sleep(Duration::from_millis(100 * iters as u64));
            running.store(false, Ordering::Relaxed);

            let _count = consumer_handle.join().unwrap();
            let elapsed = start.elapsed();

            elapsed
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_single_threaded,
    bench_pipeline_two_threads,
    bench_pipeline_full,
    bench_pipeline_throughput
);
criterion_main!(benches);
