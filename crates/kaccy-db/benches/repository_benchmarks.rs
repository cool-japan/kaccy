use criterion::{criterion_group, criterion_main, Criterion};
use kaccy_db::EncryptionProvider;
use std::hint::black_box;

fn benchmark_encryption(c: &mut Criterion) {
    let key = EncryptionProvider::generate_key();
    let provider = EncryptionProvider::new(&key).unwrap();
    let data = b"sensitive user data that needs encryption";

    c.bench_function("encrypt_data", |b| {
        b.iter(|| provider.encrypt(black_box(data)).unwrap())
    });

    let encrypted = provider.encrypt(data).unwrap();
    c.bench_function("decrypt_data", |b| {
        b.iter(|| provider.decrypt(black_box(&encrypted)).unwrap())
    });
}

fn benchmark_cache_operations(c: &mut Criterion) {
    use dashmap::DashMap;
    let cache = DashMap::new();

    c.bench_function("cache_insert", |b| {
        let mut counter = 0;
        b.iter(|| {
            let key = format!("key_{}", counter);
            cache.insert(black_box(key), black_box("value"));
            counter += 1;
        })
    });

    // Pre-populate for read bench
    for i in 0..1000 {
        cache.insert(format!("key_{}", i), "value");
    }

    c.bench_function("cache_get", |b| {
        let mut counter = 0;
        b.iter(|| {
            let key = format!("key_{}", counter % 1000);
            cache.get(black_box(&key));
            counter += 1;
        })
    });
}

fn benchmark_cursor_operations(c: &mut Criterion) {
    use kaccy_db::Cursor;

    c.bench_function("cursor_encode", |b| {
        b.iter(|| Cursor::encode(black_box("user_123_12345")))
    });

    let cursor = Cursor::encode("user_123_12345");

    c.bench_function("cursor_decode", |b| b.iter(|| cursor.decode()));
}

fn benchmark_password_hashing(c: &mut Criterion) {
    use kaccy_db::PasswordHashingService;

    let hasher = PasswordHashingService::new();
    let password = "secure_password_123";

    c.bench_function("password_hash", |b| {
        b.iter(|| hasher.hash_password(black_box(password)).unwrap())
    });

    let hash = hasher.hash_password(password).unwrap();

    c.bench_function("password_verify", |b| {
        b.iter(|| {
            hasher
                .verify_password(black_box(password), black_box(&hash))
                .unwrap()
        })
    });
}

criterion_group!(
    benches,
    benchmark_encryption,
    benchmark_cache_operations,
    benchmark_cursor_operations,
    benchmark_password_hashing
);
criterion_main!(benches);
