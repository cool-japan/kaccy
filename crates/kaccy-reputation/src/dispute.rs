//! Dispute Resolution system for commitment verification challenges
//!
//! This module allows token holders to dispute commitment verifications
//! and provides a DAO-based voting mechanism for resolution.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ReputationError, Result};

/// Dispute against a commitment verification
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Dispute {
    /// Unique identifier for this dispute.
    pub id: Uuid,
    /// Commitment being disputed.
    pub commitment_id: Uuid,
    /// User who raised the dispute.
    pub disputer_id: Uuid,
    /// Token associated with the disputed commitment.
    pub token_id: Uuid,
    /// Reason for the dispute.
    pub reason: String,
    /// Evidence supporting the dispute claim.
    pub evidence: String,
    /// String representation of the current status.
    pub status: String,
    /// Timestamp when the dispute was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the dispute was last updated.
    pub updated_at: DateTime<Utc>,
    /// When voting period ends
    pub voting_deadline: DateTime<Utc>,
    /// Final resolution if resolved
    pub resolution: Option<String>,
    /// Timestamp when the dispute was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Status of a dispute
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DisputeStatus {
    /// Waiting for votes
    Pending,
    /// Voting in progress
    Voting,
    /// Dispute was upheld (issuer was wrong)
    Upheld,
    /// Dispute was rejected (verification was correct)
    Rejected,
    /// Dispute was withdrawn by disputer
    Withdrawn,
    /// Expired without resolution
    Expired,
}

/// Vote on a dispute
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DisputeVote {
    /// Unique identifier for this vote.
    pub id: Uuid,
    /// Dispute this vote was cast on.
    pub dispute_id: Uuid,
    /// User who cast the vote.
    pub voter_id: Uuid,
    /// String representation of the vote choice.
    pub vote: String,
    /// Voting power (based on token holdings)
    pub voting_power: Decimal,
    /// Optional explanation for the vote.
    pub reason: Option<String>,
    /// Timestamp when the vote was cast.
    pub created_at: DateTime<Utc>,
}

/// Vote choice
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VoteChoice {
    /// Support the dispute (issuer was wrong)
    Uphold,
    /// Reject the dispute (verification was correct)
    Reject,
    /// Abstain from voting
    Abstain,
}

/// Appeal by an issuer against a negative decision
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Appeal {
    /// Unique identifier for this appeal.
    pub id: Uuid,
    /// Commitment whose dispute resolution is being appealed.
    pub commitment_id: Uuid,
    /// Token issuer filing the appeal.
    pub issuer_id: Uuid,
    /// Reason for the appeal.
    pub reason: String,
    /// Additional evidence presented in the appeal.
    pub additional_evidence: String,
    /// String representation of the current appeal status.
    pub status: String,
    /// Timestamp when the appeal was filed.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the appeal was last updated.
    pub updated_at: DateTime<Utc>,
    /// Appeal must be filed within this window
    pub appeal_deadline: DateTime<Utc>,
    /// Resolution text once the appeal is decided.
    pub resolution: Option<String>,
    /// Timestamp when the appeal was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Status of an appeal
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AppealStatus {
    /// Waiting for review
    Pending,
    /// Under review
    UnderReview,
    /// Appeal granted
    Granted,
    /// Appeal denied
    Denied,
    /// Appeal withdrawn
    Withdrawn,
    /// Appeal expired
    Expired,
}

/// Request to create a dispute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDisputeRequest {
    /// Commitment being disputed.
    pub commitment_id: Uuid,
    /// User raising the dispute.
    pub disputer_id: Uuid,
    /// Token associated with the commitment.
    pub token_id: Uuid,
    /// Reason for the dispute.
    pub reason: String,
    /// Evidence supporting the dispute claim.
    pub evidence: String,
}

/// Request to vote on a dispute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteDisputeRequest {
    /// Dispute being voted on.
    pub dispute_id: Uuid,
    /// User casting the vote.
    pub voter_id: Uuid,
    /// The vote choice.
    pub vote: VoteChoice,
    /// Optional explanation for the vote.
    pub reason: Option<String>,
}

/// Request to create an appeal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAppealRequest {
    /// Commitment whose dispute resolution is being appealed.
    pub commitment_id: Uuid,
    /// Token issuer filing the appeal.
    pub issuer_id: Uuid,
    /// Reason for the appeal.
    pub reason: String,
    /// Additional evidence to support the appeal.
    pub additional_evidence: String,
}

/// Dispute voting results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResults {
    /// Dispute these results belong to.
    pub dispute_id: Uuid,
    /// Total number of votes cast.
    pub total_votes: i32,
    /// Weighted voting power in favour of upholding the dispute.
    pub uphold_votes: Decimal,
    /// Weighted voting power in favour of rejecting the dispute.
    pub reject_votes: Decimal,
    /// Weighted voting power of abstentions.
    pub abstain_votes: Decimal,
    /// Total voting power across all cast votes.
    pub total_voting_power: Decimal,
    /// Final outcome of the dispute.
    pub outcome: DisputeOutcome,
}

