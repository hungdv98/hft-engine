use crate::core::types::Timestamp;
use std::sync::atomic::{AtomicU64, Ordering};

#[inline(always)]
pub fn rdtsc() -> Timestamp {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        unsafe {
            let mut aux: u32 = 0;
            let cycles = core::arch::x86_64::__rdtscp(&mut aux as *mut u32);
            Timestamp::from_cycles(cycles)
        }
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        Timestamp::from_cycles(0)
    }
}

pub struct LatencyTracker {
    count: AtomicU64,
    sum: AtomicU64,
    min: AtomicU64,
    max: AtomicU64,
}

impl LatencyTracker {
    pub const fn new() -> Self {
        LatencyTracker {
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    pub fn record(&self, cycles: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(cycles, Ordering::Relaxed);

        let mut current_min = self.min.load(Ordering::Relaxed);
        while cycles < current_min {
            match self.min.compare_exchange_weak(
                current_min,
                cycles,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        let mut current_max = self.max.load(Ordering::Relaxed);
        while cycles > current_max {
            match self.max.compare_exchange_weak(
                current_max,
                cycles,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    pub fn stats(&self) -> LatencyStats {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed);
        let min = self.min.load(Ordering::Relaxed);
        let max = self.max.load(Ordering::Relaxed);

        let avg = if count > 0 { sum / count } else { 0 };

        LatencyStats {
            count,
            min: if min == u64::MAX { 0 } else { min },
            max,
            avg,
        }
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
        self.min.store(u64::MAX, Ordering::Relaxed);
        self.max.store(0, Ordering::Relaxed);
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LatencyStats {
    pub count: u64,
    pub min: u64,
    pub max: u64,
    pub avg: u64,
}

impl LatencyStats {
    pub fn to_nanos(&self, cpu_ghz: f64) -> LatencyStatsNanos {
        let cycles_per_ns = cpu_ghz;
        LatencyStatsNanos {
            count: self.count,
            min_ns: (self.min as f64 / cycles_per_ns) as u64,
            max_ns: (self.max as f64 / cycles_per_ns) as u64,
            avg_ns: (self.avg as f64 / cycles_per_ns) as u64,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LatencyStatsNanos {
    pub count: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub avg_ns: u64,
}

impl std::fmt::Display for LatencyStatsNanos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "count={} min={}ns avg={}ns max={}ns",
            self.count, self.min_ns, self.avg_ns, self.max_ns
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdtsc_monotonic() {
        let t1 = rdtsc();
        let t2 = rdtsc();

        assert!(t2.cycles() >= t1.cycles() || t2.cycles() == 0);
    }

    #[test]
    fn test_latency_tracker() {
        let tracker = LatencyTracker::new();

        tracker.record(100);
        tracker.record(200);
        tracker.record(50);

        let stats = tracker.stats();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min, 50);
        assert_eq!(stats.max, 200);
        assert_eq!(stats.avg, 116);
    }

    #[test]
    fn test_latency_tracker_reset() {
        let tracker = LatencyTracker::new();

        tracker.record(100);
        tracker.reset();

        let stats = tracker.stats();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.min, 0);
        assert_eq!(stats.max, 0);
    }
}
