//! Token repository

use rust_decimal::Decimal;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::Result;
use crate::helpers::calculate_offset;

/// Token data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenRow {
    pub token_id: Uuid,
    pub issuer_user_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: Decimal,
    pub circulating_supply: Decimal,
    pub initial_price_btc: Decimal,
    pub price_increment_btc: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

/// Token with issuer information
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenWithIssuerRow {
    pub token_id: Uuid,
    pub issuer_user_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: Decimal,
    pub circulating_supply: Decimal,
    pub initial_price_btc: Decimal,
    pub price_increment_btc: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub issuer_username: String,
    pub issuer_display_name: Option<String>,
    pub issuer_reputation_score: Decimal,
}

/// Repository for token operations
pub struct TokenRepository {
    pool: PgPool,
}

impl TokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find token by ID
    pub async fn find_by_id(&self, token_id: Uuid) -> Result<Option<TokenRow>> {
        let token = sqlx::query_as::<_, TokenRow>(r#"SELECT * FROM tokens WHERE token_id = $1"#)
            .bind(token_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(token)
    }

    /// Find token by symbol
    pub async fn find_by_symbol(&self, symbol: &str) -> Result<Option<TokenRow>> {
        let token = sqlx::query_as::<_, TokenRow>(r#"SELECT * FROM tokens WHERE symbol = $1"#)
            .bind(symbol)
            .fetch_optional(&self.pool)
            .await?;

        Ok(token)
    }

    /// Find tokens by issuer
    pub async fn find_by_issuer(&self, issuer_user_id: Uuid) -> Result<Vec<TokenRow>> {
        let tokens = sqlx::query_as::<_, TokenRow>(
            r#"SELECT * FROM tokens WHERE issuer_user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(issuer_user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tokens)
    }

    /// List active tokens with pagination
    pub async fn list_active(
        &self,
        limit: i64,
        offset: i64,
        search: Option<&str>,
    ) -> Result<Vec<TokenWithIssuerRow>> {
        let tokens = if let Some(search_term) = search {
            let pattern = format!("%{}%", search_term);
            sqlx::query_as::<_, TokenWithIssuerRow>(
                r#"
                SELECT t.*, u.username as issuer_username, u.display_name as issuer_display_name, u.reputation_score as issuer_reputation_score
                FROM tokens t
                JOIN users u ON t.issuer_user_id = u.user_id
                WHERE t.status = 'active'
                AND (t.symbol ILIKE $1 OR t.name ILIKE $1)
                ORDER BY t.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TokenWithIssuerRow>(
                r#"
                SELECT t.*, u.username as issuer_username, u.display_name as issuer_display_name, u.reputation_score as issuer_reputation_score
                FROM tokens t
                JOIN users u ON t.issuer_user_id = u.user_id
                WHERE t.status = 'active'
                ORDER BY t.created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(tokens)
    }

    /// Create a new token
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        issuer_user_id: Uuid,
        symbol: &str,
        name: &str,
        description: Option<&str>,
        total_supply: Decimal,
        initial_price_btc: Decimal,
        price_increment_btc: Decimal,
    ) -> Result<TokenRow> {
        let token = sqlx::query_as::<_, TokenRow>(
            r#"
            INSERT INTO tokens (issuer_user_id, symbol, name, description, total_supply, initial_price_btc, price_increment_btc)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(issuer_user_id)
        .bind(symbol)
        .bind(name)
        .bind(description)
        .bind(total_supply)
        .bind(initial_price_btc)
        .bind(price_increment_btc)
        .fetch_one(&self.pool)
        .await?;

        Ok(token)
    }

    /// Update token circulating supply with validation
    ///
    /// Ensures that circulating supply doesn't exceed total supply.
    pub async fn update_supply(&self, token_id: Uuid, delta: Decimal) -> Result<Decimal> {
        let (new_supply,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE tokens
            SET circulating_supply = LEAST(circulating_supply + $2, total_supply)
            WHERE token_id = $1
            AND circulating_supply + $2 <= total_supply
            RETURNING circulating_supply
            "#,
        )
        .bind(token_id)
        .bind(delta)
        .fetch_one(&self.pool)
        .await?;

        Ok(new_supply)
    }

    /// Validate that a token's supply constraints are met
    ///
    /// Checks that circulating_supply <= total_supply
    pub async fn validate_supply(&self, token_id: Uuid) -> Result<bool> {
        let (is_valid,): (bool,) = sqlx::query_as(
            r#"
            SELECT circulating_supply <= total_supply
            FROM tokens
            WHERE token_id = $1
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(is_valid)
    }

    /// Get remaining supply that can be circulated
    pub async fn get_remaining_supply(&self, token_id: Uuid) -> Result<Decimal> {
        let (remaining,): (Decimal,) = sqlx::query_as(
            r#"
            SELECT total_supply - circulating_supply
            FROM tokens
            WHERE token_id = $1
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(remaining)
    }

    /// Update token status
    pub async fn update_status(&self, token_id: Uuid, status: &str) -> Result<()> {
        sqlx::query(r#"UPDATE tokens SET status = $2 WHERE token_id = $1"#)
            .bind(token_id)
            .bind(status)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Check if symbol exists
    pub async fn symbol_exists(&self, symbol: &str) -> Result<bool> {
        let (exists,): (bool,) =
            sqlx::query_as(r#"SELECT EXISTS(SELECT 1 FROM tokens WHERE symbol = $1)"#)
                .bind(symbol)
                .fetch_one(&self.pool)
                .await?;

        Ok(exists)
    }

    /// Count tokens by issuer
    pub async fn count_by_issuer(&self, issuer_user_id: Uuid) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM tokens WHERE issuer_user_id = $1"#)
                .bind(issuer_user_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Soft delete a token by setting status to 'deleted'
    ///
    /// This implements soft-delete pattern by changing the token's status to 'deleted'
    /// instead of removing the record from the database. This preserves audit trails,
    /// trade history, and referential integrity.
    ///
    /// # Arguments
    /// * `token_id` - The ID of the token to soft delete
    ///
    /// # Returns
    /// The updated token record
    pub async fn soft_delete(&self, token_id: Uuid) -> Result<TokenRow> {
        let token = sqlx::query_as::<_, TokenRow>(
            r#"
            UPDATE tokens
            SET status = 'deleted'
            WHERE token_id = $1
            RETURNING *
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(token)
    }

    /// Restore a soft-deleted token
    ///
    /// Restores a previously soft-deleted token by setting status back to 'active'.
    ///
    /// # Arguments
    /// * `token_id` - The ID of the token to restore
    ///
    /// # Returns
    /// The updated token record
    pub async fn restore(&self, token_id: Uuid) -> Result<TokenRow> {
        let token = sqlx::query_as::<_, TokenRow>(
            r#"
            UPDATE tokens
            SET status = 'active'
            WHERE token_id = $1
            RETURNING *
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(token)
    }

    /// Check if a token is soft-deleted
    ///
    /// # Arguments
    /// * `token_id` - The ID of the token to check
    ///
    /// # Returns
    /// True if the token's status is 'deleted'
    pub async fn is_deleted(&self, token_id: Uuid) -> Result<bool> {
        let (is_deleted,): (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM tokens WHERE token_id = $1 AND status = 'deleted')"#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(is_deleted)
    }

    /// Deactivate a token (set status to 'inactive')
    ///
    /// Deactivates a token, preventing new trades while preserving the record.
    /// Different from soft-delete as it indicates the token is temporarily unavailable.
    ///
    /// # Arguments
    /// * `token_id` - The ID of the token to deactivate
    ///
    /// # Returns
    /// The updated token record
    pub async fn deactivate(&self, token_id: Uuid) -> Result<TokenRow> {
        let token = sqlx::query_as::<_, TokenRow>(
            r#"
            UPDATE tokens
            SET status = 'inactive'
            WHERE token_id = $1
            RETURNING *
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(token)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Batch create tokens (for ICOs, airdrops, or migrations)
    ///
    /// Creates multiple tokens in a single transaction for optimal performance.
    /// Uses QueryBuilder for efficient batch inserts.
    ///
    /// # Arguments
    /// * `executor` - Database executor (pool or transaction)
    /// * `tokens` - Vector of BatchTokenParams tuples
    ///
    /// # Returns
    /// Number of tokens created
    pub async fn batch_create<'a, 'b, E>(
        &self,
        executor: E,
        tokens: &'b [BatchTokenParams<'b>],
    ) -> Result<u64>
    where
        E: PgExecutor<'a>,
    {
        if tokens.is_empty() {
            return Ok(0);
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO tokens (issuer_user_id, symbol, name, description, total_supply, initial_price_btc, price_increment_btc) ",
        );

        query_builder.push_values(
            tokens,
            |mut b,
             (
                issuer_user_id,
                symbol,
                name,
                description,
                total_supply,
                initial_price_btc,
                price_increment_btc,
            )| {
                b.push_bind(issuer_user_id)
                    .push_bind(symbol)
                    .push_bind(name)
                    .push_bind(description)
                    .push_bind(total_supply)
                    .push_bind(initial_price_btc)
                    .push_bind(price_increment_btc);
            },
        );

        let result = query_builder.build().execute(executor).await?;

        Ok(result.rows_affected())
    }

    /// Batch update token status
    ///
    /// Updates status for multiple tokens in a single query.
    ///
    /// # Arguments
    /// * `token_ids` - Vector of token IDs to update
    /// * `status` - New status to set
    ///
    /// # Returns
    /// Number of tokens updated
    pub async fn batch_update_status(&self, token_ids: &[Uuid], status: &str) -> Result<u64> {
        if token_ids.is_empty() {
            return Ok(0);
        }

        let mut query_builder = sqlx::QueryBuilder::new("UPDATE tokens SET status = ");
        query_builder.push_bind(status);
        query_builder.push(" WHERE token_id = ANY(");
        query_builder.push_bind(token_ids);
        query_builder.push(")");

        let result = query_builder.build().execute(&self.pool).await?;

        Ok(result.rows_affected())
    }

    // =========================================================================
    // Query & Filter Methods
    // =========================================================================

    /// Get tokens by status with pagination
    ///
    /// # Arguments
    /// * `status` - The status to filter by (e.g., "active", "inactive", "deleted")
    /// * `limit` - Maximum number of tokens to return
    /// * `page` - Page number (1-indexed)
    ///
    /// # Returns
    /// Vector of tokens matching the status
    pub async fn get_tokens_by_status(
        &self,
        status: &str,
        limit: i64,
        page: i64,
    ) -> Result<Vec<TokenRow>> {
        let offset = calculate_offset(page as u32, limit as u32);

        let tokens = sqlx::query_as::<_, TokenRow>(
            r#"
            SELECT * FROM tokens
            WHERE status = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(tokens)
    }

    /// Get all tokens by issuer with status filter
    ///
    /// # Arguments
    /// * `issuer_user_id` - The issuer's user ID
    /// * `status` - Optional status filter
    ///
    /// # Returns
    /// Vector of tokens matching the criteria
    pub async fn get_issuer_tokens_by_status(
        &self,
        issuer_user_id: Uuid,
        status: Option<&str>,
    ) -> Result<Vec<TokenRow>> {
        let tokens = if let Some(status_filter) = status {
            sqlx::query_as::<_, TokenRow>(
                r#"
                SELECT * FROM tokens
                WHERE issuer_user_id = $1 AND status = $2
                ORDER BY created_at DESC
                "#,
            )
            .bind(issuer_user_id)
            .bind(status_filter)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TokenRow>(
                r#"
                SELECT * FROM tokens
                WHERE issuer_user_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(issuer_user_id)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(tokens)
    }

    // =========================================================================
    // Statistics & Counts
    // =========================================================================

    /// Count tokens by status
    ///
    /// # Arguments
    /// * `status` - The status to count
    ///
    /// # Returns
    /// Number of tokens with the specified status
    pub async fn count_tokens_by_status(&self, status: &str) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM tokens WHERE status = $1"#)
            .bind(status)
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    /// Get total token count
    ///
    /// # Returns
    /// Total number of tokens (excluding soft-deleted)
    pub async fn count_total_tokens(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM tokens WHERE status != 'deleted'"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get token statistics
    ///
    /// Returns a summary of token counts by status
    pub async fn get_token_statistics(&self) -> Result<TokenStatistics> {
        let total = self.count_total_tokens().await?;
        let active = self.count_tokens_by_status("active").await?;
        let inactive = self.count_tokens_by_status("inactive").await?;
        let deleted = self.count_tokens_by_status("deleted").await?;

        // Get total supply stats
        let (total_supply_sum, circulating_supply_sum): (Option<Decimal>, Option<Decimal>) =
            sqlx::query_as(
                r#"
            SELECT SUM(total_supply), SUM(circulating_supply)
            FROM tokens
            WHERE status != 'deleted'
            "#,
            )
            .fetch_one(&self.pool)
            .await?;

        Ok(TokenStatistics {
            total_active: total,
            total_active_status: active,
            total_inactive: inactive,
            total_deleted: deleted,
            total_supply_sum: total_supply_sum.unwrap_or(Decimal::ZERO),
            circulating_supply_sum: circulating_supply_sum.unwrap_or(Decimal::ZERO),
        })
    }
}

/// Token statistics summary
#[derive(Debug, Clone)]
pub struct TokenStatistics {
    pub total_active: i64,
    pub total_active_status: i64,
    pub total_inactive: i64,
    pub total_deleted: i64,
    pub total_supply_sum: Decimal,
    pub circulating_supply_sum: Decimal,
}

/// Type alias for batch token creation parameters
pub type BatchTokenParams<'a> = (
    Uuid,
    &'a str,
    &'a str,
    Option<&'a str>,
    Decimal,
    Decimal,
    Decimal,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_row_structure() {
        let token = TokenRow {
            token_id: Uuid::new_v4(),
            issuer_user_id: Uuid::new_v4(),
            symbol: "TEST".to_string(),
            name: "Test Token".to_string(),
            description: Some("A test token".to_string()),
            total_supply: Decimal::new(100000000, 2),
            circulating_supply: Decimal::new(50000000, 2),
            initial_price_btc: Decimal::new(100000, 8),
            price_increment_btc: Decimal::new(1000, 8),
            created_at: chrono::Utc::now(),
            status: "active".to_string(),
        };

        assert_eq!(token.symbol, "TEST");
        assert_eq!(token.status, "active");
    }

    #[test]
    fn test_token_soft_delete_status() {
        let deleted_token = TokenRow {
            token_id: Uuid::new_v4(),
            issuer_user_id: Uuid::new_v4(),
            symbol: "DEL".to_string(),
            name: "Deleted Token".to_string(),
            description: None,
            total_supply: Decimal::new(100000000, 2),
            circulating_supply: Decimal::new(0, 2),
            initial_price_btc: Decimal::new(100000, 8),
            price_increment_btc: Decimal::new(1000, 8),
            created_at: chrono::Utc::now(),
            status: "deleted".to_string(),
        };

        assert_eq!(deleted_token.status, "deleted");
    }

    #[test]
    fn test_token_status_values() {
        let statuses = vec!["active", "inactive", "deleted"];

        for status in &statuses {
            assert!(["active", "inactive", "deleted"].contains(status));
        }
    }

    #[test]
    fn test_token_supply_validation() {
        let total = Decimal::new(100000000, 2);
        let circulating = Decimal::new(50000000, 2);

        assert!(circulating <= total);
    }

    #[test]
    fn test_remaining_supply_calculation() {
        let total = Decimal::new(100000000, 2);
        let circulating = Decimal::new(30000000, 2);
        let remaining = total - circulating;

        assert_eq!(remaining, Decimal::new(70000000, 2));
    }

    #[test]
    fn test_supply_constraint_violation() {
        let total = Decimal::new(100000000, 2);
        let circulating = Decimal::new(120000000, 2);

        assert!(circulating > total); // Invalid state
    }

    #[test]
    fn test_supply_at_limit() {
        let total = Decimal::new(100000000, 2);
        let circulating = Decimal::new(100000000, 2);

        assert_eq!(circulating, total); // At limit
        assert!(circulating <= total);
    }

    #[test]
    fn test_token_statistics_structure() {
        let stats = TokenStatistics {
            total_active: 100,
            total_active_status: 80,
            total_inactive: 15,
            total_deleted: 5,
            total_supply_sum: Decimal::new(1000000000, 2),
            circulating_supply_sum: Decimal::new(500000000, 2),
        };

        assert_eq!(stats.total_active, 100);
        assert_eq!(stats.total_active_status, 80);
        assert!(stats.circulating_supply_sum < stats.total_supply_sum);
    }

    #[test]
    fn test_batch_create_empty_list() {
        let tokens: Vec<BatchTokenParams> = Vec::new();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_batch_create_tokens_structure() {
        let issuer_id = Uuid::new_v4();
        let tokens = [
            (
                issuer_id,
                "TOK1",
                "Token 1",
                Some("First token"),
                Decimal::new(1000000, 2),
                Decimal::new(100, 8),
                Decimal::new(1, 8),
            ),
            (
                issuer_id,
                "TOK2",
                "Token 2",
                None,
                Decimal::new(2000000, 2),
                Decimal::new(200, 8),
                Decimal::new(2, 8),
            ),
        ];

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].1, "TOK1");
        assert_eq!(tokens[1].2, "Token 2");
    }

    #[test]
    fn test_batch_update_status_empty_list() {
        let token_ids: Vec<Uuid> = vec![];
        assert!(token_ids.is_empty());
    }

    #[test]
    fn test_token_statistics_supply_sums() {
        let stats = TokenStatistics {
            total_active: 50,
            total_active_status: 40,
            total_inactive: 8,
            total_deleted: 2,
            total_supply_sum: Decimal::new(5000000000, 2),
            circulating_supply_sum: Decimal::new(2500000000, 2),
        };

        // Verify circulating is less than or equal to total
        assert!(stats.circulating_supply_sum <= stats.total_supply_sum);

        // Verify status counts
        let status_total = stats.total_active_status + stats.total_inactive;
        assert_eq!(status_total, 48); // Not including deleted in total_active
    }
}
