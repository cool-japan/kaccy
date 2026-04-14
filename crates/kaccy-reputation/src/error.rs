//! Reputation error types

use rust_decimal::Decimal;
use thiserror::Error;

/// All error types for the reputation system.
#[derive(Error, Debug)]
pub enum ReputationError {
    // Database errors
    /// Underlying database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    // Validation errors
    /// A field or parameter failed validation.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Supplied rating value is outside the allowed range.
    #[error("Invalid rating: {0}")]
    InvalidRating(String),

    /// Score delta exceeds the maximum allowed single-event change.
    #[error("Invalid score delta: {delta}, max allowed: {max_allowed}")]
    InvalidScoreDelta {
        /// The supplied delta value.
        delta: Decimal,
        /// The maximum permitted delta for a single event.
        max_allowed: Decimal,
    },

    /// An entity cannot transition to the requested state from its current state.
    #[error("Invalid state transition: from {from} to {to}")]
    InvalidStateTransition {
        /// Current state.
        from: String,
        /// Attempted target state.
        to: String,
    },

    // Not found errors
    /// Requested user was not found.
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// Requested commitment was not found.
    #[error("Commitment not found: {0}")]
    CommitmentNotFound(String),

    /// Requested token was not found.
    #[error("Token not found: {0}")]
    TokenNotFound(String),

    /// Requested dispute was not found.
    #[error("Dispute not found: {0}")]
    DisputeNotFound(String),

    /// No SBT exists for the requested user.
    #[error("SBT not found for user: {0}")]
    SBTNotFound(String),

    // Authorization errors
    /// The caller lacks permission to perform this action.
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// A token holder attempted to rate a token they issued.
    #[error("Cannot rate own token")]
    CannotRateOwnToken,

    /// Token holder's balance is below the minimum required to submit a rating.
    #[error("Insufficient balance to rate: required {required}, current {current}")]
    InsufficientBalance {
        /// Minimum balance required to rate.
        required: Decimal,
        /// Caller's current balance.
        current: Decimal,
    },

    /// Token holder has not held the token long enough to rate.
    #[error("Insufficient holding duration: required {required_hours}h, held for {held_hours}h")]
    InsufficientHoldingDuration {
        /// Minimum holding duration required in hours.
        required_hours: i64,
        /// How long the caller has held the token in hours.
        held_hours: i64,
    },

    // Reputation errors
    /// User's reputation triggered the circuit breaker, blocking the operation.
    #[error("Circuit breaker triggered for user: {0}")]
    CircuitBreakerTriggered(String),

    /// User's reputation score is below the minimum required for this operation.
    #[error("Insufficient reputation: required {required}, current {current}")]
    InsufficientReputation {
        /// Required reputation score.
        required: Decimal,
        /// User's current reputation score.
        current: Decimal,
    },

    /// User is not permitted to issue tokens due to reputation constraints.
    #[error("Cannot issue tokens: {0}")]
    CannotIssueTokens(String),

    // Commitment errors
    /// The commitment has already been verified and cannot be verified again.
    #[error("Commitment already verified")]
    CommitmentAlreadyVerified,

    /// The commitment's deadline has passed.
    #[error("Commitment deadline expired")]
    CommitmentDeadlineExpired,

    /// Evidence must be submitted before this commitment can be verified.
    #[error("Evidence required for commitment verification")]
    EvidenceRequired,

    // Dispute errors
    /// The dispute has already been resolved and cannot be modified.
    #[error("Dispute already resolved")]
    DisputeAlreadyResolved,

    /// The dispute's voting period has ended; no more votes can be cast.
    #[error("Voting period ended")]
    VotingPeriodEnded,

    /// The dispute's voting period has not yet ended.
    #[error("Voting period not yet ended")]
    VotingPeriodNotEnded,

    /// User has already cast a vote on this dispute.
    #[error("User already voted on this dispute")]
    AlreadyVoted,

    /// User's voting power is below the minimum required to vote on this dispute.
    #[error("Insufficient voting power: required {required}, current {current}")]
    InsufficientVotingPower {
        /// Required voting power.
        required: Decimal,
        /// User's current voting power.
        current: Decimal,
    },

    // SBT errors
    /// SBT already minted for this user; minting is idempotent.
    #[error("SBT already minted for user: {0}")]
    SBTAlreadyMinted(String),

    /// SBT has not yet been minted for this user.
    #[error("SBT not yet minted for user: {0}")]
    SBTNotMinted(String),
}

/// Convenience alias for `std::result::Result<T, ReputationError>`.
pub type Result<T> = std::result::Result<T, ReputationError>;
