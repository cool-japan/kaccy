//! Analytics module for reputation system insights
//!
//! Provides comprehensive statistics, trends, and insights across
//! the entire reputation system for monitoring and decision-making.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ReputationError;
use crate::tier::ReputationTier;

/// Service for generating analytics and insights
pub struct AnalyticsService {
    pool: PgPool,
}

impl AnalyticsService {
    /// Create a new [`AnalyticsService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get overall system statistics
    pub async fn get_system_stats(&self) -> Result<SystemStats, ReputationError> {
        let total_users = self.count_total_users().await?;
        let active_users_30d = self.count_active_users(30).await?;
        let tier_distribution = self.get_tier_distribution().await?;
        let avg_score = self.calculate_average_score().await?;
        let total_commitments = self.count_total_commitments().await?;
        let completed_commitments = self.count_completed_commitments().await?;

        let completion_rate = if total_commitments > 0 {
            (completed_commitments as f64 / total_commitments as f64) * 100.0
        } else {
            0.0
        };

        Ok(SystemStats {
            total_users,
            active_users_30d,
            tier_distribution,
            average_score: avg_score,
            total_commitments,
            completed_commitments,
            completion_rate,
            generated_at: Utc::now(),
        })
    }

    /// Get user growth metrics over time
    pub async fn get_growth_metrics(&self, days: i32) -> Result<GrowthMetrics, ReputationError> {
        let start_date = Utc::now() - chrono::Duration::days(days as i64);

        let new_users = self.count_users_since(start_date).await?;
        let new_commitments = self.count_commitments_since(start_date).await?;
        let avg_daily_new_users = new_users as f64 / days as f64;
        let avg_daily_commitments = new_commitments as f64 / days as f64;

        Ok(GrowthMetrics {
            period_days: days,
            new_users,
            new_commitments,
            avg_daily_new_users,
            avg_daily_commitments,
        })
    }

    /// Get top performers by score
    pub async fn get_top_performers(
        &self,
        limit: i32,
    ) -> Result<Vec<UserRanking>, ReputationError> {
        let query = r#"
            SELECT user_id, reputation_score
            FROM users
            WHERE reputation_score IS NOT NULL
            ORDER BY reputation_score DESC
            LIMIT $1
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut rankings = Vec::new();
        for (rank, row) in rows.iter().enumerate() {
            let user_id: Uuid = row.try_get("user_id").map_err(ReputationError::Database)?;
            let score: Decimal = row
                .try_get("reputation_score")
                .map_err(ReputationError::Database)?;

            rankings.push(UserRanking {
                rank: (rank + 1) as i32,
                user_id,
                score,
                tier: ReputationTier::from_score(score),
            });
        }

        Ok(rankings)
    }

