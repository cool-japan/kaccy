//! Caching Integration Example
//!
//! This example demonstrates:
//! - Setting up reputation cache
//! - Caching scores and tiers
//! - Cache invalidation strategies
//! - Cache statistics and monitoring
//! - TTL management
//!
//! Run with: `cargo run --example caching_integration`

use kaccy_reputation::*;
use rust_decimal_macros::dec;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Caching Integration Example ===\n");

    // 1. Create cache with TTL
    println!("1. Initialize Cache:");
    let ttl_seconds = 300; // 5 minutes
    let cache = ReputationCache::new(ttl_seconds);
    println!("   Created cache with {} second TTL\n", ttl_seconds);

    // 2. Cache score data
    println!("2. Caching Reputation Scores:");
    let user_id = Uuid::new_v4();

    let reputation_score = ReputationScore {
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

    // Cache the score
    cache.set_score(user_id, reputation_score.clone())?;
    println!("   ✅ Cached score for user {}", user_id);

    // Retrieve from cache
    if let Some(cached_score) = cache.get_score(user_id) {
        println!("   ✅ Retrieved from cache:");
        println!("      - Overall score: {}", cached_score.overall_score);
        println!("      - Tier: {}", cached_score.tier);
    }
    println!();

    // 3. Cache tier data
    println!("3. Caching Tier Information:");
    cache.set_tier(user_id, ReputationTier::Gold, dec!(750))?;
    println!("   ✅ Cached tier for user {}", user_id);

    if let Some((tier, score)) = cache.get_tier(user_id) {
        println!("   ✅ Retrieved tier from cache:");
        println!("      - Tier: {}", tier);
        println!("      - Score: {}", score);
    }
    println!();

    // 4. Cache Statistics
    println!("4. Cache Statistics:");
    let stats = cache.get_stats();
    println!("   - Cached scores: {}", stats.cached_scores);
    println!("   - Cached tiers: {}", stats.cached_tiers);
    println!("   - TTL (seconds): {}", stats.ttl_seconds);
    println!();

    // 5. Cache multiple users
    println!("5. Caching Multiple Users:");
    let user_ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

    for (i, uid) in user_ids.iter().enumerate() {
        use rust_decimal::Decimal;
        let score_value = dec!(500) + Decimal::from(i * 50);
        let score = ReputationScore {
            user_id: *uid,
            overall_score: score_value,
            tier: ReputationTier::from_score(score_value),
            components: ScoreComponents::default(),
        };
        cache.set_score(*uid, score)?;
    }
    println!("   ✅ Cached {} users\n", user_ids.len());

    // 6. Check statistics again
    println!("6. Updated Cache Statistics:");
    let stats = cache.get_stats();
    println!("   - Cached scores: {}", stats.cached_scores);
    println!("   - Cached tiers: {}", stats.cached_tiers);
    println!();

    // 7. Cache Invalidation
    println!("7. Cache Invalidation:");
    println!("   When to invalidate:");
    println!("      - User's score changes");
    println!("      - Commitment verification updates score");
    println!("      - Admin adjustments");
    println!("      - Time decay applied");
    println!();

    // Invalidate specific user
    cache.invalidate_user(user_id);
    println!("   ✅ Invalidated cache for user {}", user_id);

    // Verify it's gone
    if cache.get_score(user_id).is_none() {
        println!("   ✅ Score no longer in cache\n");
    }

    // 8. Cache Cleanup
    println!("8. Cache Cleanup:");
    println!("   Cleanup removes expired entries based on TTL");

    let cleanup_stats = cache.cleanup_expired();
    println!("   Cleanup results:");
    println!(
        "      - Expired scores removed: {}",
        cleanup_stats.expired_scores
    );
    println!("      - Cleaned at: {}\n", cleanup_stats.cleaned_at);

    // 9. Best Practices
    println!("9. Caching Best Practices:");
    println!();
    println!("   ✅ DO:");
    println!("      - Set appropriate TTL for your use case");
    println!("      - Invalidate cache when data changes");
    println!("      - Monitor cache hit rates");
    println!("      - Run periodic cleanup (e.g., every hour)");
    println!("      - Use cache for frequently accessed data");
    println!();
    println!("   ❌ DON'T:");
    println!("      - Set TTL too short (high cache miss rate)");
    println!("      - Set TTL too long (stale data)");
    println!("      - Forget to invalidate on updates");
    println!("      - Cache rarely accessed data");
    println!("      - Ignore cache statistics");
    println!();

    // 10. Integration Pattern
    println!("10. Typical Integration Pattern:");
    println!();
    println!("   fn get_reputation(user_id: Uuid, cache: &Cache, db: &Pool) -> Score {{");
    println!("       // Try cache first");
    println!("       if let Some(score) = cache.get_score(user_id) {{");
    println!("           return score; // Cache hit");
    println!("       }}");
    println!();
    println!("       // Cache miss: query database");
    println!("       let score = db.query_reputation(user_id).await?;");
    println!();
    println!("       // Update cache for next time");
    println!("       cache.set_score(user_id, score.clone())?;");
    println!();
    println!("       score");
    println!("   }}");
    println!();
    println!("   fn update_reputation(user_id: Uuid, delta: Decimal, cache: &Cache, db: &Pool) {{");
    println!("       // Update database");
    println!("       db.update_score(user_id, delta).await?;");
    println!();
    println!("       // Invalidate cache to force refresh");
    println!("       cache.invalidate_user(user_id);");
    println!("   }}");
    println!();

    // 11. Performance Considerations
    println!("11. Performance Considerations:");
    println!("   - Cache lookup: O(1) HashMap access");
    println!("   - Cache invalidation: O(1) HashMap delete");
    println!("   - Cleanup: O(n) where n = cached entries");
    println!("   - Thread-safe: Uses RwLock (many readers, single writer)");
    println!();
    println!("   Recommended TTL values:");
    println!("      - High-frequency reads: 60-300 seconds (1-5 minutes)");
    println!("      - Medium-frequency: 300-900 seconds (5-15 minutes)");
    println!("      - Low-frequency: 900-3600 seconds (15-60 minutes)");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}
