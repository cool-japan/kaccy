//! Reputation system models for output commitments and reputation tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Output commitment made by a token issuer
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OutputCommitment {
    pub commitment_id: Uuid,
    /// Token associated with this commitment
    pub token_id: Uuid,
    /// User who made the commitment (token issuer)
    pub user_id: Uuid,
    /// Title/summary of the commitment
    pub title: String,
    /// Detailed description of what's being committed to
    pub description: String,
    /// Deadline for fulfilling this commitment
    pub deadline: DateTime<Utc>,
    /// Status of the commitment
    pub status: CommitmentStatus,
    /// Evidence URL or description (submitted by issuer)
    pub evidence: Option<String>,
    /// Evidence submission timestamp
    pub evidence_submitted_at: Option<DateTime<Utc>>,
    /// Verification result
    pub verification_result: Option<VerificationResult>,
    /// Admin who verified (if verified)
    pub verified_by_user_id: Option<Uuid>,
    /// Verification timestamp
    pub verified_at: Option<DateTime<Utc>>,
    /// Admin notes/comments
    pub verification_notes: Option<String>,
    /// Impact on reputation score (calculated based on importance)
    pub reputation_impact: Decimal,
    /// Category of commitment
    pub category: CommitmentCategory,
    /// Whether this commitment is public or private
    pub is_public: bool,
    /// Milestone number (for multi-milestone commitments)
    pub milestone_number: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Status of a commitment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum CommitmentStatus {
    /// Active commitment, not yet due
    #[default]
    Active,
    /// Awaiting evidence submission
    AwaitingEvidence,
    /// Evidence submitted, pending verification
    PendingVerification,
    /// Verified as fulfilled
    Fulfilled,
    /// Verified as not fulfilled
    Failed,
    /// Cancelled by issuer
    Cancelled,
    /// Expired without evidence
    Expired,
}

impl fmt::Display for CommitmentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitmentStatus::Active => write!(f, "active"),
            CommitmentStatus::AwaitingEvidence => write!(f, "awaiting_evidence"),
            CommitmentStatus::PendingVerification => write!(f, "pending_verification"),
            CommitmentStatus::Fulfilled => write!(f, "fulfilled"),
            CommitmentStatus::Failed => write!(f, "failed"),
            CommitmentStatus::Cancelled => write!(f, "cancelled"),
            CommitmentStatus::Expired => write!(f, "expired"),
        }
    }
}

/// Verification result for a commitment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum VerificationResult {
    /// Commitment was successfully fulfilled
    #[default]
    Approved,
    /// Commitment was not fulfilled or evidence insufficient
    Rejected,
    /// Partially fulfilled (may have reduced reputation impact)
    Partial,
}

impl fmt::Display for VerificationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationResult::Approved => write!(f, "approved"),
            VerificationResult::Rejected => write!(f, "rejected"),
            VerificationResult::Partial => write!(f, "partial"),
        }
    }
}

/// Category of commitment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum CommitmentCategory {
    /// Product development milestone
    Development,
    /// Content creation (blog post, video, etc.)
    Content,
    /// Community event or activity
    Community,
    /// Partnership or collaboration
    Partnership,
    /// Revenue or business metric
    Revenue,
    /// User growth metric
    Growth,
    /// Other category
    #[default]
    Other,
}

impl fmt::Display for CommitmentCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitmentCategory::Development => write!(f, "development"),
            CommitmentCategory::Content => write!(f, "content"),
            CommitmentCategory::Community => write!(f, "community"),
            CommitmentCategory::Partnership => write!(f, "partnership"),
            CommitmentCategory::Revenue => write!(f, "revenue"),
            CommitmentCategory::Growth => write!(f, "growth"),
            CommitmentCategory::Other => write!(f, "other"),
        }
    }
}

impl OutputCommitment {
    /// Check if commitment is overdue
    pub fn is_overdue(&self) -> bool {
        self.deadline < Utc::now()
            && !matches!(
                self.status,
                CommitmentStatus::Fulfilled | CommitmentStatus::Cancelled
            )
    }

    /// Check if evidence can be submitted
    pub fn can_submit_evidence(&self) -> bool {
        matches!(
            self.status,
            CommitmentStatus::Active | CommitmentStatus::AwaitingEvidence
        )
    }

