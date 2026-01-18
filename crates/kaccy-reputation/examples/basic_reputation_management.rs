//! Basic Reputation Management Example
//!
//! This example demonstrates:
//! - Creating and updating reputation scores
//! - Tier progression and benefits
//! - Circuit breaker triggering
//! - Score validation and clamping
//!
//! Run with: `cargo run --example basic_reputation_management`

use kaccy_reputation::*;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("=== Basic Reputation Management Example ===\n");

    // Note: In a real application, you would connect to a PostgreSQL database
    // let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    //
    // For this example, we'll demonstrate the data structures and logic

    // 1. Understanding Score Components
    println!("1. Score Components and Weights:");
    println!("   - Commitment Fulfillment: 35% weight");
    println!("   - Response Time: 15% weight");
    println!("   - Quality Rating: 30% weight");
    println!("   - Community Trust: 15% weight");
    println!("   - Longevity: 5% weight\n");

    // Create sample score components
    let components = ScoreComponents {
        commitment_fulfillment: dec!(85.0), // 0-100 scale
        response_time: dec!(75.0),
        quality_rating: dec!(90.0),
        community_trust: dec!(80.0),
        longevity: dec!(70.0),
    };

    // Calculate weighted overall score
    let overall = components.calculate_overall();
    println!("   Component values (0-100): {:?}", components);
    println!("   Weighted overall (0-100): {}", overall);

    // Scale to 0-1000 range
    let scaled = components.calculate_scaled_overall();
    println!("   Scaled overall (0-1000): {}\n", scaled);

    // 2. Tier System
    println!("2. Reputation Tiers:");
    let scores = vec![
        dec!(50),
        dec!(250),
        dec!(400),
        dec!(600),
        dec!(750),
        dec!(900),
    ];

    for score in scores {
        let tier = ReputationTier::from_score(score);
        let benefits = tier.benefits();

        println!("   Score {}: {} tier", score, tier);
        println!("      - Can issue tokens: {}", benefits.can_issue_tokens);
        println!(
            "      - Max token issuance: {}",
            benefits.max_token_issuance
        );
        println!("      - Fee discount: {}%", benefits.fee_discount_percent);

        if let Some(next) = tier.next_tier() {
            println!(
                "      - Next tier: {} (min score: {})",
                next,
                next.min_score()
            );
        }
        println!();
    }

    // 3. Circuit Breaker
    println!("3. Circuit Breaker System:");
    use kaccy_reputation::circuit_breaker::{CIRCUIT_BREAKER_THRESHOLD, PAUSE_TOKENS_THRESHOLD};

    println!("   Thresholds:");
    println!(
        "      - Circuit breaker triggers: score < {}",
        CIRCUIT_BREAKER_THRESHOLD
    );
    println!("      - Tokens paused: score < {}", PAUSE_TOKENS_THRESHOLD);

    let low_score = dec!(150);
    if low_score < CIRCUIT_BREAKER_THRESHOLD {
        println!(
            "\n   ⚠️  Score {} would trigger circuit breaker!",
            low_score
        );
        println!("      - User cannot issue new tokens");
        println!("      - Admin will be notified");

        if low_score < PAUSE_TOKENS_THRESHOLD {
            println!("      - All active tokens will be PAUSED");
        }
    }
    println!();

    // 4. Score Validation
    println!("4. Score Delta Validation:");
    println!("   Maximum allowed changes per update:");
    println!("      - Positive delta: +{}", score::MAX_POSITIVE_DELTA);
    println!("      - Negative delta: {}", score::MAX_NEGATIVE_DELTA);
    println!(
        "      - Score range: {}-{}",
        score::MIN_SCORE,
        score::MAX_SCORE
    );

    // Example: trying to add too large a delta
    let current_score = dec!(500);
    let large_delta = dec!(75); // Exceeds MAX_POSITIVE_DELTA (50)

    println!(
        "\n   Example: Current score = {}, attempting to add +{}",
        current_score, large_delta
    );

    if large_delta > score::MAX_POSITIVE_DELTA {
        println!(
            "   ❌ Delta {} exceeds maximum positive delta {}",
            large_delta,
            score::MAX_POSITIVE_DELTA
        );
        println!(
            "   ✅ Delta would be clamped to +{}",
            score::MAX_POSITIVE_DELTA
        );
        println!(
            "   New score would be: {}",
            current_score + score::MAX_POSITIVE_DELTA
        );
    }
    println!();

    // 5. Tier Transitions
    println!("5. Tier Transition Example:");
    let old_tier = ReputationTier::Silver;
    let new_tier = ReputationTier::Gold;

    println!("   Promotion: {} → {}", old_tier, new_tier);

    let old_benefits = old_tier.benefits();
    let new_benefits = new_tier.benefits();

    println!("   Changes:");
    println!(
        "      - Max token issuance: {} → {}",
        old_benefits.max_token_issuance, new_benefits.max_token_issuance
    );
    println!(
        "      - Fee discount: {}% → {}%",
        old_benefits.fee_discount_percent, new_benefits.fee_discount_percent
    );
    println!(
        "      - Verified badge: {} → {}",
        old_benefits.verified_badge, new_benefits.verified_badge
    );
    println!();

    // 6. Validation Utilities
    println!("6. Input Validation:");

    // Validate a score
    let test_score = dec!(750);
    match validate_score(test_score) {
        Ok(_) => println!("   ✅ Score {} is valid", test_score),
        Err(e) => println!("   ❌ Score validation failed: {}", e),
    }

    // Validate a rating
    let test_rating = 4;
    match validate_rating(test_rating) {
        Ok(_) => println!("   ✅ Rating {} is valid (1-5 stars)", test_rating),
        Err(e) => println!("   ❌ Rating validation failed: {}", e),
    }

    // Validate a commitment title
    let test_title = "Complete project deliverable";
    match validate_title(test_title) {
        Ok(_) => println!("   ✅ Title '{}' is valid", test_title),
        Err(e) => println!("   ❌ Title validation failed: {}", e),
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
