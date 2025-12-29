use crate::core::types::Quantity;
use crate::core::{LatencyTracker, SpscQueue, pin_to_cpu, rdtsc};
use crate::messages::{Order, RejectReason, RiskDecision, Side, SignalEvent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct RiskConfig {
    pub cpu_id: usize,
    pub max_position: Quantity,
    pub max_orders_per_second: u64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        RiskConfig {
            cpu_id: 2,
            max_position: Quantity::new(1000, 0),
            max_orders_per_second: 100,
        }
    }
}

struct RiskState {
    current_position: Quantity,
    order_count_this_second: u64,
    last_reset_time: u64,
    next_order_id: AtomicU64,
}

impl RiskState {
    fn new() -> Self {
        RiskState {
            current_position: Quantity::new(0, 0),
            order_count_this_second: 0,
            last_reset_time: 0,
            next_order_id: AtomicU64::new(1),
        }
    }

    fn get_next_order_id(&self) -> u64 {
        self.next_order_id.fetch_add(1, Ordering::Relaxed)
    }
}

pub fn run_risk(
    config: RiskConfig,
    input_queue: Arc<SpscQueue<SignalEvent>>,
    output_queue: Arc<SpscQueue<RiskDecision>>,
    shutdown: Arc<AtomicBool>,
    tracker: Option<Arc<LatencyTracker>>,
) {
    pin_to_cpu(config.cpu_id).expect("Failed to pin risk thread");

    let mut state = RiskState::new();
    let mut signal_count = 0u64;
    let mut approved_count = 0u64;
    let mut rejected_count = 0u64;

    println!("[Risk] Thread started on CPU {}", config.cpu_id);

    while !shutdown.load(Ordering::Relaxed) {
        if let Some(signal) = input_queue.pop() {
            let start = rdtsc();

            signal_count += 1;

            let current_time = start.cycles();
            if current_time - state.last_reset_time > 1_000_000_000 {
                state.order_count_this_second = 0;
                state.last_reset_time = current_time;
            }

            let decision = match signal {
                SignalEvent::Buy {
                    symbol,
                    price,
                    qty,
                    timestamp,
                } => {
                    if state.order_count_this_second >= config.max_orders_per_second {
                        rejected_count += 1;
                        RiskDecision::Reject {
                            reason: RejectReason::RateLimitExceeded,
                            original_signal: signal,
                        }
                    } else if state.current_position + qty > config.max_position {
                        rejected_count += 1;
                        RiskDecision::Reject {
                            reason: RejectReason::PositionLimitExceeded,
                            original_signal: signal,
                        }
                    } else {
                        let order = Order::new(
                            state.get_next_order_id(),
                            symbol,
                            price,
                            qty,
                            Side::Buy,
                            timestamp,
                        );

                        state.current_position = state.current_position + qty;
                        state.order_count_this_second += 1;
                        approved_count += 1;

                        RiskDecision::Approve(order)
                    }
                }

                SignalEvent::Sell {
                    symbol,
                    price,
                    qty,
                    timestamp,
                } => {
                    if state.order_count_this_second >= config.max_orders_per_second {
                        rejected_count += 1;
                        RiskDecision::Reject {
                            reason: RejectReason::RateLimitExceeded,
                            original_signal: signal,
                        }
                    } else if state.current_position - qty < Quantity::new(-1000, 0) {
                        rejected_count += 1;
                        RiskDecision::Reject {
                            reason: RejectReason::PositionLimitExceeded,
                            original_signal: signal,
                        }
                    } else {
                        let order = Order::new(
                            state.get_next_order_id(),
                            symbol,
                            price,
                            qty,
                            Side::Sell,
                            timestamp,
                        );

                        state.current_position = state.current_position - qty;
                        state.order_count_this_second += 1;
                        approved_count += 1;

                        RiskDecision::Approve(order)
                    }
                }

                SignalEvent::Cancel {
                    order_id,
                    timestamp,
                } => {
                    let order = Order::new(
                        order_id,
                        0,
                        crate::core::types::Price::new(0, 0),
                        Quantity::new(0, 0),
                        Side::Buy,
                        timestamp,
                    );

                    approved_count += 1;
                    RiskDecision::Approve(order)
                }
            };

            while output_queue.push(decision).is_err() {
                std::hint::spin_loop();
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
        "[Risk] Thread stopping. Processed {} signals, approved {}, rejected {}",
        signal_count, approved_count, rejected_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_config_default() {
        let config = RiskConfig::default();
        assert_eq!(config.cpu_id, 2);
        assert_eq!(config.max_position, Quantity::new(1000, 0));
        assert_eq!(config.max_orders_per_second, 100);
    }

    #[test]
    fn test_risk_state() {
        let state = RiskState::new();
        let id1 = state.get_next_order_id();
        let id2 = state.get_next_order_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
}
