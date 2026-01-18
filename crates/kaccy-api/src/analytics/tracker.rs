use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Tracker configuration
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Enable analytics tracking
    pub enabled: bool,
    /// Maximum events to store in memory
    pub max_events: usize,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_events: 100_000,
        }
    }
}

/// Analytics event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    /// Event ID
    pub id: Uuid,
    /// Event name
    pub name: String,
    /// Event properties
    pub properties: serde_json::Value,
    /// User ID
    pub user_id: Option<Uuid>,
    /// Session ID
    pub session_id: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl AnalyticsEvent {
    /// Create a new analytics event
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            properties: serde_json::json!({}),
            user_id: None,
            session_id: None,
            ip_address: None,
            user_agent: None,
            timestamp: Utc::now(),
        }
    }

    /// Add properties
    pub fn with_properties(mut self, properties: serde_json::Value) -> Self {
        self.properties = properties;
        self
    }

    /// Add user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add session ID
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add IP address
    pub fn with_ip_address(mut self, ip_address: String) -> Self {
        self.ip_address = Some(ip_address);
        self
    }

    /// Add user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }
}

/// Analytics tracker
pub struct AnalyticsTracker {
    config: TrackerConfig,
    /// In-memory event store
    events: Arc<DashMap<Uuid, AnalyticsEvent>>,
    /// Event history (ordered by timestamp)
    history: Arc<tokio::sync::RwLock<Vec<Uuid>>>,
}

impl AnalyticsTracker {
    /// Create a new analytics tracker
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            config,
            events: Arc::new(DashMap::new()),
            history: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Track an event
    pub async fn track(&self, event: AnalyticsEvent) -> anyhow::Result<Uuid> {
        if !self.config.enabled {
            return Ok(event.id);
        }

        let event_id = event.id;

        tracing::debug!("Tracking analytics event: {}", event.name);

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

    /// Get event by ID
    pub async fn get_event(&self, event_id: &Uuid) -> Option<AnalyticsEvent> {
        self.events.get(event_id).map(|e| e.value().clone())
    }

    /// Get events by name
    pub async fn get_events_by_name(&self, name: &str) -> Vec<AnalyticsEvent> {
        self.events
            .iter()
            .filter(|e| e.value().name == name)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get events by user
    pub async fn get_events_by_user(&self, user_id: &Uuid) -> Vec<AnalyticsEvent> {
        self.events
            .iter()
            .filter(|e| e.value().user_id == Some(*user_id))
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get recent events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<AnalyticsEvent> {
        let history = self.history.read().await;
        let start = history.len().saturating_sub(limit);

        history[start..]
            .iter()
            .filter_map(|id| self.events.get(id).map(|e| e.value().clone()))
            .collect()
    }

    /// Get event count
    pub async fn count(&self) -> usize {
        self.events.len()
    }

    /// Get event count by name
    pub async fn count_by_name(&self, name: &str) -> usize {
        self.events
            .iter()
            .filter(|e| e.value().name == name)
            .count()
    }

    /// Clear all events (for testing)
    pub async fn clear(&self) {
        self.events.clear();
        self.history.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracker_creation() {
        let config = TrackerConfig::default();
        let tracker = AnalyticsTracker::new(config);

        assert_eq!(tracker.count().await, 0);
    }

    #[tokio::test]
    async fn test_track_event() {
        let config = TrackerConfig::default();
        let tracker = AnalyticsTracker::new(config);

        let event = AnalyticsEvent::new("page_view".to_string())
            .with_user_id(Uuid::new_v4())
            .with_properties(serde_json::json!({
                "page": "/dashboard"
            }));

        let event_id = tracker.track(event).await.unwrap();
        assert!(event_id.as_u128() > 0);

        assert_eq!(tracker.count().await, 1);
    }

    #[tokio::test]
    async fn test_get_events_by_name() {
        let config = TrackerConfig::default();
        let tracker = AnalyticsTracker::new(config);

        tracker
            .track(AnalyticsEvent::new("page_view".to_string()))
            .await
            .unwrap();
        tracker
            .track(AnalyticsEvent::new("button_click".to_string()))
            .await
            .unwrap();
        tracker
            .track(AnalyticsEvent::new("page_view".to_string()))
            .await
            .unwrap();

        let page_views = tracker.get_events_by_name("page_view").await;
        assert_eq!(page_views.len(), 2);
    }

    #[tokio::test]
    async fn test_get_events_by_user() {
        let config = TrackerConfig::default();
        let tracker = AnalyticsTracker::new(config);

        let user_id = Uuid::new_v4();

        tracker
            .track(AnalyticsEvent::new("page_view".to_string()).with_user_id(user_id))
            .await
            .unwrap();
        tracker
            .track(AnalyticsEvent::new("button_click".to_string()).with_user_id(user_id))
            .await
            .unwrap();
        tracker
            .track(AnalyticsEvent::new("page_view".to_string()).with_user_id(Uuid::new_v4()))
            .await
            .unwrap();

        let user_events = tracker.get_events_by_user(&user_id).await;
        assert_eq!(user_events.len(), 2);
    }
}
