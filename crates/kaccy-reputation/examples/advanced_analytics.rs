//! Advanced Analytics Example
//!
//! This example demonstrates:
//! - System-wide statistics
//! - Growth metrics tracking
//! - Score distribution analysis
//! - Leaderboards and rankings
//! - User analytics
//!
//! Run with: `cargo run --example advanced_analytics`

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Advanced Analytics Example ===\n");

    // 1. System Statistics
    println!("1. System-Wide Statistics:");
    println!();
    println!("   SystemStats provides overview of the entire platform:");
    println!();
    println!("   {{");
    println!("     total_users: 10,523,");
    println!("     active_users: 8,241,  // Active in last 30 days");
    println!("     average_score: 487.3,");
    println!("     median_score: 456.0,");
    println!("     tier_distribution: {{");
    println!("       Unverified: 1,234,");
    println!("       Bronze: 3,456,");
    println!("       Silver: 2,789,");
    println!("       Gold: 1,890,");
    println!("       Platinum: 892,");
    println!("       Diamond: 262");
    println!("     }},");
    println!("     total_commitments: 45,678,");
    println!("     verified_commitments: 38,901,");
    println!("     failed_commitments: 4,123,");
    println!("     commitment_success_rate: 85.2%");
    println!("   }}");
    println!();

    // 2. Growth Metrics
    println!("2. Growth Metrics Tracking:");
    println!();
    println!("   Track growth over time periods:");
    println!();
    println!("   Last 30 Days:");
    println!("     new_users: 523,");
    println!("     avg_daily_new_users: 17.4,");
    println!("     new_commitments: 2,341,");
    println!("     avg_daily_commitments: 78.0,");
    println!("     avg_score_change: +12.3");
    println!();
    println!("   Last 90 Days:");
    println!("     new_users: 1,789,");
    println!("     avg_daily_new_users: 19.9,");
    println!("     user_growth_rate: +20.5%,");
    println!("     commitment_growth_rate: +18.3%");
    println!();

    // 3. Score Distribution
    println!("3. Score Distribution Analysis:");
    println!();
    println!("   Histogram of user scores:");
    println!();
    println!("   Score Range    | Users  | Percentage | Bar");
    println!("   --------------|--------|------------|----------------------");
    println!("   0-100         | 1,234  | 11.7%      | ███████████");
    println!("   101-200       | 2,456  | 23.3%      | ███████████████████████");
    println!("   201-300       | 1,890  | 18.0%      | ██████████████████");
    println!("   301-400       | 1,567  | 14.9%      | ███████████████");
    println!("   401-500       | 1,234  | 11.7%      | ███████████");
    println!("   501-600       | 890    | 8.5%       | ████████");
    println!("   601-700       | 567    | 5.4%       | █████");
    println!("   701-800       | 345    | 3.3%       | ███");
    println!("   801-900       | 234    | 2.2%       | ██");
    println!("   901-1000      | 106    | 1.0%       | █");
    println!();
    println!("   Insights:");
    println!("   - Most users (23.3%) are in 101-200 range (Bronze tier)");
    println!("   - Normal distribution with long tail at high scores");
    println!("   - Only 1% of users reach Diamond tier (901-1000)");
    println!();

    // 4. Leaderboards
    println!("4. Leaderboard System:");
    println!();
    println!("   Global Leaderboard (Top 10):");
    println!("   Rank | User ID                              | Score | Tier");
    println!("   -----|--------------------------------------|-------|----------");
    println!("   1    | a1b2c3d4-e5f6-7890-abcd-ef1234567890 | 987   | Diamond");
    println!("   2    | b2c3d4e5-f6a7-8901-bcde-f12345678901 | 974   | Diamond");
    println!("   3    | c3d4e5f6-a7b8-9012-cdef-123456789012 | 962   | Diamond");
    println!("   4    | d4e5f6a7-b8c9-0123-def1-234567890123 | 951   | Diamond");
    println!("   5    | e5f6a7b8-c9d0-1234-ef12-345678901234 | 945   | Diamond");
    println!("   ...");
    println!();
    println!("   Tier-Specific Leaderboard (Gold tier):");
    println!("   - Rank users within same tier");
    println!("   - Encourages competition among peers");
    println!("   - Shows top performers in each tier");
    println!();

    // 5. User Rankings
    println!("5. Individual User Rankings:");
    println!();
    println!("   UserRankInfo provides detailed ranking data:");
    println!();
    println!("   {{");
    println!("     global_rank: 1,234,  // Out of 10,523 users");
    println!("     tier_rank: 45,       // Within Gold tier (1,890 users)");
    println!("     percentile: 88.3,    // Better than 88.3% of users");
    println!("     next_rank_score: 523 // Score needed for next rank");
    println!("   }}");
    println!();
    println!("   Use cases:");
    println!("   - Show user their relative position");
    println!("   - Motivation to improve");
    println!("   - Progress tracking over time");
    println!();

    // 6. Achievement System
    println!("6. Achievement Tracking:");
    println!();
    println!("   Achievement Types:");
    println!();
    println!("   FirstToken:");
    println!("      - Awarded when user issues first token");
    println!("      - Milestone achievement");
    println!();
    println!("   Commitments100:");
    println!("      - Complete 100 commitments");
    println!("      - Shows dedication and consistency");
    println!();
    println!("   TopRank:");
    println!("      - Reach top 1% of users");
    println!("      - Elite status marker");
    println!();
    println!("   PerfectMonth:");
    println!("      - 100% commitment success rate for a month");
    println!("      - Quality achievement");
    println!();
    println!("   CommunityFavorite:");
    println!("      - Average holder rating > 4.5 stars");
    println!("      - Community recognition");
    println!();

    // 7. User Commitment Analytics
    println!("7. User Commitment Analytics:");
    println!();
    println!("   UserCommitmentAnalytics:");
    println!("   {{");
    println!("     total_commitments: 47,");
    println!("     verified: 42,");
    println!("     failed: 3,");
    println!("     expired: 2,");
    println!("     completion_rate: 89.4%,");
    println!("     avg_completion_time_days: 12.3,");
    println!("     current_streak: 8,      // Consecutive successful commitments");
    println!("     longest_streak: 15,");
    println!("     favorite_category: \"Development\"");
    println!("   }}");
    println!();
    println!("   Insights:");
    println!("   - High completion rate (89.4%) indicates reliability");
    println!("   - Fast avg completion time (12.3 days) shows efficiency");
    println!("   - Current streak of 8 shows consistent performance");
    println!();

    // 8. Trend Analysis
    println!("8. Score Trend Analysis:");
    println!();
    println!("   Score history snapshots show trends:");
    println!();
    println!("   Date       | Score | Change | Tier   | Trend");
    println!("   -----------|-------|--------|--------|-------");
    println!("   2026-01-01 | 456   | -      | Silver | -");
    println!("   2026-01-04 | 478   | +22    | Silver | ↑");
    println!("   2026-01-08 | 502   | +24    | Silver | ↑");
    println!("   2026-01-11 | 534   | +32    | Gold   | ↑↑");
    println!("   2026-01-18 | 567   | +33    | Gold   | ↑↑");
    println!();
    println!("   Trend Direction:");
    println!("   - ↑↑ Strong upward (change > +20)");
    println!("   - ↑  Upward (change > 0)");
    println!("   - →  Stable (change = 0)");
    println!("   - ↓  Downward (change < 0)");
    println!("   - ↓↓ Strong downward (change < -20)");
    println!();

    // 9. Analytics Dashboard
    println!("9. Complete Analytics Dashboard:");
    println!();
    println!("   The ReputationOrchestrator provides an integrated view:");
    println!();
    println!("   orchestrator.get_analytics_dashboard().await? returns:");
    println!("   {{");
    println!("     system_stats: {{ ... }},        // Overall system health");
    println!("     growth_metrics: {{ ... }},      // 30-day growth data");
    println!("     top_performers: [{{ ... }}],    // Top 10 users");
    println!("     score_distribution: {{ ... }},  // Histogram data");
    println!("     global_leaderboard: {{ ... }}   // Top 20 rankings");
    println!("   }}");
    println!();
    println!("   Perfect for:");
    println!("   - Admin dashboards");
    println!("   - Public statistics pages");
    println!("   - Investor reports");
    println!("   - System health monitoring");
    println!();

    // 10. Best Practices
    println!("10. Analytics Best Practices:");
    println!();
    println!("   ✅ DO:");
    println!("      - Cache analytics queries (they're expensive)");
    println!("      - Update statistics on a schedule (hourly/daily)");
    println!("      - Use materialized views for complex queries");
    println!("      - Monitor query performance");
    println!("      - Provide date range filters");
    println!();
    println!("   ❌ DON'T:");
    println!("      - Calculate analytics in real-time on every request");
    println!("      - Expose PII in public analytics");
    println!("      - Run heavy queries during peak hours");
    println!("      - Forget to paginate large result sets");
    println!();
    println!("   Performance Tips:");
    println!("      - Pre-calculate daily statistics");
    println!("      - Use database indexes on aggregation columns");
    println!("      - Cache leaderboards for 5-15 minutes");
    println!("      - Batch process trend calculations");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}
