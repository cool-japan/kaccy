use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kaccy_reputation::commitment::CommitmentStatus;
use kaccy_reputation::*;
use rust_decimal_macros::dec;
use std::hint::black_box;
use std::time::Duration;

// Benchmark score component calculations
fn bench_score_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("score_components");

    let components = ScoreComponents {
        commitment_fulfillment: dec!(85.0),
        response_time: dec!(75.0),
        quality_rating: dec!(90.0),
        community_trust: dec!(80.0),
        longevity: dec!(70.0),
    };

    group.bench_function("calculate_overall", |b| {
        b.iter(|| black_box(components.calculate_overall()))
    });

    group.bench_function("calculate_scaled_overall", |b| {
        b.iter(|| black_box(components.calculate_scaled_overall()))
    });

    group.finish();
}

// Benchmark tier calculations
fn bench_tier_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier_calculations");

    let test_scores = vec![
        dec!(0),
        dec!(100),
        dec!(250),
        dec!(400),
        dec!(600),
        dec!(750),
        dec!(900),
        dec!(1000),
    ];

    for score in test_scores {
        group.bench_with_input(
            BenchmarkId::new("from_score", score.to_string()),
            &score,
            |b, &s| b.iter(|| black_box(ReputationTier::from_score(s))),
        );
    }

    group.bench_function("tier_benefits", |b| {
        let tier = ReputationTier::Gold;
        b.iter(|| black_box(tier.benefits()))
    });

    group.bench_function("min_score", |b| {
        let tier = ReputationTier::Gold;
        b.iter(|| black_box(tier.min_score()))
    });

    group.bench_function("max_score", |b| {
        let tier = ReputationTier::Gold;
        b.iter(|| black_box(tier.max_score()))
    });

    group.bench_function("score_range", |b| {
        let tier = ReputationTier::Gold;
        b.iter(|| black_box(tier.score_range()))
    });

    group.bench_function("next_tier", |b| {
        let tier = ReputationTier::Gold;
        b.iter(|| black_box(tier.next_tier()))
    });

    group.finish();
}

// Benchmark cache operations
fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_operations");
    group.measurement_time(Duration::from_secs(5));

    let cache = ReputationCache::new(60); // 60 seconds TTL
    let user_id = uuid::Uuid::new_v4();

    let test_score = ReputationScore {
        user_id,
        overall_score: dec!(750),
        tier: ReputationTier::Gold,
        components: ScoreComponents {
            commitment_fulfillment: dec!(85.0),
            response_time: dec!(75.0),
            quality_rating: dec!(90.0),
            community_trust: dec!(80.0),
            longevity: dec!(70.0),
        },
    };

    group.bench_function("set_score", |b| {
        b.iter(|| {
            let _ = cache.set_score(user_id, test_score.clone());
        })
    });

    // Setup: add score to cache for get operations
    let _ = cache.set_score(user_id, test_score);

    group.bench_function("get_score", |b| {
        b.iter(|| black_box(cache.get_score(user_id)))
    });

    group.bench_function("set_tier", |b| {
        b.iter(|| {
            let _ = cache.set_tier(user_id, ReputationTier::Gold, dec!(750));
        })
    });

    // Setup: add tier to cache for get operations
    let _ = cache.set_tier(user_id, ReputationTier::Gold, dec!(750));

    group.bench_function("get_tier", |b| {
        b.iter(|| black_box(cache.get_tier(user_id)))
    });

    group.bench_function("invalidate_user", |b| {
        b.iter(|| {
            cache.invalidate_user(user_id);
        })
    });

    group.bench_function("get_stats", |b| b.iter(|| black_box(cache.get_stats())));

    group.finish();
}

