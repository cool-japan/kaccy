//! Platform-wide analytics and metrics models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// Platform-wide metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlatformMetrics {
    /// Unique identifier for this metrics snapshot
    pub metrics_id: Uuid,
    /// Total registered users
    pub total_users: i64,
    /// Active users (last 30 days)
    pub active_users_30d: i64,
    /// New users (last 7 days)
    pub new_users_7d: i64,
    /// Total tokens issued
    pub total_tokens: i64,
    /// Active tokens (not paused/closed)
    pub active_tokens: i64,
    /// Total orders created
    pub total_orders: i64,
    /// Total trades executed
    pub total_trades: i64,
    /// Total trading volume (BTC)
    pub total_volume_btc: Decimal,
    /// Trading volume last 24h (BTC)
    pub volume_24h_btc: Decimal,
    /// Trading volume last 7d (BTC)
    pub volume_7d_btc: Decimal,
    /// Total platform fees collected (BTC)
    pub total_fees_btc: Decimal,
    /// Number of KYC verified users
    pub kyc_verified_users: i64,
    /// Number of pending KYC submissions
    pub kyc_pending: i64,
    /// Average reputation score
    pub avg_reputation_score: Option<Decimal>,
    /// Total commitments created
    pub total_commitments: i64,
    /// Fulfilled commitments
    pub fulfilled_commitments: i64,
    /// Total liquidity pools
    pub total_liquidity_pools: i64,
    /// Total value locked (BTC)
    pub total_value_locked_btc: Decimal,
    /// Snapshot timestamp
    pub snapshot_at: DateTime<Utc>,
    /// Timestamp when this record was created
    pub created_at: DateTime<Utc>,
}

impl fmt::Display for PlatformMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PlatformMetrics(users={}, tokens={}, volume_24h={} BTC)",
            self.total_users, self.total_tokens, self.volume_24h_btc
        )
    }
}

impl PlatformMetrics {
    /// Calculate user growth rate (percentage)
    pub fn user_growth_rate_percent(&self) -> Decimal {
        if self.total_users == 0 {
            return dec!(0);
        }
        (Decimal::from(self.new_users_7d) / Decimal::from(self.total_users)) * dec!(100)
    }

    /// Calculate active user ratio (percentage)
    pub fn active_user_ratio_percent(&self) -> Decimal {
        if self.total_users == 0 {
            return dec!(0);
        }
        (Decimal::from(self.active_users_30d) / Decimal::from(self.total_users)) * dec!(100)
    }

    /// Calculate commitment fulfillment rate (percentage)
    pub fn commitment_fulfillment_rate_percent(&self) -> Decimal {
        if self.total_commitments == 0 {
            return dec!(0);
        }
        (Decimal::from(self.fulfilled_commitments) / Decimal::from(self.total_commitments))
            * dec!(100)
    }

    /// Calculate average order size (BTC)
    pub fn avg_order_size_btc(&self) -> Decimal {
        if self.total_orders == 0 {
            return dec!(0);
        }
        self.total_volume_btc / Decimal::from(self.total_orders)
    }

    /// Calculate KYC completion rate (percentage)
    pub fn kyc_completion_rate_percent(&self) -> Decimal {
        if self.total_users == 0 {
            return dec!(0);
        }
        (Decimal::from(self.kyc_verified_users) / Decimal::from(self.total_users)) * dec!(100)
    }
}

/// Time-series metrics for charting and trending
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimeSeriesMetrics {
    /// Unique identifier for this data point
    pub id: Uuid,
    /// Metric name (e.g., "daily_active_users", "trading_volume")
    pub metric_name: String,
    /// Metric value
    pub value: Decimal,
    /// Time period for this metric
    pub period_start: DateTime<Utc>,
    /// End of the time period for this metric
    pub period_end: DateTime<Utc>,
    /// Granularity (hourly, daily, weekly, monthly)
    pub granularity: MetricGranularity,
    /// Timestamp when this record was created
    pub created_at: DateTime<Utc>,
}

/// Granularity for time-series metrics
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum MetricGranularity {
    /// One data point per hour
    Hourly,
    /// One data point per day
    #[default]
    Daily,
    /// One data point per week
    Weekly,
    /// One data point per month
    Monthly,
}

impl fmt::Display for MetricGranularity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricGranularity::Hourly => write!(f, "hourly"),
            MetricGranularity::Daily => write!(f, "daily"),
            MetricGranularity::Weekly => write!(f, "weekly"),
            MetricGranularity::Monthly => write!(f, "monthly"),
        }
    }
}

/// Token-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetrics {
    /// Token this data belongs to
    pub token_id: Uuid,
    /// Token symbol
    pub symbol: String,
    /// Total holders
    pub holder_count: i64,
    /// Total trades
    pub trade_count: i64,
    /// Trading volume (BTC)
    pub volume_btc: Decimal,
    /// Current market cap (based on circulating supply * current price)
    pub market_cap_btc: Decimal,
    /// 24h price change (percentage)
    pub price_change_24h_percent: Decimal,
    /// 7d price change (percentage)
    pub price_change_7d_percent: Decimal,
    /// All-time high price
    pub ath_price_btc: Decimal,
    /// All-time high date
    pub ath_date: DateTime<Utc>,
    /// Current price
    pub current_price_btc: Decimal,
    /// Liquidity score (0-100)
    pub liquidity_score: Decimal,
}

