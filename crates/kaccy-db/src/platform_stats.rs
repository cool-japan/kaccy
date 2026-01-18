//! Platform-wide statistics and aggregations
//!
//! This module provides high-level statistics that aggregate data across
//! multiple repositories for platform monitoring and dashboard displays.

use crate::error::Result;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, PgPool, Row};

/// Platform-wide statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformStats {
    /// Total number of users
    pub total_users: i64,
    /// Total number of active users (not deleted)
    pub active_users: i64,
    /// Total number of tokens
    pub total_tokens: i64,
    /// Total number of active tokens
    pub active_tokens: i64,
    /// Total number of trades
    pub total_trades: i64,
    /// Total trading volume
    pub total_volume: Decimal,
    /// Total platform fees collected
    pub total_fees: Decimal,
    /// Total number of orders
    pub total_orders: i64,
    /// Number of pending orders
    pub pending_orders: i64,
    /// Total number of commitments
    pub total_commitments: i64,
    /// Number of verified commitments
    pub verified_commitments: i64,
}

/// Get comprehensive platform statistics
///
/// Aggregates key metrics across all repositories for monitoring dashboards
pub async fn get_platform_stats<'e, E>(executor: E) -> Result<PlatformStats>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users) as total_users,
            (SELECT COUNT(*) FROM users WHERE role != 'deleted') as active_users,
            (SELECT COUNT(*) FROM tokens) as total_tokens,
            (SELECT COUNT(*) FROM tokens WHERE status = 'active') as active_tokens,
            (SELECT COUNT(*) FROM trades) as total_trades,
            (SELECT COALESCE(SUM(btc_amount), 0) FROM trades) as total_volume,
            (SELECT COALESCE(SUM(platform_fee), 0) FROM trades) as total_fees,
            (SELECT COUNT(*) FROM orders) as total_orders,
            (SELECT COUNT(*) FROM orders WHERE status = 'pending') as pending_orders,
            (SELECT COUNT(*) FROM output_commitments) as total_commitments,
            (SELECT COUNT(*) FROM output_commitments WHERE status = 'verified') as verified_commitments
        "#,
    )
    .fetch_one(executor)
    .await?;

    Ok(PlatformStats {
        total_users: row.get("total_users"),
        active_users: row.get("active_users"),
        total_tokens: row.get("total_tokens"),
        active_tokens: row.get("active_tokens"),
        total_trades: row.get("total_trades"),
        total_volume: row.get("total_volume"),
        total_fees: row.get("total_fees"),
        total_orders: row.get("total_orders"),
        pending_orders: row.get("pending_orders"),
        total_commitments: row.get("total_commitments"),
        verified_commitments: row.get("verified_commitments"),
    })
}

/// Platform growth metrics comparing time periods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthMetrics {
    /// New users in last 24 hours
    pub users_24h: i64,
    /// New users in last 7 days
    pub users_7d: i64,
    /// New users in last 30 days
    pub users_30d: i64,
    /// New tokens in last 24 hours
    pub tokens_24h: i64,
    /// New tokens in last 7 days
    pub tokens_7d: i64,
    /// New tokens in last 30 days
    pub tokens_30d: i64,
    /// Trades in last 24 hours
    pub trades_24h: i64,
    /// Trades in last 7 days
    pub trades_7d: i64,
    /// Trades in last 30 days
    pub trades_30d: i64,
    /// Trading volume in last 24 hours
    pub volume_24h: Decimal,
    /// Trading volume in last 7 days
    pub volume_7d: Decimal,
    /// Trading volume in last 30 days
    pub volume_30d: Decimal,
}

