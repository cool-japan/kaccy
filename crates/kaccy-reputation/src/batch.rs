//! Batch operations module for efficient bulk processing
//!
//! Provides high-performance batch operations for processing multiple
//! users, commitments, and scores simultaneously.

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ReputationError;
use crate::score::ReputationEngine;
use crate::time_decay::{DecayConfig, TimeDecayService};

/// Service for batch operations
pub struct BatchService {
    pool: PgPool,
    engine: ReputationEngine,
    decay_service: TimeDecayService,
}

impl BatchService {
    pub fn new(pool: PgPool) -> Self {
        let engine = ReputationEngine::new(pool.clone());
        let decay_service = TimeDecayService::new(pool.clone(), DecayConfig::default());
        Self {
            pool,
            engine,
            decay_service,
        }
    }

    /// Recalculate scores for multiple users
    pub async fn recalculate_scores(
        &self,
        user_ids: Vec<Uuid>,
    ) -> Result<BatchScoreResult, ReputationError> {
        let mut success_count = 0;
        let mut failed_users = Vec::new();

        for user_id in user_ids {
            match self.engine.recalculate_score(user_id).await {
                Ok(_) => success_count += 1,
                Err(e) => failed_users.push(FailedOperation {
                    user_id,
                    reason: e.to_string(),
                }),
            }
        }

        Ok(BatchScoreResult {
            total_processed: success_count + failed_users.len() as i32,
            successful: success_count,
            failed: failed_users.len() as i32,
            failed_operations: failed_users,
        })
    }

    /// Apply time decay to all eligible users
    pub async fn apply_time_decay_batch(&self) -> Result<BatchDecayResult, ReputationError> {
        // Get users who haven't had activity recently
        let query = r#"
            SELECT DISTINCT u.user_id
            FROM users u
            LEFT JOIN reputation_events re ON u.user_id = re.user_id
            WHERE u.reputation_score IS NOT NULL
            AND u.reputation_score > 200  -- Only decay above floor
            GROUP BY u.user_id
            HAVING MAX(re.created_at) < NOW() - INTERVAL '90 days'
            OR MAX(re.created_at) IS NULL
        "#;

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut decayed_count = 0;
        let mut skipped_count = 0;

        for row in rows {
            let user_id: Uuid = row.try_get("user_id").map_err(ReputationError::Database)?;

            match self.decay_service.apply_decay(user_id).await {
                Ok(_) => decayed_count += 1,
                Err(_) => skipped_count += 1,
            }
        }

        Ok(BatchDecayResult {
            users_decayed: decayed_count,
            users_skipped: skipped_count,
            decay_applied_at: Utc::now(),
        })
    }

    /// Expire overdue commitments in batch
    pub async fn expire_overdue_commitments(&self) -> Result<BatchExpireResult, ReputationError> {
        let query = r#"
            UPDATE output_commitments
            SET status = 'Expired'
            WHERE deadline < NOW()
            AND status = 'Pending'
            RETURNING commitment_id, user_id
        "#;

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let expired_count = rows.len();
        let mut affected_users = Vec::new();

        for row in rows {
            let user_id: Uuid = row.try_get("user_id").map_err(ReputationError::Database)?;
            if !affected_users.contains(&user_id) {
                affected_users.push(user_id);
            }
        }

        Ok(BatchExpireResult {
            expired_count: expired_count as i32,
            affected_users,
        })
    }

    /// Update multiple user scores by delta
    pub async fn update_scores_batch(
        &self,
        updates: Vec<ScoreUpdate>,
    ) -> Result<BatchScoreResult, ReputationError> {
        let mut success_count = 0;
        let mut failed_users = Vec::new();

        for update in updates {
            match self.engine.update_score(update.user_id, update.delta).await {
                Ok(_) => success_count += 1,
                Err(e) => failed_users.push(FailedOperation {
                    user_id: update.user_id,
                    reason: e.to_string(),
                }),
            }
        }

        Ok(BatchScoreResult {
            total_processed: success_count + failed_users.len() as i32,
            successful: success_count,
            failed: failed_users.len() as i32,
            failed_operations: failed_users,
        })
    }

    /// Get all users with low scores (below threshold)
    pub async fn get_low_score_users(
        &self,
        threshold: Decimal,
    ) -> Result<Vec<LowScoreUser>, ReputationError> {
        let query = r#"
            SELECT user_id, reputation_score
            FROM users
            WHERE reputation_score < $1
            AND reputation_score IS NOT NULL
            ORDER BY reputation_score ASC
        "#;

        let rows = sqlx::query(query)
            .bind(threshold)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut users = Vec::new();
        for row in rows {
            users.push(LowScoreUser {
                user_id: row.try_get("user_id").map_err(ReputationError::Database)?,
                score: row
                    .try_get("reputation_score")
                    .map_err(ReputationError::Database)?,
            });
        }

        Ok(users)
    }