    /// Check if admin can verify
    pub fn can_verify(&self) -> bool {
        matches!(self.status, CommitmentStatus::PendingVerification)
    }

    /// Calculate days until deadline
    pub fn days_until_deadline(&self) -> i64 {
        (self.deadline - Utc::now()).num_days()
    }

    /// Get reputation impact based on verification result
    pub fn calculate_actual_reputation_impact(&self) -> Decimal {
        match self.verification_result {
            Some(VerificationResult::Approved) => self.reputation_impact,
            Some(VerificationResult::Partial) => self.reputation_impact / dec!(2),
            Some(VerificationResult::Rejected) => -self.reputation_impact,
            None => dec!(0),
        }
    }
}

impl fmt::Display for OutputCommitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Commitment({}, {}, status={}, deadline={})",
            self.commitment_id,
            self.title,
            self.status,
            self.deadline.format("%Y-%m-%d")
        )
    }
}

/// Request to create a new commitment
#[derive(Debug, Deserialize)]
pub struct CreateCommitmentRequest {
    pub title: String,
    pub description: String,
    pub deadline: DateTime<Utc>,
    pub category: CommitmentCategory,
    pub reputation_impact: Decimal,
    pub is_public: bool,
    pub milestone_number: Option<i32>,
}

impl CreateCommitmentRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.title.is_empty() {
            return Err(ValidationError("Title is required".to_string()));
        }
        if self.title.len() > 200 {
            return Err(ValidationError(
                "Title must be at most 200 characters".to_string(),
            ));
        }
        if self.description.is_empty() {
            return Err(ValidationError("Description is required".to_string()));
        }
        if self.description.len() > 5000 {
            return Err(ValidationError(
                "Description must be at most 5000 characters".to_string(),
            ));
        }
        if self.deadline <= Utc::now() {
            return Err(ValidationError(
                "Deadline must be in the future".to_string(),
            ));
        }
        // Reasonable deadline range (1 day to 2 years)
        let days_until = (self.deadline - Utc::now()).num_days();
        if days_until < 1 {
            return Err(ValidationError(
                "Deadline must be at least 1 day in the future".to_string(),
            ));
        }
        if days_until > 730 {
            return Err(ValidationError(
                "Deadline cannot be more than 2 years in the future".to_string(),
            ));
        }
        if self.reputation_impact < dec!(0) {
            return Err(ValidationError(
                "Reputation impact cannot be negative".to_string(),
            ));
        }
        if self.reputation_impact > dec!(100) {
            return Err(ValidationError(
                "Reputation impact cannot exceed 100 points".to_string(),
            ));
        }
        Ok(())
    }
}

/// Request to submit evidence for a commitment
#[derive(Debug, Deserialize)]
pub struct SubmitEvidenceRequest {
    pub evidence: String,
}

impl SubmitEvidenceRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.evidence.is_empty() {
            return Err(ValidationError("Evidence is required".to_string()));
        }
        if self.evidence.len() > 10000 {
            return Err(ValidationError(
                "Evidence must be at most 10000 characters".to_string(),
            ));
        }
        Ok(())
    }
}

/// Request to verify a commitment (admin only)
#[derive(Debug, Deserialize)]
pub struct VerifyCommitmentRequest {
    pub result: VerificationResult,
    pub notes: Option<String>,
}

