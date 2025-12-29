use hft_engine::OrderBook;
use hft_engine::core::spsc::SpscQueue;
use hft_engine::core::types::{Price, Quantity, Timestamp};
use hft_engine::messages::{
    MarketEvent, Order, PriceLevel, RejectReason, RiskDecision, Side, SignalEvent,
};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

const SYMBOL: u32 = 1;

#[test]
fn test_full_pipeline_100k_ticks() {
    const TICK_COUNT: u64 = 1000;

    let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(4096));
    let strategy_to_risk = Arc::new(SpscQueue::<SignalEvent>::new(4096));
    let risk_to_gateway = Arc::new(SpscQueue::<RiskDecision>::new(4096));

    let running = Arc::new(AtomicBool::new(true));
    let ticks_sent = Arc::new(AtomicU64::new(0));
    let signals_generated = Arc::new(AtomicU64::new(0));
    let orders_approved = Arc::new(AtomicU64::new(0));
    let orders_sent = Arc::new(AtomicU64::new(0));

    let r2g = risk_to_gateway.clone();
    let run3 = running.clone();
    let sent = orders_sent.clone();
    let gateway_handle = thread::spawn(move || {
        loop {
            if let Some(decision) = r2g.pop() {
                match decision {
                    RiskDecision::Approve(order) => {
                        assert!(order.qty.raw() > 0, "Order has zero quantity");
                        assert!(order.price.raw() > 0, "Order has invalid price");
                        sent.fetch_add(1, Ordering::Relaxed);
                    }
                    RiskDecision::Reject { .. } => {}
                }
            } else if !run3.load(Ordering::Relaxed) {
                break;
            } else {
                std::thread::yield_now();
            }
        }
    });

    let s2r = strategy_to_risk.clone();
    let r2g = risk_to_gateway.clone();
    let run2 = running.clone();
    let approved = orders_approved.clone();
    let risk_handle = thread::spawn(move || {
        let mut position = 0i64;
        let mut order_id = 1u64;

        loop {
            if let Some(signal) = s2r.pop() {
                let (qty, side, price, timestamp) = match signal {
                    SignalEvent::Buy {
                        qty,
                        price,
                        timestamp,
                        ..
                    } => (qty, Side::Buy, price, timestamp),
                    SignalEvent::Sell {
                        qty,
                        price,
                        timestamp,
                        ..
                    } => (qty, Side::Sell, price, timestamp),
                    SignalEvent::Cancel { .. } => continue,
                };

                let new_position = match side {
                    Side::Buy => position + qty.raw(),
                    Side::Sell => position - qty.raw(),
                };

                let decision = if new_position.abs() <= 1000000 {
                    position = new_position;
                    approved.fetch_add(1, Ordering::Relaxed);

                    RiskDecision::Approve(Order::new(order_id, SYMBOL, price, qty, side, timestamp))
                } else {
                    RiskDecision::Reject {
                        reason: RejectReason::PositionLimitExceeded,
                        original_signal: signal,
                    }
                };

                order_id += 1;

                while r2g.push(decision).is_err() {
                    std::hint::spin_loop();
                }
            } else if !run2.load(Ordering::Relaxed) {
                break;
            } else {
                std::thread::yield_now();
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let s2r = strategy_to_risk.clone();
    let run1 = running.clone();
    let signals = signals_generated.clone();
    let strategy_handle = thread::spawn(move || {
        loop {
            if let Some(event) = m2s.pop() {
                if let MarketEvent::BookUpdate {
                    bids,
                    asks,
                    timestamp,
                    ..
                } = event
                {
                    let best_bid = bids
                        .iter()
                        .find(|level| level.qty.raw() > 0)
                        .map(|level| level.price);
                    let best_ask = asks
                        .iter()
                        .find(|level| level.qty.raw() > 0)
                        .map(|level| level.price);

                    if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                        let spread = ask.raw() - bid.raw();

                        if spread < 5 {
                            let signal = SignalEvent::Buy {
                                symbol: SYMBOL,
                                price: bid,
                                qty: Quantity::new(10, 0),
                                timestamp,
                            };

                            signals.fetch_add(1, Ordering::Relaxed);

                            while s2r.push(signal).is_err() {
                                std::hint::spin_loop();
                            }
                        }
                    }
                }
            } else if !run1.load(Ordering::Relaxed) {
                break;
            } else {
                std::thread::yield_now();
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let ticks = ticks_sent.clone();
    let market_handle = thread::spawn(move || {
        let mut book = OrderBook::new();

        for i in 0..TICK_COUNT {
            let spread_offset = if i % 100 < 20 { 1 } else { 10 };

            let bid = Price::new(10000 + (i % 10) as i64, 0);
            let ask = Price::new(10000 + spread_offset + (i % 10) as i64, 0);

            book.update_level(Side::Buy, bid, Quantity::new(100, 0));
            book.update_level(Side::Sell, ask, Quantity::new(100, 0));

            let mut bids = [PriceLevel::empty(); 10];
            let mut asks = [PriceLevel::empty(); 10];
            let bid_slice = book.bids();
            let ask_slice = book.asks();
            bids[..bid_slice.len()].copy_from_slice(bid_slice);
            asks[..ask_slice.len()].copy_from_slice(ask_slice);

            let event = MarketEvent::BookUpdate {
                symbol: SYMBOL,
                bids,
                asks,
                timestamp: Timestamp::from_cycles(unsafe { core::arch::x86_64::_rdtsc() }),
            };

            while m2s.push(event).is_err() {
                std::hint::spin_loop();
            }

            ticks.fetch_add(1, Ordering::Relaxed);
        }
    });

    market_handle.join().unwrap();

    thread::sleep(Duration::from_secs(2));

    running.store(false, Ordering::Relaxed);

    strategy_handle.join().unwrap();
    risk_handle.join().unwrap();
    gateway_handle.join().unwrap();

    let ticks_count = ticks_sent.load(Ordering::Relaxed);
    let signals_count = signals_generated.load(Ordering::Relaxed);
    let approved_count = orders_approved.load(Ordering::Relaxed);
    let sent_count = orders_sent.load(Ordering::Relaxed);

    println!("Ticks sent: {}", ticks_count);
    println!("Signals generated: {}", signals_count);
    println!("Orders approved: {}", approved_count);
    println!("Orders sent: {}", sent_count);

    assert_eq!(
        ticks_count, TICK_COUNT,
        "Should process exactly {} ticks",
        TICK_COUNT
    );
    assert!(signals_count > 0, "Should generate at least some signals");
    assert!(approved_count > 0, "Should approve at least some orders");
    assert_eq!(
        approved_count, sent_count,
        "All approved orders should be sent"
    );
}

#[test]
fn test_pipeline_message_correctness() {
    let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(16));
    let strategy_to_risk = Arc::new(SpscQueue::<SignalEvent>::new(16));
    let risk_to_gateway = Arc::new(SpscQueue::<RiskDecision>::new(16));

    let running = Arc::new(AtomicBool::new(true));

    let r2g = risk_to_gateway.clone();
    let run3 = running.clone();
    let gateway_handle = thread::spawn(move || {
        let mut orders = Vec::new();

        while run3.load(Ordering::Relaxed) {
            if let Some(decision) = r2g.pop() {
                if let RiskDecision::Approve(order) = decision {
                    orders.push(order);
                    if orders.len() >= 10 {
                        break;
                    }
                }
            }
        }

        orders
    });

    let s2r = strategy_to_risk.clone();
    let r2g = risk_to_gateway.clone();
    let run2 = running.clone();
    let risk_handle = thread::spawn(move || {
        let mut order_id = 1u64;

        while run2.load(Ordering::Relaxed) {
            if let Some(signal) = s2r.pop() {
                let (qty, side, price, timestamp) = match signal {
                    SignalEvent::Buy {
                        qty,
                        price,
                        timestamp,
                        ..
                    } => (qty, Side::Buy, price, timestamp),
                    SignalEvent::Sell {
                        qty,
                        price,
                        timestamp,
                        ..
                    } => (qty, Side::Sell, price, timestamp),
                    SignalEvent::Cancel { .. } => continue,
                };

                let order = Order::new(order_id, SYMBOL, price, qty, side, timestamp);
                order_id += 1;

                while r2g.push(RiskDecision::Approve(order)).is_err() {
                    std::hint::spin_loop();
                }
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let s2r = strategy_to_risk.clone();
    let run1 = running.clone();
    let strategy_handle = thread::spawn(move || {
        while run1.load(Ordering::Relaxed) {
            if let Some(event) = m2s.pop() {
                if let MarketEvent::BookUpdate {
                    bids, timestamp, ..
                } = event
                {
                    if let Some(bid_level) = bids.iter().find(|l| l.qty.raw() > 0) {
                        let signal = SignalEvent::Buy {
                            symbol: SYMBOL,
                            price: bid_level.price,
                            qty: Quantity::new(10, 0),
                            timestamp,
                        };

                        while s2r.push(signal).is_err() {
                            std::hint::spin_loop();
                        }
                    }
                }
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let market_handle = thread::spawn(move || {
        let mut book = OrderBook::new();

        for i in 0..10 {
            let price = Price::new(10000 + i, 0);
            book.update_level(Side::Buy, price, Quantity::new(100, 0));

            let mut bids = [PriceLevel::empty(); 10];
            let mut asks = [PriceLevel::empty(); 10];
            let bid_slice = book.bids();
            let ask_slice = book.asks();
            bids[..bid_slice.len()].copy_from_slice(bid_slice);
            asks[..ask_slice.len()].copy_from_slice(ask_slice);

            let event = MarketEvent::BookUpdate {
                symbol: SYMBOL,
                bids,
                asks,
                timestamp: Timestamp::from_cycles(unsafe { core::arch::x86_64::_rdtsc() }),
            };

            while m2s.push(event).is_err() {
                std::hint::spin_loop();
            }

            thread::sleep(Duration::from_micros(100));
        }
    });

    market_handle.join().unwrap();
    thread::sleep(Duration::from_millis(50));
    running.store(false, Ordering::Relaxed);

    strategy_handle.join().unwrap();
    risk_handle.join().unwrap();
    let orders = gateway_handle.join().unwrap();

    assert_eq!(orders.len(), 10, "Should receive exactly 10 orders");

    for (i, order) in orders.iter().enumerate() {
        assert_eq!(order.id, (i + 1) as u64, "Order IDs should be sequential");
        assert_eq!(order.side, Side::Buy, "All orders should be Buy");
        assert_eq!(
            order.price,
            Price::new(10000 + i as i64, 0),
            "Order price should match market bid"
        );
        assert_eq!(
            order.qty,
            Quantity::new(10, 0),
            "Order quantity should be 10.0"
        );
    }
}

#[test]
fn test_pipeline_risk_rejection() {
    let market_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(16));
    let strategy_to_risk = Arc::new(SpscQueue::<SignalEvent>::new(16));
    let risk_to_gateway = Arc::new(SpscQueue::<RiskDecision>::new(16));

    let running = Arc::new(AtomicBool::new(true));
    let rejections = Arc::new(AtomicU64::new(0));

    let r2g = risk_to_gateway.clone();
    let run3 = running.clone();
    let rej = rejections.clone();
    let gateway_handle = thread::spawn(move || {
        let mut approved = 0;
        let mut rejected = 0;

        while run3.load(Ordering::Relaxed) {
            if let Some(decision) = r2g.pop() {
                match decision {
                    RiskDecision::Approve(_) => approved += 1,
                    RiskDecision::Reject { .. } => rejected += 1,
                }

                if approved + rejected >= 20 {
                    break;
                }
            }
        }

        rej.store(rejected, Ordering::Relaxed);
    });

    let s2r = strategy_to_risk.clone();
    let r2g = risk_to_gateway.clone();
    let run2 = running.clone();
    let risk_handle = thread::spawn(move || {
        let mut order_id = 1u64;
        let mut count = 0;

        while run2.load(Ordering::Relaxed) {
            if let Some(signal) = s2r.pop() {
                let decision = if count % 2 == 0 {
                    let (qty, side, price, timestamp) = match signal {
                        SignalEvent::Buy {
                            qty,
                            price,
                            timestamp,
                            ..
                        } => (qty, Side::Buy, price, timestamp),
                        SignalEvent::Sell {
                            qty,
                            price,
                            timestamp,
                            ..
                        } => (qty, Side::Sell, price, timestamp),
                        SignalEvent::Cancel { .. } => continue,
                    };

                    RiskDecision::Approve(Order::new(order_id, SYMBOL, price, qty, side, timestamp))
                } else {
                    RiskDecision::Reject {
                        reason: RejectReason::PositionLimitExceeded,
                        original_signal: signal,
                    }
                };

                order_id += 1;
                count += 1;

                while r2g.push(decision).is_err() {
                    std::hint::spin_loop();
                }
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let s2r = strategy_to_risk.clone();
    let run1 = running.clone();
    let strategy_handle = thread::spawn(move || {
        let mut count = 0;

        while run1.load(Ordering::Relaxed) {
            if let Some(event) = m2s.pop() {
                if let MarketEvent::BookUpdate {
                    bids, timestamp, ..
                } = event
                {
                    if let Some(bid_level) = bids.iter().find(|l| l.qty.raw() > 0) {
                        let signal = SignalEvent::Buy {
                            symbol: SYMBOL,
                            price: bid_level.price,
                            qty: Quantity::new(10, 0),
                            timestamp,
                        };

                        while s2r.push(signal).is_err() {
                            std::hint::spin_loop();
                        }

                        count += 1;
                        if count >= 20 {
                            break;
                        }
                    }
                }
            }
        }
    });

    let m2s = market_to_strategy.clone();
    let market_handle = thread::spawn(move || {
        let mut book = OrderBook::new();

        for i in 0..20 {
            let price = Price::new(10000 + i, 0);
            book.update_level(Side::Buy, price, Quantity::new(100, 0));

            let mut bids = [PriceLevel::empty(); 10];
            let mut asks = [PriceLevel::empty(); 10];
            let bid_slice = book.bids();
            let ask_slice = book.asks();
            bids[..bid_slice.len()].copy_from_slice(bid_slice);
            asks[..ask_slice.len()].copy_from_slice(ask_slice);

            let event = MarketEvent::BookUpdate {
                symbol: SYMBOL,
                bids,
                asks,
                timestamp: Timestamp::from_cycles(unsafe { core::arch::x86_64::_rdtsc() }),
            };

            while m2s.push(event).is_err() {
                std::hint::spin_loop();
            }

            thread::sleep(Duration::from_micros(100));
        }
    });

    market_handle.join().unwrap();
    thread::sleep(Duration::from_millis(50));
    running.store(false, Ordering::Relaxed);

    strategy_handle.join().unwrap();
    risk_handle.join().unwrap();
    gateway_handle.join().unwrap();

    let rejected = rejections.load(Ordering::Relaxed);
    assert_eq!(rejected, 10, "Should reject exactly half (10/20) signals");
}
