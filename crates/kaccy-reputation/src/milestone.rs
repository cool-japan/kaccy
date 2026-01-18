//! Milestone commitments
//!
//! Allows breaking large commitments into milestones
//! with partial credit for completed milestones.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::commitment::CommitmentStatus;
use crate::error::Result;

/// A milestone within a commitment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub milestone_id: Uuid,
    pub commitment_id: Uuid,
    pub sequence_number: i32,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    /// Weight of this milestone (percentage of total commitment)
    pub weight_percent: Decimal,
    pub status: MilestoneStatus,
    pub evidence_url: Option<String>,
    pub evidence_description: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum MilestoneStatus {
    #[default]
    Pending,
    Completed,
    Verified,
    Failed,
    Skipped,
}

impl std::fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MilestoneStatus::Pending => write!(f, "pending"),
            MilestoneStatus::Completed => write!(f, "completed"),
            MilestoneStatus::Verified => write!(f, "verified"),
            MilestoneStatus::Failed => write!(f, "failed"),
            MilestoneStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for MilestoneStatus {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(MilestoneStatus::Pending),
            "completed" => Ok(MilestoneStatus::Completed),
            "verified" => Ok(MilestoneStatus::Verified),
            "failed" => Ok(MilestoneStatus::Failed),
            "skipped" => Ok(MilestoneStatus::Skipped),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid milestone status: {}. Valid values: pending, completed, verified, failed, skipped",
                s
            ))),
        }
    }
}

/// Commitment with milestones
#[derive(Debug, Clone, Serialize)]
pub struct MilestoneCommitment {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub overall_deadline: DateTime<Utc>,
    pub status: CommitmentStatus,
    pub milestones: Vec<Milestone>,
    pub total_weight: Decimal,
    pub completed_weight: Decimal,
    pub completion_percentage: Decimal,
    pub created_at: DateTime<Utc>,
}

/// Request to create a milestone commitment
#[derive(Debug, Deserialize)]
pub struct CreateMilestoneCommitmentRequest {
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub overall_deadline: DateTime<Utc>,
    pub milestones: Vec<MilestoneRequest>,
}

#[derive(Debug, Deserialize)]
pub struct MilestoneRequest {
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    /// Weight as percentage (should sum to 100)
    pub weight_percent: Decimal,
}

