//! Commitment workflow management
//!
//! This module provides utilities for managing the full lifecycle of output commitments,
//! including creation, evidence submission, verification, and reputation impact.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use super::reputation::{
    CommitmentCategory, CommitmentStatus, OutputCommitment, ReputationCalculator, ReputationEvent,
    VerificationResult,
};
use crate::error::{CoreError, Result};

/// Commitment workflow manager
pub struct CommitmentWorkflowManager;

impl CommitmentWorkflowManager {
    /// Calculate recommended deadline based on category and complexity
    pub fn recommend_deadline(
        category: CommitmentCategory,
        complexity: CommitmentComplexity,
    ) -> DateTime<Utc> {
        let days = match (category, complexity) {
            (CommitmentCategory::Development, CommitmentComplexity::Simple) => 14,
            (CommitmentCategory::Development, CommitmentComplexity::Medium) => 30,
            (CommitmentCategory::Development, CommitmentComplexity::Complex) => 60,
            (CommitmentCategory::Content, CommitmentComplexity::Simple) => 7,
            (CommitmentCategory::Content, CommitmentComplexity::Medium) => 14,
            (CommitmentCategory::Content, CommitmentComplexity::Complex) => 21,
            (CommitmentCategory::Community, _) => 14,
            (CommitmentCategory::Partnership, _) => 30,
            (CommitmentCategory::Revenue, CommitmentComplexity::Simple) => 30,
            (CommitmentCategory::Revenue, CommitmentComplexity::Medium) => 60,
            (CommitmentCategory::Revenue, CommitmentComplexity::Complex) => 90,
            (CommitmentCategory::Growth, CommitmentComplexity::Simple) => 30,
            (CommitmentCategory::Growth, CommitmentComplexity::Medium) => 60,
            (CommitmentCategory::Growth, CommitmentComplexity::Complex) => 90,
            (CommitmentCategory::Other, _) => 30,
        };

        Utc::now() + Duration::days(days)
    }

    /// Calculate recommended reputation impact based on category, complexity, and current reputation
    pub fn calculate_reputation_impact(
        category: CommitmentCategory,
        complexity: CommitmentComplexity,
        current_reputation: Decimal,
    ) -> Decimal {
        // Base impact varies by category
        let category_weight = match category {
            CommitmentCategory::Development => dec!(1.5),
            CommitmentCategory::Content => dec!(1.0),
            CommitmentCategory::Community => dec!(0.8),
            CommitmentCategory::Partnership => dec!(1.2),
            CommitmentCategory::Revenue => dec!(2.0),
            CommitmentCategory::Growth => dec!(1.8),
            CommitmentCategory::Other => dec!(0.5),
        };

        // Complexity multiplier
        let complexity_multiplier = match complexity {
            CommitmentComplexity::Simple => dec!(1.0),
            CommitmentComplexity::Medium => dec!(1.5),
            CommitmentComplexity::Complex => dec!(2.0),
        };

        // Base impact scales with current reputation (higher rep = higher stakes)
        let reputation_factor = if current_reputation < dec!(30) {
            dec!(0.5) // New users: lower stakes
        } else if current_reputation < dec!(60) {
            dec!(1.0) // Average users: normal stakes
        } else {
            dec!(1.5) // High-reputation users: higher stakes
        };

        // Calculate final impact (capped at reasonable values)
        let impact = category_weight * complexity_multiplier * reputation_factor * dec!(5);

        // Cap between 1 and 20 points
        impact.max(dec!(1)).min(dec!(20))
    }