    /// Batch verify commitments
    pub async fn verify_commitments_batch(
        &self,
        commitment_ids: Vec<Uuid>,
        _admin_id: Uuid,
    ) -> Result<BatchVerifyResult, ReputationError> {
        let mut success_count = 0;
        let mut failed = Vec::new();

        for commitment_id in commitment_ids {
            // Update commitment status
            let update_query = r#"
                UPDATE output_commitments
                SET status = 'Verified'
                WHERE commitment_id = $1
                AND status = 'Completed'
                RETURNING user_id
            "#;

            match sqlx::query(update_query)
                .bind(commitment_id)
                .fetch_optional(&self.pool)
                .await
            {
                Ok(Some(row)) => {
                    let user_id: Uuid =
                        row.try_get("user_id").map_err(ReputationError::Database)?;

                    // Update score
                    if let Err(e) = self.engine.update_score(user_id, Decimal::new(25, 0)).await {
                        failed.push(FailedVerification {
                            commitment_id,
                            reason: e.to_string(),
                        });
                    } else {
                        success_count += 1;
                    }
                }
                Ok(None) => {
                    failed.push(FailedVerification {
                        commitment_id,
                        reason: "Commitment not found or not in Completed status".to_string(),
                    });
                }
                Err(e) => {
                    failed.push(FailedVerification {
                        commitment_id,
                        reason: e.to_string(),
                    });
                }
            }
        }

        Ok(BatchVerifyResult {
            verified_count: success_count,
            failed_count: failed.len() as i32,
            failed_verifications: failed,
        })
    }

    /// Get all pending commitments for review
    pub async fn get_pending_commitments_batch(
        &self,
        limit: i32,
    ) -> Result<Vec<PendingCommitmentInfo>, ReputationError> {
        let query = r#"
            SELECT commitment_id, user_id, title, created_at, deadline
            FROM output_commitments
            WHERE status = 'Completed'
            ORDER BY created_at ASC
            LIMIT $1
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut commitments = Vec::new();
        for row in rows {
            commitments.push(PendingCommitmentInfo {
                commitment_id: row
                    .try_get("commitment_id")
                    .map_err(ReputationError::Database)?,
                user_id: row.try_get("user_id").map_err(ReputationError::Database)?,
                title: row.try_get("title").map_err(ReputationError::Database)?,
                created_at: row
                    .try_get("created_at")
                    .map_err(ReputationError::Database)?,
                deadline: row.try_get("deadline").map_err(ReputationError::Database)?,
            });
        }

        Ok(commitments)
    }
}

/// Score update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreUpdate {
    pub user_id: Uuid,
    pub delta: Decimal,
    pub reason: String,
}

/// Result of batch score operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchScoreResult {
    pub total_processed: i32,
    pub successful: i32,
    pub failed: i32,
    pub failed_operations: Vec<FailedOperation>,
}

/// Failed operation detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedOperation {
    pub user_id: Uuid,
    pub reason: String,
}

/// Result of batch decay operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDecayResult {
    pub users_decayed: i32,
    pub users_skipped: i32,
    pub decay_applied_at: chrono::DateTime<Utc>,
}

/// Result of batch expire operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExpireResult {
    pub expired_count: i32,
    pub affected_users: Vec<Uuid>,
}

/// User with low score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowScoreUser {
    pub user_id: Uuid,
    pub score: Decimal,
}

/// Result of batch verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchVerifyResult {
    pub verified_count: i32,
    pub failed_count: i32,
    pub failed_verifications: Vec<FailedVerification>,
}

/// Failed verification detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedVerification {
    pub commitment_id: Uuid,
    pub reason: String,
}

/// Pending commitment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCommitmentInfo {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub created_at: chrono::DateTime<Utc>,
    pub deadline: chrono::DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_score_result_structure() {
        let result = BatchScoreResult {
            total_processed: 100,
            successful: 95,
            failed: 5,
            failed_operations: vec![FailedOperation {
                user_id: Uuid::new_v4(),
                reason: "Test error".to_string(),
            }],
        };

        assert_eq!(result.total_processed, 100);
        assert_eq!(result.successful, 95);
        assert_eq!(result.failed, 5);
    }

    #[test]
    fn test_score_update_structure() {
        let update = ScoreUpdate {
            user_id: Uuid::new_v4(),
            delta: Decimal::new(25, 0),
            reason: "Test update".to_string(),
        };

        assert_eq!(update.delta, Decimal::new(25, 0));
    }

    #[test]
    fn test_batch_decay_result_structure() {
        let result = BatchDecayResult {
            users_decayed: 50,
            users_skipped: 10,
            decay_applied_at: Utc::now(),
        };

        assert_eq!(result.users_decayed, 50);
        assert_eq!(result.users_skipped, 10);
    }

    #[test]
    fn test_batch_expire_result_structure() {
        let result = BatchExpireResult {
            expired_count: 20,
            affected_users: vec![Uuid::new_v4(), Uuid::new_v4()],
        };

        assert_eq!(result.expired_count, 20);
        assert_eq!(result.affected_users.len(), 2);
    }

    #[test]
    fn test_low_score_user_structure() {
        let user = LowScoreUser {
            user_id: Uuid::new_v4(),
            score: Decimal::new(150, 0),
        };

        assert_eq!(user.score, Decimal::new(150, 0));
    }

    #[test]
    fn test_batch_verify_result_structure() {
        let result = BatchVerifyResult {
            verified_count: 45,
            failed_count: 5,
            failed_verifications: vec![FailedVerification {
                commitment_id: Uuid::new_v4(),
                reason: "Not found".to_string(),
            }],
        };

        assert_eq!(result.verified_count, 45);
        assert_eq!(result.failed_count, 5);
    }
}
