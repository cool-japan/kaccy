//! Commitment repository

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;
use crate::helpers::calculate_offset;

/// Output commitment data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommitmentRow {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    pub status: String,
    pub evidence_url: Option<String>,
    pub evidence_description: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Commitment with user and token details
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommitmentWithDetails {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    pub status: String,
    pub evidence_url: Option<String>,
    pub evidence_description: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    // User details
    pub username: String,
    pub display_name: Option<String>,
    pub reputation_score: rust_decimal::Decimal,
    // Token details
    pub token_symbol: String,
    pub token_name: String,
}

/// Commitment statistics for a user
#[derive(Debug, Clone)]
pub struct CommitmentStats {
    pub user_id: Uuid,
    pub total_commitments: i64,
    pub pending_count: i64,
    pub completed_count: i64,
    pub verified_count: i64,
    pub failed_count: i64,
    pub expired_count: i64,
    pub fulfillment_rate: f64,
}

/// Repository for commitment operations
pub struct CommitmentRepository {
    pool: PgPool,
}

impl CommitmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new commitment
    pub async fn create(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        title: &str,
        description: Option<&str>,
        deadline: DateTime<Utc>,
    ) -> Result<CommitmentRow> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"
            INSERT INTO output_commitments (user_id, token_id, title, description, deadline)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .bind(title)
        .bind(description)
        .bind(deadline)
        .fetch_one(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Find commitment by ID
    pub async fn find_by_id(&self, commitment_id: Uuid) -> Result<Option<CommitmentRow>> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"SELECT * FROM output_commitments WHERE commitment_id = $1"#,
        )
        .bind(commitment_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Find commitment with user and token details
    pub async fn find_by_id_with_details(
        &self,
        commitment_id: Uuid,
    ) -> Result<Option<CommitmentWithDetails>> {
        let commitment = sqlx::query_as::<_, CommitmentWithDetails>(
            r#"
            SELECT
                c.*,
                u.username,
                u.display_name,
                u.reputation_score,
                t.symbol as token_symbol,
                t.name as token_name
            FROM output_commitments c
            JOIN users u ON c.user_id = u.user_id
            JOIN tokens t ON c.token_id = t.token_id
            WHERE c.commitment_id = $1
            "#,
        )
        .bind(commitment_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Update commitment with evidence
    pub async fn submit_evidence(
        &self,
        commitment_id: Uuid,
        user_id: Uuid,
        evidence_url: &str,
        evidence_description: Option<&str>,
    ) -> Result<Option<CommitmentRow>> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"
            UPDATE output_commitments
            SET evidence_url = $3, evidence_description = $4, status = 'completed'
            WHERE commitment_id = $1 AND user_id = $2 AND status = 'pending'
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .bind(user_id)
        .bind(evidence_url)
        .bind(evidence_description)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Mark commitment as verified
    pub async fn mark_verified(
        &self,
        commitment_id: Uuid,
        verified_by: Uuid,
    ) -> Result<Option<CommitmentRow>> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"
            UPDATE output_commitments
            SET status = 'verified', verified_at = NOW(), verified_by = $2
            WHERE commitment_id = $1 AND status = 'completed'
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .bind(verified_by)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Mark commitment as failed
    pub async fn mark_failed(
        &self,
        commitment_id: Uuid,
        verified_by: Uuid,
    ) -> Result<Option<CommitmentRow>> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"
            UPDATE output_commitments
            SET status = 'failed', verified_at = NOW(), verified_by = $2
            WHERE commitment_id = $1 AND status = 'completed'
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .bind(verified_by)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Get user's commitments with pagination
    pub async fn get_user_commitments(
        &self,
        user_id: Uuid,
        page: u32,
        limit: u32,
    ) -> Result<Vec<CommitmentRow>> {
        let offset = calculate_offset(page, limit);

        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get user's commitments filtered by status
    pub async fn get_user_commitments_by_status(
        &self,
        user_id: Uuid,
        status: &str,
        limit: u32,
    ) -> Result<Vec<CommitmentRow>> {
        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE user_id = $1 AND status = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(status)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get token's commitments
    pub async fn get_token_commitments(
        &self,
        token_id: Uuid,
        page: u32,
        limit: u32,
    ) -> Result<Vec<CommitmentRow>> {
        let offset = calculate_offset(page, limit);

        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE token_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(token_id)
        .bind(limit as i64)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get pending commitments for verification (with details)
    pub async fn get_pending_verification(&self, limit: u32) -> Result<Vec<CommitmentWithDetails>> {
        let commitments = sqlx::query_as::<_, CommitmentWithDetails>(
            r#"
            SELECT
                c.*,
                u.username,
                u.display_name,
                u.reputation_score,
                t.symbol as token_symbol,
                t.name as token_name
            FROM output_commitments c
            JOIN users u ON c.user_id = u.user_id
            JOIN tokens t ON c.token_id = t.token_id
            WHERE c.status = 'completed'
            ORDER BY c.created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get overdue commitments that need to be expired
    pub async fn get_overdue_commitments(&self) -> Result<Vec<CommitmentRow>> {
        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE status = 'pending' AND deadline < NOW()
            ORDER BY deadline ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Expire a single commitment
    pub async fn expire(&self, commitment_id: Uuid) -> Result<Option<CommitmentRow>> {
        let commitment = sqlx::query_as::<_, CommitmentRow>(
            r#"
            UPDATE output_commitments
            SET status = 'expired'
            WHERE commitment_id = $1 AND status = 'pending'
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(commitment)
    }

    /// Get commitment statistics for a user
    pub async fn get_user_stats(&self, user_id: Uuid) -> Result<CommitmentStats> {
        let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) FILTER (WHERE status = 'verified') as verified,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'expired') as expired
            FROM output_commitments
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let total_resolved = row.3 + row.4; // verified + failed
        let fulfillment_rate = if total_resolved > 0 {
            row.3 as f64 / total_resolved as f64
        } else {
            0.0
        };

        Ok(CommitmentStats {
            user_id,
            total_commitments: row.0,
            pending_count: row.1,
            completed_count: row.2,
            verified_count: row.3,
            failed_count: row.4,
            expired_count: row.5,
            fulfillment_rate,
        })
    }

    /// Count pending verification queue
    pub async fn count_pending_verification(&self) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM output_commitments WHERE status = 'completed'"#)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get commitments approaching deadline (for notifications)
    pub async fn get_approaching_deadline(&self, hours_before: i64) -> Result<Vec<CommitmentRow>> {
        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE status = 'pending'
              AND deadline > NOW()
              AND deadline < NOW() + INTERVAL '1 hour' * $1
            ORDER BY deadline ASC
            "#,
        )
        .bind(hours_before)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Batch expire overdue commitments
    pub async fn batch_expire_overdue(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE output_commitments
            SET status = 'expired'
            WHERE status = 'pending' AND deadline < NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch verify multiple commitments
    pub async fn batch_verify(&self, commitment_ids: &[Uuid], verified_by: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE output_commitments
            SET status = 'verified', verified_at = NOW(), verified_by = $2
            WHERE commitment_id = ANY($1) AND status = 'completed'
            "#,
        )
        .bind(commitment_ids)
        .bind(verified_by)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Batch mark multiple commitments as failed
    pub async fn batch_mark_failed(
        &self,
        commitment_ids: &[Uuid],
        verified_by: Uuid,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE output_commitments
            SET status = 'failed', verified_at = NOW(), verified_by = $2
            WHERE commitment_id = ANY($1) AND status = 'completed'
            "#,
        )
        .bind(commitment_ids)
        .bind(verified_by)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get commitment statistics for a token
    pub async fn get_token_stats(&self, token_id: Uuid) -> Result<TokenCommitmentStats> {
        let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) FILTER (WHERE status = 'verified') as verified,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'expired') as expired
            FROM output_commitments
            WHERE token_id = $1
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        let total_resolved = row.3 + row.4; // verified + failed
        let fulfillment_rate = if total_resolved > 0 {
            row.3 as f64 / total_resolved as f64
        } else {
            0.0
        };

        Ok(TokenCommitmentStats {
            token_id,
            total_commitments: row.0,
            pending_count: row.1,
            completed_count: row.2,
            verified_count: row.3,
            failed_count: row.4,
            expired_count: row.5,
            fulfillment_rate,
        })
    }

    /// Get top performers by fulfillment rate (minimum commitments required)
    pub async fn get_top_performers(
        &self,
        min_commitments: i64,
        limit: i64,
    ) -> Result<Vec<UserPerformance>> {
        let performers = sqlx::query_as::<_, UserPerformance>(
            r#"
            SELECT
                c.user_id,
                u.username,
                u.display_name,
                u.reputation_score,
                COUNT(*) as total_commitments,
                COUNT(*) FILTER (WHERE c.status = 'verified') as verified_count,
                COUNT(*) FILTER (WHERE c.status = 'failed') as failed_count,
                CASE
                    WHEN COUNT(*) FILTER (WHERE c.status IN ('verified', 'failed')) > 0
                    THEN CAST(COUNT(*) FILTER (WHERE c.status = 'verified') AS DOUBLE PRECISION) /
                         CAST(COUNT(*) FILTER (WHERE c.status IN ('verified', 'failed')) AS DOUBLE PRECISION)
                    ELSE 0.0
                END as fulfillment_rate
            FROM output_commitments c
            JOIN users u ON c.user_id = u.user_id
            GROUP BY c.user_id, u.username, u.display_name, u.reputation_score
            HAVING COUNT(*) >= $1
            ORDER BY fulfillment_rate DESC, verified_count DESC
            LIMIT $2
            "#,
        )
        .bind(min_commitments)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(performers)
    }

    /// Get average time to complete commitments (in hours)
    pub async fn get_average_completion_time(&self, user_id: Option<Uuid>) -> Result<Option<f64>> {
        let row: (Option<f64>,) = if let Some(uid) = user_id {
            sqlx::query_as(
                r#"
                SELECT AVG(EXTRACT(EPOCH FROM (verified_at - created_at)) / 3600.0)
                FROM output_commitments
                WHERE user_id = $1 AND status IN ('verified', 'failed')
                "#,
            )
            .bind(uid)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT AVG(EXTRACT(EPOCH FROM (verified_at - created_at)) / 3600.0)
                FROM output_commitments
                WHERE status IN ('verified', 'failed')
                "#,
            )
            .fetch_one(&self.pool)
            .await?
        };

        Ok(row.0)
    }

    /// Get average verification turnaround time (time from completed to verified/failed, in hours)
    pub async fn get_average_verification_time(&self) -> Result<Option<f64>> {
        let row: (Option<f64>,) = sqlx::query_as(
            r#"
            SELECT AVG(EXTRACT(EPOCH FROM (verified_at - updated_at)) / 3600.0)
            FROM output_commitments
            WHERE status IN ('verified', 'failed') AND verified_at IS NOT NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Get commitments by date range
    pub async fn get_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommitmentRow>> {
        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE created_at BETWEEN $1 AND $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get commitments by deadline range
    pub async fn get_by_deadline_range(
        &self,
        start_deadline: DateTime<Utc>,
        end_deadline: DateTime<Utc>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<CommitmentRow>> {
        let commitments = if let Some(s) = status {
            sqlx::query_as::<_, CommitmentRow>(
                r#"
                SELECT * FROM output_commitments
                WHERE deadline BETWEEN $1 AND $2 AND status = $3
                ORDER BY deadline ASC
                LIMIT $4
                "#,
            )
            .bind(start_deadline)
            .bind(end_deadline)
            .bind(s)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, CommitmentRow>(
                r#"
                SELECT * FROM output_commitments
                WHERE deadline BETWEEN $1 AND $2
                ORDER BY deadline ASC
                LIMIT $3
                "#,
            )
            .bind(start_deadline)
            .bind(end_deadline)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(commitments)
    }

    /// Search commitments by title (case-insensitive)
    pub async fn search_by_title(
        &self,
        search_term: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommitmentRow>> {
        let search_pattern = format!("%{}%", search_term);
        let commitments = sqlx::query_as::<_, CommitmentRow>(
            r#"
            SELECT * FROM output_commitments
            WHERE LOWER(title) LIKE LOWER($1)
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(search_pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get commitment trends (count by status over time period)
    pub async fn get_trends(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<CommitmentTrend>> {
        let trends = sqlx::query_as::<_, CommitmentTrend>(
            r#"
            SELECT
                DATE(created_at) as date,
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) FILTER (WHERE status = 'verified') as verified,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'expired') as expired
            FROM output_commitments
            WHERE created_at BETWEEN $1 AND $2
            GROUP BY DATE(created_at)
            ORDER BY DATE(created_at) ASC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(trends)
    }

    /// Count commitments by status
    pub async fn count_by_status(&self, status: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM output_commitments WHERE status = $1
            "#,
        )
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Delete old expired commitments (data retention policy)
    pub async fn cleanup_expired(&self, days_old: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM output_commitments
            WHERE status = 'expired'
            AND created_at < NOW() - INTERVAL '1 day' * $1
            "#,
        )
        .bind(days_old)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

/// Token commitment statistics
#[derive(Debug, Clone)]
pub struct TokenCommitmentStats {
    pub token_id: Uuid,
    pub total_commitments: i64,
    pub pending_count: i64,
    pub completed_count: i64,
    pub verified_count: i64,
    pub failed_count: i64,
    pub expired_count: i64,
    pub fulfillment_rate: f64,
}

/// User performance metrics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserPerformance {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub reputation_score: Decimal,
    pub total_commitments: i64,
    pub verified_count: i64,
    pub failed_count: i64,
    pub fulfillment_rate: f64,
}

/// Commitment trend data (daily aggregation)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommitmentTrend {
    pub date: chrono::NaiveDate,
    pub total: i64,
    pub pending: i64,
    pub completed: i64,
    pub verified: i64,
    pub failed: i64,
    pub expired: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_commitment_stats_creation() {
        let stats = TokenCommitmentStats {
            token_id: Uuid::new_v4(),
            total_commitments: 100,
            pending_count: 20,
            completed_count: 30,
            verified_count: 40,
            failed_count: 5,
            expired_count: 5,
            fulfillment_rate: 0.89,
        };

        assert_eq!(stats.total_commitments, 100);
        assert_eq!(stats.verified_count, 40);
        assert_eq!(stats.fulfillment_rate, 0.89);
    }

    #[test]
    fn test_user_performance_creation() {
        let perf = UserPerformance {
            user_id: Uuid::new_v4(),
            username: "testuser".to_string(),
            display_name: Some("Test User".to_string()),
            reputation_score: Decimal::new(850, 0),
            total_commitments: 50,
            verified_count: 45,
            failed_count: 5,
            fulfillment_rate: 0.9,
        };

        assert_eq!(perf.username, "testuser");
        assert_eq!(perf.total_commitments, 50);
        assert_eq!(perf.fulfillment_rate, 0.9);
    }

    #[test]
    fn test_commitment_trend_creation() {
        let trend = CommitmentTrend {
            date: chrono::NaiveDate::from_ymd_opt(2026, 1, 18).unwrap(),
            total: 100,
            pending: 30,
            completed: 20,
            verified: 40,
            failed: 5,
            expired: 5,
        };

        assert_eq!(trend.total, 100);
        assert_eq!(trend.verified, 40);
    }

    #[test]
    fn test_fulfillment_rate_calculation() {
        let verified = 90i64;
        let failed = 10i64;
        let total_resolved = verified + failed;

        let rate = if total_resolved > 0 {
            verified as f64 / total_resolved as f64
        } else {
            0.0
        };

        assert_eq!(rate, 0.9);
    }

    #[test]
    fn test_fulfillment_rate_zero_resolved() {
        let verified = 0i64;
        let failed = 0i64;
        let total_resolved = verified + failed;

        let rate = if total_resolved > 0 {
            verified as f64 / total_resolved as f64
        } else {
            0.0
        };

        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_fulfillment_rate_perfect() {
        let verified = 100i64;
        let failed = 0i64;
        let total_resolved = verified + failed;

        let rate = if total_resolved > 0 {
            verified as f64 / total_resolved as f64
        } else {
            0.0
        };

        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_commitment_stats_creation() {
        let stats = CommitmentStats {
            user_id: Uuid::new_v4(),
            total_commitments: 100,
            pending_count: 20,
            completed_count: 30,
            verified_count: 40,
            failed_count: 5,
            expired_count: 5,
            fulfillment_rate: 0.89,
        };

        assert_eq!(stats.total_commitments, 100);
        assert_eq!(stats.verified_count, 40);
    }

    #[test]
    fn test_search_pattern_generation() {
        let search_term = "token";
        let pattern = format!("%{}%", search_term);
        assert_eq!(pattern, "%token%");
    }

    #[test]
    fn test_commitment_row_clone() {
        let row = CommitmentRow {
            commitment_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            title: "Test Commitment".to_string(),
            description: Some("Description".to_string()),
            deadline: Utc::now(),
            status: "pending".to_string(),
            evidence_url: None,
            evidence_description: None,
            verified_at: None,
            verified_by: None,
            created_at: Utc::now(),
        };

        let cloned = row.clone();
        assert_eq!(cloned.title, row.title);
        assert_eq!(cloned.status, row.status);
    }

    #[test]
    fn test_commitment_with_details_clone() {
        let details = CommitmentWithDetails {
            commitment_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            title: "Test".to_string(),
            description: None,
            deadline: Utc::now(),
            status: "pending".to_string(),
            evidence_url: None,
            evidence_description: None,
            verified_at: None,
            verified_by: None,
            created_at: Utc::now(),
            username: "testuser".to_string(),
            display_name: Some("Test User".to_string()),
            reputation_score: Decimal::new(800, 0),
            token_symbol: "TEST".to_string(),
            token_name: "Test Token".to_string(),
        };

        let cloned = details.clone();
        assert_eq!(cloned.username, details.username);
        assert_eq!(cloned.token_symbol, details.token_symbol);
    }
}
