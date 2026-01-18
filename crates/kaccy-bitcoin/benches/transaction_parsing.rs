use bitcoin::Txid;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::str::FromStr;

fn bench_transaction_analysis(c: &mut Criterion) {
    // Note: These benchmarks would ideally connect to a real node
    // For now, we'll benchmark the parsing logic structure

    c.bench_function("txid_parsing", |b| {
        let txid_str = "0000000000000000000000000000000000000000000000000000000000000000";
        b.iter(|| Txid::from_str(black_box(txid_str)).unwrap())
    });
}

fn bench_hex_encoding(c: &mut Criterion) {
    let data = vec![0u8; 256];

    c.bench_function("hex_encode_256bytes", |b| {
        b.iter(|| hex::encode(black_box(&data)))
    });

    let mut group = c.benchmark_group("hex_encoding_sizes");
    for size in [32, 64, 128, 256, 512, 1024].iter() {
        let data = vec![0u8; *size];
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| hex::encode(black_box(&data)));
        });
    }
    group.finish();
}

fn bench_hex_decoding(c: &mut Criterion) {
    let hex_str = "0000000000000000000000000000000000000000000000000000000000000000";

    c.bench_function("hex_decode_32bytes", |b| {
        b.iter(|| hex::decode(black_box(hex_str)).unwrap())
    });

    let mut group = c.benchmark_group("hex_decoding_sizes");
    for size in [32, 64, 128, 256].iter() {
        let hex_str = "00".repeat(*size);
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| hex::decode(black_box(&hex_str)).unwrap());
        });
    }
    group.finish();
}

fn bench_base64_operations(c: &mut Criterion) {
    use base64::{Engine as _, engine::general_purpose};

    let data = vec![0u8; 256];

    c.bench_function("base64_encode_256bytes", |b| {
        b.iter(|| general_purpose::STANDARD.encode(black_box(&data)))
    });

    let encoded = general_purpose::STANDARD.encode(&data);
    c.bench_function("base64_decode_256bytes", |b| {
        b.iter(|| {
            general_purpose::STANDARD
                .decode(black_box(&encoded))
                .unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_transaction_analysis,
    bench_hex_encoding,
    bench_hex_decoding,
    bench_base64_operations
);
criterion_main!(benches);
