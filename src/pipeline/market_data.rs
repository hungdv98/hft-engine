use crate::core::types::{Price, Quantity};
use crate::core::{LatencyTracker, SpscQueue, pin_to_cpu, rdtsc};
use crate::messages::{MAX_LEVELS, MarketEvent, PriceLevel, Side};
use crate::order_book::OrderBook;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct MarketDataConfig {
    pub symbol: u32,
    pub cpu_id: usize,
}

impl Default for MarketDataConfig {
    fn default() -> Self {
        MarketDataConfig {
            symbol: 1,
            cpu_id: 0,
        }
    }
}

pub fn run_market_data(
    config: MarketDataConfig,
    output_queue: Arc<SpscQueue<MarketEvent>>,
    shutdown: Arc<AtomicBool>,
    tracker: Option<Arc<LatencyTracker>>,
) {
    pin_to_cpu(config.cpu_id).expect("Failed to pin market data thread");

    let mut book = OrderBook::new();
    let mut tick_count = 0u64;

    println!("[MarketData] Thread started on CPU {}", config.cpu_id);

    while !shutdown.load(Ordering::Relaxed) {
        let start = rdtsc();

        let (price, qty, side) = generate_mock_tick(tick_count);

        book.update_level(side, price, qty);

        let timestamp = rdtsc();
        let event = if tick_count % 10 == 0 {
            MarketEvent::BookUpdate {
                symbol: config.symbol,
                bids: copy_levels(book.bids()),
                asks: copy_levels(book.asks()),
                timestamp,
            }
        } else {
            MarketEvent::Tick {
                symbol: config.symbol,
                price,
                qty,
                side,
                timestamp,
            }
        };

        while output_queue.push(event).is_err() {
            std::hint::spin_loop();
        }

        if let Some(ref tracker) = tracker {
            let end = rdtsc();
            tracker.record(end - start);
        }

        tick_count += 1;

        if tick_count % 1000 == 0 {
            std::thread::yield_now();
        }

        if tick_count >= 100_000 {
            break;
        }
    }

    println!(
        "[MarketData] Thread stopping. Processed {} ticks",
        tick_count
    );
}

#[inline(always)]
fn generate_mock_tick(tick_count: u64) -> (Price, Quantity, Side) {
    let base_price = 10000;
    let variation = ((tick_count % 100) as i64 - 50) * 5;
    let price_raw = base_price + variation;

    let price = Price::from_raw(price_raw);
    let qty = Quantity::new(10 + (tick_count % 50) as i64, 0);

    let side = if tick_count % 2 == 0 {
        Side::Buy
    } else {
        Side::Sell
    };

    (price, qty, side)
}

#[inline(always)]
fn copy_levels(levels: &[PriceLevel]) -> [PriceLevel; MAX_LEVELS] {
    let mut result = [PriceLevel::empty(); MAX_LEVELS];
    let count = levels.len().min(MAX_LEVELS);
    result[..count].copy_from_slice(&levels[..count]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_tick_generation() {
        let (price, qty, side) = generate_mock_tick(0);
        assert!(price.raw() > 0);
        assert!(qty.raw() > 0);
        assert_eq!(side, Side::Buy);

        let (_, _, side2) = generate_mock_tick(1);
        assert_eq!(side2, Side::Sell);
    }

    #[test]
    fn test_copy_levels() {
        let levels = vec![
            PriceLevel::new(Price::new(100, 0), Quantity::new(10, 0)),
            PriceLevel::new(Price::new(99, 0), Quantity::new(20, 0)),
        ];

        let copied = copy_levels(&levels);
        assert_eq!(copied[0].price, Price::new(100, 0));
        assert_eq!(copied[1].price, Price::new(99, 0));
        assert!(copied[2].is_empty());
    }
}