impl TokenMetrics {
    /// Calculate velocity (trades per holder)
    pub fn velocity(&self) -> Decimal {
        if self.holder_count == 0 {
            return dec!(0);
        }
        Decimal::from(self.trade_count) / Decimal::from(self.holder_count)
    }

    /// Calculate distance from ATH (percentage)
    pub fn distance_from_ath_percent(&self) -> Decimal {
        if self.ath_price_btc == dec!(0) {
            return dec!(0);
        }
        ((self.ath_price_btc - self.current_price_btc) / self.ath_price_btc) * dec!(100)
    }
}

/// User activity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivityMetrics {
    /// User this data belongs to
    pub user_id: Uuid,
    /// Total tokens created
    pub tokens_created: i32,
    /// Total orders placed
    pub orders_placed: i32,
    /// Total trades executed
    pub trades_executed: i32,
    /// Total trading volume (BTC)
    pub total_volume_btc: Decimal,
    /// Total fees paid (BTC)
    pub total_fees_paid_btc: Decimal,
    /// Number of different tokens traded
    pub tokens_traded_count: i32,
    /// Days active on platform
    pub days_active: i32,
    /// Last activity timestamp
    pub last_activity_at: DateTime<Utc>,
    /// Account age (days)
    pub account_age_days: i32,
}

impl UserActivityMetrics {
    /// Calculate average daily volume
    pub fn avg_daily_volume_btc(&self) -> Decimal {
        if self.days_active == 0 {
            return dec!(0);
        }
        self.total_volume_btc / Decimal::from(self.days_active)
    }

    /// Check if user is considered active (activity in last 30 days)
    pub fn is_active(&self) -> bool {
        let days_since_activity = (Utc::now() - self.last_activity_at).num_days();
        days_since_activity <= 30
    }

    /// Calculate engagement score (0-100)
    pub fn engagement_score(&self) -> Decimal {
        let mut score = dec!(0);

        // Points for trading activity (max 40)
        if self.trades_executed > 0 {
            score += dec!(10);
        }
        if self.trades_executed >= 10 {
            score += dec!(10);
        }
        if self.trades_executed >= 50 {
            score += dec!(10);
        }
        if self.trades_executed >= 100 {
            score += dec!(10);
        }

        // Points for token creation (max 20)
        if self.tokens_created > 0 {
            score += dec!(10);
        }
        if self.tokens_created >= 3 {
            score += dec!(10);
        }

        // Points for volume (max 20)
        if self.total_volume_btc >= dec!(0.01) {
            score += dec!(5);
        }
        if self.total_volume_btc >= dec!(0.1) {
            score += dec!(5);
        }
        if self.total_volume_btc >= dec!(1) {
            score += dec!(5);
        }
        if self.total_volume_btc >= dec!(10) {
            score += dec!(5);
        }

        // Points for being active (max 20)
        if self.is_active() {
            score += dec!(10);
        }
        if self.days_active >= 30 {
            score += dec!(5);
        }
        if self.days_active >= 90 {
            score += dec!(5);
        }

        score.min(dec!(100))
    }
}

/// System health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    /// API uptime percentage (last 24h)
    pub uptime_24h_percent: Decimal,
    /// Average API response time (milliseconds)
    pub avg_response_time_ms: f64,
    /// Error rate percentage
    pub error_rate_percent: Decimal,
    /// Number of active database connections
    pub db_connections_active: i32,
    /// Database pool utilization percentage
    pub db_pool_utilization_percent: Decimal,
    /// Bitcoin node sync status
    pub btc_node_synced: bool,
    /// Bitcoin node block height
    pub btc_node_block_height: i64,
    /// Number of pending payment orders
    pub pending_payments: i64,
    /// Number of pending KYC reviews
    pub pending_kyc_reviews: i64,
    /// Last health check timestamp
    pub checked_at: DateTime<Utc>,
}

impl SystemHealthMetrics {
    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.uptime_24h_percent >= dec!(99)
            && self.error_rate_percent < dec!(1)
            && self.btc_node_synced
            && self.db_pool_utilization_percent < dec!(90)
    }

    /// Get health status
    pub fn health_status(&self) -> HealthStatus {
        if !self.btc_node_synced {
            return HealthStatus::Critical;
        }
        if self.uptime_24h_percent < dec!(95) || self.error_rate_percent > dec!(5) {
            return HealthStatus::Critical;
        }
        if self.uptime_24h_percent < dec!(99) || self.error_rate_percent > dec!(1) {
            return HealthStatus::Degraded;
        }
        if self.db_pool_utilization_percent > dec!(80) {
            return HealthStatus::Warning;
        }
        HealthStatus::Healthy
    }
}

