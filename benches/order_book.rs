use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use hft_engine::OrderBook;
use hft_engine::core::types::{Price, Quantity};
use hft_engine::messages::Side;

fn bench_order_book_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_update");
    group.throughput(Throughput::Elements(1));

    group.bench_function("update_single_level", |b| {
        let mut book = OrderBook::new();
        let mut counter = 0u64;

        b.iter(|| {
            let price = Price::new(100 + (counter % 10) as i64, 0);
            let qty = Quantity::new(10, 0);
            book.update_level(black_box(Side::Buy), black_box(price), black_box(qty));
            counter += 1;
        });
    });

    group.bench_function("update_filled_book", |b| {
        let mut book = OrderBook::new();

        for i in 0..5 {
            book.update_level(Side::Buy, Price::new(100 - i, 0), Quantity::new(10, 0));
            book.update_level(Side::Sell, Price::new(101 + i, 0), Quantity::new(10, 0));
        }

        let mut counter = 0u64;
        b.iter(|| {
            let price = Price::new(100 + (counter % 10) as i64, 0);
            let qty = Quantity::new(10, 0);
            book.update_level(black_box(Side::Buy), black_box(price), black_box(qty));
            counter += 1;
        });
    });

    group.finish();
}

fn bench_order_book_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_insert");
    group.throughput(Throughput::Elements(1));

    group.bench_function("insert_new_level", |b| {
        let mut counter = 0u64;

        b.iter(|| {
            let mut book = OrderBook::new();
            let price = Price::new(100 + counter as i64, 0);
            let qty = Quantity::new(10, 0);
            book.update_level(black_box(Side::Buy), black_box(price), black_box(qty));
            counter += 1;
            black_box(book);
        });
    });

    group.finish();
}

fn bench_order_book_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_remove");
    group.throughput(Throughput::Elements(1));

    group.bench_function("remove_level", |b| {
        let mut counter = 0u64;

        b.iter(|| {
            let mut book = OrderBook::new();
            let price = Price::new(100, 0);

            book.update_level(Side::Buy, price, Quantity::new(10, 0));

            book.update_level(
                black_box(Side::Buy),
                black_box(price),
                black_box(Quantity::new(0, 0)),
            );
            counter += 1;
            black_box(book);
        });
    });

    group.finish();
}

fn bench_order_book_accessors(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_accessors");
    group.throughput(Throughput::Elements(1));

    let mut book = OrderBook::new();
    book.update_level(Side::Buy, Price::new(100, 0), Quantity::new(10, 0));
    book.update_level(Side::Sell, Price::new(101, 0), Quantity::new(10, 0));

    group.bench_function("best_bid", |b| {
        b.iter(|| {
            black_box(book.best_bid());
        });
    });

    group.bench_function("best_ask", |b| {
        b.iter(|| {
            black_box(book.best_ask());
        });
    });

    group.bench_function("spread", |b| {
        b.iter(|| {
            black_box(book.spread());
        });
    });

    group.bench_function("mid_price", |b| {
        b.iter(|| {
            black_box(book.mid_price());
        });
    });

    group.finish();
}

fn bench_order_book_full_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_full_depth");

    for depth in [5, 10] {
        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.iter(|| {
                let mut book = OrderBook::new();

                for i in 0..depth {
                    book.update_level(
                        Side::Buy,
                        Price::new(100 - i as i64, 0),
                        Quantity::new(10, 0),
                    );
                    book.update_level(
                        Side::Sell,
                        Price::new(101 + i as i64, 0),
                        Quantity::new(10, 0),
                    );
                }

                black_box(book);
            });
        });
    }

    group.finish();
}

fn bench_order_book_realistic_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_book_realistic");
    group.throughput(Throughput::Elements(100));

    group.bench_function("mixed_operations", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();

            for i in 0..100 {
                let op = i % 3;

                match op {
                    0 => {
                        let price = Price::new(100 + (i % 10) as i64, 0);
                        book.update_level(Side::Buy, price, Quantity::new(10, 0));
                    }
                    1 => {
                        let price = Price::new(100, 0);
                        book.update_level(Side::Buy, price, Quantity::new(20, 0));
                    }
                    2 => {
                        let price = Price::new(100 + (i % 5) as i64, 0);
                        book.update_level(Side::Buy, price, Quantity::new(0, 0));
                    }
                    _ => unreachable!(),
                }
            }

            black_box(book);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_order_book_update,
    bench_order_book_insert,
    bench_order_book_remove,
    bench_order_book_accessors,
    bench_order_book_full_depth,
    bench_order_book_realistic_updates
);
criterion_main!(benches);
