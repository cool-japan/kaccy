//! Event bus for dispatching events to subscribers

use std::sync::Arc;
use tokio::sync::RwLock;

use super::{Event, EventPayload, EventSubscriber, SubscriberHandle};
use crate::error::Result;

/// Type alias for subscriber storage
type SubscriberList = Vec<(SubscriberHandle, Arc<dyn EventSubscriber>)>;

/// Event dispatcher trait for publishing events
#[async_trait::async_trait]
pub trait EventDispatcher: Send + Sync {
    /// Publish an event to all subscribers
    async fn dispatch(&self, event: Event) -> Result<()>;

    /// Publish an event with metadata
    async fn dispatch_with_metadata(&self, event: Event, metadata: serde_json::Value)
    -> Result<()>;
}

/// In-memory event bus implementation
pub struct EventBus {
    subscribers: Arc<RwLock<SubscriberList>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to events
    pub async fn subscribe(&self, subscriber: Arc<dyn EventSubscriber>) -> SubscriberHandle {
        let handle = SubscriberHandle::new(subscriber.name().to_string());
        let mut subs = self.subscribers.write().await;
        subs.push((handle.clone(), subscriber));
        tracing::info!(
            subscriber = handle.name(),
            subscriber_id = %handle.id(),
            "Event subscriber registered"
        );
        handle
    }

    /// Unsubscribe a subscriber
    pub async fn unsubscribe(&self, handle: &SubscriberHandle) -> bool {
        let mut subs = self.subscribers.write().await;
        let original_len = subs.len();
        subs.retain(|(h, _)| h != handle);
        let removed = subs.len() < original_len;

        if removed {
            tracing::info!(
                subscriber = handle.name(),
                subscriber_id = %handle.id(),
                "Event subscriber unregistered"
            );
        }

        removed
    }

    /// Get count of active subscribers
    pub async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }

    /// Clear all subscribers
    pub async fn clear_subscribers(&self) {
        let mut subs = self.subscribers.write().await;
        let count = subs.len();
        subs.clear();
        tracing::info!(count, "All event subscribers cleared");
    }

    /// Dispatch event to relevant subscribers
    async fn dispatch_to_subscribers(&self, event: &Event, payload: &EventPayload) -> Result<()> {
        let subscribers = self.subscribers.read().await;

        for (handle, subscriber) in subscribers.iter() {
            // Check if subscriber is interested in this event type
            if let Some(types) = subscriber.subscribed_to() {
                if !types.contains(&event.event_type()) {
                    continue;
                }
            }

            // Handle event
            match subscriber.handle(event, payload).await {
                Ok(()) => {
                    tracing::debug!(
                        subscriber = handle.name(),
                        event_type = event.event_type(),
                        "Event handled successfully"
                    );
                }
                Err(e) => {
                    let should_retry = subscriber.on_error(&e, event).await;
                    if should_retry {
                        tracing::warn!(
                            subscriber = handle.name(),
                            event_type = event.event_type(),
                            error = %e,
                            "Event handler failed, retrying"
                        );
                        // Retry once
                        if let Err(retry_err) = subscriber.handle(event, payload).await {
                            tracing::error!(
                                subscriber = handle.name(),
                                event_type = event.event_type(),
                                error = %retry_err,
                                "Event handler retry failed"
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventDispatcher for EventBus {
    async fn dispatch(&self, event: Event) -> Result<()> {
        let payload = EventPayload::new(event.clone());

        tracing::info!(
            event_id = %payload.event_id,
            event_type = event.event_type(),
            "Dispatching event"
        );

        self.dispatch_to_subscribers(&event, &payload).await
    }

    async fn dispatch_with_metadata(
        &self,
        event: Event,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let payload = EventPayload::new(event.clone()).with_metadata(metadata);

        tracing::info!(
            event_id = %payload.event_id,
            event_type = event.event_type(),
            "Dispatching event with metadata"
        );

        self.dispatch_to_subscribers(&event, &payload).await
    }
}

/// Builder for creating an event bus with pre-registered subscribers
pub struct EventBusBuilder {
    subscribers: Vec<Arc<dyn EventSubscriber>>,
}

impl EventBusBuilder {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Add a subscriber
    pub fn with_subscriber(mut self, subscriber: Arc<dyn EventSubscriber>) -> Self {
        self.subscribers.push(subscriber);
        self
    }

    /// Add default subscribers (audit log, metrics)
    pub fn with_default_subscribers(self) -> Self {
        use super::subscriber::{AuditLogSubscriber, MetricsSubscriber};

        self.with_subscriber(Arc::new(AuditLogSubscriber))
            .with_subscriber(Arc::new(MetricsSubscriber::new()))
    }

    /// Build the event bus
    pub async fn build(self) -> EventBus {
        let bus = EventBus::new();

        for subscriber in self.subscribers {
            bus.subscribe(subscriber).await;
        }

        bus
    }
}

impl Default for EventBusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::*;
    use chrono::Utc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    struct CountingSubscriber {
        count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl EventSubscriber for CountingSubscriber {
        fn name(&self) -> &str {
            "CountingSubscriber"
        }

        async fn handle(&self, _event: &Event, _payload: &EventPayload) -> Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_event_bus_subscribe() {
        let bus = EventBus::new();
        let subscriber = Arc::new(CountingSubscriber {
            count: Arc::new(AtomicUsize::new(0)),
        });

        let handle = bus.subscribe(subscriber.clone()).await;

        assert_eq!(handle.name(), "CountingSubscriber");
        assert_eq!(bus.subscriber_count().await, 1);

        bus.unsubscribe(&handle).await;
        assert_eq!(bus.subscriber_count().await, 0);
    }

    #[tokio::test]
    async fn test_event_dispatch() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let subscriber = Arc::new(CountingSubscriber {
            count: count.clone(),
        });

        bus.subscribe(subscriber).await;

        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        bus.dispatch(event).await.unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();

        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let subscriber1 = Arc::new(CountingSubscriber {
            count: count1.clone(),
        });
        let subscriber2 = Arc::new(CountingSubscriber {
            count: count2.clone(),
        });

        bus.subscribe(subscriber1).await;
        bus.subscribe(subscriber2).await;

        let event = Event::TokenCreated(TokenCreatedEvent {
            token_id: Uuid::new_v4(),
            issuer_id: Uuid::new_v4(),
            symbol: "$TEST".to_string(),
            name: "Test Token".to_string(),
            initial_supply: rust_decimal_macros::dec!(1000000),
            occurred_at: Utc::now(),
        });

        bus.dispatch(event).await.unwrap();

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_bus_builder() {
        let count = Arc::new(AtomicUsize::new(0));
        let subscriber = Arc::new(CountingSubscriber {
            count: count.clone(),
        });

        let bus = EventBusBuilder::new()
            .with_subscriber(subscriber)
            .build()
            .await;

        assert_eq!(bus.subscriber_count().await, 1);

        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        bus.dispatch(event).await.unwrap();

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_clear_subscribers() {
        let bus = EventBus::new();
        let subscriber = Arc::new(CountingSubscriber {
            count: Arc::new(AtomicUsize::new(0)),
        });

        bus.subscribe(subscriber).await;
        assert_eq!(bus.subscriber_count().await, 1);

        bus.clear_subscribers().await;
        assert_eq!(bus.subscriber_count().await, 0);
    }
}
