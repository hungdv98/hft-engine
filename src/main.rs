use hft_engine::core::{LatencyTracker, Price, Quantity, SpscQueue, pin_to_cpu, rdtsc};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== HFT Engine - Phase 1 Demo ===\n");

    demo_fixed_point_types();
    demo_spsc_queue();
    demo_latency_tracking();
    demo_cpu_pinning();
}

fn demo_fixed_point_types() {
    println!("--- Fixed-Point Types ---");

    let price1 = Price::new(100, 2500);
    let price2 = Price::new(50, 7500);

    println!("Price 1: {}", price1);
    println!("Price 2: {}", price2);
    println!("Sum: {}", price1 + price2);
    println!("Difference: {}", price1 - price2);

    let qty1 = Quantity::new(10, 5000);
    let qty2 = Quantity::new(2, 0);

    println!("\nQuantity 1: {}", qty1);
    println!("Quantity 2: {}", qty2);
    println!("Sum: {}", qty1 + qty2);
    println!("Product: {}", qty1 * qty2);
    println!();
}

fn demo_spsc_queue() {
    println!("--- SPSC Queue ---");

    let queue = Arc::new(SpscQueue::new(1024));
    let queue_clone = queue.clone();

    let messages = 100_000;

    let producer = thread::spawn(move || {
        for i in 0..messages {
            while queue_clone.push(i).is_err() {
                std::hint::spin_loop();
            }
        }
        println!("Producer: sent {} messages", messages);
    });

    let consumer = thread::spawn(move || {
        let mut count = 0;
        let mut sum = 0u64;

        while count < messages {
            if let Some(val) = queue.pop() {
                sum += val;
                count += 1;
            } else {
                std::hint::spin_loop();
            }
        }

        println!("Consumer: received {} messages, sum = {}", count, sum);
    });

    producer.join().unwrap();
    consumer.join().unwrap();
    println!();
}

fn demo_latency_tracking() {
    println!("--- Latency Tracking ---");

    let queue = SpscQueue::new(256);
    let tracker = LatencyTracker::new();

    for i in 0..10_000 {
        let start = rdtsc();
        queue.push(i).unwrap();
        let _ = queue.pop();
        let end = rdtsc();

        tracker.record(end - start);
    }

    let stats = tracker.stats();
    println!("SPSC Queue Latency (10000 iterations):");
    println!("  Count: {}", stats.count);
    println!("  Min:   {} cycles", stats.min);
    println!("  Avg:   {} cycles", stats.avg);
    println!("  Max:   {} cycles", stats.max);

    let stats_ns = stats.to_nanos(3.0);
    println!("\nConverted to nanoseconds:");
    println!("  {}", stats_ns);
    println!();
}

fn demo_cpu_pinning() {
    println!("--- CPU Affinity ---");

    let num_cpus = hft_engine::core::thread::num_cpus();
    println!("Available CPUs: {}", num_cpus);

    match pin_to_cpu(0) {
        Ok(_) => println!("Successfully pinned main thread to CPU 0"),
        Err(e) => println!("CPU pinning failed (may be unsupported): {}", e),
    }

    println!("\n=== Phase 1 Demo Complete ===");
}
