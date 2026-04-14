//! Domain event types for the Kaccy protocol

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Base event trait that all domain events must implement
pub trait DomainEvent: Send + Sync {
    /// Get the event type name
    fn event_type(&self) -> &'static str;

    /// Get the timestamp when the event occurred
    fn occurred_at(&self) -> DateTime<Utc>;

    /// Get the aggregate ID (entity that caused this event)
    fn aggregate_id(&self) -> Uuid;

    /// Get optional user ID who triggered this event
    fn user_id(&self) -> Option<Uuid> {
        None
    }
}

/// Wrapper for all domain events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "data")]
pub enum Event {
    // User events
    /// A new user has registered on the platform
    UserRegistered(UserRegisteredEvent),
    /// An existing user's profile was updated
    UserUpdated(UserUpdatedEvent),
    /// A user account was deleted
    UserDeleted(UserDeletedEvent),

    // Token events
    /// A new token was created on the platform
    TokenCreated(TokenCreatedEvent),
    /// An existing token's metadata was updated
    TokenUpdated(TokenUpdatedEvent),
    /// A token was paused and trading was suspended
    TokenPaused(TokenPausedEvent),
    /// A previously paused token was resumed
    TokenResumed(TokenResumedEvent),

    // Order events
    /// A new trade order was placed
    OrderCreated(OrderCreatedEvent),
    /// An order was cancelled before being filled
    OrderCancelled(OrderCancelledEvent),
    /// An order was fully or partially filled
    OrderFilled(OrderFilledEvent),
    /// An order expired without being filled
    OrderExpired(OrderExpiredEvent),

    // Trade events
    /// A trade was executed between buyer and seller
    TradeExecuted(TradeExecutedEvent),

    // Payment events
    /// A new payment request was created
    PaymentCreated(PaymentCreatedEvent),
    /// An on-chain payment transaction was detected
    PaymentDetected(PaymentDetectedEvent),
    /// A payment reached the required confirmations
    PaymentConfirmed(PaymentConfirmedEvent),
    /// A payment was fully completed and settled
    PaymentCompleted(PaymentCompletedEvent),
    /// A payment expired before completion
    PaymentExpired(PaymentExpiredEvent),

    // Reputation events
    /// A new user commitment was created
    CommitmentCreated(CommitmentCreatedEvent),
    /// A commitment was successfully fulfilled
    CommitmentFulfilled(CommitmentFulfilledEvent),
    /// A commitment failed or was not met by the deadline
    CommitmentFailed(CommitmentFailedEvent),
    /// A user's reputation score changed
    ReputationChanged(ReputationChangedEvent),

    // KYC events
    /// A KYC verification submission was received
    KycSubmitted(KycSubmittedEvent),
    /// A KYC submission was approved
    KycApproved(KycApprovedEvent),
    /// A KYC submission was rejected
    KycRejected(KycRejectedEvent),

    // Governance events
    /// A new governance proposal was created
    ProposalCreated(ProposalCreatedEvent),
    /// A vote was cast on a governance proposal
    VoteCast(VoteCastEvent),
    /// A governance proposal was executed
    ProposalExecuted(ProposalExecutedEvent),

    // Security events
    /// A security alert was triggered by the system
    SecurityAlert(SecurityAlertEvent),
    /// A circuit breaker was triggered for a token
    CircuitBreakerTriggered(CircuitBreakerTriggeredEvent),
}

