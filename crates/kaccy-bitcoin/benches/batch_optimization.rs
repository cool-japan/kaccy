use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kaccy_bitcoin::{BatchOptimizer, BatchStrategy, BatchWithdrawal};
use std::hint::black_box;

fn create_mock_withdrawals(count: usize) -> Vec<BatchWithdrawal> {
    (0..count)
        .map(|i| BatchWithdrawal {
            user_id: format!("user{}", i),
            address: format!("bc1q{:058x}", i),
            amount_sats: 10_000 + (i as u64 * 1000),
            priority: if i % 3 == 0 { 10 } else { 5 },
        })
        .collect()
}

fn bench_batch_optimization_strategies(c: &mut Criterion) {
    let optimizer = BatchOptimizer::new(
        25,  // max_outputs_per_tx
        2,   // min_batch_size
        5.0, // fee_rate
    );

    let mut group = c.benchmark_group("batch_optimize_minimize_transactions");
    for count in [10, 50, 100, 500, 1000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                optimizer
                    .optimize(
                        black_box(withdrawals.clone()),
                        BatchStrategy::MinimizeTransactions,
                    )
                    .unwrap()
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("batch_optimize_minimize_fees");
    for count in [10, 50, 100, 500, 1000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                optimizer
                    .optimize(black_box(withdrawals.clone()), BatchStrategy::MinimizeFees)
                    .unwrap()
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("batch_optimize_balanced");
    for count in [10, 50, 100, 500, 1000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                optimizer
                    .optimize(black_box(withdrawals.clone()), BatchStrategy::Balanced)
                    .unwrap()
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("batch_optimize_priority");
    for count in [10, 50, 100, 500, 1000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                optimizer
                    .optimize(black_box(withdrawals.clone()), BatchStrategy::Priority)
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_withdrawal_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("withdrawal_sorting_by_priority");
    for count in [100, 500, 1000, 5000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let mut sorted = withdrawals.clone();
                sorted.sort_by_key(|b| std::cmp::Reverse(b.priority));
                black_box(sorted);
            });
        });
    }
    group.finish();
}

fn bench_withdrawal_grouping(c: &mut Criterion) {
    let mut group = c.benchmark_group("withdrawal_grouping");
    for count in [100, 500, 1000].iter() {
        let withdrawals = create_mock_withdrawals(*count);
        let max_per_group = 25;

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let groups: Vec<Vec<BatchWithdrawal>> = withdrawals
                    .chunks(max_per_group)
                    .map(|chunk| chunk.to_vec())
                    .collect();
                black_box(groups);
            });
        });
    }
    group.finish();
}

fn bench_fee_calculation(c: &mut Criterion) {
    c.bench_function("single_tx_fee_estimation", |b| {
        b.iter(|| {
            let input_count = black_box(2);
            let output_count = black_box(2);
            let fee_rate = black_box(5.0);

            // Rough estimation: (inputs * 148 + outputs * 34 + 10) * fee_rate
            let vsize = input_count * 148 + output_count * 34 + 10;
            let fee = (vsize as f64 * fee_rate).ceil() as u64;
            black_box(fee);
        });
    });

    let mut group = c.benchmark_group("batch_fee_estimation");
    for output_count in [5, 10, 25, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(output_count),
            output_count,
            |b, &output_count| {
                b.iter(|| {
                    let input_count = black_box(3);
                    let fee_rate = black_box(5.0);

                    let vsize = input_count * 148 + output_count * 34 + 10;
                    let fee = (vsize as f64 * fee_rate).ceil() as u64;
                    black_box(fee);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_batch_optimization_strategies,
    bench_withdrawal_sorting,
    bench_withdrawal_grouping,
    bench_fee_calculation
);
criterion_main!(benches);
