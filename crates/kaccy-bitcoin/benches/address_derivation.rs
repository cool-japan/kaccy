use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kaccy_bitcoin::{HdWallet, HdWalletConfig};
use std::hint::black_box;

fn bench_address_generation(c: &mut Criterion) {
    let config = HdWalletConfig {
        xpub: "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string(),
        gap_limit: 20,
        ..Default::default()
    };

    let wallet = HdWallet::new(config).expect("Failed to create wallet");

    c.bench_function("single_address_generation", |b| {
        b.iter(|| wallet.derive_address(black_box(0), black_box(0)).unwrap())
    });

    let mut group = c.benchmark_group("batch_address_generation");
    for count in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                for i in 0..count {
                    let _ = wallet.derive_address(black_box(0), black_box(i));
                }
            });
        });
    }
    group.finish();
}

fn bench_change_address_generation(c: &mut Criterion) {
    let config = HdWalletConfig {
        xpub: "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string(),
        gap_limit: 20,
        ..Default::default()
    };

    let wallet = HdWallet::new(config).expect("Failed to create wallet");

    c.bench_function("change_address_generation", |b| {
        b.iter(|| wallet.derive_address(black_box(1), black_box(0)).unwrap())
    });
}

fn bench_address_caching(c: &mut Criterion) {
    let config = HdWalletConfig {
        xpub: "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string(),
        gap_limit: 20,
        enable_cache: true,
        ..Default::default()
    };

    let wallet = HdWallet::new(config).expect("Failed to create wallet");

    // Pre-populate cache
    for i in 0..100 {
        let _ = wallet.derive_address(0, i);
    }

    c.bench_function("cached_address_lookup", |b| {
        b.iter(|| wallet.derive_address(black_box(0), black_box(50)).unwrap())
    });

    c.bench_function("uncached_address_generation", |b| {
        b.iter(|| wallet.derive_address(black_box(0), black_box(200)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_address_generation,
    bench_change_address_generation,
    bench_address_caching
);
criterion_main!(benches);
