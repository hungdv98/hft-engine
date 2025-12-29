# HFT Engine Architecture

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     HFT Engine Architecture                  │
└─────────────────────────────────────────────────────────────┘

    CPU 0              CPU 1              CPU 2              CPU 3
┌──────────┐       ┌──────────┐       ┌──────────┐       ┌──────────┐
│  Market  │       │ Strategy │       │   Risk   │       │  Order   │
│   Data   │──────▶│  Engine  │──────▶│ Controls │──────▶│ Gateway  │
│ (UDP RX) │ SPSC  │          │ SPSC  │          │ SPSC  │ (UDP TX) │
└──────────┘       └──────────┘       └──────────┘       └──────────┘
     │                  │                  │                  │
     └──────────────────┴──────────────────┴──────────────────┘
                      Lock-Free Pipeline
```

## Core Design Principles

### 1. Zero Allocation in Hot Paths
- **Fixed-point types** (`Price`, `Quantity`) use `i64` primitives
- **Stack-allocated structures** for order book and messages
- **Pre-allocated buffers** for SPSC queues (power-of-2 sizing)
- **No dynamic dispatch** in critical paths (no trait objects)

### 2. Lock-Free Communication
- **SPSC queues** between pipeline stages
- **Atomic operations** with `Acquire`/`Release` ordering
- **Cache-line padding** to prevent false sharing
- **No mutexes** in hot paths

### 3. Thread-Per-Core Architecture
- **CPU pinning** to prevent context switches
- **Dedicated threads** for each pipeline stage
- **Busy-polling loops** instead of blocking
- **NUMA awareness** (future: pin memory to local node)

### 4. Explicit Latency Measurement
- **RDTSC** for cycle-accurate timing
- **Per-stage tracking** to identify bottlenecks
- **Tail latency awareness** (p99, p999)
- **Zero-overhead when disabled** (compile-time feature flags)

### 5. Deterministic Execution
- **No async runtime** (no hidden allocations, no scheduler jitter)
- **Synchronous operations** with bounded execution time
- **Replay capability** for exact market condition reproduction
- **Fixed-point arithmetic** (no floating-point edge cases)

## Module Organization

### Phase 1 - Core Infrastructure 

```rust
src/core/
├── types.rs      // Fixed-point Price, Quantity, Timestamp
├── spsc.rs       // Lock-free ring buffer (1.5ns latency)
├── metrics.rs    // RDTSC wrapper, LatencyTracker
└── thread.rs     // CPU pinning utilities
```

### Phase 2 - Data Pipeline (Next)

```rust
src/
├── order_book.rs         // Fixed-depth L2 book
└── pipeline/
    ├── market_data.rs    // UDP → normalized ticks
    ├── strategy.rs       // Trading logic
    ├── risk.rs           // Pre-trade checks
    └── gateway.rs        // Order serialization
```

### Phase 3 - Supporting Infrastructure

```rust
src/
├── replay/
│   ├── reader.rs         // Memory-mapped playback
│   ├── writer.rs         // Record market data
│   └── clock.rs          // Virtual time
└── config/
    ├── risk_limits.toml  // Position/rate limits
    └── network.toml      // Feed endpoints
```

## Data Flow

### Message Types

```rust
// Market Data → Strategy
enum MarketEvent {
    Tick { symbol: u32, price: Price, qty: Quantity, timestamp: Timestamp },
    Trade { symbol: u32, price: Price, qty: Quantity, timestamp: Timestamp },
    BookUpdate { symbol: u32, levels: [PriceLevel; 10] },
}

// Strategy → Risk
enum SignalEvent {
    Buy { symbol: u32, price: Price, qty: Quantity },
    Sell { symbol: u32, price: Price, qty: Quantity },
    Cancel { order_id: u64 },
}

// Risk → Gateway
enum RiskDecision {
    Approve(Order),
    Reject { reason: RejectReason },
}

// Gateway → Exchange
struct Order {
    id: u64,
    symbol: u32,
    price: Price,
    qty: Quantity,
    side: Side,
    timestamp: Timestamp,
}
```

All messages are:
- **Fixed size** (no dynamic strings, use symbol IDs)
- **Copy types** (no ownership transfer overhead)
- **Cache-line aligned** for optimal SPSC performance
- **#[repr(C)]** for predictable layout

### Pipeline Flow

1. **Market Data Thread (CPU 0)**
   - Receive UDP multicast packets
   - Parse binary protocol (< 200ns)
   - Update order book (< 100ns)
   - Push `MarketEvent` to Strategy SPSC queue

2. **Strategy Thread (CPU 1)**
   - Pop `MarketEvent` from queue
   - Run decision logic (< 50ns)
   - Push `SignalEvent` to Risk SPSC queue

3. **Risk Thread (CPU 2)**
   - Pop `SignalEvent` from queue
   - Check position limits, rate limits (< 50ns)
   - Push `RiskDecision` to Gateway SPSC queue

4. **Gateway Thread (CPU 3)**
   - Pop `RiskDecision` from queue
   - Serialize to exchange protocol
   - Send UDP packet

**Total target**: < 1µs tick-to-order

## Memory Layout Considerations

### Cache-Line Optimization

```rust
#[repr(align(64))]
struct ProducerState {
    tail: AtomicUsize,     // Modified by producer only
    _pad: [u8; 56],        // Padding to 64 bytes
}

