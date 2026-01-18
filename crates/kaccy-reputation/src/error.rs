//! Reputation error types

use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReputationError {
    // Database errors
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    // Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid rating: {0}")]
    InvalidRating(String),

    #[error("Invalid score delta: {delta}, max allowed: {max_allowed}")]
    InvalidScoreDelta {
        delta: Decimal,
        max_allowed: Decimal,
    },

    #[error("Invalid state transition: from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    // Not found errors
    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Commitment not found: {0}")]
    CommitmentNotFound(String),

    #[error("Token not found: {0}")]
    TokenNotFound(String),

    #[error("Dispute not found: {0}")]
    DisputeNotFound(String),

    #[error("SBT not found for user: {0}")]
    SBTNotFound(String),

    // Authorization errors
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Cannot rate own token")]
    CannotRateOwnToken,

    #[error("Insufficient balance to rate: required {required}, current {current}")]
    InsufficientBalance { required: Decimal, current: Decimal },

    #[error("Insufficient holding duration: required {required_hours}h, held for {held_hours}h")]
    InsufficientHoldingDuration {
        required_hours: i64,
        held_hours: i64,
    },

    // Reputation errors
    #[error("Circuit breaker triggered for user: {0}")]
    CircuitBreakerTriggered(String),

    #[error("Insufficient reputation: required {required}, current {current}")]
    InsufficientReputation { required: Decimal, current: Decimal },

    #[error("Cannot issue tokens: {0}")]
    CannotIssueTokens(String),

    // Commitment errors
    #[error("Commitment already verified")]
    CommitmentAlreadyVerified,

    #[error("Commitment deadline expired")]
    CommitmentDeadlineExpired,

    #[error("Evidence required for commitment verification")]
    EvidenceRequired,

    // Dispute errors
    #[error("Dispute already resolved")]
    DisputeAlreadyResolved,

    #[error("Voting period ended")]
    VotingPeriodEnded,

    #[error("Voting period not yet ended")]
    VotingPeriodNotEnded,

    #[error("User already voted on this dispute")]
    AlreadyVoted,

    #[error("Insufficient voting power: required {required}, current {current}")]
    InsufficientVotingPower { required: Decimal, current: Decimal },

    // SBT errors
    #[error("SBT already minted for user: {0}")]
    SBTAlreadyMinted(String),

    #[error("SBT not yet minted for user: {0}")]
    SBTNotMinted(String),
}

pub type Result<T> = std::result::Result<T, ReputationError>;
