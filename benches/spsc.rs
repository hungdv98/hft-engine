use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use hft_engine::core::{SpscQueue, rdtsc};
use std::sync::Arc;
use std::thread;

fn bench_spsc_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_single_threaded");

    for size in [64, 256, 1024, 4096] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let queue = SpscQueue::new(size);

            b.iter(|| {
                queue.push(black_box(42)).unwrap();
                black_box(queue.pop().unwrap());
            });
        });
    }

    group.finish();
}

fn bench_spsc_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_push");

    for size in [256, 1024, 4096] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let queue = SpscQueue::new(size);
            let mut counter = 0u64;

            b.iter(|| {
                if queue.len() >= size / 2 {
                    for _ in 0..size / 4 {
                        queue.pop();
                    }
                }
                queue.push(black_box(counter)).unwrap();
                counter += 1;
            });
        });
    }

    group.finish();
}

fn bench_spsc_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_pop");

    for size in [256, 1024, 4096] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let queue = SpscQueue::new(size);

            for i in 0..size / 2 {
                queue.push(i).unwrap();
            }

            let mut counter = size / 2;
            b.iter(|| {
                if queue.is_empty() {
                    for i in 0..size / 2 {
                        queue.push(counter + i).unwrap();
                    }
                    counter += size / 2;
                }
                black_box(queue.pop().unwrap());
            });
        });
    }

    group.finish();
}

fn bench_spsc_multi_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_multi_threaded");
    group.throughput(Throughput::Elements(10000));

    for size in [1024, 4096] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let queue = Arc::new(SpscQueue::new(size));
                let queue_clone = queue.clone();

                let producer = thread::spawn(move || {
                    for i in 0u64..10000 {
                        while queue_clone.push(i).is_err() {
                            std::hint::spin_loop();
                        }
                    }
                });

                let consumer = thread::spawn(move || {
                    for _ in 0..10000 {
                        while queue.pop().is_none() {
                            std::hint::spin_loop();
                        }
                    }
                });

                producer.join().unwrap();
                consumer.join().unwrap();
            });
        });
    }

    group.finish();
}

fn bench_rdtsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("rdtsc");
    group.throughput(Throughput::Elements(1));

    group.bench_function("rdtsc_call", |b| {
        b.iter(|| {
            black_box(rdtsc());
        });
    });

    group.finish();
}

fn bench_rdtsc_latency_measurement(c: &mut Criterion) {
    let mut group = c.benchmark_group("rdtsc_latency");
    group.throughput(Throughput::Elements(1));

    group.bench_function("measure_operation", |b| {
        let queue = SpscQueue::new(256);

        b.iter(|| {
            let start = rdtsc();
            queue.push(black_box(42)).unwrap();
            let _ = queue.pop();
            let end = rdtsc();
            black_box(end - start);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_single_threaded,
    bench_spsc_push,
    bench_spsc_pop,
    bench_spsc_multi_threaded,
    bench_rdtsc,
    bench_rdtsc_latency_measurement
);
criterion_main!(benches);
