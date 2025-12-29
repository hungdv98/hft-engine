use hft_engine::core::{LatencyTracker, SpscQueue};
use hft_engine::messages::{MarketEvent, RiskDecision, SignalEvent};
use hft_engine::pipeline::{gateway, market_data, risk, strategy};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== HFT Engine - Phase 2 Demo ===\n");
    println!("Starting 4-thread pipeline with lock-free SPSC queues...\n");

    let md_to_strategy = Arc::new(SpscQueue::<MarketEvent>::new(1024));
    let strategy_to_risk = Arc::new(SpscQueue::<SignalEvent>::new(1024));
    let risk_to_gateway = Arc::new(SpscQueue::<RiskDecision>::new(1024));

    let shutdown = Arc::new(AtomicBool::new(false));

    let md_tracker = Arc::new(LatencyTracker::new());
    let strategy_tracker = Arc::new(LatencyTracker::new());
    let risk_tracker = Arc::new(LatencyTracker::new());
    let gateway_tracker = Arc::new(LatencyTracker::new());

    let shutdown1 = shutdown.clone();
    let shutdown2 = shutdown.clone();
    let shutdown3 = shutdown.clone();
    let shutdown4 = shutdown.clone();

    let md_queue = md_to_strategy.clone();
    let strategy_in = md_to_strategy.clone();
    let strategy_out = strategy_to_risk.clone();
    let risk_in = strategy_to_risk.clone();
    let risk_out = risk_to_gateway.clone();
    let gateway_in = risk_to_gateway.clone();

    let md_track = md_tracker.clone();
    let strategy_track = strategy_tracker.clone();
    let risk_track = risk_tracker.clone();
    let gateway_track = gateway_tracker.clone();

    println!("Spawning threads on CPUs 0-3...\n");

    let md_thread = thread::spawn(move || {
        market_data::run_market_data(
            market_data::MarketDataConfig::default(),
            md_queue,
            shutdown1,
            Some(md_track),
        );
    });

    let strategy_thread = thread::spawn(move || {
        strategy::run_strategy(
            strategy::StrategyConfig::default(),
            strategy_in,
            strategy_out,
            shutdown2,
            Some(strategy_track),
        );
    });

    let risk_thread = thread::spawn(move || {
        risk::run_risk(
            risk::RiskConfig::default(),
            risk_in,
            risk_out,
            shutdown3,
            Some(risk_track),
        );
    });

    let gateway_thread = thread::spawn(move || {
        gateway::run_gateway(
            gateway::GatewayConfig::default(),
            gateway_in,
            shutdown4,
            Some(gateway_track),
        );
    });

    println!("Pipeline running...\n");
    thread::sleep(Duration::from_secs(5));

    println!("\nSignaling shutdown...\n");
    shutdown.store(true, Ordering::Relaxed);

    md_thread.join().unwrap();
    strategy_thread.join().unwrap();
    risk_thread.join().unwrap();
    gateway_thread.join().unwrap();

    println!("\n=== Latency Statistics ===\n");

    let md_stats = md_tracker.stats().to_nanos(3.0);
    println!("Market Data:  {}", md_stats);

    let strategy_stats = strategy_tracker.stats().to_nanos(3.0);
    println!("Strategy:     {}", strategy_stats);

    let risk_stats = risk_tracker.stats().to_nanos(3.0);
    println!("Risk:         {}", risk_stats);

    let gateway_stats = gateway_tracker.stats().to_nanos(3.0);
    println!("Gateway:      {}", gateway_stats);

    let total_avg_ns =
        md_stats.avg_ns + strategy_stats.avg_ns + risk_stats.avg_ns + gateway_stats.avg_ns;

    println!("\nEnd-to-End:   {} ns average", total_avg_ns);
    println!("Target:       < 1000 ns (1 µs)");

    if total_avg_ns < 1000 {
        println!("Target achieved!");
    } else {
        println!("Above target");
    }

    println!("\n=== Phase 2 Demo Complete ===");
}