impl Event {
    /// Get the underlying event type name
    pub fn event_type(&self) -> &'static str {
        match self {
            Event::UserRegistered(_) => "user_registered",
            Event::UserUpdated(_) => "user_updated",
            Event::UserDeleted(_) => "user_deleted",
            Event::TokenCreated(_) => "token_created",
            Event::TokenUpdated(_) => "token_updated",
            Event::TokenPaused(_) => "token_paused",
            Event::TokenResumed(_) => "token_resumed",
            Event::OrderCreated(_) => "order_created",
            Event::OrderCancelled(_) => "order_cancelled",
            Event::OrderFilled(_) => "order_filled",
            Event::OrderExpired(_) => "order_expired",
            Event::TradeExecuted(_) => "trade_executed",
            Event::PaymentCreated(_) => "payment_created",
            Event::PaymentDetected(_) => "payment_detected",
            Event::PaymentConfirmed(_) => "payment_confirmed",
            Event::PaymentCompleted(_) => "payment_completed",
            Event::PaymentExpired(_) => "payment_expired",
            Event::CommitmentCreated(_) => "commitment_created",
            Event::CommitmentFulfilled(_) => "commitment_fulfilled",
            Event::CommitmentFailed(_) => "commitment_failed",
            Event::ReputationChanged(_) => "reputation_changed",
            Event::KycSubmitted(_) => "kyc_submitted",
            Event::KycApproved(_) => "kyc_approved",
            Event::KycRejected(_) => "kyc_rejected",
            Event::ProposalCreated(_) => "proposal_created",
            Event::VoteCast(_) => "vote_cast",
            Event::ProposalExecuted(_) => "proposal_executed",
            Event::SecurityAlert(_) => "security_alert",
            Event::CircuitBreakerTriggered(_) => "circuit_breaker_triggered",
        }
    }

    /// Get the timestamp when this event occurred
    pub fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Event::UserRegistered(e) => e.occurred_at,
            Event::UserUpdated(e) => e.occurred_at,
            Event::UserDeleted(e) => e.occurred_at,
            Event::TokenCreated(e) => e.occurred_at,
            Event::TokenUpdated(e) => e.occurred_at,
            Event::TokenPaused(e) => e.occurred_at,
            Event::TokenResumed(e) => e.occurred_at,
            Event::OrderCreated(e) => e.occurred_at,
            Event::OrderCancelled(e) => e.occurred_at,
            Event::OrderFilled(e) => e.occurred_at,
            Event::OrderExpired(e) => e.occurred_at,
            Event::TradeExecuted(e) => e.occurred_at,
            Event::PaymentCreated(e) => e.occurred_at,
            Event::PaymentDetected(e) => e.occurred_at,
            Event::PaymentConfirmed(e) => e.occurred_at,
            Event::PaymentCompleted(e) => e.occurred_at,
            Event::PaymentExpired(e) => e.occurred_at,
            Event::CommitmentCreated(e) => e.occurred_at,
            Event::CommitmentFulfilled(e) => e.occurred_at,
            Event::CommitmentFailed(e) => e.occurred_at,
            Event::ReputationChanged(e) => e.occurred_at,
            Event::KycSubmitted(e) => e.occurred_at,
            Event::KycApproved(e) => e.occurred_at,
            Event::KycRejected(e) => e.occurred_at,
            Event::ProposalCreated(e) => e.occurred_at,
            Event::VoteCast(e) => e.occurred_at,
            Event::ProposalExecuted(e) => e.occurred_at,
            Event::SecurityAlert(e) => e.occurred_at,
            Event::CircuitBreakerTriggered(e) => e.occurred_at,
        }
    }
}

// User Events

