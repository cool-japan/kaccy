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
    UserRegistered(UserRegisteredEvent),
    UserUpdated(UserUpdatedEvent),
    UserDeleted(UserDeletedEvent),

    // Token events
    TokenCreated(TokenCreatedEvent),
    TokenUpdated(TokenUpdatedEvent),
    TokenPaused(TokenPausedEvent),
    TokenResumed(TokenResumedEvent),

    // Order events
    OrderCreated(OrderCreatedEvent),
    OrderCancelled(OrderCancelledEvent),
    OrderFilled(OrderFilledEvent),
    OrderExpired(OrderExpiredEvent),

    // Trade events
    TradeExecuted(TradeExecutedEvent),

    // Payment events
    PaymentCreated(PaymentCreatedEvent),
    PaymentDetected(PaymentDetectedEvent),
    PaymentConfirmed(PaymentConfirmedEvent),
    PaymentCompleted(PaymentCompletedEvent),
    PaymentExpired(PaymentExpiredEvent),

    // Reputation events
    CommitmentCreated(CommitmentCreatedEvent),
    CommitmentFulfilled(CommitmentFulfilledEvent),
    CommitmentFailed(CommitmentFailedEvent),
    ReputationChanged(ReputationChangedEvent),

    // KYC events
    KycSubmitted(KycSubmittedEvent),
    KycApproved(KycApprovedEvent),
    KycRejected(KycRejectedEvent),

    // Governance events
    ProposalCreated(ProposalCreatedEvent),
    VoteCast(VoteCastEvent),
    ProposalExecuted(ProposalExecutedEvent),

    // Security events
    SecurityAlert(SecurityAlertEvent),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisteredEvent {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdatedEvent {
    pub user_id: Uuid,
    pub fields_changed: Vec<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeletedEvent {
    pub user_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

// Token Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreatedEvent {
    pub token_id: Uuid,
    pub issuer_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub initial_supply: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUpdatedEvent {
    pub token_id: Uuid,
    pub updated_by: Uuid,
    pub fields_changed: Vec<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPausedEvent {
    pub token_id: Uuid,
    pub paused_by: Uuid,
    pub reason: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResumedEvent {
    pub token_id: Uuid,
    pub resumed_by: Uuid,
    pub occurred_at: DateTime<Utc>,
}

// Order Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub order_type: String,
    pub amount: Decimal,
    pub price: Option<Decimal>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancelledEvent {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub reason: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFilledEvent {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub filled_amount: Decimal,
    pub filled_price: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderExpiredEvent {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

// Trade Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecutedEvent {
    pub trade_id: Uuid,
    pub buyer_id: Uuid,
    pub seller_id: Uuid,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub price: Decimal,
    pub total_value: Decimal,
    pub platform_fee: Decimal,
    pub occurred_at: DateTime<Utc>,
}

// Payment Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCreatedEvent {
    pub payment_id: Uuid,
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub btc_address: String,
    pub expected_amount: Decimal,
    pub expires_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDetectedEvent {
    pub payment_id: Uuid,
    pub user_id: Uuid,
    pub txid: String,
    pub amount: Decimal,
    pub confirmations: i32,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentConfirmedEvent {
    pub payment_id: Uuid,
    pub user_id: Uuid,
    pub txid: String,
    pub amount: Decimal,
    pub confirmations: i32,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentCompletedEvent {
    pub payment_id: Uuid,
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub amount: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentExpiredEvent {
    pub payment_id: Uuid,
    pub user_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

// Reputation Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentCreatedEvent {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub deadline: DateTime<Utc>,
    pub reputation_impact: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFulfilledEvent {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub reputation_change: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentFailedEvent {
    pub commitment_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub reputation_change: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationChangedEvent {
    pub user_id: Uuid,
    pub old_score: Decimal,
    pub new_score: Decimal,
    pub change: Decimal,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
}

// KYC Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycSubmittedEvent {
    pub submission_id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub country: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycApprovedEvent {
    pub submission_id: Uuid,
    pub user_id: Uuid,
    pub reviewed_by: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycRejectedEvent {
    pub submission_id: Uuid,
    pub user_id: Uuid,
    pub reviewed_by: Uuid,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
}

// Governance Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalCreatedEvent {
    pub proposal_id: Uuid,
    pub proposer_id: Uuid,
    pub title: String,
    pub proposal_type: String,
    pub voting_ends_at: DateTime<Utc>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteCastEvent {
    pub vote_id: Uuid,
    pub proposal_id: Uuid,
    pub voter_id: Uuid,
    pub vote_for: bool,
    pub voting_power: Decimal,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalExecutedEvent {
    pub proposal_id: Uuid,
    pub executed_by: Uuid,
    pub result: String,
    pub occurred_at: DateTime<Utc>,
}

// Security Events

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAlertEvent {
    pub alert_id: Uuid,
    pub user_id: Option<Uuid>,
    pub alert_type: String,
    pub severity: String,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerTriggeredEvent {
    pub token_id: Uuid,
    pub reason: String,
    pub triggered_by: Option<Uuid>,
    pub cooldown_until: DateTime<Utc>,
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