/// System health status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// All systems operating normally
    Healthy,
    /// System is operating but approaching thresholds
    Warning,
    /// System is degraded and performance is impacted
    Degraded,
    /// System has a critical failure
    Critical,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Warning => write!(f, "warning"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Critical => write!(f, "critical"),
        }
    }
}

/// Leaderboard entry for top traders/issuers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformLeaderboardEntry {
    /// Position in the leaderboard (1 = first place)
    pub rank: i32,
    /// User this entry represents
    pub user_id: Uuid,
    /// User's login name
    pub username: String,
    /// User's display name if set
    pub display_name: Option<String>,
    /// Score based on leaderboard type
    pub score: Decimal,
    /// Optional secondary metric
    pub secondary_score: Option<Decimal>,
}

/// Type of leaderboard
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardType {
    /// Top traders by volume
    TopTraders,
    /// Top token issuers by success
    TopIssuers,
    /// Top reputation scores
    TopReputation,
    /// Most active users
    MostActive,
}

impl fmt::Display for LeaderboardType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaderboardType::TopTraders => write!(f, "Top Traders"),
            LeaderboardType::TopIssuers => write!(f, "Top Issuers"),
            LeaderboardType::TopReputation => write!(f, "Top Reputation"),
            LeaderboardType::MostActive => write!(f, "Most Active"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_metrics_calculations() {
        let metrics = PlatformMetrics {
            metrics_id: Uuid::new_v4(),
            total_users: 100,
            active_users_30d: 60,
            new_users_7d: 10,
            total_tokens: 50,
            active_tokens: 45,
            total_orders: 200,
            total_trades: 180,
            total_volume_btc: dec!(10),
            volume_24h_btc: dec!(1),
            volume_7d_btc: dec!(5),
            total_fees_btc: dec!(0.1),
            kyc_verified_users: 30,
            kyc_pending: 5,
            avg_reputation_score: Some(dec!(65)),
            total_commitments: 20,
            fulfilled_commitments: 15,
            total_liquidity_pools: 10,
            total_value_locked_btc: dec!(50),
            snapshot_at: Utc::now(),
            created_at: Utc::now(),
        };

        assert_eq!(metrics.user_growth_rate_percent(), dec!(10));
        assert_eq!(metrics.active_user_ratio_percent(), dec!(60));
        assert_eq!(metrics.commitment_fulfillment_rate_percent(), dec!(75));
        assert_eq!(metrics.avg_order_size_btc(), dec!(0.05));
        assert_eq!(metrics.kyc_completion_rate_percent(), dec!(30));
    }

    #[test]
    fn test_token_metrics() {
        let metrics = TokenMetrics {
            token_id: Uuid::new_v4(),
            symbol: "$TEST".to_string(),
            holder_count: 50,
            trade_count: 200,
            volume_btc: dec!(5),
            market_cap_btc: dec!(10),
            price_change_24h_percent: dec!(5),
            price_change_7d_percent: dec!(15),
            ath_price_btc: dec!(0.002),
            ath_date: Utc::now(),
            current_price_btc: dec!(0.0015),
            liquidity_score: dec!(75),
        };

        assert_eq!(metrics.velocity(), dec!(4));
        assert_eq!(metrics.distance_from_ath_percent(), dec!(25));
    }

    #[test]
    fn test_user_activity_metrics() {
        let metrics = UserActivityMetrics {
            user_id: Uuid::new_v4(),
            tokens_created: 2,
            orders_placed: 50,
            trades_executed: 45,
            total_volume_btc: dec!(1),
            total_fees_paid_btc: dec!(0.01),
            tokens_traded_count: 10,
            days_active: 30,
            last_activity_at: Utc::now() - chrono::Duration::days(5),
            account_age_days: 60,
        };

        assert!(metrics.is_active());
        assert_eq!(metrics.avg_daily_volume_btc(), dec!(1) / dec!(30));

        let score = metrics.engagement_score();
        assert!(score > dec!(0) && score <= dec!(100));
    }

    #[test]
    fn test_system_health() {
        let healthy_metrics = SystemHealthMetrics {
            uptime_24h_percent: dec!(99.9),
            avg_response_time_ms: 50.0,
            error_rate_percent: dec!(0.1),
            db_connections_active: 10,
            db_pool_utilization_percent: dec!(50),
            btc_node_synced: true,
            btc_node_block_height: 800000,
            pending_payments: 5,
            pending_kyc_reviews: 2,
            checked_at: Utc::now(),
        };

        assert!(healthy_metrics.is_healthy());
        assert_eq!(healthy_metrics.health_status(), HealthStatus::Healthy);

        let critical_metrics = SystemHealthMetrics {
            uptime_24h_percent: dec!(90),
            avg_response_time_ms: 500.0,
            error_rate_percent: dec!(10),
            db_connections_active: 50,
            db_pool_utilization_percent: dec!(95),
            btc_node_synced: false,
            btc_node_block_height: 800000,
            pending_payments: 100,
            pending_kyc_reviews: 50,
            checked_at: Utc::now(),
        };

        assert!(!critical_metrics.is_healthy());
        assert_eq!(critical_metrics.health_status(), HealthStatus::Critical);
    }
}
