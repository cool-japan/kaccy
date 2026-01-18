//! Analytics and materialized views for dashboard metrics
//!
//! This module provides SQL for creating and refreshing materialized views
//! that pre-compute common dashboard metrics for better performance.
//!
//! It also includes TimescaleDB integration for time-series data (price and volume history).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Dashboard overview metrics
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DashboardMetrics {
    /// Total registered users
    pub total_users: i64,
    /// Users registered in last 24 hours
    pub new_users_24h: i64,
    /// Users registered in last 7 days
    pub new_users_7d: i64,
    /// Total active tokens
    pub total_tokens: i64,
    /// Tokens created in last 24 hours
    pub new_tokens_24h: i64,
    /// Total trades executed
    pub total_trades: i64,
    /// Trades in last 24 hours
    pub trades_24h: i64,
    /// Total trading volume in BTC (satoshis)
    pub total_volume_sats: i64,
    /// Volume in last 24 hours (satoshis)
    pub volume_24h_sats: i64,
    /// Total platform fees collected (satoshis)
    pub total_fees_sats: i64,
    /// Fees in last 24 hours (satoshis)
    pub fees_24h_sats: i64,
    /// Pending commitments count
    pub pending_commitments: i64,
    /// Pending KYC applications
    pub pending_kyc: i64,
}

/// Token metrics summary
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TokenMetricsSummary {
    /// Token ID
    pub token_id: uuid::Uuid,
    /// Token symbol
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Total supply
    pub total_supply: rust_decimal::Decimal,
    /// Number of holders
    pub holder_count: i64,
    /// Total trades for this token
    pub trade_count: i64,
    /// 24h trade count
    pub trades_24h: i64,
    /// Total volume in BTC
    pub total_volume_btc: rust_decimal::Decimal,
    /// 24h volume in BTC
    pub volume_24h_btc: rust_decimal::Decimal,
    /// Current price (last trade or bonding curve)
    pub current_price_btc: rust_decimal::Decimal,
    /// Price change 24h percentage
    pub price_change_24h_pct: rust_decimal::Decimal,
}

