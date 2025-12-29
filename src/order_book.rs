use crate::core::types::{Price, Quantity};
use crate::messages::{MAX_LEVELS, PriceLevel, Side};

#[derive(Debug, Clone)]
pub struct OrderBook {
    bids: [PriceLevel; MAX_LEVELS],
    asks: [PriceLevel; MAX_LEVELS],
    bid_depth: usize,
    ask_depth: usize,
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook {
            bids: [PriceLevel::empty(); MAX_LEVELS],
            asks: [PriceLevel::empty(); MAX_LEVELS],
            bid_depth: 0,
            ask_depth: 0,
        }
    }

    #[inline(always)]
    pub fn best_bid(&self) -> Option<Price> {
        if self.bid_depth > 0 {
            Some(self.bids[0].price)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn best_ask(&self) -> Option<Price> {
        if self.ask_depth > 0 {
            Some(self.asks[0].price)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn spread(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => {
                let mid_raw = (ask.raw() + bid.raw()) / 2;
                Some(Price::from_raw(mid_raw))
            }
            _ => None,
        }
    }

    #[inline(always)]
    pub fn update_level(&mut self, side: Side, price: Price, qty: Quantity) {
        match side {
            Side::Buy => self.update_bid(price, qty),
            Side::Sell => self.update_ask(price, qty),
        }
    }

    #[inline(always)]
    fn update_bid(&mut self, price: Price, qty: Quantity) {
        let pos = self.find_bid_position(price);

        if qty.raw() == 0 {
            if pos < self.bid_depth && self.bids[pos].price == price {
                self.remove_bid(pos);
            }
        } else {
            if pos < self.bid_depth && self.bids[pos].price == price {
                self.bids[pos].qty = qty;
            } else {
                self.insert_bid(pos, price, qty);
            }
        }
    }

    #[inline(always)]
    fn update_ask(&mut self, price: Price, qty: Quantity) {
        let pos = self.find_ask_position(price);

        if qty.raw() == 0 {
            if pos < self.ask_depth && self.asks[pos].price == price {
                self.remove_ask(pos);
            }
        } else {
            if pos < self.ask_depth && self.asks[pos].price == price {
                self.asks[pos].qty = qty;
            } else {
                self.insert_ask(pos, price, qty);
            }
        }
    }

    #[inline(always)]
    fn find_bid_position(&self, price: Price) -> usize {
        let mut pos = 0;
        while pos < self.bid_depth && self.bids[pos].price > price {
            pos += 1;
        }
        pos
    }

    #[inline(always)]
    fn find_ask_position(&self, price: Price) -> usize {
        let mut pos = 0;
        while pos < self.ask_depth && self.asks[pos].price < price {
            pos += 1;
        }
        pos
    }

    #[inline(always)]
    fn insert_bid(&mut self, pos: usize, price: Price, qty: Quantity) {
        if self.bid_depth >= MAX_LEVELS {
            if pos >= MAX_LEVELS {
                return;
            }
            self.bid_depth = MAX_LEVELS - 1;
        }

        for i in (pos..self.bid_depth).rev() {
            self.bids[i + 1] = self.bids[i];
        }

        self.bids[pos] = PriceLevel::new(price, qty);
        self.bid_depth += 1;
    }

    #[inline(always)]
    fn insert_ask(&mut self, pos: usize, price: Price, qty: Quantity) {
        if self.ask_depth >= MAX_LEVELS {
            if pos >= MAX_LEVELS {
                return;
            }
            self.ask_depth = MAX_LEVELS - 1;
        }

        for i in (pos..self.ask_depth).rev() {
            self.asks[i + 1] = self.asks[i];
        }

        self.asks[pos] = PriceLevel::new(price, qty);
        self.ask_depth += 1;
    }

    #[inline(always)]
    fn remove_bid(&mut self, pos: usize) {
        for i in pos..self.bid_depth - 1 {
            self.bids[i] = self.bids[i + 1];
        }
        self.bid_depth -= 1;
        self.bids[self.bid_depth] = PriceLevel::empty();
    }

    #[inline(always)]
    fn remove_ask(&mut self, pos: usize) {
        for i in pos..self.ask_depth - 1 {
            self.asks[i] = self.asks[i + 1];
        }
        self.ask_depth -= 1;
        self.asks[self.ask_depth] = PriceLevel::empty();
    }

    #[inline(always)]
    pub fn bids(&self) -> &[PriceLevel] {
        &self.bids[..self.bid_depth]
    }

    #[inline(always)]
    pub fn asks(&self) -> &[PriceLevel] {
        &self.asks[..self.ask_depth]
    }

    #[cfg(test)]
    pub fn is_sorted(&self) -> bool {
        for i in 0..self.bid_depth.saturating_sub(1) {
            if self.bids[i].price <= self.bids[i + 1].price {
                return false;
            }
        }

        for i in 0..self.ask_depth.saturating_sub(1) {
            if self.asks[i].price >= self.asks[i + 1].price {
                return false;
            }
        }

        true
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_book() {
        let book = OrderBook::new();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
        assert_eq!(book.mid_price(), None);
    }

    #[test]
    fn test_insert_bids() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        assert_eq!(book.best_bid(), Some(Price::new(100, 0)));
        assert_eq!(book.bid_depth, 1);

        book.update_level(Side::Buy, Price::new(101, 0), Quantity::new(5, 0));
        assert_eq!(book.best_bid(), Some(Price::new(101, 0)));
        assert_eq!(book.bid_depth, 2);
        assert!(book.is_sorted());

        book.update_level(Side::Buy, Price::new(99, 0), Quantity::new(15, 0));
        assert_eq!(book.best_bid(), Some(Price::new(101, 0)));
        assert_eq!(book.bid_depth, 3);
        assert!(book.is_sorted());
    }

    #[test]
    fn test_insert_asks() {
        let mut book = OrderBook::new();

        book.update_level(Side::Sell, Price::new(102, 0), Quantity::new(10, 0));
        assert_eq!(book.best_ask(), Some(Price::new(102, 0)));

        book.update_level(Side::Sell, Price::new(101, 0), Quantity::new(5, 0));
        assert_eq!(book.best_ask(), Some(Price::new(101, 0)));
        assert!(book.is_sorted());

        book.update_level(Side::Sell, Price::new(103, 0), Quantity::new(15, 0));
        assert_eq!(book.best_ask(), Some(Price::new(101, 0)));
        assert_eq!(book.ask_depth, 3);
        assert!(book.is_sorted());
    }

    #[test]
    fn test_update_existing() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        assert_eq!(book.bids()[0].qty, Quantity::new(10, 0));

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(20, 0));
        assert_eq!(book.bids()[0].qty, Quantity::new(20, 0));
        assert_eq!(book.bid_depth, 1);
    }

    #[test]
    fn test_remove_level() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        book.update_level(Side::Buy, Price::new(99, 0), Quantity::new(5, 0));
        assert_eq!(book.bid_depth, 2);

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(0, 0));
        assert_eq!(book.bid_depth, 1);
        assert_eq!(book.best_bid(), Some(Price::new(99, 0)));
    }

    #[test]
    fn test_spread() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        book.update_level(Side::Sell, Price::new(102, 0), Quantity::new(10, 0));

        assert_eq!(book.spread(), Some(Price::new(2, 0)));
    }

    #[test]
    fn test_mid_price() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        book.update_level(Side::Sell, Price::new(102, 0), Quantity::new(10, 0));

        assert_eq!(book.mid_price(), Some(Price::new(101, 0)));
    }

    #[test]
    fn test_max_depth() {
        let mut book = OrderBook::new();

        for i in 0..15 {
            book.update_level(Side::Buy, Price::new(100 - i, 0), Quantity::new(10, 0));
        }

        assert_eq!(book.bid_depth, MAX_LEVELS);
        assert!(book.is_sorted());

        assert_eq!(book.best_bid(), Some(Price::new(100, 0)));
    }

    #[test]
    fn test_complex_operations() {
        let mut book = OrderBook::new();

        book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
        book.update_level(Side::Buy, Price::new(99, 0), Quantity::new(15, 0));
        book.update_level(Side::Buy, Price::new(98, 0), Quantity::new(20, 0));
        book.update_level(Side::Sell, Price::new(101, 0), Quantity::new(10, 0));
        book.update_level(Side::Sell, Price::new(102, 0), Quantity::new(15, 0));

        assert!(book.is_sorted());
        assert_eq!(book.bid_depth, 3);
        assert_eq!(book.ask_depth, 2);

        book.update_level(Side::Buy, Price::new(99, 0), Quantity::new(0, 0));
        assert_eq!(book.bid_depth, 2);
        assert!(book.is_sorted());

        book.update_level(Side::Buy, Price::new(99, 5000), Quantity::new(12, 0));
        assert_eq!(book.bid_depth, 3);
        assert!(book.is_sorted());
    }
}