impl VerifyCommitmentRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref notes) = self.notes {
            if notes.len() > 2000 {
                return Err(ValidationError(
                    "Notes must be at most 2000 characters".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Reputation event tracking changes to user reputation
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReputationEvent {
    pub event_id: Uuid,
    pub user_id: Uuid,
    /// Type of event that caused reputation change
    pub event_type: ReputationEventType,
    /// Change in reputation score (can be positive or negative)
    pub score_change: Decimal,
    /// Reputation score after this event
    pub new_score: Decimal,
    /// Related commitment (if applicable)
    pub commitment_id: Option<Uuid>,
    /// Related trade (if applicable)
    pub trade_id: Option<Uuid>,
    /// Description of the event
    pub description: String,
    /// Admin who triggered this event (if manually adjusted)
    pub triggered_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Type of reputation event
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum ReputationEventType {
    /// Commitment fulfilled successfully
    CommitmentFulfilled,
    /// Commitment failed or rejected
    CommitmentFailed,
    /// Commitment partially fulfilled
    CommitmentPartial,
    /// Successful trade completion
    TradeFeedback,
    /// Manual adjustment by admin
    #[default]
    ManualAdjustment,
    /// KYC verification completed
    KycVerified,
    /// Community report or flag
    CommunityReport,
    /// Token performance milestone
    TokenMilestone,
    /// Time-based decay
    TimeDecay,
}

impl fmt::Display for ReputationEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReputationEventType::CommitmentFulfilled => write!(f, "commitment_fulfilled"),
            ReputationEventType::CommitmentFailed => write!(f, "commitment_failed"),
            ReputationEventType::CommitmentPartial => write!(f, "commitment_partial"),
            ReputationEventType::TradeFeedback => write!(f, "trade_feedback"),
            ReputationEventType::ManualAdjustment => write!(f, "manual_adjustment"),
            ReputationEventType::KycVerified => write!(f, "kyc_verified"),
            ReputationEventType::CommunityReport => write!(f, "community_report"),
            ReputationEventType::TokenMilestone => write!(f, "token_milestone"),
            ReputationEventType::TimeDecay => write!(f, "time_decay"),
        }
    }
}

impl fmt::Display for ReputationEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReputationEvent({}, user={}, type={}, change={:+})",
            self.event_id, self.user_id, self.event_type, self.score_change
        )
    }
}

/// Reputation score calculator
pub struct ReputationCalculator;

impl ReputationCalculator {
    /// Base reputation for new users
    pub const BASE_REPUTATION: Decimal = dec!(50);

    /// Minimum possible reputation
    pub const MIN_REPUTATION: Decimal = dec!(0);

    /// Maximum possible reputation
    pub const MAX_REPUTATION: Decimal = dec!(100);

    /// Reputation bonus for KYC verification
    pub const KYC_BONUS: Decimal = dec!(10);

    /// Create a new reputation event
    #[allow(clippy::too_many_arguments)]
    pub fn create_event(
        user_id: Uuid,
        event_type: ReputationEventType,
        score_change: Decimal,
        current_score: Decimal,
        description: String,
        commitment_id: Option<Uuid>,
        trade_id: Option<Uuid>,
        triggered_by_user_id: Option<Uuid>,
    ) -> ReputationEvent {
        let new_score = Self::clamp_score(current_score + score_change);

        ReputationEvent {
            event_id: Uuid::new_v4(),
            user_id,
            event_type,
            score_change,
            new_score,
            commitment_id,
            trade_id,
            description,
            triggered_by_user_id,
            created_at: Utc::now(),
        }
    }

    /// Clamp reputation score within valid range
    pub fn clamp_score(score: Decimal) -> Decimal {
        if score < Self::MIN_REPUTATION {
            Self::MIN_REPUTATION
        } else if score > Self::MAX_REPUTATION {
            Self::MAX_REPUTATION
        } else {
            score
        }
    }

    /// Calculate reputation change for a commitment based on verification result
    pub fn calculate_commitment_impact(
        commitment: &OutputCommitment,
        result: VerificationResult,
    ) -> (Decimal, ReputationEventType) {
        match result {
            VerificationResult::Approved => (
                commitment.reputation_impact,
                ReputationEventType::CommitmentFulfilled,
            ),
            VerificationResult::Partial => (
                commitment.reputation_impact / dec!(2),
                ReputationEventType::CommitmentPartial,
            ),
            VerificationResult::Rejected => (
                -commitment.reputation_impact,
                ReputationEventType::CommitmentFailed,
            ),
        }
    }

    /// Calculate time-based reputation decay (optional feature)
    /// Returns negative change if user has been inactive
    #[allow(dead_code)]
    pub fn calculate_time_decay(days_since_last_activity: i64, current_score: Decimal) -> Decimal {
        // Only decay if score is above base and user has been inactive for 90+ days
        if days_since_last_activity < 90 || current_score <= Self::BASE_REPUTATION {
            return dec!(0);
        }

        // Decay 1 point per 30 days of inactivity (up to max of 10 points)
        let decay_amount = Decimal::from(days_since_last_activity / 30).min(dec!(10));
        -decay_amount
    }