/// Outcome of a dispute vote
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DisputeOutcome {
    /// Dispute upheld (issuer was wrong)
    Upheld,
    /// Dispute rejected (verification was correct)
    Rejected,
    /// Not enough votes to decide
    Inconclusive,
}

impl std::fmt::Display for DisputeOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upheld => write!(f, "Upheld"),
            Self::Rejected => write!(f, "Rejected"),
            Self::Inconclusive => write!(f, "Inconclusive"),
        }
    }
}

impl std::str::FromStr for DisputeOutcome {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "upheld" => Ok(DisputeOutcome::Upheld),
            "rejected" => Ok(DisputeOutcome::Rejected),
            "inconclusive" => Ok(DisputeOutcome::Inconclusive),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid dispute outcome: {}. Valid values: upheld, rejected, inconclusive",
                s
            ))),
        }
    }
}

/// Service for dispute resolution
pub struct DisputeService {
    pool: PgPool,
    /// Minimum voting power required for quorum
    quorum_threshold: Decimal,
    /// Voting period in hours
    voting_period_hours: i64,
    /// Appeal window in hours after negative decision
    appeal_window_hours: i64,
}

impl DisputeService {
    /// Create a new [`DisputeService`] with default quorum (1000 tokens), 72h voting, 48h appeal window.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            quorum_threshold: Decimal::new(1000, 0), // 1000 tokens
            voting_period_hours: 72,                 // 3 days
            appeal_window_hours: 48,                 // 2 days
        }
    }

    /// Create a [`DisputeService`] with custom quorum and time-window configuration.
    pub fn with_config(
        pool: PgPool,
        quorum_threshold: Decimal,
        voting_period_hours: i64,
        appeal_window_hours: i64,
    ) -> Self {
        Self {
            pool,
            quorum_threshold,
            voting_period_hours,
            appeal_window_hours,
        }
    }

    /// Create a new dispute
    pub async fn create_dispute(&self, request: CreateDisputeRequest) -> Result<Dispute> {
        // Verify disputer holds tokens
        let holdings = self
            .get_token_holdings(request.disputer_id, request.token_id)
            .await?;
        if holdings <= Decimal::ZERO {
            return Err(ReputationError::Validation(
                "Must hold tokens to dispute".to_string(),
            ));
        }

        let id = Uuid::new_v4();
        let voting_deadline = Utc::now() + chrono::Duration::hours(self.voting_period_hours);

        let dispute = sqlx::query_as::<_, Dispute>(
            r#"
            INSERT INTO disputes (
                id, commitment_id, disputer_id, token_id, reason, evidence,
                status, voting_deadline, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(request.commitment_id)
        .bind(request.disputer_id)
        .bind(request.token_id)
        .bind(request.reason)
        .bind(request.evidence)
        .bind(DisputeStatus::Voting.to_string())
        .bind(voting_deadline)
        .fetch_one(&self.pool)
        .await?;

        Ok(dispute)
    }

    /// Vote on a dispute
    pub async fn vote(&self, request: VoteDisputeRequest) -> Result<DisputeVote> {
        // Get dispute
        let dispute = self.get_dispute(request.dispute_id).await?;

        // Check if voting is still open
        if dispute.voting_deadline < Utc::now() {
            return Err(ReputationError::Validation(
                "Voting period has ended".to_string(),
            ));
        }

        // Get voter's token holdings (voting power)
        let token_id = sqlx::query_scalar::<_, Uuid>("SELECT token_id FROM disputes WHERE id = $1")
            .bind(request.dispute_id)
            .fetch_one(&self.pool)
            .await?;

        let voting_power = self.get_token_holdings(request.voter_id, token_id).await?;
        if voting_power <= Decimal::ZERO {
            return Err(ReputationError::Validation(
                "Must hold tokens to vote".to_string(),
            ));
        }

        // Check if already voted
        let existing = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM dispute_votes WHERE dispute_id = $1 AND voter_id = $2)",
        )
        .bind(request.dispute_id)
        .bind(request.voter_id)
        .fetch_one(&self.pool)
        .await?;

        if existing {
            return Err(ReputationError::Validation(
                "Already voted on this dispute".to_string(),
            ));
        }

        let vote_id = Uuid::new_v4();

        let vote = sqlx::query_as::<_, DisputeVote>(
            r#"
            INSERT INTO dispute_votes (
                id, dispute_id, voter_id, vote, voting_power, reason, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
            RETURNING *
            "#,
        )
        .bind(vote_id)
        .bind(request.dispute_id)
        .bind(request.voter_id)
        .bind(request.vote.to_string())
        .bind(voting_power)
        .bind(request.reason)
        .fetch_one(&self.pool)
        .await?;

        Ok(vote)
    }

    /// Get dispute by ID
    pub async fn get_dispute(&self, dispute_id: Uuid) -> Result<Dispute> {
        let dispute = sqlx::query_as::<_, Dispute>("SELECT * FROM disputes WHERE id = $1")
            .bind(dispute_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(dispute)
    }

    /// Calculate dispute results
    pub async fn calculate_results(&self, dispute_id: Uuid) -> Result<DisputeResults> {
        let votes =
            sqlx::query_as::<_, DisputeVote>("SELECT * FROM dispute_votes WHERE dispute_id = $1")
                .bind(dispute_id)
                .fetch_all(&self.pool)
                .await?;

        let mut uphold = Decimal::ZERO;
        let mut reject = Decimal::ZERO;
        let mut abstain = Decimal::ZERO;

        for vote in &votes {
            match vote.vote.as_str() {
                "uphold" => uphold += vote.voting_power,
                "reject" => reject += vote.voting_power,
                "abstain" => abstain += vote.voting_power,
                _ => {}
            }
        }

        let total_power = uphold + reject + abstain;
        let outcome = if total_power < self.quorum_threshold {
            DisputeOutcome::Inconclusive
        } else if uphold > reject {
            DisputeOutcome::Upheld
        } else {
            DisputeOutcome::Rejected
        };

        Ok(DisputeResults {
            dispute_id,
            total_votes: votes.len() as i32,
            uphold_votes: uphold,
            reject_votes: reject,
            abstain_votes: abstain,
            total_voting_power: total_power,
            outcome,
        })
    }

    /// Resolve a dispute (called after voting period ends)
    pub async fn resolve_dispute(&self, dispute_id: Uuid) -> Result<DisputeResults> {
        let results = self.calculate_results(dispute_id).await?;

        let status = match results.outcome {
            DisputeOutcome::Upheld => DisputeStatus::Upheld,
            DisputeOutcome::Rejected => DisputeStatus::Rejected,
            DisputeOutcome::Inconclusive => DisputeStatus::Expired,
        };

        let resolution = format!(
            "Resolved: {} (Uphold: {}, Reject: {}, Total Power: {})",
            status, results.uphold_votes, results.reject_votes, results.total_voting_power
        );

        sqlx::query(
            r#"
            UPDATE disputes
            SET status = $1, resolution = $2, resolved_at = NOW(), updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(status.to_string())
        .bind(&resolution)
        .bind(dispute_id)
        .execute(&self.pool)
        .await?;

        Ok(results)
    }

    /// Create an appeal
    pub async fn create_appeal(&self, request: CreateAppealRequest) -> Result<Appeal> {
        let id = Uuid::new_v4();
        let appeal_deadline = Utc::now() + chrono::Duration::hours(self.appeal_window_hours);

        let appeal = sqlx::query_as::<_, Appeal>(
            r#"
            INSERT INTO appeals (
                id, commitment_id, issuer_id, reason, additional_evidence,
                status, appeal_deadline, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(request.commitment_id)
        .bind(request.issuer_id)
        .bind(request.reason)
        .bind(request.additional_evidence)
        .bind(AppealStatus::Pending.to_string())
        .bind(appeal_deadline)
        .fetch_one(&self.pool)
        .await?;

        Ok(appeal)
    }

    /// Review an appeal (admin action)
    pub async fn review_appeal(
        &self,
        appeal_id: Uuid,
        granted: bool,
        resolution: String,
    ) -> Result<Appeal> {
        let status = if granted {
            AppealStatus::Granted
        } else {
            AppealStatus::Denied
        };

        let appeal = sqlx::query_as::<_, Appeal>(
            r#"
            UPDATE appeals
            SET status = $1, resolution = $2, resolved_at = NOW(), updated_at = NOW()
            WHERE id = $3
            RETURNING *
            "#,
        )
        .bind(status.to_string())
        .bind(resolution)
        .bind(appeal_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(appeal)
    }

    /// Get active disputes for a token
    pub async fn get_active_disputes(&self, token_id: Uuid) -> Result<Vec<Dispute>> {
        let disputes = sqlx::query_as::<_, Dispute>(
            r#"
            SELECT * FROM disputes
            WHERE token_id = $1 AND status = $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(token_id)
        .bind(DisputeStatus::Voting.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(disputes)
    }

    /// Get pending appeals
    pub async fn get_pending_appeals(&self) -> Result<Vec<Appeal>> {
        let appeals = sqlx::query_as::<_, Appeal>(
            r#"
            SELECT * FROM appeals
            WHERE status = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(AppealStatus::Pending.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(appeals)
    }

    // Helper methods

    async fn get_token_holdings(&self, user_id: Uuid, token_id: Uuid) -> Result<Decimal> {
        let balance = sqlx::query_scalar::<_, Decimal>(
            "SELECT COALESCE(balance, 0) FROM balances WHERE user_id = $1 AND token_id = $2",
        )
        .bind(user_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(Decimal::ZERO);

        Ok(balance)
    }
}

impl std::fmt::Display for DisputeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisputeStatus::Pending => write!(f, "pending"),
            DisputeStatus::Voting => write!(f, "voting"),
            DisputeStatus::Upheld => write!(f, "upheld"),
            DisputeStatus::Rejected => write!(f, "rejected"),
            DisputeStatus::Withdrawn => write!(f, "withdrawn"),
            DisputeStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for DisputeStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(DisputeStatus::Pending),
            "voting" => Ok(DisputeStatus::Voting),
            "upheld" => Ok(DisputeStatus::Upheld),
            "rejected" => Ok(DisputeStatus::Rejected),
            "withdrawn" => Ok(DisputeStatus::Withdrawn),
            "expired" => Ok(DisputeStatus::Expired),
            _ => Err(format!("Invalid dispute status: {}", s)),
        }
    }
}

impl TryFrom<String> for DisputeStatus {
    type Error = String;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for DisputeStatus {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl std::fmt::Display for VoteChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoteChoice::Uphold => write!(f, "uphold"),
            VoteChoice::Reject => write!(f, "reject"),
            VoteChoice::Abstain => write!(f, "abstain"),
        }
    }
}

impl std::str::FromStr for VoteChoice {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "uphold" => Ok(VoteChoice::Uphold),
            "reject" => Ok(VoteChoice::Reject),
            "abstain" => Ok(VoteChoice::Abstain),
            _ => Err(format!("Invalid vote choice: {}", s)),
        }
    }
}

impl TryFrom<String> for VoteChoice {
    type Error = String;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for VoteChoice {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl std::fmt::Display for AppealStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppealStatus::Pending => write!(f, "pending"),
            AppealStatus::UnderReview => write!(f, "under_review"),
            AppealStatus::Granted => write!(f, "granted"),
            AppealStatus::Denied => write!(f, "denied"),
            AppealStatus::Withdrawn => write!(f, "withdrawn"),
            AppealStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for AppealStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(AppealStatus::Pending),
            "under_review" => Ok(AppealStatus::UnderReview),
            "granted" => Ok(AppealStatus::Granted),
            "denied" => Ok(AppealStatus::Denied),
            "withdrawn" => Ok(AppealStatus::Withdrawn),
            "expired" => Ok(AppealStatus::Expired),
            _ => Err(format!("Invalid appeal status: {}", s)),
        }
    }
}

impl TryFrom<String> for AppealStatus {
    type Error = String;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for AppealStatus {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispute_status_display() {
        assert_eq!(DisputeStatus::Pending.to_string(), "pending");
        assert_eq!(DisputeStatus::Voting.to_string(), "voting");
        assert_eq!(DisputeStatus::Upheld.to_string(), "upheld");
    }

    #[test]
    fn test_vote_choice_display() {
        assert_eq!(VoteChoice::Uphold.to_string(), "uphold");
        assert_eq!(VoteChoice::Reject.to_string(), "reject");
        assert_eq!(VoteChoice::Abstain.to_string(), "abstain");
    }

    #[test]
    fn test_appeal_status_display() {
        assert_eq!(AppealStatus::Pending.to_string(), "pending");
        assert_eq!(AppealStatus::Granted.to_string(), "granted");
        assert_eq!(AppealStatus::Denied.to_string(), "denied");
    }

    #[test]
    fn test_dispute_outcome_display() {
        assert_eq!(DisputeOutcome::Upheld.to_string(), "Upheld");
        assert_eq!(DisputeOutcome::Rejected.to_string(), "Rejected");
        assert_eq!(DisputeOutcome::Inconclusive.to_string(), "Inconclusive");
    }

    #[test]
    fn test_dispute_outcome_from_str_valid() {
        assert_eq!(
            "upheld".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Upheld
        );
        assert_eq!(
            "rejected".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Rejected
        );
        assert_eq!(
            "inconclusive".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Inconclusive
        );
    }

    #[test]
    fn test_dispute_outcome_from_str_case_insensitive() {
        assert_eq!(
            "UPHELD".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Upheld
        );
        assert_eq!(
            "Rejected".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Rejected
        );
        assert_eq!(
            "INCONCLUSIVE".parse::<DisputeOutcome>().unwrap(),
            DisputeOutcome::Inconclusive
        );
    }

    #[test]
    fn test_dispute_outcome_from_str_invalid() {
        assert!("invalid".parse::<DisputeOutcome>().is_err());
        assert!("unknown".parse::<DisputeOutcome>().is_err());
        assert!("".parse::<DisputeOutcome>().is_err());
    }
}