    /// Validate commitment before creation
    pub fn validate_commitment(
        title: &str,
        description: &str,
        deadline: DateTime<Utc>,
        reputation_impact: Decimal,
    ) -> Result<()> {
        if title.is_empty() {
            return Err(CoreError::Validation("Title cannot be empty".to_string()));
        }

        if title.len() > 200 {
            return Err(CoreError::Validation(
                "Title must be at most 200 characters".to_string(),
            ));
        }

        if description.is_empty() {
            return Err(CoreError::Validation(
                "Description cannot be empty".to_string(),
            ));
        }

        if description.len() < 50 {
            return Err(CoreError::Validation(
                "Description must be at least 50 characters".to_string(),
            ));
        }

        if deadline <= Utc::now() {
            return Err(CoreError::Validation(
                "Deadline must be in the future".to_string(),
            ));
        }

        let max_deadline = Utc::now() + Duration::days(365);
        if deadline > max_deadline {
            return Err(CoreError::Validation(
                "Deadline cannot be more than 1 year in the future".to_string(),
            ));
        }

        if reputation_impact <= dec!(0) {
            return Err(CoreError::Validation(
                "Reputation impact must be positive".to_string(),
            ));
        }

        if reputation_impact > dec!(20) {
            return Err(CoreError::Validation(
                "Reputation impact cannot exceed 20 points".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if commitment can transition to a new status
    pub fn can_transition(current_status: CommitmentStatus, new_status: CommitmentStatus) -> bool {
        match (current_status, new_status) {
            // From Active
            (CommitmentStatus::Active, CommitmentStatus::AwaitingEvidence) => true,
            (CommitmentStatus::Active, CommitmentStatus::Cancelled) => true,
            (CommitmentStatus::Active, CommitmentStatus::Expired) => true,

            // From AwaitingEvidence
            (CommitmentStatus::AwaitingEvidence, CommitmentStatus::PendingVerification) => true,
            (CommitmentStatus::AwaitingEvidence, CommitmentStatus::Cancelled) => true,
            (CommitmentStatus::AwaitingEvidence, CommitmentStatus::Expired) => true,

            // From PendingVerification
            (CommitmentStatus::PendingVerification, CommitmentStatus::Fulfilled) => true,
            (CommitmentStatus::PendingVerification, CommitmentStatus::Failed) => true,

            // Terminal states cannot transition
            (CommitmentStatus::Fulfilled, _)
            | (CommitmentStatus::Failed, _)
            | (CommitmentStatus::Cancelled, _)
            | (CommitmentStatus::Expired, _) => false,

            // Any other transition is invalid
            _ => false,
        }
    }

    /// Get commitment urgency level based on deadline
    pub fn get_urgency_level(commitment: &OutputCommitment) -> CommitmentUrgency {
        if matches!(
            commitment.status,
            CommitmentStatus::Fulfilled
                | CommitmentStatus::Failed
                | CommitmentStatus::Cancelled
                | CommitmentStatus::Expired
        ) {
            return CommitmentUrgency::None;
        }

        let days_remaining = commitment.days_until_deadline();

        if days_remaining < 0 {
            CommitmentUrgency::Overdue
        } else if days_remaining <= 3 {
            CommitmentUrgency::Critical
        } else if days_remaining <= 7 {
            CommitmentUrgency::High
        } else if days_remaining <= 14 {
            CommitmentUrgency::Medium
        } else {
            CommitmentUrgency::Low
        }
    }

    /// Generate reminder schedule for a commitment
    pub fn generate_reminder_schedule(deadline: DateTime<Utc>) -> Vec<DateTime<Utc>> {
        let mut reminders = Vec::new();

        // 30 days before
        let reminder_30d = deadline - Duration::days(30);
        if reminder_30d > Utc::now() {
            reminders.push(reminder_30d);
        }

        // 14 days before
        let reminder_14d = deadline - Duration::days(14);
        if reminder_14d > Utc::now() {
            reminders.push(reminder_14d);
        }

        // 7 days before
        let reminder_7d = deadline - Duration::days(7);
        if reminder_7d > Utc::now() {
            reminders.push(reminder_7d);
        }

        // 3 days before
        let reminder_3d = deadline - Duration::days(3);
        if reminder_3d > Utc::now() {
            reminders.push(reminder_3d);
        }

        // 1 day before
        let reminder_1d = deadline - Duration::days(1);
        if reminder_1d > Utc::now() {
            reminders.push(reminder_1d);
        }

        // On deadline day
        if deadline > Utc::now() {
            reminders.push(deadline);
        }

        reminders
    }

    /// Validate evidence submission
    pub fn validate_evidence(evidence: &str) -> Result<()> {
        if evidence.is_empty() {
            return Err(CoreError::Validation(
                "Evidence cannot be empty".to_string(),
            ));
        }

        if evidence.len() < 20 {
            return Err(CoreError::Validation(
                "Evidence description must be at least 20 characters".to_string(),
            ));
        }

        if evidence.len() > 5000 {
            return Err(CoreError::Validation(
                "Evidence description must be at most 5000 characters".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate completion rate for a user's commitments
    pub fn calculate_completion_rate(
        fulfilled: usize,
        failed: usize,
        pending: usize,
    ) -> CompletionStats {
        let total = fulfilled + failed + pending;

        if total == 0 {
            return CompletionStats {
                completion_rate: dec!(0),
                success_rate: dec!(0),
                total_commitments: 0,
                fulfilled_count: 0,
                failed_count: 0,
                pending_count: 0,
            };
        }

        let completed = fulfilled + failed;
        let completion_rate = if completed > 0 {
            Decimal::from(completed) / Decimal::from(total) * dec!(100)
        } else {
            dec!(0)
        };

        let success_rate = if completed > 0 {
            Decimal::from(fulfilled) / Decimal::from(completed) * dec!(100)
        } else {
            dec!(0)
        };

        CompletionStats {
            completion_rate,
            success_rate,
            total_commitments: total,
            fulfilled_count: fulfilled,
            failed_count: failed,
            pending_count: pending,
        }
    }

    /// Create reputation event for commitment verification
    pub fn create_verification_event(
        commitment: &OutputCommitment,
        result: VerificationResult,
        verified_by_user_id: Uuid,
    ) -> ReputationEvent {
        let (score_change, event_type) =
            ReputationCalculator::calculate_commitment_impact(commitment, result);

        let description = format!(
            "Commitment '{}' was {}",
            commitment.title,
            match result {
                VerificationResult::Approved => "fulfilled",
                VerificationResult::Partial => "partially fulfilled",
                VerificationResult::Rejected => "not fulfilled",
            }
        );

        ReputationEvent {
            event_id: Uuid::new_v4(),
            user_id: commitment.user_id,
            event_type,
            score_change,
            new_score: dec!(0), // This will be calculated when applied
            commitment_id: Some(commitment.commitment_id),
            trade_id: None,
            description,
            triggered_by_user_id: Some(verified_by_user_id),
            created_at: Utc::now(),
        }
    }
}

/// Commitment complexity level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommitmentComplexity {
    /// Low-effort commitment with straightforward execution.
    Simple,
    /// Moderate effort with some conditional steps.
    Medium,
    /// High-effort commitment requiring multiple coordinated steps.
    Complex,
}

impl fmt::Display for CommitmentComplexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitmentComplexity::Simple => write!(f, "simple"),
            CommitmentComplexity::Medium => write!(f, "medium"),
            CommitmentComplexity::Complex => write!(f, "complex"),
        }
    }
}

/// Commitment urgency level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum CommitmentUrgency {
    /// No urgency assigned.
    None,
    /// Low urgency; can be deferred.
    Low,
    /// Moderate urgency; should be addressed soon.
    Medium,
    /// High urgency; requires prompt attention.
    High,
    /// Critical urgency; requires immediate action.
    Critical,
    /// The commitment deadline has already passed.
    Overdue,
}

impl fmt::Display for CommitmentUrgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitmentUrgency::None => write!(f, "none"),
            CommitmentUrgency::Low => write!(f, "low"),
            CommitmentUrgency::Medium => write!(f, "medium"),
            CommitmentUrgency::High => write!(f, "high"),
            CommitmentUrgency::Critical => write!(f, "critical"),
            CommitmentUrgency::Overdue => write!(f, "overdue"),
        }
    }
}

