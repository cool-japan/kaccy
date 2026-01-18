use super::types::{Event, EventId};
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;

/// Publisher configuration
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    /// Enable event publishing
    pub enabled: bool,
    /// Maximum events to store in memory
    pub max_events: usize,
    /// Enable event persistence
    pub persist_events: bool,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_events: 10_000,
            persist_events: true,
        }
    }
}

/// Event publisher
pub struct EventPublisher {
    config: PublisherConfig,
    /// In-memory event store
    events: Arc<DashMap<EventId, Event>>,
    /// Event history (ordered by timestamp)
    history: Arc<tokio::sync::RwLock<Vec<EventId>>>,
}

impl EventPublisher {
    /// Create a new event publisher
    pub fn new(config: PublisherConfig) -> Self {
        Self {
            config,
            events: Arc::new(DashMap::new()),
            history: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Publish an event
    pub async fn publish(&self, event: Event) -> Result<EventId> {
        if !self.config.enabled {
            tracing::warn!("Event publishing is disabled");
            return Ok(event.id());
        }

        let event_id = event.id();

        tracing::debug!(
            "Publishing event {} of type {:?}",
            event_id,
            std::mem::discriminant(&event.event_type)
        );

        // Store event
        self.events.insert(event_id, event);

        // Add to history
        {
            let mut history = self.history.write().await;
            history.push(event_id);

            // Enforce max events limit
            if history.len() > self.config.max_events {
                let to_remove = history.len() - self.config.max_events;
                for event_id in history.drain(0..to_remove) {
                    self.events.remove(&event_id);
                }
            }
        }

        Ok(event_id)
    }

    /// Get an event by ID
    pub async fn get_event(&self, event_id: &EventId) -> Option<Event> {
        self.events.get(event_id).map(|e| e.value().clone())
    }

    /// Get recent events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<Event> {
        let history = self.history.read().await;
        let start = history.len().saturating_sub(limit);

        history[start..]
            .iter()
            .filter_map(|id| self.events.get(id).map(|e| e.value().clone()))
            .collect()
    }

    /// Get all events (for testing/debugging)
    pub async fn get_all_events(&self) -> Vec<Event> {
        self.events
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get event count
    pub async fn count(&self) -> usize {
        self.events.len()
    }

    /// Clear all events (for testing)
    pub async fn clear(&self) {
        self.events.clear();
        self.history.write().await.clear();
    }

    /// Get configuration
    pub fn get_config(&self) -> &PublisherConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventType;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_publisher_creation() {
        let config = PublisherConfig::default();
        let publisher = EventPublisher::new(config);

        assert!(publisher.get_config().enabled);
    }

    #[tokio::test]
    async fn test_publish_event() {
        let config = PublisherConfig::default();
        let publisher = EventPublisher::new(config);

        let event = Event::new(EventType::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
        });

        let event_id = publisher.publish(event).await.unwrap();
        assert!(event_id.as_u128() > 0);

        let retrieved = publisher.get_event(&event_id).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_recent_events() {
        let config = PublisherConfig::default();
        let publisher = EventPublisher::new(config);

        // Publish multiple events
        for i in 0..5 {
            let event = Event::new(EventType::UserRegistered {
                user_id: Uuid::new_v4(),
                email: format!("test{}@example.com", i),
            });
            publisher.publish(event).await.unwrap();
        }

        let recent = publisher.get_recent_events(3).await;
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_max_events_limit() {
        let config = PublisherConfig {
            enabled: true,
            max_events: 5,
            persist_events: false,
        };
        let publisher = EventPublisher::new(config);

        // Publish more than max_events
        for i in 0..10 {
            let event = Event::new(EventType::UserRegistered {
                user_id: Uuid::new_v4(),
                email: format!("test{}@example.com", i),
            });
            publisher.publish(event).await.unwrap();
        }

        let count = publisher.count().await;
        assert_eq!(count, 5);
    }
}