/// Get platform growth metrics
///
/// Calculates growth statistics over different time periods
pub async fn get_growth_metrics<'e, E>(executor: E) -> Result<GrowthMetrics>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users WHERE created_at >= NOW() - INTERVAL '24 hours') as users_24h,
            (SELECT COUNT(*) FROM users WHERE created_at >= NOW() - INTERVAL '7 days') as users_7d,
            (SELECT COUNT(*) FROM users WHERE created_at >= NOW() - INTERVAL '30 days') as users_30d,
            (SELECT COUNT(*) FROM tokens WHERE created_at >= NOW() - INTERVAL '24 hours') as tokens_24h,
            (SELECT COUNT(*) FROM tokens WHERE created_at >= NOW() - INTERVAL '7 days') as tokens_7d,
            (SELECT COUNT(*) FROM tokens WHERE created_at >= NOW() - INTERVAL '30 days') as tokens_30d,
            (SELECT COUNT(*) FROM trades WHERE created_at >= NOW() - INTERVAL '24 hours') as trades_24h,
            (SELECT COUNT(*) FROM trades WHERE created_at >= NOW() - INTERVAL '7 days') as trades_7d,
            (SELECT COUNT(*) FROM trades WHERE created_at >= NOW() - INTERVAL '30 days') as trades_30d,
            (SELECT COALESCE(SUM(btc_amount), 0) FROM trades WHERE created_at >= NOW() - INTERVAL '24 hours') as volume_24h,
            (SELECT COALESCE(SUM(btc_amount), 0) FROM trades WHERE created_at >= NOW() - INTERVAL '7 days') as volume_7d,
            (SELECT COALESCE(SUM(btc_amount), 0) FROM trades WHERE created_at >= NOW() - INTERVAL '30 days') as volume_30d
        "#,
    )
    .fetch_one(executor)
    .await?;

    Ok(GrowthMetrics {
        users_24h: row.get("users_24h"),
        users_7d: row.get("users_7d"),
        users_30d: row.get("users_30d"),
        tokens_24h: row.get("tokens_24h"),
        tokens_7d: row.get("tokens_7d"),
        tokens_30d: row.get("tokens_30d"),
        trades_24h: row.get("trades_24h"),
        trades_7d: row.get("trades_7d"),
        trades_30d: row.get("trades_30d"),
        volume_24h: row.get("volume_24h"),
        volume_7d: row.get("volume_7d"),
        volume_30d: row.get("volume_30d"),
    })
}

/// Platform health indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIndicators {
    /// Average reputation score of all users
    pub avg_reputation: Decimal,
    /// Percentage of verified commitments
    pub commitment_success_rate: Decimal,
    /// Percentage of completed orders
    pub order_completion_rate: Decimal,
    /// Average trade size
    pub avg_trade_size: Decimal,
    /// Number of unique traders in last 30 days
    pub active_traders_30d: i64,
    /// Number of tokens with active trading
    pub liquid_tokens: i64,
}

/// Get platform health indicators
///
/// Provides key metrics for assessing platform health and engagement
pub async fn get_health_indicators<'e, E>(executor: E) -> Result<HealthIndicators>
where
    E: Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query(
        r#"
        SELECT
            COALESCE(AVG(reputation_score), 0) as avg_reputation,
            CASE
                WHEN (SELECT COUNT(*) FROM output_commitments) > 0
                THEN (SELECT COUNT(*)::decimal FROM output_commitments WHERE status = 'verified') * 100.0 /
                     (SELECT COUNT(*) FROM output_commitments)
                ELSE 0
            END as commitment_success_rate,
            CASE
                WHEN (SELECT COUNT(*) FROM orders) > 0
                THEN (SELECT COUNT(*)::decimal FROM orders WHERE status = 'completed') * 100.0 /
                     (SELECT COUNT(*) FROM orders)
                ELSE 0
            END as order_completion_rate,
            CASE
                WHEN (SELECT COUNT(*) FROM trades) > 0
                THEN (SELECT AVG(btc_amount) FROM trades)
                ELSE 0
            END as avg_trade_size,
            (
                SELECT COUNT(DISTINCT user_id) FROM (
                    SELECT buyer_user_id as user_id FROM trades WHERE created_at >= NOW() - INTERVAL '30 days'
                    UNION
                    SELECT seller_user_id as user_id FROM trades WHERE created_at >= NOW() - INTERVAL '30 days'
                ) as active_users
            ) as active_traders_30d,
            (
                SELECT COUNT(DISTINCT token_id) FROM trades WHERE created_at >= NOW() - INTERVAL '7 days'
            ) as liquid_tokens
        FROM users
        WHERE role != 'deleted'
        "#,
    )
    .fetch_one(executor)
    .await?;

    Ok(HealthIndicators {
        avg_reputation: row.get("avg_reputation"),
        commitment_success_rate: row.get("commitment_success_rate"),
        order_completion_rate: row.get("order_completion_rate"),
        avg_trade_size: row.get("avg_trade_size"),
        active_traders_30d: row.get("active_traders_30d"),
        liquid_tokens: row.get("liquid_tokens"),
    })
}

