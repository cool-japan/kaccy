//! Event payload wrapper for storing and retrieving events

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::Event;

/// Event payload with metadata for persistence and routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    /// Unique event ID
    pub event_id: Uuid,
    /// Event type name
    pub event_type: String,
    /// When the event occurred
    pub occurred_at: DateTime<Utc>,
    /// When the event was persisted
    pub persisted_at: DateTime<Utc>,
    /// Aggregate/entity ID that caused this event
    pub aggregate_id: Option<Uuid>,
    /// User who triggered this event (if applicable)
    pub user_id: Option<Uuid>,
    /// Event data as JSON
    pub data: JsonValue,
    /// Event metadata (tags, correlation IDs, etc.)
    pub metadata: Option<JsonValue>,
    /// Event version (for schema evolution)
    pub version: String,
}

impl EventPayload {
    /// Create a new event payload from an event
    pub fn new(event: Event) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event.event_type().to_string(),
            occurred_at: event.occurred_at(),
            persisted_at: Utc::now(),
            aggregate_id: None,
            user_id: None,
            data: serde_json::to_value(&event).unwrap_or(JsonValue::Null),
            metadata: None,
            version: "1.0".to_string(),
        }
    }

    /// Create payload with metadata
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Create payload with aggregate ID
    pub fn with_aggregate_id(mut self, aggregate_id: Uuid) -> Self {
        self.aggregate_id = Some(aggregate_id);
        self
    }

    /// Create payload with user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Deserialize the event from payload data
    pub fn to_event(&self) -> Result<Event, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }

    /// Check if event matches a specific type
    pub fn is_type(&self, event_type: &str) -> bool {
        self.event_type == event_type
    }

    /// Get age of event in seconds
    pub fn age_seconds(&self) -> i64 {
        (Utc::now() - self.occurred_at).num_seconds()
    }

    /// Check if event is recent (less than 1 minute old)
    pub fn is_recent(&self) -> bool {
        self.age_seconds() < 60
    }
}

/// Event filter for querying events
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub event_types: Option<Vec<String>>,
    pub aggregate_ids: Option<Vec<Uuid>>,
    pub user_ids: Option<Vec<Uuid>>,
    pub from_time: Option<DateTime<Utc>>,
    pub to_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_event_type(mut self, event_type: String) -> Self {
        self.event_types = Some(vec![event_type]);
        self
    }

    pub fn with_event_types(mut self, event_types: Vec<String>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    pub fn with_aggregate_id(mut self, aggregate_id: Uuid) -> Self {
        self.aggregate_ids = Some(vec![aggregate_id]);
        self
    }

    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_ids = Some(vec![user_id]);
        self
    }

    pub fn with_time_range(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.from_time = Some(from);
        self.to_time = Some(to);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if payload matches this filter
    pub fn matches(&self, payload: &EventPayload) -> bool {
        // Check event type
        if let Some(ref types) = self.event_types {
            if !types.contains(&payload.event_type) {
                return false;
            }
        }

        // Check aggregate ID
        if let Some(ref ids) = self.aggregate_ids {
            if let Some(aggregate_id) = payload.aggregate_id {
                if !ids.contains(&aggregate_id) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check user ID
        if let Some(ref ids) = self.user_ids {
            if let Some(user_id) = payload.user_id {
                if !ids.contains(&user_id) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check time range
        if let Some(from) = self.from_time {
            if payload.occurred_at < from {
                return false;
            }
        }

        if let Some(to) = self.to_time {
            if payload.occurred_at > to {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_event_payload_creation() {
        let event = Event::TokenCreated(TokenCreatedEvent {
            token_id: Uuid::new_v4(),
            issuer_id: Uuid::new_v4(),
            symbol: "$TEST".to_string(),
            name: "Test Token".to_string(),
            initial_supply: dec!(1000000),
            occurred_at: Utc::now(),
        });

        let payload = EventPayload::new(event);

        assert_eq!(payload.event_type, "token_created");
        assert_eq!(payload.version, "1.0");
    }

    #[test]
    fn test_event_payload_with_metadata() {
        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        let metadata = serde_json::json!({
            "source": "api",
            "ip_address": "127.0.0.1"
        });

        let payload = EventPayload::new(event).with_metadata(metadata);

        assert!(payload.metadata.is_some());
    }

    #[test]
    fn test_event_filter() {
        let token_id = Uuid::new_v4();
        let event = Event::TokenCreated(TokenCreatedEvent {
            token_id,
            issuer_id: Uuid::new_v4(),
            symbol: "$TEST".to_string(),
            name: "Test Token".to_string(),
            initial_supply: dec!(1000000),
            occurred_at: Utc::now(),
        });

        let payload = EventPayload::new(event).with_aggregate_id(token_id);

        let filter = EventFilter::new()
            .with_event_type("token_created".to_string())
            .with_aggregate_id(token_id);

        assert!(filter.matches(&payload));

        let wrong_filter = EventFilter::new().with_event_type("user_registered".to_string());

        assert!(!wrong_filter.matches(&payload));
    }

    #[test]
    fn test_event_age() {
        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        let payload = EventPayload::new(event);

        assert!(payload.is_recent());
        assert!(payload.age_seconds() < 5);
    }
}