#[repr(align(64))]
struct ConsumerState {
    head: AtomicUsize,     // Modified by consumer only
    _pad: [u8; 56],
}

// BAD: False sharing - both atomics in same cache line
struct SharedState {
    head: AtomicUsize,     // Modified by consumer
    tail: AtomicUsize,     // Modified by producer
}
```

### NUMA Considerations (Future)

```rust
// Pin thread AND memory to same NUMA node
thread::spawn(move || {
    pin_to_cpu(0);  // CPU 0 on socket 0
    
    // Allocate on local NUMA node
    let queue = numa_alloc_on_node::<SpscQueue>(0);
    // ...
});
```

## Error Handling Strategy

### Hot Path: Crash-Only

```rust
// In hot path: panic on invariant violation
queue.push(event).unwrap();  // Queue should never be full in steady state

// Or use unchecked for max performance (after extensive testing)
unsafe { queue.push_unchecked(event); }
```

### Slow Path: Result Types

```rust
// Initialization, configuration: use Result
fn load_config(path: &Path) -> Result<Config, ConfigError> {
    // Can fail gracefully before trading starts
}
```

## Testing Strategy

### Unit Tests
- Fixed-point arithmetic edge cases
- SPSC queue wraparound, full/empty conditions
- Order book add/modify/delete correctness

### Property-Based Tests
```rust
proptest! {
    #[test]
    fn order_book_maintains_sorted_invariant(ops: Vec<BookOp>) {
        let mut book = OrderBook::new();
        for op in ops {
            book.apply(op);
        }
        assert!(book.is_sorted());
    }
}
```

### Integration Tests
- End-to-end pipeline with mock market data
- Deterministic replay of historical scenarios
- Latency measurement under load

### Benchmarks
- Micro-benchmarks for each component
- Full pipeline throughput tests
- Latency percentile analysis (p50, p99, p999, p9999)

## Performance Monitoring

### Compile-Time Metrics (Feature Flags)

```rust
#[cfg(feature = "metrics")]
fn record_latency(start: Timestamp, stage: Stage) {
    LATENCY_TRACKER[stage].record(rdtsc() - start);
}

#[cfg(not(feature = "metrics"))]
fn record_latency(_start: Timestamp, _stage: Stage) {
    // No-op, zero overhead
}
```

### Runtime Observability

- **Shared memory segment** for metrics export
- **Separate monitoring process** reads metrics
- **Zero I/O in pipeline threads**

```
Pipeline Thread          Monitor Process
      │                        │
      ├─ Write to shmem        │
      │   (lock-free)          │
      │                        ├─ Read from shmem
      │                        ├─ Export to Prometheus
      │                        └─ Generate flamegraphs
```

## Build Configuration

### Development
```bash
cargo build
cargo test
cargo run
```

### Benchmarking
```bash
cargo bench
```

### Production
```bash
# CPU-specific optimizations
RUSTFLAGS="-C target-cpu=native -C opt-level=3" cargo build --release

# Link-time optimization
cargo build --release --config "profile.release.lto=true"

# Profile-guided optimization (advanced)
cargo pgo build
```

### Linux Kernel Tuning
```bash
# Disable CPU frequency scaling
sudo cpupower frequency-set -g performance

# Isolate CPUs 0-3 from scheduler
# Add to kernel cmdline: isolcpus=0-3 nohz_full=0-3

# Increase network buffer sizes
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
```

## Future Enhancements (Roadmap)

### Phase 4 - Advanced Features
- [ ] Multi-instrument support (symbol table)
- [ ] Exchange-specific protocols (FIX, OUCH, etc.)
- [ ] Strategy hot-reload (separate process + IPC)
- [ ] Drop-copy logging (separate thread, batched writes)

### Phase 5 - Kernel Bypass
- [ ] DPDK integration for < 500ns network latency
- [ ] Zero-copy DMA to NIC
- [ ] Custom UDP stack
- [ ] Hardware timestamping

### Phase 6 - FPGA Offload
- [ ] Order book updates in FPGA
- [ ] Strategy logic in hardware
- [ ] Sub-microsecond tick-to-order

---
  
**Next Milestone**: Phase 2 - Order Book & Market Data Pipeline
