use crate::core::types::{Price, Quantity, Timestamp};

pub const MAX_LEVELS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Side {
    Buy = 0,
    Sell = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct PriceLevel {
    pub price: Price,
    pub qty: Quantity,
    pub order_count: u32,
    _pad: [u8; 36],
}

impl PriceLevel {
    #[inline(always)]
    pub const fn new(price: Price, qty: Quantity) -> Self {
        PriceLevel {
            price,
            qty,
            order_count: 1,
            _pad: [0; 36],
        }
    }

    #[inline(always)]
    pub const fn empty() -> Self {
        PriceLevel {
            price: Price::new(0, 0),
            qty: Quantity::new(0, 0),
            order_count: 0,
            _pad: [0; 36],
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.order_count == 0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub enum MarketEvent {
    Tick {
        symbol: u32,
        price: Price,
        qty: Quantity,
        side: Side,
        timestamp: Timestamp,
    },

    Trade {
        symbol: u32,
        price: Price,
        qty: Quantity,
        timestamp: Timestamp,
    },

    BookUpdate {
        symbol: u32,
        bids: [PriceLevel; MAX_LEVELS],
        asks: [PriceLevel; MAX_LEVELS],
        timestamp: Timestamp,
    },
}

impl MarketEvent {
    #[inline(always)]
    pub fn symbol(&self) -> u32 {
        match self {
            MarketEvent::Tick { symbol, .. } => *symbol,
            MarketEvent::Trade { symbol, .. } => *symbol,
            MarketEvent::BookUpdate { symbol, .. } => *symbol,
        }
    }

    #[inline(always)]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketEvent::Tick { timestamp, .. } => *timestamp,
            MarketEvent::Trade { timestamp, .. } => *timestamp,
            MarketEvent::BookUpdate { timestamp, .. } => *timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub enum SignalEvent {
    Buy {
        symbol: u32,
        price: Price,
        qty: Quantity,
        timestamp: Timestamp,
    },

    Sell {
        symbol: u32,
        price: Price,
        qty: Quantity,
        timestamp: Timestamp,
    },

    Cancel {
        order_id: u64,
        timestamp: Timestamp,
    },
}

impl SignalEvent {
    #[inline(always)]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            SignalEvent::Buy { timestamp, .. } => *timestamp,
            SignalEvent::Sell { timestamp, .. } => *timestamp,
            SignalEvent::Cancel { timestamp, .. } => *timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct Order {
    pub id: u64,
    pub symbol: u32,
    pub price: Price,
    pub qty: Quantity,
    pub side: Side,
    pub timestamp: Timestamp,
    _pad: [u8; 27],
}

impl Order {
    #[inline(always)]
    pub const fn new(
        id: u64,
        symbol: u32,
        price: Price,
        qty: Quantity,
        side: Side,
        timestamp: Timestamp,
    ) -> Self {
        Order {
            id,
            symbol,
            price,
            qty,
            side,
            timestamp,
            _pad: [0; 27],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RejectReason {
    PositionLimitExceeded = 0,
    RateLimitExceeded = 1,
    InvalidPrice = 2,
    InvalidQuantity = 3,
    UnknownSymbol = 4,
    InternalError = 5,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub enum RiskDecision {
    Approve(Order),

    Reject {
        reason: RejectReason,
        original_signal: SignalEvent,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_level_size() {
        assert_eq!(std::mem::size_of::<PriceLevel>(), 64);
        assert_eq!(std::mem::align_of::<PriceLevel>(), 64);
    }

    #[test]
    fn test_price_level_empty() {
        let level = PriceLevel::empty();
        assert!(level.is_empty());
        assert_eq!(level.order_count, 0);
    }

    #[test]
    fn test_market_event_accessors() {
        let tick = MarketEvent::Tick {
            symbol: 123,
            price: Price::new(100, 0),
            qty: Quantity::new(10, 0),
            side: Side::Buy,
            timestamp: Timestamp::from_cycles(1000),
        };

        assert_eq!(tick.symbol(), 123);
        assert_eq!(tick.timestamp(), Timestamp::from_cycles(1000));
    }

    #[test]
    fn test_order_creation() {
        let order = Order::new(
            1,
            123,
            Price::new(100, 0),
            Quantity::new(10, 0),
            Side::Buy,
            Timestamp::from_cycles(1000),
        );

        assert_eq!(order.id, 1);
        assert_eq!(order.symbol, 123);

        assert_eq!(std::mem::align_of::<Order>(), 64);
        assert!(std::mem::size_of::<Order>() <= 128);
    }
}
