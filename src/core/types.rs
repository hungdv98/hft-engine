use std::fmt;
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Price(i64);

impl Price {
    const SCALE: i64 = 10_000;

    #[inline(always)]
    pub const fn from_raw(raw: i64) -> Self {
        Price(raw)
    }

    #[inline(always)]
    pub const fn new(integer: i64, fractional: i64) -> Self {
        Price(integer * Self::SCALE + fractional)
    }

    #[inline]
    pub fn from_f64(value: f64) -> Self {
        Price((value * Self::SCALE as f64).round() as i64)
    }

    #[inline(always)]
    pub const fn raw(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn to_f64(&self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }
}

impl Add for Price {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Price(self.0 + rhs.0)
    }
}

impl Sub for Price {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Price(self.0 - rhs.0)
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let integer = self.0 / Self::SCALE;
        let fractional = (self.0 % Self::SCALE).abs();
        write!(f, "{}.{:04}", integer, fractional)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Quantity(i64);

impl Quantity {
    const SCALE: i64 = 10_000;

    #[inline(always)]
    pub const fn from_raw(raw: i64) -> Self {
        Quantity(raw)
    }

    #[inline(always)]
    pub const fn new(integer: i64, fractional: i64) -> Self {
        Quantity(integer * Self::SCALE + fractional)
    }

    #[inline]
    pub fn from_f64(value: f64) -> Self {
        Quantity((value * Self::SCALE as f64).round() as i64)
    }

    #[inline(always)]
    pub const fn raw(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn to_f64(&self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }
}

impl Add for Quantity {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Quantity(self.0 + rhs.0)
    }
}

impl Sub for Quantity {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Quantity(self.0 - rhs.0)
    }
}

impl Mul for Quantity {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Quantity((self.0 * rhs.0) / Self::SCALE)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let integer = self.0 / Self::SCALE;
        let fractional = (self.0 % Self::SCALE).abs();
        write!(f, "{}.{:04}", integer, fractional)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    #[inline(always)]
    pub const fn from_cycles(cycles: u64) -> Self {
        Timestamp(cycles)
    }

    #[inline(always)]
    pub const fn cycles(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub const fn elapsed_since(&self, earlier: Timestamp) -> u64 {
        self.0 - earlier.0
    }
}

impl Sub for Timestamp {
    type Output = u64;

    #[inline(always)]
    fn sub(self, rhs: Self) -> u64 {
        self.0 - rhs.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_creation() {
        let p1 = Price::new(100, 1234);
        assert_eq!(p1.raw(), 1001234);

        let p2 = Price::from_f64(100.1234);
        assert_eq!(p2, p1);
    }

    #[test]
    fn test_price_arithmetic() {
        let p1 = Price::new(100, 0);
        let p2 = Price::new(50, 0);

        assert_eq!(p1 + p2, Price::new(150, 0));
        assert_eq!(p1 - p2, Price::new(50, 0));
    }

    #[test]
    fn test_price_display() {
        let p = Price::new(100, 1234);
        assert_eq!(format!("{}", p), "100.1234");

        let p_neg = Price::new(-50, -500);
        assert_eq!(format!("{}", p_neg), "-50.0500");
    }

    #[test]
    fn test_quantity_arithmetic() {
        let q1 = Quantity::new(10, 0);
        let q2 = Quantity::new(5, 0);

        assert_eq!(q1 + q2, Quantity::new(15, 0));
        assert_eq!(q1 - q2, Quantity::new(5, 0));
    }

    #[test]
    fn test_quantity_multiply() {
        let q1 = Quantity::new(2, 0);
        let q2 = Quantity::new(3, 0);

        assert_eq!(q1 * q2, Quantity::new(6, 0));
    }

    #[test]
    fn test_timestamp_elapsed() {
        let t1 = Timestamp::from_cycles(1000);
        let t2 = Timestamp::from_cycles(1500);

        assert_eq!(t2.elapsed_since(t1), 500);
        assert_eq!(t2 - t1, 500);
    }
}
