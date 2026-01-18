//! Governance system for protocol changes

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Governance proposal
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Proposal {
    pub proposal_id: Uuid,
    pub proposer_user_id: Uuid,
    /// Proposal title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Proposal type
    pub proposal_type: ProposalType,
    /// Voting power required (quorum)
    pub quorum_required: Decimal,
    /// Percentage of votes needed to pass (e.g., 0.66 for 66%)
    pub approval_threshold: Decimal,
    /// Current voting power in favor
    pub votes_for: Decimal,
    /// Current voting power against
    pub votes_against: Decimal,
    /// Current voting power abstained
    pub votes_abstain: Decimal,
    /// Proposal status
    pub status: ProposalStatus,
    /// Voting start time
    pub voting_start: DateTime<Utc>,
    /// Voting end time
    pub voting_end: DateTime<Utc>,
    /// Execution time (if time-locked)
    pub execution_time: Option<DateTime<Utc>>,
    /// Time lock duration in seconds
    pub time_lock_seconds: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum ProposalType {
    /// Update protocol parameters
    ParameterChange,
    /// Add/remove token from platform
    TokenListing,
    /// Upgrade smart contract
    ContractUpgrade,
    /// Treasury allocation
    TreasurySpend,
    /// Fee structure change
    FeeChange,
    /// Emergency action
    EmergencyAction,
    /// General proposal
    #[default]
    General,
}

impl fmt::Display for ProposalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalType::ParameterChange => write!(f, "parameter_change"),
            ProposalType::TokenListing => write!(f, "token_listing"),
            ProposalType::ContractUpgrade => write!(f, "contract_upgrade"),
            ProposalType::TreasurySpend => write!(f, "treasury_spend"),
            ProposalType::FeeChange => write!(f, "fee_change"),
            ProposalType::EmergencyAction => write!(f, "emergency_action"),
            ProposalType::General => write!(f, "general"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum ProposalStatus {
    /// Voting in progress
    #[default]
    Active,
    /// Proposal passed, waiting for time lock
    Passed,
    /// Proposal rejected
    Rejected,
    /// Proposal executed
    Executed,
    /// Proposal cancelled
    Cancelled,
    /// Proposal expired without reaching quorum
    Expired,
}

impl fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalStatus::Active => write!(f, "active"),
            ProposalStatus::Passed => write!(f, "passed"),
            ProposalStatus::Rejected => write!(f, "rejected"),
            ProposalStatus::Executed => write!(f, "executed"),
            ProposalStatus::Cancelled => write!(f, "cancelled"),
            ProposalStatus::Expired => write!(f, "expired"),
        }
    }
}

impl Proposal {
    /// Calculate total votes cast
    pub fn total_votes(&self) -> Decimal {
        self.votes_for + self.votes_against + self.votes_abstain
    }

    /// Check if quorum is reached
    pub fn is_quorum_reached(&self) -> bool {
        self.total_votes() >= self.quorum_required
    }

    /// Check if proposal is approved
    pub fn is_approved(&self) -> bool {
        let total_decisive_votes = self.votes_for + self.votes_against;
        if total_decisive_votes == dec!(0) {
            return false;
        }

        let approval_rate = self.votes_for / total_decisive_votes;
        approval_rate >= self.approval_threshold
    }

    /// Check if voting period has ended
    pub fn is_voting_ended(&self) -> bool {
        Utc::now() >= self.voting_end
    }

    /// Check if proposal can be executed
    pub fn can_execute(&self) -> bool {
        if self.status != ProposalStatus::Passed {
            return false;
        }

        if let Some(exec_time) = self.execution_time {
            Utc::now() >= exec_time
        } else {
            true
        }
    }

    /// Calculate approval percentage
    pub fn approval_percentage(&self) -> Decimal {
        let total_decisive = self.votes_for + self.votes_against;
        if total_decisive == dec!(0) {
            return dec!(0);
        }
        (self.votes_for / total_decisive) * dec!(100)
    }

    /// Calculate participation rate
    pub fn participation_rate(&self, total_voting_power: Decimal) -> Decimal {
        if total_voting_power == dec!(0) {
            return dec!(0);
        }
        (self.total_votes() / total_voting_power) * dec!(100)
    }

    /// Validate proposal
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
        if self.quorum_required <= dec!(0) {
            return Err(ValidationError("Quorum must be positive".to_string()));
        }
        if self.approval_threshold <= dec!(0) || self.approval_threshold > dec!(1) {
            return Err(ValidationError(
                "Approval threshold must be between 0 and 1".to_string(),
            ));
        }
        if self.voting_end <= self.voting_start {
            return Err(ValidationError(
                "Voting end must be after voting start".to_string(),
            ));
        }
        if self.time_lock_seconds < 0 {
            return Err(ValidationError("Time lock cannot be negative".to_string()));
        }

        Ok(())
    }
}

impl fmt::Display for Proposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Proposal({}, title='{}', status={})",
            self.proposal_id, self.title, self.status
        )
    }
}