    /// Get reputation tier based on score
    pub fn get_reputation_tier(score: Decimal) -> ReputationTier {
        if score >= dec!(90) {
            ReputationTier::Excellent
        } else if score >= dec!(75) {
            ReputationTier::VeryGood
        } else if score >= dec!(60) {
            ReputationTier::Good
        } else if score >= dec!(40) {
            ReputationTier::Fair
        } else if score >= dec!(25) {
            ReputationTier::Poor
        } else {
            ReputationTier::VeryPoor
        }
    }

    /// Calculate commitment fulfillment rate for a user
    pub fn calculate_fulfillment_rate(
        total_commitments: i32,
        fulfilled_commitments: i32,
    ) -> Decimal {
        if total_commitments == 0 {
            return dec!(0);
        }
        (Decimal::from(fulfilled_commitments) / Decimal::from(total_commitments)) * dec!(100)
    }
}

/// Reputation tier classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReputationTier {
    VeryPoor,
    Poor,
    Fair,
    Good,
    VeryGood,
    Excellent,
}

impl fmt::Display for ReputationTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReputationTier::VeryPoor => write!(f, "Very Poor"),
            ReputationTier::Poor => write!(f, "Poor"),
            ReputationTier::Fair => write!(f, "Fair"),
            ReputationTier::Good => write!(f, "Good"),
            ReputationTier::VeryGood => write!(f, "Very Good"),
            ReputationTier::Excellent => write!(f, "Excellent"),
        }
    }
}

/// Reputation statistics for a user
#[derive(Debug, Serialize)]
pub struct ReputationStats {
    pub user_id: Uuid,
    pub current_score: Decimal,
    pub tier: ReputationTier,
    pub total_commitments: i32,
    pub fulfilled_commitments: i32,
    pub failed_commitments: i32,
    pub pending_commitments: i32,
    pub fulfillment_rate_percent: Decimal,
    pub total_reputation_gained: Decimal,
    pub total_reputation_lost: Decimal,
}

/// Circuit breaker for reputation-based actions
pub struct ReputationCircuitBreaker;

impl ReputationCircuitBreaker {
    /// Minimum reputation to create a token
    pub const MIN_REPUTATION_CREATE_TOKEN: Decimal = dec!(40);

    /// Minimum reputation to create high-value commitments (impact > 20)
    pub const MIN_REPUTATION_HIGH_COMMITMENT: Decimal = dec!(60);

    /// Maximum pending commitments allowed
    pub const MAX_PENDING_COMMITMENTS: i32 = 10;

    /// Check if user can create a token
    pub fn can_create_token(reputation_score: Decimal) -> bool {
        reputation_score >= Self::MIN_REPUTATION_CREATE_TOKEN
    }

    /// Check if user can create a high-value commitment
    pub fn can_create_high_value_commitment(
        reputation_score: Decimal,
        commitment_impact: Decimal,
    ) -> bool {
        if commitment_impact <= dec!(20) {
            return true; // Low-impact commitments allowed for everyone
        }
        reputation_score >= Self::MIN_REPUTATION_HIGH_COMMITMENT
    }

    /// Check if user can create more commitments
    pub fn can_create_commitment(pending_count: i32, failed_count: i32) -> Result<(), String> {
        if pending_count >= Self::MAX_PENDING_COMMITMENTS {
            return Err(format!(
                "Maximum pending commitments reached ({})",
                Self::MAX_PENDING_COMMITMENTS
            ));
        }

        // Additional check: if user has 3+ recent failures, require some successes first
        if failed_count >= 3 && pending_count > 0 {
            return Err("Please fulfill existing commitments before creating new ones".to_string());
        }

        Ok(())
    }