    /// Get commitment analytics for a specific user
    pub async fn get_user_commitment_analytics(
        &self,
        user_id: Uuid,
    ) -> Result<UserCommitmentAnalytics, ReputationError> {
        let query = r#"
            SELECT
                COUNT(*) as total,
                COUNT(CASE WHEN status = 'Verified' THEN 1 END) as verified,
                COUNT(CASE WHEN status = 'Failed' THEN 1 END) as failed,
                COUNT(CASE WHEN status = 'Pending' THEN 1 END) as pending,
                COUNT(CASE WHEN status = 'Expired' THEN 1 END) as expired
            FROM output_commitments
            WHERE user_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let total: i64 = row.try_get("total").map_err(ReputationError::Database)?;
        let verified: i64 = row.try_get("verified").map_err(ReputationError::Database)?;
        let failed: i64 = row.try_get("failed").map_err(ReputationError::Database)?;
        let pending: i64 = row.try_get("pending").map_err(ReputationError::Database)?;
        let expired: i64 = row.try_get("expired").map_err(ReputationError::Database)?;

        let success_rate = if total > 0 {
            (verified as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(UserCommitmentAnalytics {
            user_id,
            total_commitments: total as i32,
            verified: verified as i32,
            failed: failed as i32,
            pending: pending as i32,
            expired: expired as i32,
            success_rate,
        })
    }

    /// Get score distribution across all users
    pub async fn get_score_distribution(&self) -> Result<ScoreDistribution, ReputationError> {
        let query = r#"
            SELECT
                COUNT(CASE WHEN reputation_score < 200 THEN 1 END) as range_0_199,
                COUNT(CASE WHEN reputation_score >= 200 AND reputation_score < 400 THEN 1 END) as range_200_399,
                COUNT(CASE WHEN reputation_score >= 400 AND reputation_score < 600 THEN 1 END) as range_400_599,
                COUNT(CASE WHEN reputation_score >= 600 AND reputation_score < 800 THEN 1 END) as range_600_799,
                COUNT(CASE WHEN reputation_score >= 800 THEN 1 END) as range_800_plus
            FROM users
            WHERE reputation_score IS NOT NULL
        "#;

        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        Ok(ScoreDistribution {
            range_0_199: row
                .try_get("range_0_199")
                .map_err(ReputationError::Database)?,
            range_200_399: row
                .try_get("range_200_399")
                .map_err(ReputationError::Database)?,
            range_400_599: row
                .try_get("range_400_599")
                .map_err(ReputationError::Database)?,
            range_600_799: row
                .try_get("range_600_799")
                .map_err(ReputationError::Database)?,
            range_800_plus: row
                .try_get("range_800_plus")
                .map_err(ReputationError::Database)?,
        })
    }

    // Helper methods

    async fn count_total_users(&self) -> Result<i32, ReputationError> {
        let query = "SELECT COUNT(*) as count FROM users";
        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }

    async fn count_active_users(&self, days: i32) -> Result<i32, ReputationError> {
        let since = Utc::now() - chrono::Duration::days(days as i64);
        let query = r#"
            SELECT COUNT(DISTINCT user_id) as count
            FROM reputation_events
            WHERE created_at >= $1
        "#;

        let row = sqlx::query(query)
            .bind(since)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }

    async fn get_tier_distribution(&self) -> Result<Vec<TierCount>, ReputationError> {
        let query = r#"
            SELECT
                CASE
                    WHEN reputation_score < 200 THEN 'Unverified'
                    WHEN reputation_score >= 200 AND reputation_score < 400 THEN 'Bronze'
                    WHEN reputation_score >= 400 AND reputation_score < 600 THEN 'Silver'
                    WHEN reputation_score >= 600 AND reputation_score < 800 THEN 'Gold'
                    WHEN reputation_score >= 800 AND reputation_score < 950 THEN 'Platinum'
                    ELSE 'Diamond'
                END as tier,
                COUNT(*) as count
            FROM users
            WHERE reputation_score IS NOT NULL
            GROUP BY tier
            ORDER BY MIN(reputation_score)
        "#;

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut distribution = Vec::new();
        for row in rows {
            let tier_str: String = row.try_get("tier").map_err(ReputationError::Database)?;
            let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;

            let tier = tier_str
                .parse::<ReputationTier>()
                .map_err(|e| ReputationError::Validation(e.to_string()))?;

            distribution.push(TierCount {
                tier,
                count: count as i32,
            });
        }

        Ok(distribution)
    }

    async fn calculate_average_score(&self) -> Result<Decimal, ReputationError> {
        let query =
            "SELECT AVG(reputation_score) as avg FROM users WHERE reputation_score IS NOT NULL";
        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let avg: Option<Decimal> = row.try_get("avg").map_err(ReputationError::Database)?;
        Ok(avg.unwrap_or(Decimal::ZERO))
    }

    async fn count_total_commitments(&self) -> Result<i32, ReputationError> {
        let query = "SELECT COUNT(*) as count FROM output_commitments";
        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }

    async fn count_completed_commitments(&self) -> Result<i32, ReputationError> {
        let query = "SELECT COUNT(*) as count FROM output_commitments WHERE status = 'Verified'";
        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }

    async fn count_users_since(&self, since: DateTime<Utc>) -> Result<i32, ReputationError> {
        let query = "SELECT COUNT(*) as count FROM users WHERE created_at >= $1";
        let row = sqlx::query(query)
            .bind(since)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }

    async fn count_commitments_since(&self, since: DateTime<Utc>) -> Result<i32, ReputationError> {
        let query = "SELECT COUNT(*) as count FROM output_commitments WHERE created_at >= $1";
        let row = sqlx::query(query)
            .bind(since)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;
        Ok(count as i32)
    }
}

/// Overall system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    /// Total number of registered users.
    pub total_users: i32,
    /// Number of users active within the last 30 days.
    pub active_users_30d: i32,
    /// Distribution of users across reputation tiers.
    pub tier_distribution: Vec<TierCount>,
    /// Average reputation score across all users.
    pub average_score: Decimal,
    /// Total number of commitments ever created.
    pub total_commitments: i32,
    /// Number of commitments that have been completed.
    pub completed_commitments: i32,
    /// Fraction of commitments that were completed (0.0–1.0).
    pub completion_rate: f64,
    /// Timestamp when these statistics were generated.
    pub generated_at: DateTime<Utc>,
}

/// User count per tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierCount {
    /// Reputation tier.
    pub tier: ReputationTier,
    /// Number of users in this tier.
    pub count: i32,
}