/// Completion statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionStats {
    /// Percentage of commitments that are completed (fulfilled or failed)
    pub completion_rate: Decimal,
    /// Percentage of completed commitments that were fulfilled
    pub success_rate: Decimal,
    /// Total number of commitments
    pub total_commitments: usize,
    /// Number of fulfilled commitments
    pub fulfilled_count: usize,
    /// Number of failed commitments
    pub failed_count: usize,
    /// Number of pending commitments
    pub pending_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommend_deadline() {
        let deadline = CommitmentWorkflowManager::recommend_deadline(
            CommitmentCategory::Development,
            CommitmentComplexity::Simple,
        );
        assert!(deadline > Utc::now());
    }

    #[test]
    fn test_calculate_reputation_impact() {
        let impact = CommitmentWorkflowManager::calculate_reputation_impact(
            CommitmentCategory::Development,
            CommitmentComplexity::Medium,
            dec!(50),
        );
        assert!(impact > dec!(0));
        assert!(impact <= dec!(20));
    }

    #[test]
    fn test_validate_commitment_empty_title() {
        let result = CommitmentWorkflowManager::validate_commitment(
            "",
            "A valid description that is definitely longer than fifty characters for testing",
            Utc::now() + Duration::days(30),
            dec!(5),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_commitment_short_description() {
        let result = CommitmentWorkflowManager::validate_commitment(
            "Valid Title",
            "Too short",
            Utc::now() + Duration::days(30),
            dec!(5),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_commitment_past_deadline() {
        let result = CommitmentWorkflowManager::validate_commitment(
            "Valid Title",
            "A valid description that is definitely longer than fifty characters for testing",
            Utc::now() - Duration::days(1),
            dec!(5),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_commitment_valid() {
        let result = CommitmentWorkflowManager::validate_commitment(
            "Valid Title",
            "A valid description that is definitely longer than fifty characters for testing",
            Utc::now() + Duration::days(30),
            dec!(5),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_transition_valid() {
        assert!(CommitmentWorkflowManager::can_transition(
            CommitmentStatus::Active,
            CommitmentStatus::AwaitingEvidence
        ));

        assert!(CommitmentWorkflowManager::can_transition(
            CommitmentStatus::PendingVerification,
            CommitmentStatus::Fulfilled
        ));
    }

    #[test]
    fn test_can_transition_invalid() {
        assert!(!CommitmentWorkflowManager::can_transition(
            CommitmentStatus::Fulfilled,
            CommitmentStatus::Active
        ));

        assert!(!CommitmentWorkflowManager::can_transition(
            CommitmentStatus::Active,
            CommitmentStatus::Fulfilled
        ));
    }

    #[test]
    fn test_validate_evidence_empty() {
        let result = CommitmentWorkflowManager::validate_evidence("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_evidence_too_short() {
        let result = CommitmentWorkflowManager::validate_evidence("Too short");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_evidence_valid() {
        let result = CommitmentWorkflowManager::validate_evidence(
            "Here is valid evidence with sufficient detail to be accepted",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_calculate_completion_rate() {
        let stats = CommitmentWorkflowManager::calculate_completion_rate(8, 2, 5);
        assert_eq!(stats.total_commitments, 15);
        assert_eq!(stats.fulfilled_count, 8);
        assert_eq!(stats.failed_count, 2);
        assert_eq!(stats.pending_count, 5);
        // 10/15 * 100 ≈ 66.67%
        assert!(stats.completion_rate > dec!(66.6) && stats.completion_rate < dec!(66.7));
        assert_eq!(stats.success_rate, dec!(80)); // 8/10 * 100
    }

    #[test]
    fn test_calculate_completion_rate_no_commitments() {
        let stats = CommitmentWorkflowManager::calculate_completion_rate(0, 0, 0);
        assert_eq!(stats.total_commitments, 0);
        assert_eq!(stats.completion_rate, dec!(0));
        assert_eq!(stats.success_rate, dec!(0));
    }

    #[test]
    fn test_generate_reminder_schedule() {
        let deadline = Utc::now() + Duration::days(60);
        let reminders = CommitmentWorkflowManager::generate_reminder_schedule(deadline);

        // Should have multiple reminders
        assert!(!reminders.is_empty());
        // All reminders should be before the deadline
        assert!(reminders.iter().all(|&r| r <= deadline));
    }
}
