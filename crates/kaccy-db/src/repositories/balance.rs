//! Balance repository

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Balance data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BalanceRow {
    pub balance_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub locked_amount: Decimal,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Balance with token information
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BalanceWithTokenRow {
    pub balance_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub locked_amount: Decimal,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub token_symbol: String,
    pub token_name: String,
}

/// Repository for balance operations
pub struct BalanceRepository {
    pool: PgPool,
}

impl BalanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get balance for user and token
    pub async fn get_balance(&self, user_id: Uuid, token_id: Uuid) -> Result<Option<BalanceRow>> {
        let balance = sqlx::query_as::<_, BalanceRow>(
            r#"SELECT * FROM balances WHERE user_id = $1 AND token_id = $2"#,
        )
        .bind(user_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(balance)
    }

    /// Get all balances for a user
    pub async fn get_user_balances(&self, user_id: Uuid) -> Result<Vec<BalanceWithTokenRow>> {
        let balances = sqlx::query_as::<_, BalanceWithTokenRow>(
            r#"
            SELECT b.*, t.symbol as token_symbol, t.name as token_name
            FROM balances b
            JOIN tokens t ON b.token_id = t.token_id
            WHERE b.user_id = $1 AND b.amount > 0
            ORDER BY b.updated_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(balances)
    }

    /// Get all holders of a token
    pub async fn get_token_holders(&self, token_id: Uuid) -> Result<Vec<BalanceRow>> {
        let balances = sqlx::query_as::<_, BalanceRow>(
            r#"
            SELECT * FROM balances
            WHERE token_id = $1 AND amount > 0
            ORDER BY amount DESC
            "#,
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(balances)
    }

    /// Update balance (add or subtract)
    pub async fn update_balance(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        delta: Decimal,
    ) -> Result<BalanceRow> {
        let balance = sqlx::query_as::<_, BalanceRow>(
            r#"
            INSERT INTO balances (balance_id, user_id, token_id, amount, locked_amount, updated_at)
            VALUES (gen_random_uuid(), $1, $2, GREATEST(0, $3), 0, NOW())
            ON CONFLICT (user_id, token_id)
            DO UPDATE SET amount = GREATEST(0, balances.amount + $3), updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .bind(delta)
        .fetch_one(&self.pool)
        .await?;

        Ok(balance)
    }

    /// Lock tokens for a pending order
    pub async fn lock_tokens(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        amount: Decimal,
    ) -> Result<BalanceRow> {
        let balance = sqlx::query_as::<_, BalanceRow>(
            r#"
            UPDATE balances
            SET locked_amount = locked_amount + $3, updated_at = NOW()
            WHERE user_id = $1 AND token_id = $2 AND (amount - locked_amount) >= $3
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .bind(amount)
        .fetch_one(&self.pool)
        .await?;

        Ok(balance)
    }

    /// Unlock tokens (order cancelled)
    pub async fn unlock_tokens(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        amount: Decimal,
    ) -> Result<BalanceRow> {
        let balance = sqlx::query_as::<_, BalanceRow>(
            r#"
            UPDATE balances
            SET locked_amount = GREATEST(0, locked_amount - $3), updated_at = NOW()
            WHERE user_id = $1 AND token_id = $2
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .bind(amount)
        .fetch_one(&self.pool)
        .await?;

        Ok(balance)
    }

    /// Count holders of a token
    pub async fn count_holders(&self, token_id: Uuid) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM balances WHERE token_id = $1 AND amount > 0"#)
                .bind(token_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get available balance (amount - locked_amount) for user and token
    pub async fn get_available_balance(&self, user_id: Uuid, token_id: Uuid) -> Result<Decimal> {
        let available = sqlx::query_scalar::<_, Decimal>(
            r#"
            SELECT COALESCE(amount - locked_amount, 0)
            FROM balances
            WHERE user_id = $1 AND token_id = $2
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(available.unwrap_or(Decimal::ZERO))
    }

    /// Batch update multiple balances (optimized for mass balance adjustments)
    ///
    /// Updates multiple user-token balance pairs in a single transaction.
    /// Each tuple contains (user_id, token_id, delta).
    ///
    /// # Arguments
    /// * `updates` - Vector of (user_id, token_id, delta) tuples
    ///
    /// # Returns
    /// Number of balances updated
    pub async fn batch_update_balances(&self, updates: Vec<(Uuid, Uuid, Decimal)>) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for (user_id, token_id, delta) in updates {
            let result = sqlx::query(
                r#"
                INSERT INTO balances (balance_id, user_id, token_id, amount, locked_amount, updated_at)
                VALUES (gen_random_uuid(), $1, $2, GREATEST(0, $3), 0, NOW())
                ON CONFLICT (user_id, token_id)
                DO UPDATE SET amount = GREATEST(0, balances.amount + $3), updated_at = NOW()
                "#,
            )
            .bind(user_id)
            .bind(token_id)
            .bind(delta)
            .execute(&mut *tx)
            .await?;

            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Batch lock tokens for multiple user-token pairs (optimized for bulk operations)
    ///
    /// Locks tokens for multiple users atomically in a single transaction.
    /// Each tuple contains (user_id, token_id, amount).
    ///
    /// # Arguments
    /// * `locks` - Vector of (user_id, token_id, amount) tuples
    ///
    /// # Returns
    /// Number of locks applied
    pub async fn batch_lock_tokens(&self, locks: Vec<(Uuid, Uuid, Decimal)>) -> Result<u64> {
        if locks.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for (user_id, token_id, amount) in locks {
            let result = sqlx::query(
                r#"
                UPDATE balances
                SET locked_amount = locked_amount + $3, updated_at = NOW()
                WHERE user_id = $1 AND token_id = $2 AND (amount - locked_amount) >= $3
                "#,
            )
            .bind(user_id)
            .bind(token_id)
            .bind(amount)
            .execute(&mut *tx)
            .await?;

            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Batch unlock tokens for multiple user-token pairs (optimized for bulk operations)
    ///
    /// Unlocks tokens for multiple users atomically in a single transaction.
    /// Each tuple contains (user_id, token_id, amount).
    ///
    /// # Arguments
    /// * `unlocks` - Vector of (user_id, token_id, amount) tuples
    ///
    /// # Returns
    /// Number of unlocks applied
    pub async fn batch_unlock_tokens(&self, unlocks: Vec<(Uuid, Uuid, Decimal)>) -> Result<u64> {
        if unlocks.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for (user_id, token_id, amount) in unlocks {
            let result = sqlx::query(
                r#"
                UPDATE balances
                SET locked_amount = GREATEST(0, locked_amount - $3), updated_at = NOW()
                WHERE user_id = $1 AND token_id = $2
                "#,
            )
            .bind(user_id)
            .bind(token_id)
            .bind(amount)
            .execute(&mut *tx)
            .await?;

            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Get top holders of a token by balance (whale tracking)
    ///
    /// Returns the top N holders sorted by balance amount.
    /// Useful for analyzing token distribution and identifying whales.
    ///
    /// # Arguments
    /// * `token_id` - Token to analyze
    /// * `limit` - Maximum number of holders to return
    pub async fn get_top_holders(&self, token_id: Uuid, limit: i64) -> Result<Vec<HolderInfo>> {
        let holders = sqlx::query_as::<_, HolderInfo>(
            r#"
            SELECT user_id, amount, locked_amount,
                   (amount + locked_amount) as total_balance,
                   updated_at
            FROM balances
            WHERE token_id = $1 AND amount > 0
            ORDER BY (amount + locked_amount) DESC
            LIMIT $2
            "#,
        )
        .bind(token_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(holders)
    }

    /// Get total locked supply for a token
    ///
    /// Sums all locked_amount values for a token.
    /// Useful for understanding liquidity constraints.
    pub async fn get_total_supply_locked(&self, token_id: Uuid) -> Result<Decimal> {
        let locked = sqlx::query_scalar::<_, Option<Decimal>>(
            r#"SELECT COALESCE(SUM(locked_amount), 0) FROM balances WHERE token_id = $1"#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(locked.unwrap_or(Decimal::ZERO))
    }

    /// Get total circulating supply for a token (all user balances)
    ///
    /// Sums all amount values (excluding locked) for a token.
    pub async fn get_total_circulating(&self, token_id: Uuid) -> Result<Decimal> {
        let circulating = sqlx::query_scalar::<_, Option<Decimal>>(
            r#"SELECT COALESCE(SUM(amount), 0) FROM balances WHERE token_id = $1"#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(circulating.unwrap_or(Decimal::ZERO))
    }

    /// Count distinct tokens a user holds (portfolio diversity)
    pub async fn get_user_token_count(&self, user_id: Uuid) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(
            r#"SELECT COUNT(DISTINCT token_id) FROM balances WHERE user_id = $1 AND amount > 0"#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get balances above a threshold (whale watching)
    ///
    /// Returns all balances where total balance (amount + locked) exceeds threshold.
    /// Useful for whale alerts and large holder analysis.
    pub async fn get_balances_above_threshold(
        &self,
        token_id: Uuid,
        threshold: Decimal,
    ) -> Result<Vec<HolderInfo>> {
        let balances = sqlx::query_as::<_, HolderInfo>(
            r#"
            SELECT user_id, amount, locked_amount,
                   (amount + locked_amount) as total_balance,
                   updated_at
            FROM balances
            WHERE token_id = $1 AND (amount + locked_amount) >= $2
            ORDER BY (amount + locked_amount) DESC
            "#,
        )
        .bind(token_id)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(balances)
    }

    /// Get all balances with locked tokens (liquidity analysis)
    pub async fn get_locked_balances(&self, token_id: Uuid) -> Result<Vec<BalanceRow>> {
        let balances = sqlx::query_as::<_, BalanceRow>(
            r#"
            SELECT * FROM balances
            WHERE token_id = $1 AND locked_amount > 0
            ORDER BY locked_amount DESC
            "#,
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(balances)
    }

    /// Get balance distribution statistics for a token
    ///
    /// Returns comprehensive statistics about balance distribution including:
    /// - Total holders
    /// - Total circulating supply
    /// - Total locked supply
    /// - Average balance
    /// - Median balance (approximate)
    /// - Concentration (% held by top 10 holders)
    pub async fn get_balance_statistics(&self, token_id: Uuid) -> Result<BalanceStatistics> {
        // Get basic statistics
        let basic_stats = sqlx::query_as::<_, BalanceStatisticsRow>(
            r#"
            SELECT
                COUNT(*) as holder_count,
                COALESCE(SUM(amount), 0) as total_amount,
                COALESCE(SUM(locked_amount), 0) as total_locked,
                COALESCE(AVG(amount), 0) as average_balance,
                COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY amount), 0) as median_balance
            FROM balances
            WHERE token_id = $1 AND amount > 0
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        // Get top 10 holders concentration
        let top_10_sum = sqlx::query_scalar::<_, Option<Decimal>>(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM (
                SELECT amount FROM balances
                WHERE token_id = $1 AND amount > 0
                ORDER BY amount DESC
                LIMIT 10
            ) AS top_holders
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(Decimal::ZERO);

        let top_10_percentage = if basic_stats.total_amount > Decimal::ZERO {
            (top_10_sum / basic_stats.total_amount) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        Ok(BalanceStatistics {
            token_id,
            holder_count: basic_stats.holder_count,
            total_amount: basic_stats.total_amount,
            total_locked: basic_stats.total_locked,
            average_balance: basic_stats.average_balance,
            median_balance: basic_stats.median_balance,
            top_10_concentration: top_10_percentage,
        })
    }

    /// Get user's total portfolio summary
    ///
    /// Returns count of distinct tokens and total number of balances.
    pub async fn get_user_portfolio_summary(&self, user_id: Uuid) -> Result<PortfolioSummary> {
        let summary = sqlx::query_as::<_, PortfolioSummary>(
            r#"
            SELECT
                $1 as user_id,
                COUNT(DISTINCT token_id) as token_count,
                COUNT(*) as balance_count
            FROM balances
            WHERE user_id = $1 AND amount > 0
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(summary)
    }
}

/// Holder information with balance details
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HolderInfo {
    pub user_id: Uuid,
    pub amount: Decimal,
    pub locked_amount: Decimal,
    pub total_balance: Decimal,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Balance statistics for a token (used internally for queries)
#[derive(Debug, Clone, sqlx::FromRow)]
struct BalanceStatisticsRow {
    pub holder_count: i64,
    pub total_amount: Decimal,
    pub total_locked: Decimal,
    pub average_balance: Decimal,
    pub median_balance: Decimal,
}

/// Balance distribution statistics for a token
#[derive(Debug, Clone)]
pub struct BalanceStatistics {
    pub token_id: Uuid,
    pub holder_count: i64,
    pub total_amount: Decimal,
    pub total_locked: Decimal,
    pub average_balance: Decimal,
    pub median_balance: Decimal,
    pub top_10_concentration: Decimal,
}

/// User portfolio summary
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PortfolioSummary {
    pub user_id: Uuid,
    pub token_count: i64,
    pub balance_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_row_structure() {
        let balance = BalanceRow {
            balance_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: Decimal::new(10000, 2),
            locked_amount: Decimal::new(2500, 2),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(balance.amount, Decimal::new(10000, 2));
        assert_eq!(balance.locked_amount, Decimal::new(2500, 2));
    }

    #[test]
    fn test_balance_with_token_row_structure() {
        let balance = BalanceWithTokenRow {
            balance_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: Decimal::new(10000, 2),
            locked_amount: Decimal::new(2500, 2),
            updated_at: chrono::Utc::now(),
            token_symbol: "BTC".to_string(),
            token_name: "Bitcoin".to_string(),
        };

        assert_eq!(balance.token_symbol, "BTC");
        assert_eq!(balance.token_name, "Bitcoin");
    }

    #[test]
    fn test_available_balance_calculation() {
        let amount = Decimal::new(10000, 2);
        let locked = Decimal::new(2500, 2);
        let available = amount - locked;

        assert_eq!(available, Decimal::new(7500, 2));
    }

    #[test]
    fn test_batch_update_empty_vector() {
        let updates: Vec<(Uuid, Uuid, Decimal)> = vec![];
        assert_eq!(updates.len(), 0);
    }

    #[test]
    fn test_batch_lock_empty_vector() {
        let locks: Vec<(Uuid, Uuid, Decimal)> = vec![];
        assert_eq!(locks.len(), 0);
    }

    #[test]
    fn test_batch_unlock_empty_vector() {
        let unlocks: Vec<(Uuid, Uuid, Decimal)> = vec![];
        assert_eq!(unlocks.len(), 0);
    }

    #[test]
    fn test_batch_operations_tuple_structure() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let amount = Decimal::new(1000, 2);

        let update = (user_id, token_id, amount);
        assert_eq!(update.0, user_id);
        assert_eq!(update.1, token_id);
        assert_eq!(update.2, amount);
    }

    #[test]
    fn test_holder_info_structure() {
        let holder = HolderInfo {
            user_id: Uuid::new_v4(),
            amount: Decimal::new(10000, 2),
            locked_amount: Decimal::new(2000, 2),
            total_balance: Decimal::new(12000, 2),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(holder.amount, Decimal::new(10000, 2));
        assert_eq!(holder.locked_amount, Decimal::new(2000, 2));
        assert_eq!(holder.total_balance, Decimal::new(12000, 2));
    }

    #[test]
    fn test_balance_statistics_structure() {
        let stats = BalanceStatistics {
            token_id: Uuid::new_v4(),
            holder_count: 100,
            total_amount: Decimal::new(1000000, 2),
            total_locked: Decimal::new(250000, 2),
            average_balance: Decimal::new(10000, 2),
            median_balance: Decimal::new(5000, 2),
            top_10_concentration: Decimal::new(4500, 2), // 45.00%
        };

        assert_eq!(stats.holder_count, 100);
        assert_eq!(stats.total_amount, Decimal::new(1000000, 2));
        assert_eq!(stats.top_10_concentration, Decimal::new(4500, 2));
    }

    #[test]
    fn test_portfolio_summary_structure() {
        let summary = PortfolioSummary {
            user_id: Uuid::new_v4(),
            token_count: 10,
            balance_count: 12,
        };

        assert_eq!(summary.token_count, 10);
        assert_eq!(summary.balance_count, 12);
    }

    #[test]
    fn test_concentration_calculation() {
        let total = Decimal::new(100000, 2);
        let top_10 = Decimal::new(45000, 2);
        let concentration = (top_10 / total) * Decimal::from(100);

        assert_eq!(concentration, Decimal::new(4500, 2)); // 45.00%
    }

    #[test]
    fn test_total_balance_calculation() {
        let amount = Decimal::new(10000, 2);
        let locked = Decimal::new(3000, 2);
        let total = amount + locked;

        assert_eq!(total, Decimal::new(13000, 2));
    }

    #[test]
    fn test_zero_concentration_for_zero_supply() {
        let total = Decimal::ZERO;
        let top_10 = Decimal::new(1000, 2);

        let concentration = if total > Decimal::ZERO {
            (top_10 / total) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        assert_eq!(concentration, Decimal::ZERO);
    }

    #[test]
    fn test_balance_statistics_row_structure() {
        let row = BalanceStatisticsRow {
            holder_count: 50,
            total_amount: Decimal::new(500000, 2),
            total_locked: Decimal::new(100000, 2),
            average_balance: Decimal::new(10000, 2),
            median_balance: Decimal::new(8000, 2),
        };

        assert_eq!(row.holder_count, 50);
        assert_eq!(row.total_amount, Decimal::new(500000, 2));
        assert_eq!(row.median_balance, Decimal::new(8000, 2));
    }
}
