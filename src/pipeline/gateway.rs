use crate::core::{LatencyTracker, SpscQueue, pin_to_cpu, rdtsc};
use crate::messages::{Order, RiskDecision};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct GatewayConfig {
    pub cpu_id: usize,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        GatewayConfig { cpu_id: 3 }
    }
}

pub fn run_gateway(
    config: GatewayConfig,
    input_queue: Arc<SpscQueue<RiskDecision>>,
    shutdown: Arc<AtomicBool>,
    tracker: Option<Arc<LatencyTracker>>,
) {
    pin_to_cpu(config.cpu_id).expect("Failed to pin gateway thread");

    let mut decision_count = 0u64;
    let mut sent_count = 0u64;
    let mut rejected_count = 0u64;

    println!("[Gateway] Thread started on CPU {}", config.cpu_id);

    while !shutdown.load(Ordering::Relaxed) {
        if let Some(decision) = input_queue.pop() {
            let start = rdtsc();

            decision_count += 1;

            match decision {
                RiskDecision::Approve(order) => {
                    send_order_mock(&order);
                    sent_count += 1;
                }

                RiskDecision::Reject { reason, .. } => {
                    rejected_count += 1;
                    let _ = reason;
                }
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
        "[Gateway] Thread stopping. Processed {} decisions, sent {} orders, rejected {}",
        decision_count, sent_count, rejected_count
    );
}

#[inline(always)]
fn send_order_mock(_order: &Order) {
    std::hint::black_box(_order);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Price, Quantity, Timestamp};
    use crate::messages::Side;

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert_eq!(config.cpu_id, 3);
    }

    #[test]
    fn test_send_order_mock() {
        let order = Order::new(
            1,
            123,
            Price::new(100, 0),
            Quantity::new(10, 0),
            Side::Buy,
            Timestamp::from_cycles(1000),
        );

        send_order_mock(&order);
    }
}
