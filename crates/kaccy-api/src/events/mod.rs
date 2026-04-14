/// Event-driven architecture for kaccy-api
///
/// This module provides a comprehensive event system for decoupled, scalable architecture:
///
/// ## Features
///
/// - **Pub/Sub Pattern**: Real-time event broadcasting to multiple subscribers
/// - **Event Sourcing**: Complete audit trail with database persistence
/// - **Event Handlers**: Pluggable handlers for different concerns
/// - **Async Processing**: Non-blocking event handling with Tokio
///
/// ## Built-in Event Handlers
///
/// - **AuditLogHandler**: Persists all events to database for compliance and debugging
/// - **MetricsHandler**: Updates Prometheus metrics for business intelligence
/// - **NotificationHandler**: Triggers appropriate notifications (email, SMS, push)
/// - **WebhookHandler**: Delivers events to subscribed webhook endpoints with HMAC signatures
/// - **AnalyticsHandler**: Tracks user behavior and system metrics
///
/// ## Event Types
///
/// - User events: registration, login, profile updates
/// - Token events: creation, trading
/// - Order events: creation, completion, cancellation
/// - Trade events: execution, settlement
/// - Commitment events: creation, verification
/// - KYC events: submission, verification
///
/// ## Usage Example
///
/// ```rust,ignore
/// use kaccy_api::events::{EventBus, Event, EventType};
///
/// let bus = EventBus::new(publisher);
/// bus.register_handler(Arc::new(AuditLogHandler::new(pool))).await;
/// bus.register_handler(Arc::new(MetricsHandler)).await;
///
/// // Publish event
/// let event = Event::new(EventType::UserRegistered {
///     user_id,
///     email: "user@example.com".to_string(),
/// });
/// bus.publish(event).await?;
/// ```
/// Event handler trait and built-in handlers
pub mod handlers;
/// Event publisher for dispatching events
pub mod publisher;
/// Event subscriber for receiving events
pub mod subscriber;
/// Event type definitions and metadata
pub mod types;

pub use handlers::EventHandler;
pub use publisher::{EventPublisher, PublisherConfig};
pub use subscriber::{EventSubscriber, Subscription};
pub use types::{Event, EventId, EventMetadata, EventType};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Event bus that coordinates publishers and subscribers
pub struct EventBus {
    publisher: Arc<EventPublisher>,
    subscribers: Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new(publisher: EventPublisher) -> Self {
        Self {
            publisher: Arc::new(publisher),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        self.subscribers.write().await.push(handler);
    }

    /// Publish an event
    pub async fn publish(&self, event: Event) -> Result<EventId> {
        // Store event
        let event_id = self.publisher.publish(event.clone()).await?;

        // Notify all subscribers
        let subscribers = self.subscribers.read().await;
        for subscriber in subscribers.iter() {
            let event_clone = event.clone();
            let subscriber_clone = subscriber.clone();

            // Handle event asynchronously
            tokio::spawn(async move {
                if let Err(e) = subscriber_clone.handle(event_clone).await {
                    tracing::error!("Event handler error: {}", e);
                }
            });
        }

        Ok(event_id)
    }

    /// Get publisher
    pub fn publisher(&self) -> Arc<EventPublisher> {
        self.publisher.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventType;

    #[tokio::test]
    async fn test_event_bus_creation() {
        let config = PublisherConfig::default();
        let publisher = EventPublisher::new(config);
        let bus = EventBus::new(publisher);

        assert!(bus.publisher().get_config().enabled);
    }

    #[tokio::test]
    async fn test_event_publish() {
        let config = PublisherConfig::default();
        let publisher = EventPublisher::new(config);
        let bus = EventBus::new(publisher);

        let event = Event::new(EventType::UserRegistered {
            user_id: uuid::Uuid::new_v4(),
            email: "test@example.com".to_string(),
        });

        let event_id = bus.publish(event).await.unwrap();
        assert!(event_id.as_u128() > 0);
    }
}