// Benchmark cache cleanup with different cache sizes
fn bench_cache_cleanup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_cleanup");
    group.measurement_time(Duration::from_secs(10));

    for size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("cleanup", size), size, |b, &size| {
            // Setup: create cache with short TTL (1 second)
            let cache = ReputationCache::new(1);
            for _i in 0..size {
                let user_id = uuid::Uuid::new_v4();
                let test_score = ReputationScore {
                    user_id,
                    overall_score: dec!(500),
                    tier: ReputationTier::Silver,
                    components: ScoreComponents::default(),
                };
                let _ = cache.set_score(user_id, test_score);
                let _ = cache.set_tier(user_id, ReputationTier::Silver, dec!(500));
            }

            // Wait for entries to expire
            std::thread::sleep(Duration::from_secs(2));

            b.iter(|| black_box(cache.cleanup_expired()));
        });
    }

    group.finish();
}

// Benchmark confidence level calculations
fn bench_confidence_calculations(c: &mut Criterion) {
    let mut group = c.benchmark_group("confidence_calculations");

    let test_sample_sizes = vec![1, 5, 10, 50, 100, 500, 1000];

    for samples in test_sample_sizes {
        group.bench_with_input(
            BenchmarkId::new("confidence_from_data_points", samples),
            &samples,
            |b, &s| b.iter(|| black_box(ConfidenceLevel::from_data_points(s))),
        );
    }

    group.bench_function("confidence_weight_multiplier", |b| {
        let level = ConfidenceLevel::Medium;
        b.iter(|| black_box(level.weight_multiplier()))
    });

    group.finish();
}

// Benchmark validation functions
fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    group.bench_function("validate_score", |b| {
        b.iter(|| black_box(validate_score(dec!(750))))
    });

    group.bench_function("validate_rating", |b| {
        b.iter(|| black_box(validate_rating(black_box(4))))
    });

    group.bench_function("validate_percentage", |b| {
        b.iter(|| black_box(validate_percentage(dec!(75.0))))
    });

    group.bench_function("validate_weight", |b| {
        b.iter(|| black_box(validate_weight(dec!(0.5))))
    });

    group.bench_function("validate_title", |b| {
        b.iter(|| black_box(validate_title(black_box("Test commitment title"))))
    });

    group.bench_function("validate_description", |b| {
        let desc = "This is a test description for a commitment that should be validated";
        b.iter(|| black_box(validate_description(black_box(desc))))
    });

    group.bench_function("validate_uuid", |b| {
        let id = uuid::Uuid::new_v4();
        b.iter(|| black_box(validate_uuid(id, "user_id")))
    });

    group.finish();
}

// Benchmark commitment status transitions
fn bench_commitment_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("commitment_status");

    let statuses = vec![
        CommitmentStatus::Pending,
        CommitmentStatus::Completed,
        CommitmentStatus::Verified,
        CommitmentStatus::Failed,
        CommitmentStatus::Expired,
    ];

    for status in &statuses {
        group.bench_with_input(
            BenchmarkId::new("is_terminal", format!("{:?}", status)),
            status,
            |b, s| b.iter(|| black_box(s.is_terminal())),
        );

        group.bench_with_input(
            BenchmarkId::new("is_active", format!("{:?}", status)),
            status,
            |b, s| b.iter(|| black_box(s.is_active())),
        );

        group.bench_with_input(
            BenchmarkId::new("possible_next_states", format!("{:?}", status)),
            status,
            |b, s| b.iter(|| black_box(s.possible_next_states())),
        );
    }

    group.finish();
}

// Benchmark circuit breaker constants
fn bench_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker");

    use kaccy_reputation::circuit_breaker::{CIRCUIT_BREAKER_THRESHOLD, PAUSE_TOKENS_THRESHOLD};

    group.bench_function("check_circuit_breaker_threshold", |b| {
        let score = dec!(150);
        b.iter(|| black_box(score < CIRCUIT_BREAKER_THRESHOLD))
    });

    group.bench_function("check_pause_tokens_threshold", |b| {
        let score = dec!(75);
        b.iter(|| black_box(score < PAUSE_TOKENS_THRESHOLD))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_score_components,
    bench_tier_calculations,
    bench_cache_operations,
    bench_cache_cleanup,
    bench_confidence_calculations,
    bench_validation,
    bench_commitment_status,
    bench_circuit_breaker
);

criterion_main!(benches);
