# Rust HFT Engine

> **A deterministic, lock-free, ultra-low-latency trading engine written in Rust.**
> Designed for microsecond-level decision making, reproducible market replay, and precise latency measurement.

---

## Design Philosophy

This project is built around **HFT engineering principles**, not retail trading patterns:

* **Deterministic execution** over throughput
* **Zero heap allocation in hot paths**
* **Lock-free communication**
* **Thread-per-core architecture**
* **Explicit latency measurement**
* **Crash-only, fail-fast design**

No async runtime. No hidden allocations. No unpredictable schedulers.

---

## Architecture Overview

```
CPU 0 ─ Market Data (UDP)
          │
          ▼
CPU 1 ─ Strategy Engine
          │
          ▼
CPU 2 ─ Risk Controls
          │
          ▼
CPU 3 ─ Order Gateway
```

* **One thread per responsibility**
* **CPU pinned**
* **SPSC lock-free queues between stages**

---

## Components

| Component     | Description                          |
| ------------- | ------------------------------------ |
| `market_data` | Zero-copy UDP multicast feed handler |
| `order_book`  | Fixed-depth L2 order book (no heap)  |
| `strategy`    | Pure, allocation-free decision logic |
| `risk`        | Pre-trade risk & kill switch         |
| `gateway`     | Binary order entry                   |
| `replay`      | Deterministic market replay engine   |
| `metrics`     | `rdtsc`-based latency profiler       |
| `core::spsc`  | Lock-free ring buffers               |

---

## Key Technical Decisions

### No Async Runtime

Async runtimes introduce:

* Scheduler jitter
* Hidden allocations
* Unpredictable wakeups

This engine uses **synchronous busy-polling loops** for deterministic latency.

---

### Lock-Free Data Flow

* Single-Producer / Single-Consumer queues
* No mutexes in hot paths
* Cache-line padded atomics

```rust
MarketData → SPSC → Strategy → SPSC → Risk → SPSC → Gateway
```

---

### Explicit Latency Measurement

* CPU timestamp counter (`rdtsc`)
* Per-stage latency tracking
* Tail-latency aware (p99+)

---

## Performance Targets

| Stage             | Target   |
| ----------------- | -------- |
| Market data parse | < 200 ns |
| Order book update | < 100 ns |
| Strategy decision | < 50 ns  |
| Risk checks       | < 50 ns  |
| Tick → order      | < 1 µs   |

> Actual numbers depend on CPU, NIC, and kernel tuning.

---

## Build & Run

### Requirements

* Rust 1.92.0
* CPU frequency scaling disabled 

### Build

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

---

## Risk Controls

* Max position limits
* Order rate throttling
* Fat-finger protection
* Global kill switch
* Drop-copy logging (off hot path)

> **No order leaves the system without passing risk checks.**

## Roadmap

* [ ] Multi-instrument order books
* [ ] Exchange binary protocol support
* [ ] NUMA-aware memory layout
* [ ] Strategy hot-reload
* [ ] Kernel-bypass experiments (DPDK)

---

## Disclaimer

This project is **for educational and research purposes only**.
It does **not** connect to real exchanges and should **not** be used for live trading.