    /// Get recommended reputation impact based on user's current score
    pub fn recommend_commitment_impact(reputation_score: Decimal) -> Decimal {
        if reputation_score >= dec!(80) {
            dec!(30) // High reputation users can make high-impact commitments
        } else if reputation_score >= dec!(60) {
            dec!(20)
        } else if reputation_score >= dec!(40) {
            dec!(10)
        } else {
            dec!(5) // Low reputation users should start small
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_is_overdue() {
        let mut commitment = OutputCommitment {
            commitment_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            title: "Test".to_string(),
            description: "Test commitment".to_string(),
            deadline: Utc::now() - chrono::Duration::days(1),
            status: CommitmentStatus::Active,
            evidence: None,
            evidence_submitted_at: None,
            verification_result: None,
            verified_by_user_id: None,
            verified_at: None,
            verification_notes: None,
            reputation_impact: dec!(10),
            category: CommitmentCategory::Development,
            is_public: true,
            milestone_number: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(commitment.is_overdue());

        commitment.status = CommitmentStatus::Fulfilled;
        assert!(!commitment.is_overdue());
    }

    #[test]
    fn test_reputation_calculator_clamp() {
        assert_eq!(ReputationCalculator::clamp_score(dec!(-10)), dec!(0));
        assert_eq!(ReputationCalculator::clamp_score(dec!(150)), dec!(100));
        assert_eq!(ReputationCalculator::clamp_score(dec!(75)), dec!(75));
    }

    #[test]
    fn test_reputation_tiers() {
        assert_eq!(
            ReputationCalculator::get_reputation_tier(dec!(95)),
            ReputationTier::Excellent
        );
        assert_eq!(
            ReputationCalculator::get_reputation_tier(dec!(70)),
            ReputationTier::Good
        );
        assert_eq!(
            ReputationCalculator::get_reputation_tier(dec!(20)),
            ReputationTier::VeryPoor
        );
    }

    #[test]
    fn test_commitment_impact_calculation() {
        let commitment = OutputCommitment {
            commitment_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            title: "Test".to_string(),
            description: "Test commitment".to_string(),
            deadline: Utc::now() + chrono::Duration::days(30),
            status: CommitmentStatus::Active,
            evidence: None,
            evidence_submitted_at: None,
            verification_result: None,
            verified_by_user_id: None,
            verified_at: None,
            verification_notes: None,
            reputation_impact: dec!(20),
            category: CommitmentCategory::Development,
            is_public: true,
            milestone_number: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let (change, event_type) = ReputationCalculator::calculate_commitment_impact(
            &commitment,
            VerificationResult::Approved,
        );
        assert_eq!(change, dec!(20));
        assert_eq!(event_type, ReputationEventType::CommitmentFulfilled);

        let (change, _) = ReputationCalculator::calculate_commitment_impact(
            &commitment,
            VerificationResult::Partial,
        );
        assert_eq!(change, dec!(10));

        let (change, event_type) = ReputationCalculator::calculate_commitment_impact(
            &commitment,
            VerificationResult::Rejected,
        );
        assert_eq!(change, dec!(-20));
        assert_eq!(event_type, ReputationEventType::CommitmentFailed);
    }

    #[test]
    fn test_circuit_breaker() {
        assert!(ReputationCircuitBreaker::can_create_token(dec!(50)));
        assert!(!ReputationCircuitBreaker::can_create_token(dec!(30)));

        assert!(ReputationCircuitBreaker::can_create_high_value_commitment(
            dec!(70),
            dec!(25)
        ));
        assert!(!ReputationCircuitBreaker::can_create_high_value_commitment(
            dec!(50),
            dec!(25)
        ));
        assert!(ReputationCircuitBreaker::can_create_high_value_commitment(
            dec!(50),
            dec!(15)
        ));
    }

    #[test]
    fn test_fulfillment_rate() {
        let rate = ReputationCalculator::calculate_fulfillment_rate(10, 8);
        assert_eq!(rate, dec!(80));

        let rate = ReputationCalculator::calculate_fulfillment_rate(0, 0);
        assert_eq!(rate, dec!(0));
    }

    #[test]
    fn test_create_commitment_validation() {
        let valid_request = CreateCommitmentRequest {
            title: "Valid commitment".to_string(),
            description: "This is a valid commitment description".to_string(),
            deadline: Utc::now() + chrono::Duration::days(30),
            category: CommitmentCategory::Development,
            reputation_impact: dec!(15),
            is_public: true,
            milestone_number: None,
        };
        assert!(valid_request.validate().is_ok());

        let invalid_request = CreateCommitmentRequest {
            title: "".to_string(),
            description: "desc".to_string(),
            deadline: Utc::now() + chrono::Duration::days(30),
            category: CommitmentCategory::Development,
            reputation_impact: dec!(15),
            is_public: true,
            milestone_number: None,
        };
        assert!(invalid_request.validate().is_err());
    }
}
