//! Commitment tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::fmt;
use uuid::Uuid;

use crate::error::{ReputationError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OutputCommitment {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
    pub status: CommitmentStatus,
    pub evidence_url: Option<String>,
    pub evidence_description: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl fmt::Display for OutputCommitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Commitment({}, {}, status={:?})",
            self.commitment_id, self.title, self.status
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum CommitmentStatus {
    #[default]
    Pending,
    Completed,
    Verified,
    Failed,
    Expired,
}

impl CommitmentStatus {
    /// Check if this is a terminal status (cannot be changed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            CommitmentStatus::Verified | CommitmentStatus::Failed | CommitmentStatus::Expired
        )
    }

    /// Check if this commitment is still active (can be updated)
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            CommitmentStatus::Pending | CommitmentStatus::Completed
        )
    }

    /// Check if this commitment has a positive outcome
    pub fn is_successful(&self) -> bool {
        matches!(self, CommitmentStatus::Verified)
    }

    /// Check if this commitment has a negative outcome
    pub fn is_failed(&self) -> bool {
        matches!(self, CommitmentStatus::Failed | CommitmentStatus::Expired)
    }

    /// Check if transition to another status is allowed
    pub fn can_transition_to(&self, target: CommitmentStatus) -> bool {
        match self {
            CommitmentStatus::Pending => matches!(
                target,
                CommitmentStatus::Completed | CommitmentStatus::Expired | CommitmentStatus::Failed
            ),
            CommitmentStatus::Completed => matches!(
                target,
                CommitmentStatus::Verified | CommitmentStatus::Failed
            ),
            // Terminal states cannot transition
            CommitmentStatus::Verified | CommitmentStatus::Failed | CommitmentStatus::Expired => {
                false
            }
        }
    }

    /// Get all possible next states from current status
    pub fn possible_next_states(&self) -> Vec<CommitmentStatus> {
        match self {
            CommitmentStatus::Pending => vec![
                CommitmentStatus::Completed,
                CommitmentStatus::Expired,
                CommitmentStatus::Failed,
            ],
            CommitmentStatus::Completed => {
                vec![CommitmentStatus::Verified, CommitmentStatus::Failed]
            }
            // Terminal states have no next states
            CommitmentStatus::Verified | CommitmentStatus::Failed | CommitmentStatus::Expired => {
                vec![]
            }
        }
    }

    /// Get all status variants
    pub fn all_variants() -> [Self; 5] {
        [
            Self::Pending,
            Self::Completed,
            Self::Verified,
            Self::Failed,
            Self::Expired,
        ]
    }

    /// Get status as string (lowercase)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