/// User activity summary
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserActivitySummary {
    /// User ID
    pub user_id: uuid::Uuid,
    /// Username
    pub username: String,
    /// Total trades made
    pub trade_count: i64,
    /// Total trading volume
    pub total_volume_btc: rust_decimal::Decimal,
    /// Number of tokens held
    pub tokens_held: i64,
    /// Number of tokens issued
    pub tokens_issued: i64,
    /// Reputation score
    pub reputation_score: i32,
    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Daily statistics
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DailyStats {
    /// Date
    pub date: chrono::NaiveDate,
    /// New users
    pub new_users: i64,
    /// New tokens
    pub new_tokens: i64,
    /// Number of trades
    pub trade_count: i64,
    /// Trading volume (satoshis)
    pub volume_sats: i64,
    /// Fees collected (satoshis)
    pub fees_sats: i64,
    /// Active users (made a trade)
    pub active_users: i64,
}

/// Price history record (TimescaleDB hypertable)
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PriceHistory {
    /// Timestamp
    pub time: DateTime<Utc>,
    /// Token ID
    pub token_id: Uuid,
    /// Price in satoshis
    pub price_satoshis: i64,
    /// Total supply at this time
    pub supply: i64,
    /// Market cap in satoshis
    pub market_cap_satoshis: i64,
}

/// Volume history record (TimescaleDB hypertable)
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct VolumeHistory {
    /// Timestamp
    pub time: DateTime<Utc>,
    /// Token ID
    pub token_id: Uuid,
    /// Buy volume in satoshis
    pub buy_volume_satoshis: i64,
    /// Sell volume in satoshis
    pub sell_volume_satoshis: i64,
    /// Number of trades
    pub trade_count: i32,
    /// Number of unique traders
    pub unique_traders: i32,
}

/// Platform volume history record (TimescaleDB hypertable)
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PlatformVolumeHistory {
    /// Timestamp
    pub time: DateTime<Utc>,
    /// Total volume in satoshis
    pub total_volume_satoshis: i64,
    /// Number of trades
    pub trade_count: i32,
    /// Number of active tokens
    pub active_tokens: i32,
    /// Number of active traders
    pub active_traders: i32,
    /// Fees collected in satoshis
    pub fees_collected_satoshis: i64,
}

/// OHLC (Open-High-Low-Close) price data
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OhlcData {
    /// Time bucket
    pub bucket: DateTime<Utc>,
    /// Token ID
    pub token_id: Uuid,
    /// Opening price
    pub open_price: i64,
    /// Highest price
    pub high_price: i64,
    /// Lowest price
    pub low_price: i64,
    /// Closing price
    pub close_price: i64,
    /// Final supply
    pub final_supply: i64,
    /// Final market cap
    pub final_market_cap: i64,
}

/// Analytics service for dashboard metrics
pub struct AnalyticsService {
    pool: PgPool,
}

impl AnalyticsService {
    /// Create a new analytics service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create all materialized views
    pub async fn create_materialized_views(&self) -> Result<()> {
        // Create dashboard metrics view
        sqlx::query(DASHBOARD_METRICS_VIEW_SQL)
            .execute(&self.pool)
            .await?;

        // Create token metrics view
        sqlx::query(TOKEN_METRICS_VIEW_SQL)
            .execute(&self.pool)
            .await?;

        // Create daily stats view
        sqlx::query(DAILY_STATS_VIEW_SQL)
            .execute(&self.pool)
            .await?;

        // Create user activity view
        sqlx::query(USER_ACTIVITY_VIEW_SQL)
            .execute(&self.pool)
            .await?;

        tracing::info!("Created all materialized views for analytics");
        Ok(())
    }

    /// Refresh all materialized views
    pub async fn refresh_all_views(&self) -> Result<()> {
        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY IF EXISTS mv_dashboard_metrics")
            .execute(&self.pool)
            .await
            .ok(); // Ignore if doesn't exist

        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY IF EXISTS mv_token_metrics")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY IF EXISTS mv_daily_stats")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY IF EXISTS mv_user_activity")
            .execute(&self.pool)
            .await
            .ok();

        tracing::debug!("Refreshed all materialized views");
        Ok(())
    }

    /// Get dashboard metrics (from materialized view if available, otherwise compute)
    pub async fn get_dashboard_metrics(&self) -> Result<DashboardMetrics> {
        // Try materialized view first
        let result =
            sqlx::query_as::<_, DashboardMetrics>("SELECT * FROM mv_dashboard_metrics LIMIT 1")
                .fetch_optional(&self.pool)
                .await;

        if let Ok(Some(metrics)) = result {
            return Ok(metrics);
        }

        // Fall back to computed metrics
        self.compute_dashboard_metrics().await
    }

    /// Compute dashboard metrics directly (without materialized view)
    pub async fn compute_dashboard_metrics(&self) -> Result<DashboardMetrics> {
        let metrics = sqlx::query_as::<_, DashboardMetrics>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM users) as total_users,
                (SELECT COUNT(*) FROM users WHERE created_at > NOW() - INTERVAL '24 hours') as new_users_24h,
                (SELECT COUNT(*) FROM users WHERE created_at > NOW() - INTERVAL '7 days') as new_users_7d,
                (SELECT COUNT(*) FROM tokens WHERE status = 'active') as total_tokens,
                (SELECT COUNT(*) FROM tokens WHERE created_at > NOW() - INTERVAL '24 hours') as new_tokens_24h,
                (SELECT COUNT(*) FROM trades) as total_trades,
                (SELECT COUNT(*) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours') as trades_24h,
                COALESCE((SELECT SUM((total_btc * 100000000)::bigint) FROM trades), 0) as total_volume_sats,
                COALESCE((SELECT SUM((total_btc * 100000000)::bigint) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours'), 0) as volume_24h_sats,
                COALESCE((SELECT SUM((platform_fee * 100000000)::bigint) FROM trades), 0) as total_fees_sats,
                COALESCE((SELECT SUM((platform_fee * 100000000)::bigint) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours'), 0) as fees_24h_sats,
                (SELECT COUNT(*) FROM output_commitments WHERE status = 'pending') as pending_commitments,
                (SELECT COUNT(*) FROM kyc_applications WHERE status = 'pending') as pending_kyc
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(metrics)
    }

    /// Get token metrics (top tokens by volume)
    pub async fn get_top_tokens(&self, limit: i64) -> Result<Vec<TokenMetricsSummary>> {
        // Try materialized view first
        let result = sqlx::query_as::<_, TokenMetricsSummary>(
            r#"
            SELECT * FROM mv_token_metrics
            ORDER BY volume_24h_btc DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await;

        if let Ok(tokens) = result {
            if !tokens.is_empty() {
                return Ok(tokens);
            }
        }

        // Fall back to computed query
        self.compute_top_tokens(limit).await
    }

    /// Compute top tokens directly
    async fn compute_top_tokens(&self, limit: i64) -> Result<Vec<TokenMetricsSummary>> {
        let tokens = sqlx::query_as::<_, TokenMetricsSummary>(
            r#"
            SELECT
                t.token_id,
                t.symbol,
                t.name,
                t.total_supply,
                COALESCE(h.holder_count, 0) as holder_count,
                COALESCE(tr.trade_count, 0) as trade_count,
                COALESCE(tr.trades_24h, 0) as trades_24h,
                COALESCE(tr.total_volume_btc, 0) as total_volume_btc,
                COALESCE(tr.volume_24h_btc, 0) as volume_24h_btc,
                COALESCE(tr.last_price, t.base_price) as current_price_btc,
                COALESCE(
                    CASE WHEN tr.price_24h_ago > 0
                         THEN ((tr.last_price - tr.price_24h_ago) / tr.price_24h_ago * 100)
                         ELSE 0
                    END,
                    0
                ) as price_change_24h_pct
            FROM tokens t
            LEFT JOIN (
                SELECT token_id, COUNT(DISTINCT user_id) as holder_count
                FROM balances
                WHERE amount > 0
                GROUP BY token_id
            ) h ON h.token_id = t.token_id
            LEFT JOIN (
                SELECT
                    token_id,
                    COUNT(*) as trade_count,
                    COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '24 hours') as trades_24h,
                    SUM(total_btc) as total_volume_btc,
                    SUM(total_btc) FILTER (WHERE created_at > NOW() - INTERVAL '24 hours') as volume_24h_btc,
                    (SELECT price_btc FROM trades tr2 WHERE tr2.token_id = trades.token_id ORDER BY created_at DESC LIMIT 1) as last_price,
                    (SELECT price_btc FROM trades tr2 WHERE tr2.token_id = trades.token_id AND tr2.created_at < NOW() - INTERVAL '24 hours' ORDER BY created_at DESC LIMIT 1) as price_24h_ago
                FROM trades
                GROUP BY token_id
            ) tr ON tr.token_id = t.token_id
            WHERE t.status = 'active'
            ORDER BY COALESCE(tr.volume_24h_btc, 0) DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tokens)
    }

    /// Get daily statistics for a date range
    pub async fn get_daily_stats(
        &self,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<DailyStats>> {
        // Try materialized view first
        let result = sqlx::query_as::<_, DailyStats>(
            r#"
            SELECT * FROM mv_daily_stats
            WHERE date >= $1 AND date <= $2
            ORDER BY date DESC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await;

        if let Ok(stats) = result {
            if !stats.is_empty() {
                return Ok(stats);
            }
        }

        // Fall back to computed query
        self.compute_daily_stats(start_date, end_date).await
    }

    /// Compute daily statistics directly
    async fn compute_daily_stats(
        &self,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> Result<Vec<DailyStats>> {
        let stats = sqlx::query_as::<_, DailyStats>(
            r#"
            WITH dates AS (
                SELECT generate_series($1::date, $2::date, '1 day'::interval)::date as date
            )
            SELECT
                d.date,
                COALESCE(u.new_users, 0) as new_users,
                COALESCE(t.new_tokens, 0) as new_tokens,
                COALESCE(tr.trade_count, 0) as trade_count,
                COALESCE(tr.volume_sats, 0) as volume_sats,
                COALESCE(tr.fees_sats, 0) as fees_sats,
                COALESCE(tr.active_users, 0) as active_users
            FROM dates d
            LEFT JOIN (
                SELECT DATE(created_at) as date, COUNT(*) as new_users
                FROM users
                WHERE DATE(created_at) >= $1 AND DATE(created_at) <= $2
                GROUP BY DATE(created_at)
            ) u ON u.date = d.date
            LEFT JOIN (
                SELECT DATE(created_at) as date, COUNT(*) as new_tokens
                FROM tokens
                WHERE DATE(created_at) >= $1 AND DATE(created_at) <= $2
                GROUP BY DATE(created_at)
            ) t ON t.date = d.date
            LEFT JOIN (
                SELECT
                    DATE(created_at) as date,
                    COUNT(*) as trade_count,
                    SUM((total_btc * 100000000)::bigint) as volume_sats,
                    SUM((platform_fee * 100000000)::bigint) as fees_sats,
                    COUNT(DISTINCT buyer_id) + COUNT(DISTINCT seller_id) as active_users
                FROM trades
                WHERE DATE(created_at) >= $1 AND DATE(created_at) <= $2
                GROUP BY DATE(created_at)
            ) tr ON tr.date = d.date
            ORDER BY d.date DESC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get top users by trading volume
    pub async fn get_top_users(&self, limit: i64) -> Result<Vec<UserActivitySummary>> {
        let users = sqlx::query_as::<_, UserActivitySummary>(
            r#"
            SELECT
                u.user_id,
                u.username,
                COALESCE(t.trade_count, 0) as trade_count,
                COALESCE(t.total_volume_btc, 0) as total_volume_btc,
                COALESCE(b.tokens_held, 0) as tokens_held,
                COALESCE(tk.tokens_issued, 0) as tokens_issued,
                u.reputation_score,
                GREATEST(t.last_trade, u.created_at) as last_activity
            FROM users u
            LEFT JOIN (
                SELECT
                    user_id,
                    COUNT(*) as trade_count,
                    SUM(total_btc) as total_volume_btc,
                    MAX(created_at) as last_trade
                FROM (
                    SELECT buyer_id as user_id, total_btc, created_at FROM trades
                    UNION ALL
                    SELECT seller_id as user_id, total_btc, created_at FROM trades
                ) all_trades
                GROUP BY user_id
            ) t ON t.user_id = u.user_id
            LEFT JOIN (
                SELECT user_id, COUNT(DISTINCT token_id) as tokens_held
                FROM balances
                WHERE amount > 0
                GROUP BY user_id
            ) b ON b.user_id = u.user_id
            LEFT JOIN (
                SELECT issuer_id as user_id, COUNT(*) as tokens_issued
                FROM tokens
                GROUP BY issuer_id
            ) tk ON tk.user_id = u.user_id
            ORDER BY COALESCE(t.total_volume_btc, 0) DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Drop all materialized views
    pub async fn drop_materialized_views(&self) -> Result<()> {
        sqlx::query("DROP MATERIALIZED VIEW IF EXISTS mv_dashboard_metrics CASCADE")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP MATERIALIZED VIEW IF EXISTS mv_token_metrics CASCADE")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP MATERIALIZED VIEW IF EXISTS mv_daily_stats CASCADE")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP MATERIALIZED VIEW IF EXISTS mv_user_activity CASCADE")
            .execute(&self.pool)
            .await?;

        tracing::info!("Dropped all materialized views");
        Ok(())
    }

    // ========================================================================
    // TimescaleDB Time-Series Functions
    // ========================================================================

    /// Record price history data point
    ///
    /// Inserts a price history record into the TimescaleDB hypertable.
    /// Safe to call even if TimescaleDB is not installed (uses regular table).
    pub async fn record_price_history(
        &self,
        token_id: Uuid,
        price_satoshis: i64,
        supply: i64,
    ) -> Result<()> {
        let market_cap_satoshis = price_satoshis.saturating_mul(supply);

        sqlx::query(
            r#"
            INSERT INTO price_history (time, token_id, price_satoshis, supply, market_cap_satoshis)
            VALUES (NOW(), $1, $2, $3, $4)
            ON CONFLICT (time, token_id) DO UPDATE
            SET price_satoshis = EXCLUDED.price_satoshis,
                supply = EXCLUDED.supply,
                market_cap_satoshis = EXCLUDED.market_cap_satoshis
            "#,
        )
        .bind(token_id)
        .bind(price_satoshis)
        .bind(supply)
        .bind(market_cap_satoshis)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record volume history data point
    ///
    /// Inserts or updates a volume history record for a specific time bucket.
    /// Typically called on trade execution to update the current hour's volume.
    pub async fn record_volume_history(
        &self,
        token_id: Uuid,
        buy_volume_satoshis: i64,
        sell_volume_satoshis: i64,
        trade_count: i32,
        unique_traders: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO volume_history
                (time, token_id, buy_volume_satoshis, sell_volume_satoshis, trade_count, unique_traders)
            VALUES (date_trunc('hour', NOW()), $1, $2, $3, $4, $5)
            ON CONFLICT (time, token_id) DO UPDATE
            SET buy_volume_satoshis = volume_history.buy_volume_satoshis + EXCLUDED.buy_volume_satoshis,
                sell_volume_satoshis = volume_history.sell_volume_satoshis + EXCLUDED.sell_volume_satoshis,
                trade_count = volume_history.trade_count + EXCLUDED.trade_count,
                unique_traders = GREATEST(volume_history.unique_traders, EXCLUDED.unique_traders)
            "#,
        )
        .bind(token_id)
        .bind(buy_volume_satoshis)
        .bind(sell_volume_satoshis)
        .bind(trade_count)
        .bind(unique_traders)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record platform-wide volume history
    pub async fn record_platform_volume(
        &self,
        total_volume_satoshis: i64,
        trade_count: i32,
        active_tokens: i32,
        active_traders: i32,
        fees_collected_satoshis: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO platform_volume_history
                (time, total_volume_satoshis, trade_count, active_tokens, active_traders, fees_collected_satoshis)
            VALUES (date_trunc('hour', NOW()), $1, $2, $3, $4, $5)
            ON CONFLICT (time) DO UPDATE
            SET total_volume_satoshis = platform_volume_history.total_volume_satoshis + EXCLUDED.total_volume_satoshis,
                trade_count = platform_volume_history.trade_count + EXCLUDED.trade_count,
                active_tokens = GREATEST(platform_volume_history.active_tokens, EXCLUDED.active_tokens),
                active_traders = GREATEST(platform_volume_history.active_traders, EXCLUDED.active_traders),
                fees_collected_satoshis = platform_volume_history.fees_collected_satoshis + EXCLUDED.fees_collected_satoshis
            "#,
        )
        .bind(total_volume_satoshis)
        .bind(trade_count)
        .bind(active_tokens)
        .bind(active_traders)
        .bind(fees_collected_satoshis)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get price history for a token within a time range
    pub async fn get_price_history(
        &self,
        token_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<PriceHistory>> {
        let history = sqlx::query_as::<_, PriceHistory>(
            r#"
            SELECT time, token_id, price_satoshis, supply, market_cap_satoshis
            FROM price_history
            WHERE token_id = $1 AND time >= $2 AND time <= $3
            ORDER BY time ASC
            "#,
        )
        .bind(token_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Get volume history for a token within a time range
    pub async fn get_volume_history(
        &self,
        token_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<VolumeHistory>> {
        let history = sqlx::query_as::<_, VolumeHistory>(
            r#"
            SELECT time, token_id, buy_volume_satoshis, sell_volume_satoshis, trade_count, unique_traders
            FROM volume_history
            WHERE token_id = $1 AND time >= $2 AND time <= $3
            ORDER BY time ASC
            "#,
        )
        .bind(token_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Get OHLC (candlestick) data for a token
    ///
    /// Returns OHLC data with configurable time buckets (e.g., '1 hour', '1 day', '1 week').
    /// Uses TimescaleDB's continuous aggregate if available, otherwise computes from raw data.
    pub async fn get_ohlc_data(
        &self,
        token_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval: &str, // e.g., "1 hour", "1 day", "1 week"
    ) -> Result<Vec<OhlcData>> {
        // Try to use continuous aggregate for hourly data
        if bucket_interval == "1 hour" {
            if let Ok(data) = self
                .get_ohlc_from_aggregate(token_id, start_time, end_time)
                .await
            {
                if !data.is_empty() {
                    return Ok(data);
                }
            }
        }

        // Fall back to computing from raw data
        self.compute_ohlc_data(token_id, start_time, end_time, bucket_interval)
            .await
    }

    /// Get OHLC data from continuous aggregate (if TimescaleDB is available)
    async fn get_ohlc_from_aggregate(
        &self,
        token_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<OhlcData>> {
        let data = sqlx::query_as::<_, OhlcData>(
            r#"
            SELECT bucket, token_id, open_price, high_price, low_price, close_price,
                   final_supply, final_market_cap
            FROM price_history_hourly
            WHERE token_id = $1 AND bucket >= $2 AND bucket <= $3
            ORDER BY bucket ASC
            "#,
        )
        .bind(token_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        Ok(data)
    }

    /// Compute OHLC data from raw price history
    async fn compute_ohlc_data(
        &self,
        token_id: Uuid,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval: &str,
    ) -> Result<Vec<OhlcData>> {
        let data = sqlx::query_as::<_, OhlcData>(
            r#"
            SELECT
                time_bucket($1::interval, time) AS bucket,
                token_id,
                first(price_satoshis, time) AS open_price,
                max(price_satoshis) AS high_price,
                min(price_satoshis) AS low_price,
                last(price_satoshis, time) AS close_price,
                last(supply, time) AS final_supply,
                last(market_cap_satoshis, time) AS final_market_cap
            FROM price_history
            WHERE token_id = $2 AND time >= $3 AND time <= $4
            GROUP BY bucket, token_id
            ORDER BY bucket ASC
            "#,
        )
        .bind(bucket_interval)
        .bind(token_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        Ok(data)
    }

    /// Get aggregated volume data by time bucket
    pub async fn get_aggregated_volume(
        &self,
        token_id: Option<Uuid>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval: &str, // e.g., "1 hour", "1 day"
    ) -> Result<Vec<VolumeHistory>> {
        let volume = if let Some(tid) = token_id {
            // Token-specific volume
            sqlx::query_as::<_, VolumeHistory>(
                r#"
                SELECT
                    time_bucket($1::interval, time) AS time,
                    token_id,
                    sum(buy_volume_satoshis) AS buy_volume_satoshis,
                    sum(sell_volume_satoshis) AS sell_volume_satoshis,
                    sum(trade_count)::int AS trade_count,
                    max(unique_traders)::int AS unique_traders
                FROM volume_history
                WHERE token_id = $2 AND time >= $3 AND time <= $4
                GROUP BY time_bucket($1::interval, time), token_id
                ORDER BY time ASC
                "#,
            )
            .bind(bucket_interval)
            .bind(tid)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await?
        } else {
            // Platform-wide volume (aggregate across all tokens)
            sqlx::query_as::<_, VolumeHistory>(
                r#"
                SELECT
                    time_bucket($1::interval, time) AS time,
                    '00000000-0000-0000-0000-000000000000'::uuid AS token_id,
                    sum(buy_volume_satoshis) AS buy_volume_satoshis,
                    sum(sell_volume_satoshis) AS sell_volume_satoshis,
                    sum(trade_count)::int AS trade_count,
                    sum(unique_traders)::int AS unique_traders
                FROM volume_history
                WHERE time >= $2 AND time <= $3
                GROUP BY time_bucket($1::interval, time)
                ORDER BY time ASC
                "#,
            )
            .bind(bucket_interval)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(volume)
    }

    /// Get platform-wide volume history
    pub async fn get_platform_volume_history(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<PlatformVolumeHistory>> {
        let history = sqlx::query_as::<_, PlatformVolumeHistory>(
            r#"
            SELECT time, total_volume_satoshis, trade_count, active_tokens,
                   active_traders, fees_collected_satoshis
            FROM platform_volume_history
            WHERE time >= $1 AND time <= $2
            ORDER BY time ASC
            "#,
        )
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Check if TimescaleDB extension is available
    pub async fn is_timescaledb_available(&self) -> bool {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'timescaledb')",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false)
    }

    /// Get hypertable information (only works with TimescaleDB)
    pub async fn get_hypertable_info(&self, table_name: &str) -> Result<Option<String>> {
        let info = sqlx::query_scalar::<_, String>(
            r#"
            SELECT format('Hypertable: %s, Chunks: %s, Compression: %s',
                          hypertable_name,
                          num_chunks,
                          compression_enabled)
            FROM timescaledb_information.hypertables
            WHERE hypertable_name = $1
            "#,
        )
        .bind(table_name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(info)
    }
}

/// SQL to create dashboard metrics materialized view
const DASHBOARD_METRICS_VIEW_SQL: &str = r#"
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_dashboard_metrics AS
SELECT
    (SELECT COUNT(*) FROM users) as total_users,
    (SELECT COUNT(*) FROM users WHERE created_at > NOW() - INTERVAL '24 hours') as new_users_24h,
    (SELECT COUNT(*) FROM users WHERE created_at > NOW() - INTERVAL '7 days') as new_users_7d,
    (SELECT COUNT(*) FROM tokens WHERE status = 'active') as total_tokens,
    (SELECT COUNT(*) FROM tokens WHERE created_at > NOW() - INTERVAL '24 hours') as new_tokens_24h,
    (SELECT COUNT(*) FROM trades) as total_trades,
    (SELECT COUNT(*) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours') as trades_24h,
    COALESCE((SELECT SUM((total_btc * 100000000)::bigint) FROM trades), 0) as total_volume_sats,
    COALESCE((SELECT SUM((total_btc * 100000000)::bigint) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours'), 0) as volume_24h_sats,
    COALESCE((SELECT SUM((platform_fee * 100000000)::bigint) FROM trades), 0) as total_fees_sats,
    COALESCE((SELECT SUM((platform_fee * 100000000)::bigint) FROM trades WHERE created_at > NOW() - INTERVAL '24 hours'), 0) as fees_24h_sats,
    (SELECT COUNT(*) FROM output_commitments WHERE status = 'pending') as pending_commitments,
    COALESCE((SELECT COUNT(*) FROM kyc_applications WHERE status = 'pending'), 0) as pending_kyc;

CREATE UNIQUE INDEX IF NOT EXISTS mv_dashboard_metrics_idx ON mv_dashboard_metrics ((1));
"#;

/// SQL to create token metrics materialized view
const TOKEN_METRICS_VIEW_SQL: &str = r#"
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_token_metrics AS
SELECT
    t.token_id,
    t.symbol,
    t.name,
    t.total_supply,
    COALESCE(h.holder_count, 0) as holder_count,
    COALESCE(tr.trade_count, 0) as trade_count,
    COALESCE(tr.trades_24h, 0) as trades_24h,
    COALESCE(tr.total_volume_btc, 0) as total_volume_btc,
    COALESCE(tr.volume_24h_btc, 0) as volume_24h_btc,
    COALESCE(tr.last_price, t.base_price) as current_price_btc,
    COALESCE(
        CASE WHEN tr.price_24h_ago > 0
             THEN ((tr.last_price - tr.price_24h_ago) / tr.price_24h_ago * 100)
             ELSE 0
        END,
        0
    ) as price_change_24h_pct
FROM tokens t
LEFT JOIN (
    SELECT token_id, COUNT(DISTINCT user_id) as holder_count
    FROM balances
    WHERE amount > 0
    GROUP BY token_id
) h ON h.token_id = t.token_id
LEFT JOIN (
    SELECT
        token_id,
        COUNT(*) as trade_count,
        COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '24 hours') as trades_24h,
        SUM(total_btc) as total_volume_btc,
        SUM(total_btc) FILTER (WHERE created_at > NOW() - INTERVAL '24 hours') as volume_24h_btc,
        (SELECT price_btc FROM trades tr2 WHERE tr2.token_id = trades.token_id ORDER BY created_at DESC LIMIT 1) as last_price,
        (SELECT price_btc FROM trades tr2 WHERE tr2.token_id = trades.token_id AND tr2.created_at < NOW() - INTERVAL '24 hours' ORDER BY created_at DESC LIMIT 1) as price_24h_ago
    FROM trades
    GROUP BY token_id
) tr ON tr.token_id = t.token_id
WHERE t.status = 'active';

CREATE UNIQUE INDEX IF NOT EXISTS mv_token_metrics_token_id_idx ON mv_token_metrics (token_id);
CREATE INDEX IF NOT EXISTS mv_token_metrics_volume_idx ON mv_token_metrics (volume_24h_btc DESC);
"#;

/// SQL to create daily stats materialized view
const DAILY_STATS_VIEW_SQL: &str = r#"
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_daily_stats AS
WITH dates AS (
    SELECT generate_series(
        (SELECT COALESCE(MIN(DATE(created_at)), CURRENT_DATE - INTERVAL '30 days') FROM users),
        CURRENT_DATE,
        '1 day'::interval
    )::date as date
)
SELECT
    d.date,
    COALESCE(u.new_users, 0)::bigint as new_users,
    COALESCE(t.new_tokens, 0)::bigint as new_tokens,
    COALESCE(tr.trade_count, 0)::bigint as trade_count,
    COALESCE(tr.volume_sats, 0)::bigint as volume_sats,
    COALESCE(tr.fees_sats, 0)::bigint as fees_sats,
    COALESCE(tr.active_users, 0)::bigint as active_users
FROM dates d
LEFT JOIN (
    SELECT DATE(created_at) as date, COUNT(*) as new_users
    FROM users
    GROUP BY DATE(created_at)
) u ON u.date = d.date
LEFT JOIN (
    SELECT DATE(created_at) as date, COUNT(*) as new_tokens
    FROM tokens
    GROUP BY DATE(created_at)
) t ON t.date = d.date
LEFT JOIN (
    SELECT
        DATE(created_at) as date,
        COUNT(*) as trade_count,
        SUM((total_btc * 100000000)::bigint) as volume_sats,
        SUM((platform_fee * 100000000)::bigint) as fees_sats,
        COUNT(DISTINCT buyer_id) + COUNT(DISTINCT seller_id) as active_users
    FROM trades
    GROUP BY DATE(created_at)
) tr ON tr.date = d.date;

CREATE UNIQUE INDEX IF NOT EXISTS mv_daily_stats_date_idx ON mv_daily_stats (date);
"#;

/// SQL to create user activity materialized view
const USER_ACTIVITY_VIEW_SQL: &str = r#"
CREATE MATERIALIZED VIEW IF NOT EXISTS mv_user_activity AS
SELECT
    u.user_id,
    u.username,
    COALESCE(t.trade_count, 0)::bigint as trade_count,
    COALESCE(t.total_volume_btc, 0) as total_volume_btc,
    COALESCE(b.tokens_held, 0)::bigint as tokens_held,
    COALESCE(tk.tokens_issued, 0)::bigint as tokens_issued,
    u.reputation_score,
    GREATEST(t.last_trade, u.created_at) as last_activity
FROM users u
LEFT JOIN (
    SELECT
        user_id,
        COUNT(*) as trade_count,
        SUM(total_btc) as total_volume_btc,
        MAX(created_at) as last_trade
    FROM (
        SELECT buyer_id as user_id, total_btc, created_at FROM trades
        UNION ALL
        SELECT seller_id as user_id, total_btc, created_at FROM trades
    ) all_trades
    GROUP BY user_id
) t ON t.user_id = u.user_id
LEFT JOIN (
    SELECT user_id, COUNT(DISTINCT token_id) as tokens_held
    FROM balances
    WHERE amount > 0
    GROUP BY user_id
) b ON b.user_id = u.user_id
LEFT JOIN (
    SELECT issuer_id as user_id, COUNT(*) as tokens_issued
    FROM tokens
    GROUP BY issuer_id
) tk ON tk.user_id = u.user_id;

CREATE UNIQUE INDEX IF NOT EXISTS mv_user_activity_user_id_idx ON mv_user_activity (user_id);
CREATE INDEX IF NOT EXISTS mv_user_activity_volume_idx ON mv_user_activity (total_volume_btc DESC);
"#;

/// Refresh job configuration
#[derive(Debug, Clone)]
pub struct RefreshConfig {
    /// Interval between refreshes in seconds
    pub refresh_interval_secs: u64,
    /// Whether to use concurrent refresh
    pub concurrent: bool,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            refresh_interval_secs: 300, // 5 minutes
            concurrent: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_config_defaults() {
        let config = RefreshConfig::default();
        assert_eq!(config.refresh_interval_secs, 300);
        assert!(config.concurrent);
    }

    #[test]
    fn test_price_history_creation() {
        let token_id = Uuid::new_v4();
        let price_history = PriceHistory {
            time: Utc::now(),
            token_id,
            price_satoshis: 100_000,
            supply: 1_000_000,
            market_cap_satoshis: 100_000_000_000,
        };

        assert_eq!(price_history.token_id, token_id);
        assert_eq!(price_history.price_satoshis, 100_000);
        assert_eq!(price_history.supply, 1_000_000);
        assert_eq!(price_history.market_cap_satoshis, 100_000_000_000);
    }

    #[test]
    fn test_volume_history_creation() {
        let token_id = Uuid::new_v4();
        let volume_history = VolumeHistory {
            time: Utc::now(),
            token_id,
            buy_volume_satoshis: 50_000_000,
            sell_volume_satoshis: 30_000_000,
            trade_count: 42,
            unique_traders: 15,
        };

        assert_eq!(volume_history.token_id, token_id);
        assert_eq!(volume_history.buy_volume_satoshis, 50_000_000);
        assert_eq!(volume_history.sell_volume_satoshis, 30_000_000);
        assert_eq!(volume_history.trade_count, 42);
        assert_eq!(volume_history.unique_traders, 15);
    }

    #[test]
    fn test_platform_volume_history_creation() {
        let platform_volume = PlatformVolumeHistory {
            time: Utc::now(),
            total_volume_satoshis: 1_000_000_000,
            trade_count: 1000,
            active_tokens: 50,
            active_traders: 200,
            fees_collected_satoshis: 5_000_000,
        };

        assert_eq!(platform_volume.total_volume_satoshis, 1_000_000_000);
        assert_eq!(platform_volume.trade_count, 1000);
        assert_eq!(platform_volume.active_tokens, 50);
        assert_eq!(platform_volume.active_traders, 200);
        assert_eq!(platform_volume.fees_collected_satoshis, 5_000_000);
    }

    #[test]
    fn test_ohlc_data_creation() {
        let token_id = Uuid::new_v4();
        let ohlc = OhlcData {
            bucket: Utc::now(),
            token_id,
            open_price: 95_000,
            high_price: 105_000,
            low_price: 90_000,
            close_price: 100_000,
            final_supply: 1_000_000,
            final_market_cap: 100_000_000_000,
        };

        assert_eq!(ohlc.token_id, token_id);
        assert_eq!(ohlc.open_price, 95_000);
        assert_eq!(ohlc.high_price, 105_000);
        assert_eq!(ohlc.low_price, 90_000);
        assert_eq!(ohlc.close_price, 100_000);
        assert!(ohlc.high_price >= ohlc.open_price);
        assert!(ohlc.high_price >= ohlc.close_price);
        assert!(ohlc.low_price <= ohlc.open_price);
        assert!(ohlc.low_price <= ohlc.close_price);
    }

    #[test]
    fn test_timescaledb_structures_are_serializable() {
        let token_id = Uuid::new_v4();
        let time = Utc::now();

        let price = PriceHistory {
            time,
            token_id,
            price_satoshis: 100_000,
            supply: 1_000_000,
            market_cap_satoshis: 100_000_000_000,
        };

        let volume = VolumeHistory {
            time,
            token_id,
            buy_volume_satoshis: 50_000_000,
            sell_volume_satoshis: 30_000_000,
            trade_count: 42,
            unique_traders: 15,
        };

        let platform = PlatformVolumeHistory {
            time,
            total_volume_satoshis: 1_000_000_000,
            trade_count: 1000,
            active_tokens: 50,
            active_traders: 200,
            fees_collected_satoshis: 5_000_000,
        };

        let ohlc = OhlcData {
            bucket: time,
            token_id,
            open_price: 95_000,
            high_price: 105_000,
            low_price: 90_000,
            close_price: 100_000,
            final_supply: 1_000_000,
            final_market_cap: 100_000_000_000,
        };

        // Verify all can be serialized
        assert!(serde_json::to_string(&price).is_ok());
        assert!(serde_json::to_string(&volume).is_ok());
        assert!(serde_json::to_string(&platform).is_ok());
        assert!(serde_json::to_string(&ohlc).is_ok());
    }

    #[test]
    fn test_price_history_market_cap_calculation() {
        let token_id = Uuid::new_v4();
        let price_satoshis = 100_000i64;
        let supply = 1_000_000i64;
        let expected_market_cap = price_satoshis.saturating_mul(supply);

        let price_history = PriceHistory {
            time: Utc::now(),
            token_id,
            price_satoshis,
            supply,
            market_cap_satoshis: expected_market_cap,
        };

        assert_eq!(price_history.market_cap_satoshis, 100_000_000_000);
        assert_eq!(
            price_history.market_cap_satoshis,
            price_history.price_satoshis * price_history.supply
        );
    }
}
