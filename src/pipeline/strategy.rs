use crate::core::types::{Price, Quantity};
use crate::core::{LatencyTracker, SpscQueue, pin_to_cpu, rdtsc};
use crate::messages::{MarketEvent, Side, SignalEvent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct StrategyConfig {
    pub cpu_id: usize,
    pub spread_threshold: Price,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        StrategyConfig {
            cpu_id: 1,
            spread_threshold: Price::new(0, 5000),
        }
    }
}

pub fn run_strategy(
    config: StrategyConfig,
    input_queue: Arc<SpscQueue<MarketEvent>>,
    output_queue: Arc<SpscQueue<SignalEvent>>,
    shutdown: Arc<AtomicBool>,
    tracker: Option<Arc<LatencyTracker>>,
) {
    pin_to_cpu(config.cpu_id).expect("Failed to pin strategy thread");

    let mut event_count = 0u64;
    let mut signal_count = 0u64;

    println!("[Strategy] Thread started on CPU {}", config.cpu_id);

    let mut best_bid: Option<Price> = None;
    let mut best_ask: Option<Price> = None;

    while !shutdown.load(Ordering::Relaxed) {
        if let Some(event) = input_queue.pop() {
            let start = rdtsc();

            event_count += 1;

            match event {
                MarketEvent::Tick {
                    symbol,
                    price,
                    qty: _,
                    side,
                    timestamp: _,
                } => {
                    match side {
                        Side::Buy => {
                            if best_bid.is_none() || price > best_bid.unwrap() {
                                best_bid = Some(price);
                            }
                        }
                        Side::Sell => {
                            if best_ask.is_none() || price < best_ask.unwrap() {
                                best_ask = Some(price);
                            }
                        }
                    }

                    if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                        let spread = ask - bid;

                        if spread <= config.spread_threshold {
                            let signal = if event_count % 2 == 0 {
                                SignalEvent::Buy {
                                    symbol,
                                    price: ask,
                                    qty: Quantity::new(10, 0),
                                    timestamp: rdtsc(),
                                }
                            } else {
                                SignalEvent::Sell {
                                    symbol,
                                    price: bid,
                                    qty: Quantity::new(10, 0),
                                    timestamp: rdtsc(),
                                }
                            };

                            while output_queue.push(signal).is_err() {
                                std::hint::spin_loop();
                            }

                            signal_count += 1;
                        }
                    }
                }

                MarketEvent::BookUpdate { bids, asks, .. } => {
                    if !bids[0].is_empty() {
                        best_bid = Some(bids[0].price);
                    }
                    if !asks[0].is_empty() {
                        best_ask = Some(asks[0].price);
                    }
                }

                MarketEvent::Trade { .. } => {}
            }

            if let Some(ref tracker) = tracker {
                let end = rdtsc();
                tracker.record(end - start);
            }
        } else {
            std::hint::spin_loop();
        }
    }

    println!(
        "[Strategy] Thread stopping. Processed {} events, generated {} signals",
        event_count, signal_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_config_default() {
        let config = StrategyConfig::default();
        assert_eq!(config.cpu_id, 1);
        assert_eq!(config.spread_threshold, Price::new(0, 5000));
    }
}