/// Get efficient platform snapshot (optimized single query)
///
/// Returns the most essential metrics in a single database round-trip
pub async fn get_platform_snapshot(pool: &PgPool) -> Result<PlatformStats> {
    get_platform_stats(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_stats_structure() {
        let stats = PlatformStats {
            total_users: 100,
            active_users: 95,
            total_tokens: 50,
            active_tokens: 45,
            total_trades: 1000,
            total_volume: Decimal::new(50000, 2),
            total_fees: Decimal::new(500, 2),
            total_orders: 800,
            pending_orders: 50,
            total_commitments: 200,
            verified_commitments: 180,
        };

        assert_eq!(stats.total_users, 100);
        assert_eq!(stats.active_users, 95);
        assert_eq!(stats.total_tokens, 50);
    }

    #[test]
    fn test_growth_metrics_structure() {
        let metrics = GrowthMetrics {
            users_24h: 10,
            users_7d: 50,
            users_30d: 200,
            tokens_24h: 5,
            tokens_7d: 25,
            tokens_30d: 100,
            trades_24h: 100,
            trades_7d: 500,
            trades_30d: 2000,
            volume_24h: Decimal::new(1000, 2),
            volume_7d: Decimal::new(5000, 2),
            volume_30d: Decimal::new(20000, 2),
        };

        assert_eq!(metrics.users_24h, 10);
        assert_eq!(metrics.trades_7d, 500);
        assert_eq!(metrics.volume_30d, Decimal::new(20000, 2));
    }

    #[test]
    fn test_health_indicators_structure() {
        let indicators = HealthIndicators {
            avg_reputation: Decimal::new(7500, 2),
            commitment_success_rate: Decimal::new(9000, 2),
            order_completion_rate: Decimal::new(8500, 2),
            avg_trade_size: Decimal::new(50, 2),
            active_traders_30d: 150,
            liquid_tokens: 30,
        };

        assert_eq!(indicators.avg_reputation, Decimal::new(7500, 2));
        assert_eq!(indicators.active_traders_30d, 150);
    }

    #[test]
    fn test_platform_stats_serialization() {
        let stats = PlatformStats {
            total_users: 100,
            active_users: 95,
            total_tokens: 50,
            active_tokens: 45,
            total_trades: 1000,
            total_volume: Decimal::new(50000, 2),
            total_fees: Decimal::new(500, 2),
            total_orders: 800,
            pending_orders: 50,
            total_commitments: 200,
            verified_commitments: 180,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: PlatformStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.total_users, 100);
        assert_eq!(deserialized.total_volume, Decimal::new(50000, 2));
    }

    #[test]
    fn test_growth_metrics_time_periods() {
        let metrics = GrowthMetrics {
            users_24h: 10,
            users_7d: 50,
            users_30d: 200,
            tokens_24h: 5,
            tokens_7d: 25,
            tokens_30d: 100,
            trades_24h: 100,
            trades_7d: 500,
            trades_30d: 2000,
            volume_24h: Decimal::new(1000, 2),
            volume_7d: Decimal::new(5000, 2),
            volume_30d: Decimal::new(20000, 2),
        };

        // Growth should typically be increasing over time
        assert!(metrics.users_7d >= metrics.users_24h);
        assert!(metrics.users_30d >= metrics.users_7d);
        assert!(metrics.trades_30d >= metrics.trades_7d);
    }

    #[test]
    fn test_health_indicators_percentage_ranges() {
        let indicators = HealthIndicators {
            avg_reputation: Decimal::new(7500, 2),
            commitment_success_rate: Decimal::new(9000, 2),
            order_completion_rate: Decimal::new(8500, 2),
            avg_trade_size: Decimal::new(50, 2),
            active_traders_30d: 150,
            liquid_tokens: 30,
        };

        // Success rates should be between 0 and 100
        assert!(indicators.commitment_success_rate >= Decimal::ZERO);
        assert!(indicators.commitment_success_rate <= Decimal::new(100, 0));
        assert!(indicators.order_completion_rate >= Decimal::ZERO);
        assert!(indicators.order_completion_rate <= Decimal::new(100, 0));
    }
}
