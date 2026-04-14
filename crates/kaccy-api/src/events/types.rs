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
    /// A new user registered
    UserRegistered {
        /// ID of the newly registered user
        user_id: Uuid,
        /// Email address of the new user
        email: String,
    },
    /// A user's profile was updated
    UserUpdated {
        /// ID of the updated user
        user_id: Uuid,
        /// List of updated field names
        fields: Vec<String>,
    },
    /// A user account was deleted
    UserDeleted {
        /// ID of the deleted user
        user_id: Uuid,
    },

    // Token events
    /// A new creator token was created
    TokenCreated {
        /// ID of the new token
        token_id: Uuid,
        /// Token symbol
        symbol: String,
        /// User ID of the token issuer
        issuer_id: Uuid,
    },
    /// A token was updated
    TokenUpdated {
        /// ID of the updated token
        token_id: Uuid,
        /// List of updated field names
        fields: Vec<String>,
    },

    // Order events
    /// A new order was created
    OrderCreated {
        /// ID of the new order
        order_id: Uuid,
        /// ID of the user who placed the order
        user_id: Uuid,
        /// ID of the token being ordered
        token_id: Uuid,
        /// Order type (buy or sell)
        order_type: String,
        /// Order amount as string
        amount: String,
    },
    /// An order was completed
    OrderCompleted {
        /// ID of the completed order
        order_id: Uuid,
        /// ID of the user who placed the order
        user_id: Uuid,
        /// ID of the token
        token_id: Uuid,
    },
    /// An order was cancelled
    OrderCancelled {
        /// ID of the cancelled order
        order_id: Uuid,
        /// ID of the user who placed the order
        user_id: Uuid,
        /// Reason for cancellation
        reason: String,
    },

    // Trade events
    /// A trade was executed
    TradeExecuted {
        /// ID of the executed trade
        trade_id: Uuid,
        /// ID of the token traded
        token_id: Uuid,
        /// ID of the buyer
        buyer_id: Uuid,
        /// ID of the seller (None for bonding curve)
        seller_id: Option<Uuid>,
        /// Amount traded as string
        amount: String,
        /// Price in BTC as string
        price_btc: String,
    },

    // Commitment events
    /// A new output commitment was created
    CommitmentCreated {
        /// ID of the new commitment
        commitment_id: Uuid,
        /// ID of the user who created the commitment
        user_id: Uuid,
        /// ID of the associated token
        token_id: Uuid,
        /// Commitment title
        title: String,
    },
    /// Evidence was submitted for a commitment
    CommitmentEvidenceSubmitted {
        /// ID of the commitment
        commitment_id: Uuid,
        /// ID of the user submitting evidence
        user_id: Uuid,
    },
    /// A commitment was verified by an admin
    CommitmentVerified {
        /// ID of the verified commitment
        commitment_id: Uuid,
        /// ID of the admin who verified
        admin_id: Uuid,
        /// Whether the commitment was approved
        approved: bool,
        /// Optional quality score (0-100)
        quality_score: Option<i32>,
    },

    // KYC events
    /// A KYC application was submitted
    KycSubmitted {
        /// ID of the user who submitted
        user_id: Uuid,
        /// KYC verification level requested
        level: String,
    },
    /// A KYC application was reviewed
    KycVerified {
        /// ID of the user whose KYC was reviewed
        user_id: Uuid,
        /// ID of the admin who reviewed
        admin_id: Uuid,
        /// Whether the application was approved
        approved: bool,
    },

    // Security events
    /// A login attempt was made
    LoginAttempt {
        /// ID of the user attempting login (None if user not found)
        user_id: Option<Uuid>,
        /// Email address used in the attempt
        email: String,
        /// Whether the attempt was successful
        success: bool,
        /// IP address of the requester
        ip_address: String,
    },
    /// A login succeeded
    LoginSuccess {
        /// ID of the user who logged in
        user_id: Uuid,
        /// IP address of the login
        ip_address: String,
    },
    /// A login failed
    LoginFailure {
        /// Email address used in the failed attempt
        email: String,
        /// Reason for failure
        reason: String,
        /// IP address of the requester
        ip_address: String,
    },
    /// An account was locked due to too many failures
    AccountLocked {
        /// ID of the locked user account
        user_id: Uuid,
        /// Reason for locking
        reason: String,
    },

    // System events
    /// A health check failed
    HealthCheckFailed {
        /// Component that failed the health check
        component: String,
        /// Error message
        error: String,
    },
    /// Rate limit was exceeded
    RateLimitExceeded {
        /// IP address that exceeded the rate limit
        ip_address: String,
        /// Endpoint that was rate limited
        endpoint: String,
    },

    // Custom events
    /// A custom user-defined event
    Custom {
        /// Event name
        name: String,
        /// Event payload as JSON string
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
