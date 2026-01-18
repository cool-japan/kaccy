//! Load testing benchmarks for Kaccy API
//!
//! These benchmarks measure API performance under load.
//! Run with: cargo bench --bench load_tests

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::json;
use std::hint::black_box;
use std::time::Duration;

/// Benchmark data serialization performance
fn benchmark_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    // Token data
    let token_data = json!({
        "token_id": "550e8400-e29b-41d4-a716-446655440001",
        "issuer_id": "550e8400-e29b-41d4-a716-446655440000",
        "symbol": "ALICE",
        "name": "Alice Token",
        "description": "Support Alice's creative work",
        "current_supply": 1000.0,
        "current_price_btc": 0.00001,
        "status": "active",
        "created_at": "2024-01-15T10:00:00Z"
    });

    group.bench_function("single_token", |b| {
        b.iter(|| {
            let json_string = serde_json::to_string(black_box(&token_data)).unwrap();
            black_box(json_string);
        });
    });

    // Multiple tokens
    let tokens: Vec<_> = (0..100).map(|_| token_data.clone()).collect();
    group.throughput(Throughput::Elements(100));

    group.bench_function("token_list_100", |b| {
        b.iter(|| {
            let json_string = serde_json::to_string(black_box(&tokens)).unwrap();
            black_box(json_string);
        });
    });

    group.finish();
}

/// Benchmark data deserialization performance
fn benchmark_json_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deserialization");

    let json_str = r#"{
        "email": "user@example.com",
        "password": "SecurePass123!",
        "username": "creator_alice"
    }"#;

    group.bench_function("register_request", |b| {
        b.iter(|| {
            let value: serde_json::Value = serde_json::from_str(black_box(json_str)).unwrap();
            black_box(value);
        });
    });

    group.finish();
}

/// Benchmark validation logic
fn benchmark_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    group.bench_function("email_validation", |b| {
        b.iter(|| {
            let email = black_box("user@example.com");
            let is_valid = email.contains('@') && email.contains('.');
            black_box(is_valid);
        });
    });

    group.bench_function("password_strength", |b| {
        b.iter(|| {
            let password = black_box("SecurePass123!");
            let has_upper = password.chars().any(|c| c.is_uppercase());
            let has_lower = password.chars().any(|c| c.is_lowercase());
            let has_digit = password.chars().any(|c| c.is_ascii_digit());
            let is_strong = password.len() >= 8 && has_upper && has_lower && has_digit;
            black_box(is_strong);
        });
    });

    group.finish();
}

/// Benchmark pagination logic
fn benchmark_pagination(c: &mut Criterion) {
    let mut group = c.benchmark_group("pagination");

    let total_items = 10000;
    let page_size = 20;

    group.bench_function("offset_calculation", |b| {
        b.iter(|| {
            let page = black_box(5);
            let offset = (page - 1) * page_size;
            let limit = page_size;
            black_box((offset, limit));
        });
    });

    group.bench_function("total_pages_calculation", |b| {
        b.iter(|| {
            let total = black_box(total_items);
            let per_page = black_box(page_size);
            let total_pages = (total + per_page - 1) / per_page;
            black_box(total_pages);
        });
    });

    group.finish();
}

/// Benchmark price calculations (bonding curve simulation)
fn benchmark_price_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("price_calculations");

    let base_price = 0.000001_f64;
    let increment = 0.0000001_f64;
    let current_supply = 1000_f64;

    group.bench_function("linear_bonding_curve", |b| {
        b.iter(|| {
            let amount = black_box(100.0);
            let price = base_price + (current_supply * increment);
            let total = price * amount;
            black_box(total);
        });
    });

    group.bench_function("quadratic_bonding_curve", |b| {
        b.iter(|| {
            let amount = black_box(100.0);
            let supply = black_box(current_supply);
            let price = base_price + (supply * supply * increment / 1000000.0);
            let total = price * amount;
            black_box(total);
        });
    });

    group.finish();
}

/// Benchmark token balance calculations
fn benchmark_token_balances(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_balances");

    // Simulate calculating portfolio value
    let holdings = vec![
        (100.0, 0.00001),  // 100 tokens at 0.00001 BTC each
        (50.0, 0.00002),   // 50 tokens at 0.00002 BTC each
        (200.0, 0.000005), // 200 tokens at 0.000005 BTC each
    ];

    group.bench_function("portfolio_value", |b| {
        b.iter(|| {
            let total: f64 = black_box(&holdings)
                .iter()
                .map(|(amount, price)| amount * price)
                .sum();
            black_box(total);
        });
    });

    group.finish();
}

/// Benchmark rate limit checking
fn benchmark_rate_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiting");

    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut rate_limit_data: HashMap<String, Vec<u64>> = HashMap::new();
    let max_requests = 100;
    let window_seconds = 60;

    group.bench_function("check_rate_limit", |b| {
        b.iter(|| {
            let ip = black_box("192.168.1.1");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let timestamps = rate_limit_data.entry(ip.to_string()).or_default();

            // Remove old timestamps
            timestamps.retain(|&t| now - t < window_seconds);

            // Check if over limit
            let is_allowed = timestamps.len() < max_requests;

            if is_allowed {
                timestamps.push(now);
            }

            black_box(is_allowed);
        });
    });

    group.finish();
}

/// Benchmark UUID generation and parsing
fn benchmark_uuid_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("uuid_operations");

    group.bench_function("uuid_generation", |b| {
        b.iter(|| {
            let uuid = uuid::Uuid::new_v4();
            black_box(uuid);
        });
    });

    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    group.bench_function("uuid_parsing", |b| {
        b.iter(|| {
            let uuid = uuid::Uuid::parse_str(black_box(uuid_str)).unwrap();
            black_box(uuid);
        });
    });

    group.finish();
}

/// Benchmark timestamp operations
fn benchmark_timestamp_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("timestamp_operations");

    group.bench_function("current_timestamp", |b| {
        b.iter(|| {
            let now = chrono::Utc::now();
            black_box(now);
        });
    });

    group.bench_function("timestamp_formatting", |b| {
        b.iter(|| {
            let now = chrono::Utc::now();
            let formatted = now.to_rfc3339();
            black_box(formatted);
        });
    });

    group.finish();
}

/// Benchmark concurrent request simulation
fn benchmark_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_operations");
    group.measurement_time(Duration::from_secs(10));

    // Simulate multiple concurrent operations
    for n in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            b.iter(|| {
                let operations: Vec<_> = (0..n)
                    .map(|i| {
                        // Simulate some work
                        let result = i * 2;
                        black_box(result);
                        result
                    })
                    .collect();
                black_box(operations);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_json_serialization,
    benchmark_json_deserialization,
    benchmark_validation,
    benchmark_pagination,
    benchmark_price_calculations,
    benchmark_token_balances,
    benchmark_rate_limiting,
    benchmark_uuid_operations,
    benchmark_timestamp_operations,
    benchmark_concurrent_operations
);

criterion_main!(benches);
