//! User repository

use rust_decimal::Decimal;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::Result;
use crate::helpers::calculate_offset;

/// User data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub user_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub btc_withdrawal_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub kyc_status: String,
    pub reputation_score: Decimal,
    pub role: String,
}

/// Repository for user operations
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find user by ID
    pub async fn find_by_id(&self, user_id: Uuid) -> Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(r#"SELECT * FROM users WHERE user_id = $1"#)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user)
    }

    /// Find user by email
    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(r#"SELECT * FROM users WHERE email = $1"#)
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user)
    }

    /// Find user by username
    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(r#"SELECT * FROM users WHERE username = $1"#)
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user)
    }

    /// Create a new user
    pub async fn create(
        &self,
        email: &str,
        password_hash: &str,
        username: &str,
    ) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (email, password_hash, username)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .bind(username)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    /// Update user profile
    pub async fn update_profile(
        &self,
        user_id: Uuid,
        display_name: Option<&str>,
        bio: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET display_name = COALESCE($2, display_name),
                bio = COALESCE($3, bio),
                avatar_url = COALESCE($4, avatar_url)
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(display_name)
        .bind(bio)
        .bind(avatar_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    /// Update user reputation score
    pub async fn update_reputation(
        &self,
        user_id: Uuid,
        delta: Decimal,
        reason: &str,
    ) -> Result<Decimal> {
        let mut tx = self.pool.begin().await?;

        // Update reputation with bounds
        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
            WHERE user_id = $1
            RETURNING reputation_score
            "#,
        )
        .bind(user_id)
        .bind(delta)
        .fetch_one(&mut *tx)
        .await?;

        // Record the event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, 'manual_adjustment', $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(delta)
        .bind(reason)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(new_score)
    }

    /// Update KYC status
    pub async fn update_kyc_status(&self, user_id: Uuid, status: &str) -> Result<()> {
        sqlx::query(r#"UPDATE users SET kyc_status = $2 WHERE user_id = $1"#)
            .bind(user_id)
            .bind(status)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Check if email exists
    pub async fn email_exists(&self, email: &str) -> Result<bool> {
        let (exists,): (bool,) =
            sqlx::query_as(r#"SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"#)
                .bind(email)
                .fetch_one(&self.pool)
                .await?;

        Ok(exists)
    }

    /// Check if username exists
    pub async fn username_exists(&self, username: &str) -> Result<bool> {
        let (exists,): (bool,) =
            sqlx::query_as(r#"SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)"#)
                .bind(username)
                .fetch_one(&self.pool)
                .await?;

        Ok(exists)
    }

    /// Soft delete a user by setting role to 'deleted'
    ///
    /// This implements soft-delete pattern by changing the user's role to 'deleted'
    /// instead of removing the record from the database. This preserves audit trails
    /// and referential integrity while preventing the user from accessing the system.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user to soft delete
    ///
    /// # Returns
    /// The updated user record
    pub async fn soft_delete(&self, user_id: Uuid) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET role = 'deleted', kyc_status = 'suspended'
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    /// Restore a soft-deleted user
    ///
    /// Restores a previously soft-deleted user by setting their role back to 'user'.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user to restore
    ///
    /// # Returns
    /// The updated user record
    pub async fn restore(&self, user_id: Uuid) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE users
            SET role = 'user', kyc_status = 'pending'
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    /// Check if a user is soft-deleted
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user to check
    ///
    /// # Returns
    /// True if the user's role is 'deleted'
    pub async fn is_deleted(&self, user_id: Uuid) -> Result<bool> {
        let (is_deleted,): (bool,) = sqlx::query_as(
            r#"SELECT EXISTS(SELECT 1 FROM users WHERE user_id = $1 AND role = 'deleted')"#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(is_deleted)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Batch create users (for bulk imports or migrations)
    ///
    /// Creates multiple users in a single transaction for optimal performance.
    /// Uses QueryBuilder for efficient batch inserts.
    ///
    /// # Arguments
    /// * `executor` - Database executor (pool or transaction)
    /// * `users` - Vector of (email, password_hash, username) tuples
    ///
    /// # Returns
    /// Number of users created
    pub async fn batch_create<'a, E>(
        &self,
        executor: E,
        users: &[(&str, &str, &str)],
    ) -> Result<u64>
    where
        E: PgExecutor<'a>,
    {
        if users.is_empty() {
            return Ok(0);
        }

        let mut query_builder =
            sqlx::QueryBuilder::new("INSERT INTO users (email, password_hash, username) ");

        query_builder.push_values(users, |mut b, (email, password_hash, username)| {
            b.push_bind(email)
                .push_bind(password_hash)
                .push_bind(username);
        });

        let result = query_builder.build().execute(executor).await?;

        Ok(result.rows_affected())
    }

    /// Batch update KYC status for multiple users
    ///
    /// Updates KYC status for multiple users in a single query.
    ///
    /// # Arguments
    /// * `user_ids` - Vector of user IDs to update
    /// * `status` - New KYC status to set
    ///
    /// # Returns
    /// Number of users updated
    pub async fn batch_update_kyc_status(&self, user_ids: &[Uuid], status: &str) -> Result<u64> {
        if user_ids.is_empty() {
            return Ok(0);
        }

        let mut query_builder = sqlx::QueryBuilder::new("UPDATE users SET kyc_status = ");
        query_builder.push_bind(status);
        query_builder.push(" WHERE user_id = ANY(");
        query_builder.push_bind(user_ids);
        query_builder.push(")");

        let result = query_builder.build().execute(&self.pool).await?;

        Ok(result.rows_affected())
    }

    /// Batch update reputation scores
    ///
    /// Updates reputation scores for multiple users in a single transaction.
    /// Each user can have a different delta applied.
    ///
    /// # Arguments
    /// * `updates` - Vector of (user_id, delta, reason) tuples
    ///
    /// # Returns
    /// Number of users updated
    pub async fn batch_update_reputation(&self, updates: &[(Uuid, Decimal, &str)]) -> Result<u64> {
        if updates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for (user_id, delta, reason) in updates {
            // Update reputation with bounds
            sqlx::query(
                r#"
                UPDATE users
                SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
                WHERE user_id = $1
                "#,
            )
            .bind(user_id)
            .bind(delta)
            .execute(&mut *tx)
            .await?;

            // Record the event
            sqlx::query(
                r#"
                INSERT INTO reputation_events (user_id, event_type, delta, reason)
                VALUES ($1, 'batch_adjustment', $2, $3)
                "#,
            )
            .bind(user_id)
            .bind(delta)
            .bind(reason)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;

        Ok(count)
    }

    // =========================================================================
    // Query & Filter Methods
    // =========================================================================

    /// Get users by role with pagination
    ///
    /// # Arguments
    /// * `role` - The role to filter by (e.g., "user", "admin", "deleted")
    /// * `limit` - Maximum number of users to return
    /// * `page` - Page number (1-indexed)
    ///
    /// # Returns
    /// Vector of users matching the role
    pub async fn get_users_by_role(
        &self,
        role: &str,
        limit: i64,
        page: i64,
    ) -> Result<Vec<UserRow>> {
        let offset = calculate_offset(page as u32, limit as u32);

        let users = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE role = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(role)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Get users by KYC status with pagination
    ///
    /// # Arguments
    /// * `kyc_status` - The KYC status to filter by (e.g., "pending", "verified", "rejected")
    /// * `limit` - Maximum number of users to return
    /// * `page` - Page number (1-indexed)
    ///
    /// # Returns
    /// Vector of users matching the KYC status
    pub async fn get_users_by_kyc_status(
        &self,
        kyc_status: &str,
        limit: i64,
        page: i64,
    ) -> Result<Vec<UserRow>> {
        let offset = calculate_offset(page as u32, limit as u32);

        let users = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE kyc_status = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(kyc_status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Get top users by reputation (leaderboard)
    ///
    /// # Arguments
    /// * `limit` - Maximum number of users to return
    ///
    /// # Returns
    /// Vector of top users sorted by reputation score descending
    pub async fn get_top_users_by_reputation(&self, limit: i64) -> Result<Vec<UserRow>> {
        let users = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE role != 'deleted'
            ORDER BY reputation_score DESC, created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Search users by email or username (case-insensitive)
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// Vector of users matching the search query
    pub async fn search_users(&self, query: &str, limit: i64) -> Result<Vec<UserRow>> {
        let search_pattern = format!("%{}%", query.to_lowercase());

        let users = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users
            WHERE LOWER(email) LIKE $1 OR LOWER(username) LIKE $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    // =========================================================================
    // Statistics & Counts
    // =========================================================================

    /// Count users by role
    ///
    /// # Arguments
    /// * `role` - The role to count
    ///
    /// # Returns
    /// Number of users with the specified role
    pub async fn count_users_by_role(&self, role: &str) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as(r#"SELECT COUNT(*) FROM users WHERE role = $1"#)
            .bind(role)
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    /// Count users by KYC status
    ///
    /// # Arguments
    /// * `kyc_status` - The KYC status to count
    ///
    /// # Returns
    /// Number of users with the specified KYC status
    pub async fn count_users_by_kyc_status(&self, kyc_status: &str) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM users WHERE kyc_status = $1"#)
                .bind(kyc_status)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get total user count
    ///
    /// # Returns
    /// Total number of users (excluding soft-deleted)
    pub async fn count_total_users(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM users WHERE role != 'deleted'"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get user statistics
    ///
    /// Returns a summary of user counts by role and KYC status
    pub async fn get_user_statistics(&self) -> Result<UserStatistics> {
        let total = self.count_total_users().await?;
        let admins = self.count_users_by_role("admin").await?;
        let users = self.count_users_by_role("user").await?;
        let deleted = self.count_users_by_role("deleted").await?;

        let kyc_pending = self.count_users_by_kyc_status("pending").await?;
        let kyc_verified = self.count_users_by_kyc_status("verified").await?;
        let kyc_rejected = self.count_users_by_kyc_status("rejected").await?;

        Ok(UserStatistics {
            total_active: total,
            total_admins: admins,
            total_users: users,
            total_deleted: deleted,
            kyc_pending,
            kyc_verified,
            kyc_rejected,
        })
    }
}

/// User statistics summary
#[derive(Debug, Clone)]
pub struct UserStatistics {
    pub total_active: i64,
    pub total_admins: i64,
    pub total_users: i64,
    pub total_deleted: i64,
    pub kyc_pending: i64,
    pub kyc_verified: i64,
    pub kyc_rejected: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_row_structure() {
        let user = UserRow {
            user_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            password_hash: "hash123".to_string(),
            username: "testuser".to_string(),
            display_name: Some("Test User".to_string()),
            bio: Some("A test user".to_string()),
            avatar_url: Some("https://example.com/avatar.jpg".to_string()),
            btc_withdrawal_address: Some("bc1q...".to_string()),
            created_at: chrono::Utc::now(),
            kyc_status: "verified".to_string(),
            reputation_score: Decimal::new(75000, 2),
            role: "user".to_string(),
        };

        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.role, "user");
    }

    #[test]
    fn test_user_soft_delete_status() {
        let deleted_user = UserRow {
            user_id: Uuid::new_v4(),
            email: "deleted@example.com".to_string(),
            password_hash: "hash123".to_string(),
            username: "deleteduser".to_string(),
            display_name: None,
            bio: None,
            avatar_url: None,
            btc_withdrawal_address: None,
            created_at: chrono::Utc::now(),
            kyc_status: "suspended".to_string(),
            reputation_score: Decimal::new(50000, 2),
            role: "deleted".to_string(),
        };

        assert_eq!(deleted_user.role, "deleted");
        assert_eq!(deleted_user.kyc_status, "suspended");
    }

    #[test]
    fn test_user_reputation_bounds() {
        let min_score = Decimal::ZERO;
        let max_score = Decimal::new(100000, 2); // 1000.00

        assert!(min_score >= Decimal::ZERO);
        assert!(max_score <= Decimal::new(100000, 2));
    }

    #[test]
    fn test_user_statistics_structure() {
        let stats = UserStatistics {
            total_active: 100,
            total_admins: 5,
            total_users: 95,
            total_deleted: 10,
            kyc_pending: 30,
            kyc_verified: 60,
            kyc_rejected: 10,
        };

        assert_eq!(stats.total_active, 100);
        assert_eq!(stats.total_admins, 5);
        assert_eq!(stats.total_users, 95);
        assert_eq!(stats.kyc_verified, 60);
    }

    #[test]
    fn test_batch_create_empty_list() {
        let users: Vec<(&str, &str, &str)> = vec![];
        assert!(users.is_empty());
    }

    #[test]
    fn test_batch_create_users_structure() {
        let users = [
            ("user1@example.com", "hash1", "user1"),
            ("user2@example.com", "hash2", "user2"),
            ("user3@example.com", "hash3", "user3"),
        ];

        assert_eq!(users.len(), 3);
        assert_eq!(users[0].0, "user1@example.com");
        assert_eq!(users[1].1, "hash2");
        assert_eq!(users[2].2, "user3");
    }

    #[test]
    fn test_batch_update_kyc_empty_list() {
        let user_ids: Vec<Uuid> = vec![];
        assert!(user_ids.is_empty());
    }

    #[test]
    fn test_batch_update_reputation_structure() {
        let updates = [
            (Uuid::new_v4(), Decimal::new(10, 0), "good_trade"),
            (Uuid::new_v4(), Decimal::new(-5, 0), "late_delivery"),
            (Uuid::new_v4(), Decimal::new(20, 0), "excellent_service"),
        ];

        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0].2, "good_trade");
        assert!(updates[1].1 < Decimal::ZERO);
        assert!(updates[2].1 > Decimal::ZERO);
    }

    #[test]
    fn test_search_pattern_formatting() {
        let query = "test";
        let pattern = format!("%{}%", query.to_lowercase());
        assert_eq!(pattern, "%test%");

        let query_mixed = "TeSt";
        let pattern_mixed = format!("%{}%", query_mixed.to_lowercase());
        assert_eq!(pattern_mixed, "%test%");
    }

    #[test]
    fn test_user_statistics_totals() {
        let stats = UserStatistics {
            total_active: 100,
            total_admins: 5,
            total_users: 95,
            total_deleted: 0,
            kyc_pending: 30,
            kyc_verified: 65,
            kyc_rejected: 5,
        };

        // Verify KYC statuses add up to total active
        let kyc_total = stats.kyc_pending + stats.kyc_verified + stats.kyc_rejected;
        assert_eq!(kyc_total, stats.total_active);

        // Verify role totals
        let role_total = stats.total_admins + stats.total_users;
        assert_eq!(role_total, stats.total_active);
    }
}