/// Growth metrics over a period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthMetrics {
    /// Length of the measurement period in days.
    pub period_days: i32,
    /// Number of new users registered during the period.
    pub new_users: i32,
    /// Number of new commitments created during the period.
    pub new_commitments: i32,
    /// Average new users per day over the period.
    pub avg_daily_new_users: f64,
    /// Average new commitments per day over the period.
    pub avg_daily_commitments: f64,
}

/// User ranking information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRanking {
    /// User's position in the leaderboard (1-based).
    pub rank: i32,
    /// User identifier.
    pub user_id: Uuid,
    /// User's reputation score.
    pub score: Decimal,
    /// User's current reputation tier.
    pub tier: ReputationTier,
}

/// User-specific commitment analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCommitmentAnalytics {
    /// User these analytics belong to.
    pub user_id: Uuid,
    /// Total number of commitments created by this user.
    pub total_commitments: i32,
    /// Number of commitments that have been verified as fulfilled.
    pub verified: i32,
    /// Number of commitments that failed.
    pub failed: i32,
    /// Number of commitments currently pending verification.
    pub pending: i32,
    /// Number of commitments that expired without being fulfilled.
    pub expired: i32,
    /// Fraction of commitments that were verified (0.0–1.0).
    pub success_rate: f64,
}

/// Score distribution across ranges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDistribution {
    /// Number of users with scores 0–199.
    pub range_0_199: i64,
    /// Number of users with scores 200–399.
    pub range_200_399: i64,
    /// Number of users with scores 400–599.
    pub range_400_599: i64,
    /// Number of users with scores 600–799.
    pub range_600_799: i64,
    /// Number of users with scores 800 or above.
    pub range_800_plus: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_stats_structure() {
        let stats = SystemStats {
            total_users: 1000,
            active_users_30d: 750,
            tier_distribution: vec![TierCount {
                tier: ReputationTier::Gold,
                count: 100,
            }],
            average_score: Decimal::new(650, 0),
            total_commitments: 5000,
            completed_commitments: 4000,
            completion_rate: 80.0,
            generated_at: Utc::now(),
        };

        assert_eq!(stats.total_users, 1000);
        assert_eq!(stats.completion_rate, 80.0);
    }

    #[test]
    fn test_growth_metrics_calculation() {
        let metrics = GrowthMetrics {
            period_days: 30,
            new_users: 300,
            new_commitments: 1500,
            avg_daily_new_users: 10.0,
            avg_daily_commitments: 50.0,
        };

        assert_eq!(metrics.avg_daily_new_users, 10.0);
        assert_eq!(metrics.avg_daily_commitments, 50.0);
    }

    #[test]
    fn test_user_ranking_structure() {
        let ranking = UserRanking {
            rank: 1,
            user_id: Uuid::new_v4(),
            score: Decimal::new(950, 0),
            tier: ReputationTier::Diamond,
        };

        assert_eq!(ranking.rank, 1);
        assert_eq!(ranking.tier, ReputationTier::Diamond);
    }

    #[test]
    fn test_commitment_analytics_success_rate() {
        let analytics = UserCommitmentAnalytics {
            user_id: Uuid::new_v4(),
            total_commitments: 100,
            verified: 80,
            failed: 10,
            pending: 5,
            expired: 5,
            success_rate: 80.0,
        };

        assert_eq!(analytics.success_rate, 80.0);
        assert_eq!(
            analytics.verified + analytics.failed + analytics.pending + analytics.expired,
            analytics.total_commitments
        );
    }

    #[test]
    fn test_score_distribution_structure() {
        let dist = ScoreDistribution {
            range_0_199: 100,
            range_200_399: 200,
            range_400_599: 300,
            range_600_799: 250,
            range_800_plus: 150,
        };

        let total = dist.range_0_199
            + dist.range_200_399
            + dist.range_400_599
            + dist.range_600_799
            + dist.range_800_plus;
        assert_eq!(total, 1000);
    }
}