/// Event emitted when a new user registers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisteredEvent {
    /// Unique identifier of the registered user
    pub user_id: Uuid,
    /// Username chosen by the user
    pub username: String,
    /// Email address of the user
    pub email: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when user profile data is updated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdatedEvent {
    /// Unique identifier of the updated user
    pub user_id: Uuid,
    /// Names of the fields that were changed
    pub fields_changed: Vec<String>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a user account is deleted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeletedEvent {
    /// Unique identifier of the deleted user
    pub user_id: Uuid,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Token Events

/// Event emitted when a new token is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreatedEvent {
    /// Unique identifier of the created token
    pub token_id: Uuid,
    /// Unique identifier of the token issuer
    pub issuer_id: Uuid,
    /// Ticker symbol of the token
    pub symbol: String,
    /// Full name of the token
    pub name: String,
    /// Initial token supply at creation
    pub initial_supply: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a token is updated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdatedEvent {
    /// Unique identifier of the updated token
    pub token_id: Uuid,
    /// User who performed the update
    pub updated_by: Uuid,
    /// Names of the fields that were changed
    pub fields_changed: Vec<String>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a token is paused
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPausedEvent {
    /// Unique identifier of the paused token
    pub token_id: Uuid,
    /// User who paused the token
    pub paused_by: Uuid,
    /// Optional reason for pausing
    pub reason: Option<String>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a paused token is resumed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResumedEvent {
    /// Unique identifier of the resumed token
    pub token_id: Uuid,
    /// User who resumed the token
    pub resumed_by: Uuid,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Order Events

/// Event emitted when a new order is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    /// Unique identifier of the created order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token being traded
    pub token_id: Uuid,
    /// Type of order (buy/sell/limit/market)
    pub order_type: String,
    /// Order amount
    pub amount: Decimal,
    /// Optional limit price
    pub price: Option<Decimal>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when an order is cancelled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancelledEvent {
    /// Unique identifier of the cancelled order
    pub order_id: Uuid,
    /// User who cancelled the order
    pub user_id: Uuid,
    /// Token associated with the order
    pub token_id: Uuid,
    /// Optional reason for cancellation
    pub reason: Option<String>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when an order is filled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFilledEvent {
    /// Unique identifier of the filled order
    pub order_id: Uuid,
    /// User whose order was filled
    pub user_id: Uuid,
    /// Token that was traded
    pub token_id: Uuid,
    /// Amount that was filled
    pub filled_amount: Decimal,
    /// Price at which the order was filled
    pub filled_price: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when an order expires without being filled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderExpiredEvent {
    /// Unique identifier of the expired order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token associated with the order
    pub token_id: Uuid,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Trade Events

/// Event emitted when a trade is executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecutedEvent {
    /// Unique identifier of the executed trade
    pub trade_id: Uuid,
    /// User who bought in this trade
    pub buyer_id: Uuid,
    /// User who sold in this trade
    pub seller_id: Uuid,
    /// Token that was traded
    pub token_id: Uuid,
    /// Amount traded
    pub amount: Decimal,
    /// Execution price
    pub price: Decimal,
    /// Total value of the trade
    pub total_value: Decimal,
    /// Platform fee collected
    pub platform_fee: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Payment Events

/// Event emitted when a payment is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCreatedEvent {
    /// Unique identifier of the payment
    pub payment_id: Uuid,
    /// User making the payment
    pub user_id: Uuid,
    /// Order associated with this payment
    pub order_id: Uuid,
    /// Bitcoin address for the payment
    pub btc_address: String,
    /// Expected payment amount in BTC
    pub expected_amount: Decimal,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a payment transaction is detected on-chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDetectedEvent {
    /// Unique identifier of the payment
    pub payment_id: Uuid,
    /// User associated with the payment
    pub user_id: Uuid,
    /// Bitcoin transaction ID
    pub txid: String,
    /// Detected payment amount
    pub amount: Decimal,
    /// Number of blockchain confirmations
    pub confirmations: i32,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a payment reaches required confirmations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentConfirmedEvent {
    /// Unique identifier of the payment
    pub payment_id: Uuid,
    /// User associated with the payment
    pub user_id: Uuid,
    /// Bitcoin transaction ID
    pub txid: String,
    /// Confirmed payment amount
    pub amount: Decimal,
    /// Number of blockchain confirmations
    pub confirmations: i32,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a payment is fully completed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCompletedEvent {
    /// Unique identifier of the payment
    pub payment_id: Uuid,
    /// User who made the payment
    pub user_id: Uuid,
    /// Order fulfilled by this payment
    pub order_id: Uuid,
    /// Final payment amount
    pub amount: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a payment expires without completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentExpiredEvent {
    /// Unique identifier of the expired payment
    pub payment_id: Uuid,
    /// User associated with the payment
    pub user_id: Uuid,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Reputation Events

/// Event emitted when a new commitment is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentCreatedEvent {
    /// Unique identifier of the commitment
    pub commitment_id: Uuid,
    /// User who created the commitment
    pub user_id: Uuid,
    /// Token associated with the commitment
    pub token_id: Uuid,
    /// Title of the commitment
    pub title: String,
    /// Deadline by which the commitment must be fulfilled
    pub deadline: DateTime<Utc>,
    /// Expected reputation impact on fulfillment
    pub reputation_impact: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a commitment is successfully fulfilled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFulfilledEvent {
    /// Unique identifier of the fulfilled commitment
    pub commitment_id: Uuid,
    /// User who fulfilled the commitment
    pub user_id: Uuid,
    /// Token associated with the commitment
    pub token_id: Uuid,
    /// Change in reputation score
    pub reputation_change: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a commitment fails or is not fulfilled by the deadline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFailedEvent {
    /// Unique identifier of the failed commitment
    pub commitment_id: Uuid,
    /// User who failed the commitment
    pub user_id: Uuid,
    /// Token associated with the commitment
    pub token_id: Uuid,
    /// Change in reputation score (negative)
    pub reputation_change: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a user's reputation score changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationChangedEvent {
    /// User whose reputation changed
    pub user_id: Uuid,
    /// Previous reputation score
    pub old_score: Decimal,
    /// New reputation score
    pub new_score: Decimal,
    /// Net change in score
    pub change: Decimal,
    /// Reason for the reputation change
    pub reason: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// KYC Events

/// Event emitted when a KYC submission is made
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycSubmittedEvent {
    /// Unique identifier of the KYC submission
    pub submission_id: Uuid,
    /// User who submitted for KYC
    pub user_id: Uuid,
    /// Full legal name of the user
    pub full_name: String,
    /// Country of residence or citizenship
    pub country: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a KYC submission is approved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycApprovedEvent {
    /// Unique identifier of the KYC submission
    pub submission_id: Uuid,
    /// User whose KYC was approved
    pub user_id: Uuid,
    /// Admin who reviewed and approved the submission
    pub reviewed_by: Uuid,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a KYC submission is rejected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycRejectedEvent {
    /// Unique identifier of the KYC submission
    pub submission_id: Uuid,
    /// User whose KYC was rejected
    pub user_id: Uuid,
    /// Admin who reviewed and rejected the submission
    pub reviewed_by: Uuid,
    /// Reason for rejection
    pub reason: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Governance Events

/// Event emitted when a governance proposal is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalCreatedEvent {
    /// Unique identifier of the proposal
    pub proposal_id: Uuid,
    /// User who submitted the proposal
    pub proposer_id: Uuid,
    /// Title of the proposal
    pub title: String,
    /// Type classification of the proposal
    pub proposal_type: String,
    /// Timestamp when voting closes
    pub voting_ends_at: DateTime<Utc>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a vote is cast on a governance proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteCastEvent {
    /// Unique identifier of the vote
    pub vote_id: Uuid,
    /// Proposal being voted on
    pub proposal_id: Uuid,
    /// User who cast the vote
    pub voter_id: Uuid,
    /// Whether the vote is in favour of the proposal
    pub vote_for: bool,
    /// Voting power of this vote
    pub voting_power: Decimal,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a governance proposal is executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalExecutedEvent {
    /// Unique identifier of the executed proposal
    pub proposal_id: Uuid,
    /// User who executed the proposal
    pub executed_by: Uuid,
    /// Result of the execution
    pub result: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

// Security Events

/// Event emitted when a security alert is triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlertEvent {
    /// Unique identifier of the alert
    pub alert_id: Uuid,
    /// Optional user associated with the alert
    pub user_id: Option<Uuid>,
    /// Type of security alert
    pub alert_type: String,
    /// Severity level of the alert
    pub severity: String,
    /// Human-readable description of the alert
    pub description: String,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

/// Event emitted when a circuit breaker is triggered for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerTriggeredEvent {
    /// Token for which the circuit breaker was triggered
    pub token_id: Uuid,
    /// Reason the circuit breaker fired
    pub reason: String,
    /// Optional user or system that triggered the circuit breaker
    pub triggered_by: Option<Uuid>,
    /// Timestamp until which trading is suspended
    pub cooldown_until: DateTime<Utc>,
    /// Timestamp when this event occurred
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();

        assert_eq!(event.event_type(), deserialized.event_type());
    }

    #[test]
    fn test_event_type_names() {
        let event = Event::TokenCreated(TokenCreatedEvent {
            token_id: Uuid::new_v4(),
            issuer_id: Uuid::new_v4(),
            symbol: "$TEST".to_string(),
            name: "Test Token".to_string(),
            initial_supply: rust_decimal_macros::dec!(1000000),
            occurred_at: Utc::now(),
        });

        assert_eq!(event.event_type(), "token_created");
    }
}
