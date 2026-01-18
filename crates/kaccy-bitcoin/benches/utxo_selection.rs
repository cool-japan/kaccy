use bitcoin::Txid;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kaccy_bitcoin::{
    BitcoinClient, BitcoinNetwork, ConsolidationConfig, SelectionStrategy, Utxo, UtxoManager,
};
use std::hint::black_box;
use std::str::FromStr;
use std::sync::Arc;

fn create_mock_utxos(count: usize) -> Vec<Utxo> {
    (0..count)
        .map(|i| Utxo {
            txid: Txid::from_str(&format!("{:064x}", i)).unwrap(),
            vout: 0,
            address: format!("bc1q{:058x}", i),
            amount_sats: 10_000 + (i as u64 * 1000),
            confirmations: 6,
            spendable: true,
            safe: true,
        })
        .collect()
}

fn bench_utxo_selection_strategies(c: &mut Criterion) {
    let client = Arc::new(
        BitcoinClient::new("http://localhost", "user", "pass", BitcoinNetwork::Regtest).unwrap(),
    );

    let config = ConsolidationConfig {
        min_utxo_count: 100,
        max_fee_rate: 10.0,
        target_utxo_count: 50,
        min_utxo_value_sats: 10_000,
        consolidation_address: None,
    };

    let manager = UtxoManager::new(client, config);

    let mut group = c.benchmark_group("utxo_selection_largest_first");
    for utxo_count in [10, 50, 100, 500, 1000].iter() {
        let _utxos = create_mock_utxos(*utxo_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(utxo_count),
            utxo_count,
            |b, _| {
                b.iter(|| {
                    manager.select_utxos(
                        black_box(50_000),
                        black_box(5.0),
                        SelectionStrategy::LargestFirst,
                    )
                });
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("utxo_selection_smallest_first");
    for utxo_count in [10, 50, 100, 500, 1000].iter() {
        let _utxos = create_mock_utxos(*utxo_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(utxo_count),
            utxo_count,
            |b, _| {
                b.iter(|| {
                    manager.select_utxos(
                        black_box(50_000),
                        black_box(5.0),
                        SelectionStrategy::SmallestFirst,
                    )
                });
            },
        );
    }
    group.finish();

    let mut group = c.benchmark_group("utxo_selection_branch_and_bound");
    for utxo_count in [10, 50, 100].iter() {
        let _utxos = create_mock_utxos(*utxo_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(utxo_count),
            utxo_count,
            |b, _| {
                b.iter(|| {
                    manager.select_utxos(
                        black_box(50_000),
                        black_box(5.0),
                        SelectionStrategy::BranchAndBound,
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_utxo_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_filtering");
    for count in [100, 500, 1000, 5000].iter() {
        let utxos = create_mock_utxos(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                utxos
                    .iter()
                    .filter(|u| u.amount_sats > black_box(20_000))
                    .filter(|u| u.confirmations >= black_box(6))
                    .count()
            });
        });
    }
    group.finish();
}

fn bench_utxo_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("utxo_sorting_by_amount");
    for count in [100, 500, 1000, 5000].iter() {
        let utxos = create_mock_utxos(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                let mut sorted = utxos.clone();
                sorted.sort_by(|a, b| b.amount_sats.cmp(&a.amount_sats));
                black_box(sorted);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_utxo_selection_strategies,
    bench_utxo_filtering,
    bench_utxo_sorting
);
criterion_main!(benches);
