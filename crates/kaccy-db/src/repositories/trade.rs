//! Trade repository

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Trade data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TradeRow {
    pub trade_id: Uuid,
    pub buyer_user_id: Uuid,
    pub seller_user_id: Option<Uuid>,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub platform_fee_btc: Decimal,
    pub issuer_royalty_btc: Decimal,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

/// Trade with additional context
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TradeWithContextRow {
    pub trade_id: Uuid,
    pub buyer_user_id: Uuid,
    pub seller_user_id: Option<Uuid>,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub platform_fee_btc: Decimal,
    pub issuer_royalty_btc: Decimal,
    pub executed_at: chrono::DateTime<chrono::Utc>,
    pub buyer_username: String,
    pub seller_username: Option<String>,
    pub token_symbol: String,
}

/// Repository for trade operations
pub struct TradeRepository {
    pool: PgPool,
}

impl TradeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new trade
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        buyer_user_id: Uuid,
        seller_user_id: Option<Uuid>,
        token_id: Uuid,
        amount: Decimal,
        price_btc: Decimal,
        total_btc: Decimal,
        platform_fee_btc: Decimal,
        issuer_royalty_btc: Decimal,
    ) -> Result<TradeRow> {
        let trade = sqlx::query_as::<_, TradeRow>(
            r#"
            INSERT INTO trades (buyer_user_id, seller_user_id, token_id, amount, price_btc, total_btc, platform_fee_btc, issuer_royalty_btc)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(buyer_user_id)
        .bind(seller_user_id)
        .bind(token_id)
        .bind(amount)
        .bind(price_btc)
        .bind(total_btc)
        .bind(platform_fee_btc)
        .bind(issuer_royalty_btc)
        .fetch_one(&self.pool)
        .await?;

        Ok(trade)
    }

    /// Get trades for a token
    pub async fn get_token_trades(
        &self,
        token_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TradeWithContextRow>> {
        let trades = sqlx::query_as::<_, TradeWithContextRow>(
            r#"
            SELECT
                t.*,
                buyer.username as buyer_username,
                seller.username as seller_username,
                tok.symbol as token_symbol
            FROM trades t
            JOIN users buyer ON t.buyer_user_id = buyer.user_id
            LEFT JOIN users seller ON t.seller_user_id = seller.user_id
            JOIN tokens tok ON t.token_id = tok.token_id
            WHERE t.token_id = $1
            ORDER BY t.executed_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(token_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(trades)
    }

    /// Get trades for a user (optimized with UNION to avoid N+1 query issues)
    ///
    /// Uses UNION ALL to efficiently query both buyer and seller indexes separately,
    /// which is much faster than OR conditions on large tables.
    pub async fn get_user_trades(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TradeWithContextRow>> {
        let trades = sqlx::query_as::<_, TradeWithContextRow>(
            r#"
            SELECT
                t.*,
                buyer.username as buyer_username,
                seller.username as seller_username,
                tok.symbol as token_symbol
            FROM (
                SELECT * FROM trades WHERE buyer_user_id = $1
                UNION ALL
                SELECT * FROM trades WHERE seller_user_id = $1
            ) t
            JOIN users buyer ON t.buyer_user_id = buyer.user_id
            LEFT JOIN users seller ON t.seller_user_id = seller.user_id
            JOIN tokens tok ON t.token_id = tok.token_id
            ORDER BY t.executed_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(trades)
    }

    /// Get trade summary for a token
    pub async fn get_token_summary(&self, token_id: Uuid) -> Result<TradeSummary> {
        let summary = sqlx::query_as::<_, TradeSummary>(
            r#"
            SELECT
                COUNT(*) as total_trades,
                COALESCE(SUM(total_btc), 0) as total_volume_btc,
                COALESCE(AVG(price_btc), 0) as avg_price_btc,
                COALESCE(MAX(price_btc), 0) as high_price_btc,
                COALESCE(MIN(price_btc), 0) as low_price_btc
            FROM trades
            WHERE token_id = $1
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Get 24h volume for a token
    pub async fn get_24h_volume(&self, token_id: Uuid) -> Result<Decimal> {
        let (volume,): (Decimal,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(total_btc), 0)
            FROM trades
            WHERE token_id = $1
            AND executed_at > NOW() - INTERVAL '24 hours'
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(volume)
    }

    /// Get volume for a token within a specific time range
    ///
    /// # Arguments
    /// * `token_id` - Token ID
    /// * `hours` - Number of hours to look back (e.g., 24 for 24h, 168 for 7d, 720 for 30d)
    pub async fn get_volume_by_hours(&self, token_id: Uuid, hours: i32) -> Result<Decimal> {
        let (volume,): (Decimal,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(total_btc), 0)
            FROM trades
            WHERE token_id = $1
            AND executed_at > NOW() - INTERVAL '1 hour' * $2
            "#,
        )
        .bind(token_id)
        .bind(hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(volume)
    }

    /// Get trade summary for a token within a specific time range
    ///
    /// # Arguments
    /// * `token_id` - Token ID
    /// * `hours` - Number of hours to look back (e.g., 24 for 24h, 168 for 7d, 720 for 30d)
    pub async fn get_token_summary_by_hours(
        &self,
        token_id: Uuid,
        hours: i32,
    ) -> Result<TradeSummary> {
        let summary = sqlx::query_as::<_, TradeSummary>(
            r#"
            SELECT
                COUNT(*) as total_trades,
                COALESCE(SUM(total_btc), 0) as total_volume_btc,
                COALESCE(AVG(price_btc), 0) as avg_price_btc,
                COALESCE(MAX(price_btc), 0) as high_price_btc,
                COALESCE(MIN(price_btc), 0) as low_price_btc
            FROM trades
            WHERE token_id = $1
            AND executed_at > NOW() - INTERVAL '1 hour' * $2
            "#,
        )
        .bind(token_id)
        .bind(hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Get total platform fees
    pub async fn get_total_platform_fees(&self) -> Result<Decimal> {
        let (fees,): (Decimal,) =
            sqlx::query_as(r#"SELECT COALESCE(SUM(platform_fee_btc), 0) FROM trades"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(fees)
    }

    /// Get total platform fees within a specific time range
    ///
    /// # Arguments
    /// * `hours` - Number of hours to look back
    pub async fn get_platform_fees_by_hours(&self, hours: i32) -> Result<Decimal> {
        let (fees,): (Decimal,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(platform_fee_btc), 0)
            FROM trades
            WHERE executed_at > NOW() - INTERVAL '1 hour' * $1
            "#,
        )
        .bind(hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(fees)
    }

    /// Batch create multiple trades (optimized for bulk operations)
    ///
    /// Creates multiple trades in a single transaction.
    /// Useful for migrations, bulk imports, or settlement operations.
    #[allow(clippy::too_many_arguments)]
    pub async fn batch_create(&self, trades: Vec<CreateTradeParams>) -> Result<u64> {
        if trades.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for trade in trades {
            let result = sqlx::query(
                r#"
                INSERT INTO trades (buyer_user_id, seller_user_id, token_id, amount, price_btc, total_btc, platform_fee_btc, issuer_royalty_btc)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(trade.buyer_user_id)
            .bind(trade.seller_user_id)
            .bind(trade.token_id)
            .bind(trade.amount)
            .bind(trade.price_btc)
            .bind(trade.total_btc)
            .bind(trade.platform_fee_btc)
            .bind(trade.issuer_royalty_btc)
            .execute(&mut *tx)
            .await?;

            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Get comprehensive trading statistics for a user
    ///
    /// Returns buy/sell breakdown, total volume, and profit/loss indicators.
    pub async fn get_user_trade_stats(&self, user_id: Uuid) -> Result<UserTradeStats> {
        let stats = sqlx::query_as::<_, UserTradeStats>(
            r#"
            SELECT
                COUNT(CASE WHEN buyer_user_id = $1 THEN 1 END) as buy_count,
                COUNT(CASE WHEN seller_user_id = $1 THEN 1 END) as sell_count,
                COALESCE(SUM(CASE WHEN buyer_user_id = $1 THEN total_btc ELSE 0 END), 0) as total_bought_btc,
                COALESCE(SUM(CASE WHEN seller_user_id = $1 THEN total_btc ELSE 0 END), 0) as total_sold_btc,
                COALESCE(SUM(CASE WHEN buyer_user_id = $1 THEN platform_fee_btc ELSE 0 END), 0) as fees_paid_btc
            FROM trades
            WHERE buyer_user_id = $1 OR seller_user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get most active traders by trade count
    ///
    /// Returns users sorted by number of trades (both buy and sell).
    pub async fn get_most_active_traders(&self, limit: i64) -> Result<Vec<TraderActivity>> {
        let traders = sqlx::query_as::<_, TraderActivity>(
            r#"
            SELECT
                user_id,
                trade_count,
                total_volume_btc
            FROM (
                SELECT
                    buyer_user_id as user_id,
                    COUNT(*) as trade_count,
                    COALESCE(SUM(total_btc), 0) as total_volume_btc
                FROM trades
                GROUP BY buyer_user_id
                UNION ALL
                SELECT
                    seller_user_id as user_id,
                    COUNT(*) as trade_count,
                    COALESCE(SUM(total_btc), 0) as total_volume_btc
                FROM trades
                WHERE seller_user_id IS NOT NULL
                GROUP BY seller_user_id
            ) combined
            GROUP BY user_id
            ORDER BY SUM(trade_count) DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(traders)
    }

    /// Get most traded tokens by volume
    ///
    /// Returns tokens sorted by total trading volume in BTC.
    pub async fn get_most_traded_tokens(&self, limit: i64) -> Result<Vec<TokenTradingVolume>> {
        let tokens = sqlx::query_as::<_, TokenTradingVolume>(
            r#"
            SELECT
                token_id,
                COUNT(*) as trade_count,
                COALESCE(SUM(total_btc), 0) as total_volume_btc,
                COALESCE(SUM(amount), 0) as total_amount
            FROM trades
            GROUP BY token_id
            ORDER BY total_volume_btc DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tokens)
    }

    /// Get recent trades across the entire platform (for activity feed)
    ///
    /// Returns most recent trades with user and token context.
    pub async fn get_recent_trades(&self, limit: i64) -> Result<Vec<TradeWithContextRow>> {
        let trades = sqlx::query_as::<_, TradeWithContextRow>(
            r#"
            SELECT
                t.*,
                buyer.username as buyer_username,
                seller.username as seller_username,
                tok.symbol as token_symbol
            FROM trades t
            JOIN users buyer ON t.buyer_user_id = buyer.user_id
            LEFT JOIN users seller ON t.seller_user_id = seller.user_id
            JOIN tokens tok ON t.token_id = tok.token_id
            ORDER BY t.executed_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(trades)
    }

    /// Get daily trading statistics for a time range
    ///
    /// Returns daily aggregated statistics useful for trend analysis.
    pub async fn get_daily_trade_stats(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<DailyTradeStats>> {
        let stats = sqlx::query_as::<_, DailyTradeStats>(
            r#"
            SELECT
                DATE(executed_at) as date,
                COUNT(*) as trade_count,
                COUNT(DISTINCT buyer_user_id) as unique_buyers,
                COUNT(DISTINCT token_id) as unique_tokens,
                COALESCE(SUM(total_btc), 0) as total_volume_btc,
                COALESCE(SUM(platform_fee_btc), 0) as total_fees_btc
            FROM trades
            WHERE executed_at >= $1 AND executed_at <= $2
            GROUP BY DATE(executed_at)
            ORDER BY DATE(executed_at) DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get total issuer royalties paid
    pub async fn get_total_royalties(&self) -> Result<Decimal> {
        let (royalties,): (Decimal,) =
            sqlx::query_as(r#"SELECT COALESCE(SUM(issuer_royalty_btc), 0) FROM trades"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(royalties)
    }

    /// Count trades for a specific token
    pub async fn count_trades_by_token(&self, token_id: Uuid) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM trades WHERE token_id = $1"#)
            .bind(token_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    /// Get count of unique traders (buyers or sellers)
    pub async fn get_unique_traders_count(&self) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(DISTINCT user_id) FROM (
                SELECT buyer_user_id as user_id FROM trades
                UNION
                SELECT seller_user_id as user_id FROM trades WHERE seller_user_id IS NOT NULL
            ) unique_users
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get price history for a token (useful for charting)
    ///
    /// Returns chronological price points with volume.
    pub async fn get_price_history(&self, token_id: Uuid, limit: i64) -> Result<Vec<PricePoint>> {
        let prices = sqlx::query_as::<_, PricePoint>(
            r#"
            SELECT
                executed_at as timestamp,
                price_btc,
                amount,
                total_btc
            FROM trades
            WHERE token_id = $1
            ORDER BY executed_at DESC
            LIMIT $2
            "#,
        )
        .bind(token_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(prices)
    }

    /// Get average trade size for a token
    pub async fn get_average_trade_size(&self, token_id: Uuid) -> Result<Decimal> {
        let avg = sqlx::query_scalar::<_, Option<Decimal>>(
            r#"SELECT COALESCE(AVG(amount), 0) FROM trades WHERE token_id = $1"#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(avg.unwrap_or(Decimal::ZERO))
    }

    /// Get total count of all trades
    pub async fn count_total_trades(&self) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM trades"#)
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }
}

/// Time range for trade queries
#[derive(Debug, Clone, Copy)]
pub enum TimeRange {
    /// Last 24 hours
    Hours24,
    /// Last 7 days
    Days7,
    /// Last 30 days
    Days30,
    /// All time
    AllTime,
}

impl TimeRange {
    /// Convert time range to hours
    pub fn to_hours(&self) -> Option<i32> {
        match self {
            TimeRange::Hours24 => Some(24),
            TimeRange::Days7 => Some(24 * 7),   // 168 hours
            TimeRange::Days30 => Some(24 * 30), // 720 hours
            TimeRange::AllTime => None,
        }
    }
}

/// Trade summary statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TradeSummary {
    pub total_trades: i64,
    pub total_volume_btc: Decimal,
    pub avg_price_btc: Decimal,
    pub high_price_btc: Decimal,
    pub low_price_btc: Decimal,
}

/// Parameters for creating a trade (used in batch operations)
#[derive(Debug, Clone)]
pub struct CreateTradeParams {
    pub buyer_user_id: Uuid,
    pub seller_user_id: Option<Uuid>,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub platform_fee_btc: Decimal,
    pub issuer_royalty_btc: Decimal,
}

/// User trading statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserTradeStats {
    pub buy_count: Option<i64>,
    pub sell_count: Option<i64>,
    pub total_bought_btc: Decimal,
    pub total_sold_btc: Decimal,
    pub fees_paid_btc: Decimal,
}

/// Trader activity statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TraderActivity {
    pub user_id: Uuid,
    pub trade_count: Option<i64>,
    pub total_volume_btc: Option<Decimal>,
}

/// Token trading volume statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenTradingVolume {
    pub token_id: Uuid,
    pub trade_count: i64,
    pub total_volume_btc: Decimal,
    pub total_amount: Decimal,
}

/// Daily trade statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DailyTradeStats {
    pub date: chrono::NaiveDate,
    pub trade_count: i64,
    pub unique_buyers: i64,
    pub unique_tokens: i64,
    pub total_volume_btc: Decimal,
    pub total_fees_btc: Decimal,
}

/// Price point for charting
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PricePoint {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub price_btc: Decimal,
    pub amount: Decimal,
    pub total_btc: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_range_to_hours() {
        assert_eq!(TimeRange::Hours24.to_hours(), Some(24));
        assert_eq!(TimeRange::Days7.to_hours(), Some(168));
        assert_eq!(TimeRange::Days30.to_hours(), Some(720));
        assert_eq!(TimeRange::AllTime.to_hours(), None);
    }

    #[test]
    fn test_trade_summary_structure() {
        let summary = TradeSummary {
            total_trades: 100,
            total_volume_btc: Decimal::new(500000000, 8),
            avg_price_btc: Decimal::new(100000, 8),
            high_price_btc: Decimal::new(150000, 8),
            low_price_btc: Decimal::new(50000, 8),
        };

        assert_eq!(summary.total_trades, 100);
        assert!(summary.high_price_btc > summary.low_price_btc);
    }

    #[test]
    fn test_create_trade_params_structure() {
        let params = CreateTradeParams {
            buyer_user_id: Uuid::new_v4(),
            seller_user_id: Some(Uuid::new_v4()),
            token_id: Uuid::new_v4(),
            amount: Decimal::new(100, 0),
            price_btc: Decimal::new(50000, 8),
            total_btc: Decimal::new(500000, 8),
            platform_fee_btc: Decimal::new(5000, 8),
            issuer_royalty_btc: Decimal::new(2500, 8),
        };

        assert_eq!(params.amount, Decimal::new(100, 0));
        assert!(params.seller_user_id.is_some());
    }

    #[test]
    fn test_user_trade_stats_structure() {
        let stats = UserTradeStats {
            buy_count: Some(50),
            sell_count: Some(30),
            total_bought_btc: Decimal::new(5000000, 8),
            total_sold_btc: Decimal::new(3000000, 8),
            fees_paid_btc: Decimal::new(50000, 8),
        };

        assert_eq!(stats.buy_count, Some(50));
        assert_eq!(stats.sell_count, Some(30));
    }

    #[test]
    fn test_trader_activity_structure() {
        let activity = TraderActivity {
            user_id: Uuid::new_v4(),
            trade_count: Some(100),
            total_volume_btc: Some(Decimal::new(10000000, 8)),
        };

        assert_eq!(activity.trade_count, Some(100));
    }

    #[test]
    fn test_token_trading_volume_structure() {
        let volume = TokenTradingVolume {
            token_id: Uuid::new_v4(),
            trade_count: 500,
            total_volume_btc: Decimal::new(50000000, 8),
            total_amount: Decimal::new(10000, 0),
        };

        assert_eq!(volume.trade_count, 500);
        assert_eq!(volume.total_amount, Decimal::new(10000, 0));
    }

    #[test]
    fn test_daily_trade_stats_structure() {
        let stats = DailyTradeStats {
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            trade_count: 1000,
            unique_buyers: 250,
            unique_tokens: 50,
            total_volume_btc: Decimal::new(100000000, 8),
            total_fees_btc: Decimal::new(1000000, 8),
        };

        assert_eq!(stats.trade_count, 1000);
        assert_eq!(stats.unique_buyers, 250);
        assert_eq!(stats.unique_tokens, 50);
    }

    #[test]
    fn test_price_point_structure() {
        let point = PricePoint {
            timestamp: chrono::Utc::now(),
            price_btc: Decimal::new(50000, 8),
            amount: Decimal::new(100, 0),
            total_btc: Decimal::new(500000, 8),
        };

        assert_eq!(point.price_btc, Decimal::new(50000, 8));
        assert_eq!(point.amount, Decimal::new(100, 0));
    }

    #[test]
    fn test_batch_create_empty_vector() {
        let trades: Vec<CreateTradeParams> = vec![];
        assert_eq!(trades.len(), 0);
    }

    #[test]
    fn test_trade_row_structure() {
        let trade = TradeRow {
            trade_id: Uuid::new_v4(),
            buyer_user_id: Uuid::new_v4(),
            seller_user_id: Some(Uuid::new_v4()),
            token_id: Uuid::new_v4(),
            amount: Decimal::new(100, 0),
            price_btc: Decimal::new(50000, 8),
            total_btc: Decimal::new(500000, 8),
            platform_fee_btc: Decimal::new(5000, 8),
            issuer_royalty_btc: Decimal::new(2500, 8),
            executed_at: chrono::Utc::now(),
        };

        assert_eq!(trade.amount, Decimal::new(100, 0));
        assert!(trade.seller_user_id.is_some());
    }

    #[test]
    fn test_trade_with_context_row_structure() {
        let trade = TradeWithContextRow {
            trade_id: Uuid::new_v4(),
            buyer_user_id: Uuid::new_v4(),
            seller_user_id: Some(Uuid::new_v4()),
            token_id: Uuid::new_v4(),
            amount: Decimal::new(100, 0),
            price_btc: Decimal::new(50000, 8),
            total_btc: Decimal::new(500000, 8),
            platform_fee_btc: Decimal::new(5000, 8),
            issuer_royalty_btc: Decimal::new(2500, 8),
            executed_at: chrono::Utc::now(),
            buyer_username: "alice".to_string(),
            seller_username: Some("bob".to_string()),
            token_symbol: "BTC".to_string(),
        };

        assert_eq!(trade.buyer_username, "alice");
        assert_eq!(trade.seller_username, Some("bob".to_string()));
        assert_eq!(trade.token_symbol, "BTC");
    }
}