impl fmt::Display for CommitmentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitmentStatus::Pending => write!(f, "pending"),
            CommitmentStatus::Completed => write!(f, "completed"),
            CommitmentStatus::Verified => write!(f, "verified"),
            CommitmentStatus::Failed => write!(f, "failed"),
            CommitmentStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for CommitmentStatus {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(CommitmentStatus::Pending),
            "completed" => Ok(CommitmentStatus::Completed),
            "verified" => Ok(CommitmentStatus::Verified),
            "failed" => Ok(CommitmentStatus::Failed),
            "expired" => Ok(CommitmentStatus::Expired),
            _ => Err(ReputationError::Validation(format!(
                "Invalid commitment status: {}. Valid values: pending, completed, verified, failed, expired",
                s
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateCommitmentRequest {
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub deadline: DateTime<Utc>,
}

impl CreateCommitmentRequest {
    pub fn validate(&self) -> Result<()> {
        if self.title.is_empty() {
            return Err(ReputationError::Validation("Title is required".to_string()));
        }
        if self.title.len() > 200 {
            return Err(ReputationError::Validation(
                "Title must be at most 200 characters".to_string(),
            ));
        }
        if let Some(ref desc) = self.description {
            if desc.len() > 2000 {
                return Err(ReputationError::Validation(
                    "Description must be at most 2000 characters".to_string(),
                ));
            }
        }
        if self.deadline < Utc::now() {
            return Err(ReputationError::Validation(
                "Deadline must be in the future".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a new builder for CreateCommitmentRequest
    pub fn builder(token_id: Uuid, title: impl Into<String>) -> CreateCommitmentRequestBuilder {
        CreateCommitmentRequestBuilder::new(token_id, title)
    }
}

/// Builder for CreateCommitmentRequest
pub struct CreateCommitmentRequestBuilder {
    token_id: Uuid,
    title: String,
    description: Option<String>,
    deadline: Option<DateTime<Utc>>,
}

impl CreateCommitmentRequestBuilder {
    /// Create a new builder with required fields
    pub fn new(token_id: Uuid, title: impl Into<String>) -> Self {
        Self {
            token_id,
            title: title.into(),
            description: None,
            deadline: None,
        }
    }

    /// Set the description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the deadline
    pub fn deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set deadline as days from now
    pub fn deadline_days_from_now(mut self, days: i64) -> Self {
        self.deadline = Some(Utc::now() + chrono::Duration::days(days));
        self
    }

    /// Build the request
    pub fn build(self) -> Result<CreateCommitmentRequest> {
        let deadline = self
            .deadline
            .ok_or_else(|| ReputationError::Validation("Deadline is required".to_string()))?;

        let request = CreateCommitmentRequest {
            token_id: self.token_id,
            title: self.title,
            description: self.description,
            deadline,
        };

        request.validate()?;
        Ok(request)
    }
}

#[derive(Debug, Deserialize)]
pub struct SubmitEvidenceRequest {
    pub evidence_url: String,
    pub evidence_description: Option<String>,
}

impl SubmitEvidenceRequest {
    pub fn validate(&self) -> Result<()> {
        if self.evidence_url.is_empty() {
            return Err(ReputationError::Validation(
                "Evidence URL is required".to_string(),
            ));
        }
        if !self.evidence_url.starts_with("https://") {
            return Err(ReputationError::Validation(
                "Evidence URL must be HTTPS".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyCommitmentRequest {
    pub fulfilled: bool,
    pub quality_score: Option<Decimal>,
    pub notes: Option<String>,
}

/// Service for managing commitments
pub struct CommitmentService {
    pool: PgPool,
}

impl CommitmentService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new commitment
    pub async fn create(
        &self,
        user_id: Uuid,
        request: &CreateCommitmentRequest,
    ) -> Result<OutputCommitment> {
        request.validate()?;

        let commitment = sqlx::query_as::<_, OutputCommitment>(
            r#"
            INSERT INTO output_commitments (user_id, token_id, title, description, deadline)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(request.token_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(request.deadline)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(commitment_id = %commitment.commitment_id, "Commitment created");
        Ok(commitment)
    }

    /// Submit evidence for a commitment
    pub async fn submit_evidence(
        &self,
        commitment_id: Uuid,
        user_id: Uuid,
        request: &SubmitEvidenceRequest,
    ) -> Result<OutputCommitment> {
        request.validate()?;

        let commitment = sqlx::query_as::<_, OutputCommitment>(
            r#"
            UPDATE output_commitments
            SET evidence_url = $3, evidence_description = $4, status = 'completed'
            WHERE commitment_id = $1 AND user_id = $2 AND status = 'pending'
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .bind(user_id)
        .bind(&request.evidence_url)
        .bind(&request.evidence_description)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ReputationError::CommitmentNotFound(commitment_id.to_string()))?;

        tracing::info!(commitment_id = %commitment_id, "Evidence submitted");
        Ok(commitment)
    }

    /// Verify a commitment (admin only)
    pub async fn verify(
        &self,
        commitment_id: Uuid,
        admin_id: Uuid,
        request: &VerifyCommitmentRequest,
    ) -> Result<(OutputCommitment, Decimal)> {
        let mut tx = self.pool.begin().await?;

        // Get the commitment
        let commitment: OutputCommitment = sqlx::query_as(
            r#"
            SELECT * FROM output_commitments
            WHERE commitment_id = $1 AND status = 'completed'
            FOR UPDATE
            "#,
        )
        .bind(commitment_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ReputationError::CommitmentNotFound(commitment_id.to_string()))?;

        let new_status = if request.fulfilled {
            CommitmentStatus::Verified
        } else {
            CommitmentStatus::Failed
        };

        // Update commitment status
        let updated: OutputCommitment = sqlx::query_as(
            r#"
            UPDATE output_commitments
            SET status = $2, verified_at = NOW(), verified_by = $3
            WHERE commitment_id = $1
            RETURNING *
            "#,
        )
        .bind(commitment_id)
        .bind(new_status.to_string())
        .bind(admin_id)
        .fetch_one(&mut *tx)
        .await?;

        // Calculate reputation delta
        let base_delta = if request.fulfilled {
            dec!(10) // +10 for fulfillment
        } else {
            dec!(-20) // -20 for failure (2x penalty)
        };

        // Add quality bonus if fulfilled
        let quality_bonus = if request.fulfilled {
            request
                .quality_score
                .map(|q| (q - dec!(50)) / dec!(10)) // Quality 50 = 0 bonus, 100 = +5
                .unwrap_or(dec!(0))
        } else {
            dec!(0)
        };

        let total_delta = base_delta + quality_bonus;

        // Update user reputation
        sqlx::query(
            r#"
            UPDATE users
            SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
            WHERE user_id = $1
            "#,
        )
        .bind(commitment.user_id)
        .bind(total_delta)
        .execute(&mut *tx)
        .await?;

        // Record reputation event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(commitment.user_id)
        .bind(if request.fulfilled {
            "commitment_fulfilled"
        } else {
            "commitment_failed"
        })
        .bind(total_delta)
        .bind(&request.notes)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            commitment_id = %commitment_id,
            fulfilled = request.fulfilled,
            delta = %total_delta,
            "Commitment verified"
        );

        Ok((updated, total_delta))
    }

    /// Get user's commitments
    pub async fn get_user_commitments(&self, user_id: Uuid) -> Result<Vec<OutputCommitment>> {
        let commitments = sqlx::query_as::<_, OutputCommitment>(
            r#"
            SELECT * FROM output_commitments
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get token's commitments
    pub async fn get_token_commitments(&self, token_id: Uuid) -> Result<Vec<OutputCommitment>> {
        let commitments = sqlx::query_as::<_, OutputCommitment>(
            r#"
            SELECT * FROM output_commitments
            WHERE token_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Get pending commitments for admin verification
    pub async fn get_pending_verification(&self, limit: i64) -> Result<Vec<OutputCommitment>> {
        let commitments = sqlx::query_as::<_, OutputCommitment>(
            r#"
            SELECT * FROM output_commitments
            WHERE status = 'completed'
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(commitments)
    }

    /// Expire overdue commitments
    pub async fn expire_overdue(&self) -> Result<u64> {
        let mut tx = self.pool.begin().await?;

        // Get overdue commitments
        let overdue: Vec<OutputCommitment> = sqlx::query_as(
            r#"
            SELECT * FROM output_commitments
            WHERE status = 'pending' AND deadline < NOW()
            FOR UPDATE
            "#,
        )
        .fetch_all(&mut *tx)
        .await?;

        let count = overdue.len() as u64;

        for commitment in &overdue {
            // Mark as expired
            sqlx::query(
                r#"
                UPDATE output_commitments
                SET status = 'expired'
                WHERE commitment_id = $1
                "#,
            )
            .bind(commitment.commitment_id)
            .execute(&mut *tx)
            .await?;

            // Penalize reputation
            let penalty = dec!(-15);
            sqlx::query(
                r#"
                UPDATE users
                SET reputation_score = GREATEST(0, reputation_score + $2)
                WHERE user_id = $1
                "#,
            )
            .bind(commitment.user_id)
            .bind(penalty)
            .execute(&mut *tx)
            .await?;

            // Record event
            sqlx::query(
                r#"
                INSERT INTO reputation_events (user_id, event_type, delta, reason)
                VALUES ($1, 'commitment_expired', $2, $3)
                "#,
            )
            .bind(commitment.user_id)
            .bind(penalty)
            .bind(format!("Commitment expired: {}", commitment.title))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        if count > 0 {
            tracing::info!(count = count, "Expired overdue commitments");
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_status_is_terminal() {
        assert!(CommitmentStatus::Verified.is_terminal());
        assert!(CommitmentStatus::Failed.is_terminal());
        assert!(CommitmentStatus::Expired.is_terminal());
        assert!(!CommitmentStatus::Pending.is_terminal());
        assert!(!CommitmentStatus::Completed.is_terminal());
    }

    #[test]
    fn test_commitment_status_is_active() {
        assert!(CommitmentStatus::Pending.is_active());
        assert!(CommitmentStatus::Completed.is_active());
        assert!(!CommitmentStatus::Verified.is_active());
        assert!(!CommitmentStatus::Failed.is_active());
        assert!(!CommitmentStatus::Expired.is_active());
    }

    #[test]
    fn test_commitment_status_is_successful() {
        assert!(CommitmentStatus::Verified.is_successful());
        assert!(!CommitmentStatus::Pending.is_successful());
        assert!(!CommitmentStatus::Completed.is_successful());
        assert!(!CommitmentStatus::Failed.is_successful());
        assert!(!CommitmentStatus::Expired.is_successful());
    }

    #[test]
    fn test_commitment_status_is_failed() {
        assert!(CommitmentStatus::Failed.is_failed());
        assert!(CommitmentStatus::Expired.is_failed());
        assert!(!CommitmentStatus::Pending.is_failed());
        assert!(!CommitmentStatus::Completed.is_failed());
        assert!(!CommitmentStatus::Verified.is_failed());
    }

    #[test]
    fn test_commitment_status_can_transition() {
        // Pending can transition to Completed, Expired, or Failed
        assert!(CommitmentStatus::Pending.can_transition_to(CommitmentStatus::Completed));
        assert!(CommitmentStatus::Pending.can_transition_to(CommitmentStatus::Expired));
        assert!(CommitmentStatus::Pending.can_transition_to(CommitmentStatus::Failed));
        assert!(!CommitmentStatus::Pending.can_transition_to(CommitmentStatus::Verified));
        assert!(!CommitmentStatus::Pending.can_transition_to(CommitmentStatus::Pending));

        // Completed can transition to Verified or Failed
        assert!(CommitmentStatus::Completed.can_transition_to(CommitmentStatus::Verified));
        assert!(CommitmentStatus::Completed.can_transition_to(CommitmentStatus::Failed));
        assert!(!CommitmentStatus::Completed.can_transition_to(CommitmentStatus::Pending));
        assert!(!CommitmentStatus::Completed.can_transition_to(CommitmentStatus::Expired));
        assert!(!CommitmentStatus::Completed.can_transition_to(CommitmentStatus::Completed));

        // Terminal states cannot transition
        assert!(!CommitmentStatus::Verified.can_transition_to(CommitmentStatus::Failed));
        assert!(!CommitmentStatus::Failed.can_transition_to(CommitmentStatus::Verified));
        assert!(!CommitmentStatus::Expired.can_transition_to(CommitmentStatus::Pending));
    }

    #[test]
    fn test_commitment_status_possible_next_states() {
        let pending_next = CommitmentStatus::Pending.possible_next_states();
        assert_eq!(pending_next.len(), 3);
        assert!(pending_next.contains(&CommitmentStatus::Completed));
        assert!(pending_next.contains(&CommitmentStatus::Expired));
        assert!(pending_next.contains(&CommitmentStatus::Failed));

        let completed_next = CommitmentStatus::Completed.possible_next_states();
        assert_eq!(completed_next.len(), 2);
        assert!(completed_next.contains(&CommitmentStatus::Verified));
        assert!(completed_next.contains(&CommitmentStatus::Failed));

        // Terminal states have no next states
        assert_eq!(CommitmentStatus::Verified.possible_next_states().len(), 0);
        assert_eq!(CommitmentStatus::Failed.possible_next_states().len(), 0);
        assert_eq!(CommitmentStatus::Expired.possible_next_states().len(), 0);
    }

    #[test]
    fn test_commitment_status_display() {
        assert_eq!(CommitmentStatus::Pending.to_string(), "pending");
        assert_eq!(CommitmentStatus::Completed.to_string(), "completed");
        assert_eq!(CommitmentStatus::Verified.to_string(), "verified");
        assert_eq!(CommitmentStatus::Failed.to_string(), "failed");
        assert_eq!(CommitmentStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn test_commitment_status_default() {
        assert_eq!(CommitmentStatus::default(), CommitmentStatus::Pending);
    }

    #[test]
    fn test_commitment_status_from_str_valid() {
        assert_eq!(
            "pending".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Pending
        );
        assert_eq!(
            "completed".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Completed
        );
        assert_eq!(
            "verified".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Verified
        );
        assert_eq!(
            "failed".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Failed
        );
        assert_eq!(
            "expired".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Expired
        );
    }

    #[test]
    fn test_commitment_status_from_str_case_insensitive() {
        assert_eq!(
            "PENDING".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Pending
        );
        assert_eq!(
            "Completed".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Completed
        );
        assert_eq!(
            "VERIFIED".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Verified
        );
        assert_eq!(
            "Failed".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Failed
        );
        assert_eq!(
            "EXPIRED".parse::<CommitmentStatus>().unwrap(),
            CommitmentStatus::Expired
        );
    }

    #[test]
    fn test_commitment_status_from_str_invalid() {
        assert!("invalid".parse::<CommitmentStatus>().is_err());
        assert!("unknown".parse::<CommitmentStatus>().is_err());
        assert!("".parse::<CommitmentStatus>().is_err());
    }

    // Phase 18 tests - New functionality
    #[test]
    fn test_commitment_status_all_variants() {
        let variants = CommitmentStatus::all_variants();
        assert_eq!(variants.len(), 5);
        assert_eq!(variants[0], CommitmentStatus::Pending);
        assert_eq!(variants[1], CommitmentStatus::Completed);
        assert_eq!(variants[2], CommitmentStatus::Verified);
        assert_eq!(variants[3], CommitmentStatus::Failed);
        assert_eq!(variants[4], CommitmentStatus::Expired);
    }

    #[test]
    fn test_commitment_status_as_str() {
        assert_eq!(CommitmentStatus::Pending.as_str(), "pending");
        assert_eq!(CommitmentStatus::Completed.as_str(), "completed");
        assert_eq!(CommitmentStatus::Verified.as_str(), "verified");
        assert_eq!(CommitmentStatus::Failed.as_str(), "failed");
        assert_eq!(CommitmentStatus::Expired.as_str(), "expired");
    }

    #[test]
    fn test_create_commitment_request_builder() {
        let token_id = Uuid::new_v4();
        let request = CreateCommitmentRequest::builder(token_id, "Test Commitment")
            .description("Test description")
            .deadline_days_from_now(7)
            .build();

        assert!(request.is_ok());
        let req = request.unwrap();
        assert_eq!(req.token_id, token_id);
        assert_eq!(req.title, "Test Commitment");
        assert_eq!(req.description, Some("Test description".to_string()));
    }

    #[test]
    fn test_create_commitment_request_builder_no_deadline() {
        let token_id = Uuid::new_v4();
        let request = CreateCommitmentRequest::builder(token_id, "Test")
            .description("Desc")
            .build();

        assert!(request.is_err());
    }

    #[test]
    fn test_create_commitment_request_builder_validation() {
        let token_id = Uuid::new_v4();
        // Title too long
        let long_title = "a".repeat(201);
        let request = CreateCommitmentRequest::builder(token_id, long_title)
            .deadline_days_from_now(7)
            .build();

        assert!(request.is_err());
    }
}
