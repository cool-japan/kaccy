use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an event
pub type EventId = Uuid;

/// Event metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Event ID
    pub id: EventId,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event source/origin
    pub source: String,
    /// Correlation ID for tracing
    pub correlation_id: Option<String>,
    /// User ID if applicable
    pub user_id: Option<Uuid>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: "kaccy-api".to_string(),
            correlation_id: None,
            user_id: None,
        }
    }
}

/// Event types in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventType {
    // User events
    UserRegistered {
        user_id: Uuid,
        email: String,
    },
    UserUpdated {
        user_id: Uuid,
        fields: Vec<String>,
    },
    UserDeleted {
        user_id: Uuid,
    },

    // Token events
    TokenCreated {
        token_id: Uuid,
        symbol: String,
        issuer_id: Uuid,
    },
    TokenUpdated {
        token_id: Uuid,
        fields: Vec<String>,
    },

    // Order events
    OrderCreated {
        order_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
        order_type: String,
        amount: String,
    },
    OrderCompleted {
        order_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
    },
    OrderCancelled {
        order_id: Uuid,
        user_id: Uuid,
        reason: String,
    },

    // Trade events
    TradeExecuted {
        trade_id: Uuid,
        token_id: Uuid,
        buyer_id: Uuid,
        seller_id: Option<Uuid>,
        amount: String,
        price_btc: String,
    },

    // Commitment events
    CommitmentCreated {
        commitment_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
        title: String,
    },
    CommitmentEvidenceSubmitted {
        commitment_id: Uuid,
        user_id: Uuid,
    },
    CommitmentVerified {
        commitment_id: Uuid,
        admin_id: Uuid,
        approved: bool,
        quality_score: Option<i32>,
    },

    // KYC events
    KycSubmitted {
        user_id: Uuid,
        level: String,
    },
    KycVerified {
        user_id: Uuid,
        admin_id: Uuid,
        approved: bool,
    },

    // Security events
    LoginAttempt {
        user_id: Option<Uuid>,
        email: String,
        success: bool,
        ip_address: String,
    },
    LoginSuccess {
        user_id: Uuid,
        ip_address: String,
    },
    LoginFailure {
        email: String,
        reason: String,
        ip_address: String,
    },
    AccountLocked {
        user_id: Uuid,
        reason: String,
    },

    // System events
    HealthCheckFailed {
        component: String,
        error: String,
    },
    RateLimitExceeded {
        ip_address: String,
        endpoint: String,
    },

    // Custom events
    Custom {
        name: String,
        payload: String,
    },
}

/// Event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event metadata
    pub metadata: EventMetadata,
    /// Event type and data
    pub event_type: EventType,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType) -> Self {
        Self {
            metadata: EventMetadata::default(),
            event_type,
        }
    }

    /// Create an event with correlation ID
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.metadata.correlation_id = Some(correlation_id);
        self
    }

    /// Create an event with user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.metadata.user_id = Some(user_id);
        self
    }

    /// Create an event with source
    pub fn with_source(mut self, source: String) -> Self {
        self.metadata.source = source;
        self
    }

    /// Get event ID
    pub fn id(&self) -> EventId {
        self.metadata.id
    }

    /// Get event timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.metadata.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let user_id = Uuid::new_v4();
        let event = Event::new(EventType::UserRegistered {
            user_id,
            email: "test@example.com".to_string(),
        });

        assert!(event.id().as_u128() > 0);
        assert_eq!(event.metadata.source, "kaccy-api");
    }

    #[test]
    fn test_event_with_correlation_id() {
        let event = Event::new(EventType::LoginSuccess {
            user_id: Uuid::new_v4(),
            ip_address: "127.0.0.1".to_string(),
        })
        .with_correlation_id("test-correlation-id".to_string());

        assert_eq!(
            event.metadata.correlation_id,
            Some("test-correlation-id".to_string())
        );
    }

    #[test]
    fn test_event_with_user_id() {
        let user_id = Uuid::new_v4();
        let event = Event::new(EventType::Custom {
            name: "test_event".to_string(),
            payload: "{}".to_string(),
        })
        .with_user_id(user_id);

        assert_eq!(event.metadata.user_id, Some(user_id));
    }

    #[test]
    fn test_event_types() {
        // Test various event types can be created
        let _user_event = Event::new(EventType::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
        });

        let _token_event = Event::new(EventType::TokenCreated {
            token_id: Uuid::new_v4(),
            symbol: "TEST".to_string(),
            issuer_id: Uuid::new_v4(),
        });

        let _order_event = Event::new(EventType::OrderCreated {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            order_type: "buy".to_string(),
            amount: "100".to_string(),
        });
    }
}