/// Individual vote on a proposal
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Vote {
    pub vote_id: Uuid,
    pub proposal_id: Uuid,
    pub voter_user_id: Uuid,
    /// Voting power (based on token holdings)
    pub voting_power: Decimal,
    /// Vote choice
    pub choice: VoteChoice,
    /// Optional reasoning
    pub reason: Option<String>,
    pub voted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum VoteChoice {
    For,
    Against,
    #[default]
    Abstain,
}

impl fmt::Display for VoteChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VoteChoice::For => write!(f, "for"),
            VoteChoice::Against => write!(f, "against"),
            VoteChoice::Abstain => write!(f, "abstain"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProposalRequest {
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub voting_duration_hours: i64,
    pub time_lock_hours: Option<i64>,
}

impl CreateProposalRequest {
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
        if self.voting_duration_hours <= 0 {
            return Err(ValidationError(
                "Voting duration must be positive".to_string(),
            ));
        }
        if let Some(time_lock) = self.time_lock_hours {
            if time_lock < 0 {
                return Err(ValidationError("Time lock cannot be negative".to_string()));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CastVoteRequest {
    pub proposal_id: Uuid,
    pub choice: VoteChoice,
    pub reason: Option<String>,
}

impl CastVoteRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref reason) = self.reason {
            if reason.len() > 1000 {
                return Err(ValidationError(
                    "Reason must be at most 1000 characters".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Minimum voting power to create proposal
    pub min_proposal_power: Decimal,
    /// Default quorum percentage (e.g., 0.1 for 10%)
    pub default_quorum: Decimal,
    /// Default approval threshold (e.g., 0.66 for 66%)
    pub default_approval_threshold: Decimal,
    /// Default voting duration in hours
    pub default_voting_duration_hours: i64,
    /// Default time lock in hours
    pub default_time_lock_hours: i64,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            min_proposal_power: dec!(1000),         // Need 1000 tokens to propose
            default_quorum: dec!(0.1),              // 10% quorum
            default_approval_threshold: dec!(0.66), // 66% approval
            default_voting_duration_hours: 72,      // 3 days
            default_time_lock_hours: 24,            // 1 day time lock
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_quorum() {
        let proposal = Proposal {
            proposal_id: Uuid::new_v4(),
            proposer_user_id: Uuid::new_v4(),
            title: "Test Proposal".to_string(),
            description: "Test Description".to_string(),
            proposal_type: ProposalType::General,
            quorum_required: dec!(1000),
            approval_threshold: dec!(0.66),
            votes_for: dec!(600),
            votes_against: dec!(200),
            votes_abstain: dec!(100),
            status: ProposalStatus::Active,
            voting_start: Utc::now(),
            voting_end: Utc::now() + chrono::Duration::hours(72),
            execution_time: None,
            time_lock_seconds: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(proposal.total_votes(), dec!(900));
        assert!(!proposal.is_quorum_reached()); // 900 < 1000

        let mut proposal_with_quorum = proposal.clone();
        proposal_with_quorum.votes_for = dec!(700);
        assert_eq!(proposal_with_quorum.total_votes(), dec!(1000));
        assert!(proposal_with_quorum.is_quorum_reached());
    }

    #[test]
    fn test_proposal_approval() {
        let proposal = Proposal {
            proposal_id: Uuid::new_v4(),
            proposer_user_id: Uuid::new_v4(),
            title: "Test Proposal".to_string(),
            description: "Test Description".to_string(),
            proposal_type: ProposalType::General,
            quorum_required: dec!(100),
            approval_threshold: dec!(0.66),
            votes_for: dec!(700),
            votes_against: dec!(300),
            votes_abstain: dec!(0),
            status: ProposalStatus::Active,
            voting_start: Utc::now(),
            voting_end: Utc::now() + chrono::Duration::hours(72),
            execution_time: None,
            time_lock_seconds: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 700 / (700 + 300) = 0.7 = 70% > 66%
        assert!(proposal.is_approved());

        let mut rejected_proposal = proposal.clone();
        rejected_proposal.votes_for = dec!(600);
        rejected_proposal.votes_against = dec!(400);
        // 600 / 1000 = 0.6 = 60% < 66%
        assert!(!rejected_proposal.is_approved());
    }

    #[test]
    fn test_proposal_approval_percentage() {
        let proposal = Proposal {
            proposal_id: Uuid::new_v4(),
            proposer_user_id: Uuid::new_v4(),
            title: "Test Proposal".to_string(),
            description: "Test Description".to_string(),
            proposal_type: ProposalType::General,
            quorum_required: dec!(100),
            approval_threshold: dec!(0.5),
            votes_for: dec!(750),
            votes_against: dec!(250),
            votes_abstain: dec!(0),
            status: ProposalStatus::Active,
            voting_start: Utc::now(),
            voting_end: Utc::now() + chrono::Duration::hours(72),
            execution_time: None,
            time_lock_seconds: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(proposal.approval_percentage(), dec!(75));
    }

    #[test]
    fn test_participation_rate() {
        let proposal = Proposal {
            proposal_id: Uuid::new_v4(),
            proposer_user_id: Uuid::new_v4(),
            title: "Test Proposal".to_string(),
            description: "Test Description".to_string(),
            proposal_type: ProposalType::General,
            quorum_required: dec!(100),
            approval_threshold: dec!(0.5),
            votes_for: dec!(300),
            votes_against: dec!(200),
            votes_abstain: dec!(100),
            status: ProposalStatus::Active,
            voting_start: Utc::now(),
            voting_end: Utc::now() + chrono::Duration::hours(72),
            execution_time: None,
            time_lock_seconds: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 600 / 2000 = 0.3 = 30%
        assert_eq!(proposal.participation_rate(dec!(2000)), dec!(30));
    }
}