impl CreateMilestoneCommitmentRequest {
    pub fn validate(&self) -> Result<()> {
        if self.title.is_empty() {
            return Err(crate::error::ReputationError::Validation(
                "Title is required".to_string(),
            ));
        }
        if self.title.len() > 200 {
            return Err(crate::error::ReputationError::Validation(
                "Title must be at most 200 characters".to_string(),
            ));
        }
        if self.milestones.is_empty() {
            return Err(crate::error::ReputationError::Validation(
                "At least one milestone is required".to_string(),
            ));
        }
        if self.milestones.len() > 20 {
            return Err(crate::error::ReputationError::Validation(
                "Maximum 20 milestones allowed".to_string(),
            ));
        }

        // Validate weights sum to 100
        let total_weight: Decimal = self.milestones.iter().map(|m| m.weight_percent).sum();
        if (total_weight - dec!(100)).abs() > dec!(0.01) {
            return Err(crate::error::ReputationError::Validation(
                "Milestone weights must sum to 100%".to_string(),
            ));
        }

        // Validate individual milestones
        let mut prev_deadline = Utc::now();
        for (i, milestone) in self.milestones.iter().enumerate() {
            if milestone.title.is_empty() {
                return Err(crate::error::ReputationError::Validation(format!(
                    "Milestone {} title is required",
                    i + 1
                )));
            }
            if milestone.deadline < prev_deadline {
                return Err(crate::error::ReputationError::Validation(format!(
                    "Milestone {} deadline must be after previous milestone",
                    i + 1
                )));
            }
            if milestone.weight_percent <= dec!(0) {
                return Err(crate::error::ReputationError::Validation(format!(
                    "Milestone {} weight must be positive",
                    i + 1
                )));
            }
            prev_deadline = milestone.deadline;
        }

        // Last milestone deadline should not exceed overall deadline
        if let Some(last) = self.milestones.last() {
            if last.deadline > self.overall_deadline {
                return Err(crate::error::ReputationError::Validation(
                    "Last milestone deadline cannot exceed overall deadline".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Service for managing milestone commitments
pub struct MilestoneService {
    pool: PgPool,
}

impl MilestoneService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new milestone commitment
    pub async fn create_commitment(
        &self,
        user_id: Uuid,
        request: &CreateMilestoneCommitmentRequest,
    ) -> Result<MilestoneCommitment> {
        request.validate()?;

        let mut tx = self.pool.begin().await?;

        // Create the main commitment
        let commitment_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO output_commitments (commitment_id, user_id, token_id, title, description, deadline, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7)
            "#,
        )
        .bind(commitment_id)
        .bind(user_id)
        .bind(request.token_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(request.overall_deadline)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Create milestones
        let mut milestones = Vec::with_capacity(request.milestones.len());
        for (i, m) in request.milestones.iter().enumerate() {
            let milestone_id = Uuid::new_v4();

            sqlx::query(
                r#"
                INSERT INTO commitment_milestones (milestone_id, commitment_id, sequence_number, title, description, deadline, weight_percent, status, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8)
                "#,
            )
            .bind(milestone_id)
            .bind(commitment_id)
            .bind(i as i32 + 1)
            .bind(&m.title)
            .bind(&m.description)
            .bind(m.deadline)
            .bind(m.weight_percent)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            milestones.push(Milestone {
                milestone_id,
                commitment_id,
                sequence_number: i as i32 + 1,
                title: m.title.clone(),
                description: m.description.clone(),
                deadline: m.deadline,
                weight_percent: m.weight_percent,
                status: MilestoneStatus::Pending,
                evidence_url: None,
                evidence_description: None,
                verified_at: None,
                verified_by: None,
                created_at: now,
            });
        }

        tx.commit().await?;

        Ok(MilestoneCommitment {
            commitment_id,
            user_id,
            token_id: request.token_id,
            title: request.title.clone(),
            description: request.description.clone(),
            overall_deadline: request.overall_deadline,
            status: CommitmentStatus::Pending,
            milestones,
            total_weight: dec!(100),
            completed_weight: dec!(0),
            completion_percentage: dec!(0),
            created_at: now,
        })
    }

    /// Submit evidence for a milestone
    pub async fn submit_milestone_evidence(
        &self,
        milestone_id: Uuid,
        user_id: Uuid,
        evidence_url: &str,
        evidence_description: Option<&str>,
    ) -> Result<Milestone> {
        // Verify ownership
        let row = sqlx::query(
            r#"
            SELECT m.*, c.user_id as commitment_user_id
            FROM commitment_milestones m
            JOIN output_commitments c ON m.commitment_id = c.commitment_id
            WHERE m.milestone_id = $1
            "#,
        )
        .bind(milestone_id)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| {
            crate::error::ReputationError::Validation("Milestone not found".to_string())
        })?;

        let commitment_user_id: Uuid = row.get("commitment_user_id");
        if commitment_user_id != user_id {
            return Err(crate::error::ReputationError::Validation(
                "You cannot submit evidence for this milestone".to_string(),
            ));
        }

        let status: String = row.get("status");
        if status != "pending" {
            return Err(crate::error::ReputationError::Validation(
                "Milestone is not pending".to_string(),
            ));
        }

        // Update milestone
        sqlx::query(
            r#"
            UPDATE commitment_milestones
            SET evidence_url = $2, evidence_description = $3, status = 'completed'
            WHERE milestone_id = $1
            "#,
        )
        .bind(milestone_id)
        .bind(evidence_url)
        .bind(evidence_description)
        .execute(&self.pool)
        .await?;

        Ok(Milestone {
            milestone_id,
            commitment_id: row.get("commitment_id"),
            sequence_number: row.get("sequence_number"),
            title: row.get("title"),
            description: row.get("description"),
            deadline: row.get("deadline"),
            weight_percent: row.get("weight_percent"),
            status: MilestoneStatus::Completed,
            evidence_url: Some(evidence_url.to_string()),
            evidence_description: evidence_description.map(|s| s.to_string()),
            verified_at: None,
            verified_by: None,
            created_at: row.get("created_at"),
        })
    }

    /// Verify a milestone (admin only)
    pub async fn verify_milestone(
        &self,
        milestone_id: Uuid,
        admin_id: Uuid,
        approved: bool,
    ) -> Result<(Milestone, Decimal)> {
        let mut tx = self.pool.begin().await?;

        // Get milestone and commitment details
        let row = sqlx::query(
            r#"
            SELECT m.*, c.user_id as commitment_user_id, c.commitment_id
            FROM commitment_milestones m
            JOIN output_commitments c ON m.commitment_id = c.commitment_id
            WHERE m.milestone_id = $1 AND m.status = 'completed'
            FOR UPDATE
            "#,
        )
        .bind(milestone_id)
        .fetch_optional(&mut *tx)
        .await?;

        let row = row.ok_or_else(|| {
            crate::error::ReputationError::Validation(
                "Milestone not found or not ready for verification".to_string(),
            )
        })?;

        let commitment_id: Uuid = row.get("commitment_id");
        let commitment_user_id: Uuid = row.get("commitment_user_id");
        let weight_percent: Decimal = row.get("weight_percent");

        let new_status = if approved {
            MilestoneStatus::Verified
        } else {
            MilestoneStatus::Failed
        };

        // Update milestone
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE commitment_milestones
            SET status = $2, verified_at = $3, verified_by = $4
            WHERE milestone_id = $1
            "#,
        )
        .bind(milestone_id)
        .bind(new_status.to_string())
        .bind(now)
        .bind(admin_id)
        .execute(&mut *tx)
        .await?;

        // Calculate reputation delta based on milestone weight
        // Base: +10 for full commitment, scale by weight
        let base_delta = if approved {
            dec!(10) * (weight_percent / dec!(100))
        } else {
            dec!(-20) * (weight_percent / dec!(100))
        };

        // Update user reputation
        sqlx::query(
            r#"
            UPDATE users
            SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
            WHERE user_id = $1
            "#,
        )
        .bind(commitment_user_id)
        .bind(base_delta)
        .execute(&mut *tx)
        .await?;

        // Record reputation event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(commitment_user_id)
        .bind(if approved {
            "milestone_verified"
        } else {
            "milestone_failed"
        })
        .bind(base_delta)
        .bind(format!(
            "Milestone: {} ({}%)",
            row.get::<String, _>("title"),
            weight_percent
        ))
        .execute(&mut *tx)
        .await?;

        // Check if all milestones are complete to update parent commitment
        let pending_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM commitment_milestones
            WHERE commitment_id = $1 AND status NOT IN ('verified', 'failed', 'skipped')
            "#,
        )
        .bind(commitment_id)
        .fetch_one(&mut *tx)
        .await?;

        if pending_count.0 == 0 {
            // All milestones done, update parent commitment
            let verified_weight: (Decimal,) = sqlx::query_as(
                r#"
                SELECT COALESCE(SUM(weight_percent), 0)
                FROM commitment_milestones
                WHERE commitment_id = $1 AND status = 'verified'
                "#,
            )
            .bind(commitment_id)
            .fetch_one(&mut *tx)
            .await?;

            let final_status = if verified_weight.0 >= dec!(50) {
                "verified"
            } else {
                "failed"
            };

            sqlx::query(
                r#"
                UPDATE output_commitments
                SET status = $2, verified_at = $3, verified_by = $4
                WHERE commitment_id = $1
                "#,
            )
            .bind(commitment_id)
            .bind(final_status)
            .bind(now)
            .bind(admin_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok((
            Milestone {
                milestone_id,
                commitment_id,
                sequence_number: row.get("sequence_number"),
                title: row.get("title"),
                description: row.get("description"),
                deadline: row.get("deadline"),
                weight_percent,
                status: new_status,
                evidence_url: row.get("evidence_url"),
                evidence_description: row.get("evidence_description"),
                verified_at: Some(now),
                verified_by: Some(admin_id),
                created_at: row.get("created_at"),
            },
            base_delta,
        ))
    }

    /// Get a milestone commitment with all milestones
    pub async fn get_commitment(&self, commitment_id: Uuid) -> Result<Option<MilestoneCommitment>> {
        let commitment_row = sqlx::query(
            r#"
            SELECT * FROM output_commitments WHERE commitment_id = $1
            "#,
        )
        .bind(commitment_id)
        .fetch_optional(&self.pool)
        .await?;

        let commitment_row = match commitment_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let milestone_rows = sqlx::query(
            r#"
            SELECT * FROM commitment_milestones
            WHERE commitment_id = $1
            ORDER BY sequence_number
            "#,
        )
        .bind(commitment_id)
        .fetch_all(&self.pool)
        .await?;

        let milestones: Vec<Milestone> = milestone_rows
            .into_iter()
            .map(|row| {
                let status_str: String = row.get("status");
                let status = match status_str.as_str() {
                    "pending" => MilestoneStatus::Pending,
                    "completed" => MilestoneStatus::Completed,
                    "verified" => MilestoneStatus::Verified,
                    "failed" => MilestoneStatus::Failed,
                    "skipped" => MilestoneStatus::Skipped,
                    _ => MilestoneStatus::Pending,
                };

                Milestone {
                    milestone_id: row.get("milestone_id"),
                    commitment_id: row.get("commitment_id"),
                    sequence_number: row.get("sequence_number"),
                    title: row.get("title"),
                    description: row.get("description"),
                    deadline: row.get("deadline"),
                    weight_percent: row.get("weight_percent"),
                    status,
                    evidence_url: row.get("evidence_url"),
                    evidence_description: row.get("evidence_description"),
                    verified_at: row.get("verified_at"),
                    verified_by: row.get("verified_by"),
                    created_at: row.get("created_at"),
                }
            })
            .collect();

        let completed_weight: Decimal = milestones
            .iter()
            .filter(|m| m.status == MilestoneStatus::Verified)
            .map(|m| m.weight_percent)
            .sum();

        let status_str: String = commitment_row.get("status");
        let status = match status_str.as_str() {
            "pending" => CommitmentStatus::Pending,
            "completed" => CommitmentStatus::Completed,
            "verified" => CommitmentStatus::Verified,
            "failed" => CommitmentStatus::Failed,
            "expired" => CommitmentStatus::Expired,
            _ => CommitmentStatus::Pending,
        };

        Ok(Some(MilestoneCommitment {
            commitment_id,
            user_id: commitment_row.get("user_id"),
            token_id: commitment_row.get("token_id"),
            title: commitment_row.get("title"),
            description: commitment_row.get("description"),
            overall_deadline: commitment_row.get("deadline"),
            status,
            milestones,
            total_weight: dec!(100),
            completed_weight,
            completion_percentage: completed_weight,
            created_at: commitment_row.get("created_at"),
        }))
    }

    /// Get pending milestones for verification
    pub async fn get_pending_milestones(&self, limit: u32) -> Result<Vec<Milestone>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM commitment_milestones
            WHERE status = 'completed'
            ORDER BY deadline ASC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| Milestone {
                milestone_id: row.get("milestone_id"),
                commitment_id: row.get("commitment_id"),
                sequence_number: row.get("sequence_number"),
                title: row.get("title"),
                description: row.get("description"),
                deadline: row.get("deadline"),
                weight_percent: row.get("weight_percent"),
                status: MilestoneStatus::Completed,
                evidence_url: row.get("evidence_url"),
                evidence_description: row.get("evidence_description"),
                verified_at: row.get("verified_at"),
                verified_by: row.get("verified_by"),
                created_at: row.get("created_at"),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_milestone_status_from_str_valid() {
        assert_eq!(
            "pending".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Pending
        );
        assert_eq!(
            "completed".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Completed
        );
        assert_eq!(
            "verified".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Verified
        );
        assert_eq!(
            "failed".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Failed
        );
        assert_eq!(
            "skipped".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Skipped
        );
    }

    #[test]
    fn test_milestone_status_from_str_case_insensitive() {
        assert_eq!(
            "PENDING".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Pending
        );
        assert_eq!(
            "Completed".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Completed
        );
        assert_eq!(
            "VERIFIED".parse::<MilestoneStatus>().unwrap(),
            MilestoneStatus::Verified
        );
    }

    #[test]
    fn test_milestone_status_from_str_invalid() {
        assert!("invalid".parse::<MilestoneStatus>().is_err());
        assert!("unknown".parse::<MilestoneStatus>().is_err());
        assert!("".parse::<MilestoneStatus>().is_err());
    }

    #[test]
    fn test_milestone_status_default() {
        assert_eq!(MilestoneStatus::default(), MilestoneStatus::Pending);
    }

    #[test]
    fn test_milestone_status_display() {
        assert_eq!(MilestoneStatus::Pending.to_string(), "pending");
        assert_eq!(MilestoneStatus::Completed.to_string(), "completed");
        assert_eq!(MilestoneStatus::Verified.to_string(), "verified");
        assert_eq!(MilestoneStatus::Failed.to_string(), "failed");
        assert_eq!(MilestoneStatus::Skipped.to_string(), "skipped");
    }
}
