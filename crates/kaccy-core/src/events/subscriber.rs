//! Event subscriber trait and management

use async_trait::async_trait;
use std::fmt;
use uuid::Uuid;

use super::{Event, EventPayload};
use crate::error::Result;

/// Trait for event subscribers
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Get the subscriber name (for logging/debugging)
    fn name(&self) -> &str;

    /// Get the event types this subscriber is interested in
    /// Return None to subscribe to all events
    fn subscribed_to(&self) -> Option<Vec<&'static str>> {
        None
    }

    /// Handle an event
    async fn handle(&self, event: &Event, payload: &EventPayload) -> Result<()>;

    /// Called when subscriber encounters an error
    /// Return true to retry, false to skip
    async fn on_error(&self, error: &crate::CoreError, event: &Event) -> bool {
        tracing::error!(
            subscriber = self.name(),
            event_type = event.event_type(),
            error = %error,
            "Event handler error"
        );
        false // Don't retry by default
    }
}

/// Handle to a registered subscriber
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberHandle {
    id: Uuid,
    name: String,
}

impl SubscriberHandle {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for SubscriberHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.id)
    }
}

/// Example: Audit log subscriber that logs all events
pub struct AuditLogSubscriber;

#[async_trait]
impl EventSubscriber for AuditLogSubscriber {
    fn name(&self) -> &str {
        "AuditLogSubscriber"
    }

    async fn handle(&self, event: &Event, payload: &EventPayload) -> Result<()> {
        tracing::info!(
            event_id = %payload.event_id,
            event_type = event.event_type(),
            occurred_at = %payload.occurred_at,
            "Event logged to audit trail"
        );
        Ok(())
    }
}

/// Example: Notification subscriber that sends notifications for specific events
pub struct NotificationSubscriber {
    event_types: Vec<&'static str>,
}

impl NotificationSubscriber {
    pub fn new(event_types: Vec<&'static str>) -> Self {
        Self { event_types }
    }

    /// Create a notification from an event
    /// In a real implementation, this would be stored in the database
    fn create_notification_from_event(&self, event: &Event) -> Option<NotificationInfo> {
        match event {
            Event::OrderCreated(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "Order Created".to_string(),
                message: "Your order has been created successfully".to_string(),
                event_type: event.event_type().to_string(),
            }),
            Event::OrderFilled(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "Order Filled".to_string(),
                message: "Your order has been filled".to_string(),
                event_type: event.event_type().to_string(),
            }),
            Event::TradeExecuted(e) => Some(NotificationInfo {
                user_id: e.buyer_id,
                title: "Trade Executed".to_string(),
                message: "Trade executed successfully".to_string(),
                event_type: event.event_type().to_string(),
            }),
            Event::TokenCreated(e) => Some(NotificationInfo {
                user_id: e.issuer_id,
                title: "Token Created".to_string(),
                message: format!("Token {} created successfully", e.symbol),
                event_type: event.event_type().to_string(),
            }),
            Event::PaymentConfirmed(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "Payment Confirmed".to_string(),
                message: "Your payment has been confirmed".to_string(),
                event_type: event.event_type().to_string(),
            }),
            Event::ReputationChanged(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "Reputation Changed".to_string(),
                message: format!("Your reputation score changed: {:+}", e.change),
                event_type: event.event_type().to_string(),
            }),
            Event::KycApproved(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "KYC Approved".to_string(),
                message: "Your KYC verification has been approved".to_string(),
                event_type: event.event_type().to_string(),
            }),
            Event::KycRejected(e) => Some(NotificationInfo {
                user_id: e.user_id,
                title: "KYC Rejected".to_string(),
                message: format!("Your KYC verification was rejected: {}", e.reason),
                event_type: event.event_type().to_string(),
            }),
            Event::SecurityAlert(e) => e.user_id.map(|uid| NotificationInfo {
                user_id: uid,
                title: "Security Alert".to_string(),
                message: format!("Security alert: {}", e.description),
                event_type: event.event_type().to_string(),
            }),
            _ => None,
        }
    }
}

/// Information for creating a notification
#[derive(Debug)]
struct NotificationInfo {
    user_id: Uuid,
    title: String,
    #[allow(dead_code)]
    message: String,
    #[allow(dead_code)]
    event_type: String,
}

#[async_trait]
impl EventSubscriber for NotificationSubscriber {
    fn name(&self) -> &str {
        "NotificationSubscriber"
    }

    fn subscribed_to(&self) -> Option<Vec<&'static str>> {
        Some(self.event_types.clone())
    }

    async fn handle(&self, event: &Event, payload: &EventPayload) -> Result<()> {
        if let Some(notif_info) = self.create_notification_from_event(event) {
            tracing::info!(
                event_id = %payload.event_id,
                event_type = event.event_type(),
                user_id = %notif_info.user_id,
                title = %notif_info.title,
                "Notification created for event"
            );

            // In a real implementation, this would:
            // 1. Insert notification into database
            // 2. Send push notification
            // 3. Send email notification
            // 4. Send SMS notification (if enabled)
            // Example:
            // sqlx::query("INSERT INTO notifications ...")
            //     .bind(notif_info.user_id)
            //     .bind(notif_info.title)
            //     .execute(pool)
            //     .await?;
        }
        Ok(())
    }
}

/// Example: Metrics subscriber that tracks event statistics
pub struct MetricsSubscriber {
    event_counts: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, u64>>>,
}

impl MetricsSubscriber {
    pub fn new() -> Self {
        Self {
            event_counts: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Get event count for a specific event type
    pub fn get_count(&self, event_type: &str) -> u64 {
        self.event_counts
            .lock()
            .unwrap()
            .get(event_type)
            .copied()
            .unwrap_or(0)
    }

    /// Get all event counts
    pub fn get_all_counts(&self) -> std::collections::HashMap<String, u64> {
        self.event_counts.lock().unwrap().clone()
    }

    /// Reset all counts
    pub fn reset(&self) {
        self.event_counts.lock().unwrap().clear();
    }
}

impl Default for MetricsSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventSubscriber for MetricsSubscriber {
    fn name(&self) -> &str {
        "MetricsSubscriber"
    }

    async fn handle(&self, event: &Event, _payload: &EventPayload) -> Result<()> {
        let event_type = event.event_type().to_string();

        // Increment counter for this event type
        {
            let mut counts = self.event_counts.lock().unwrap();
            *counts.entry(event_type.clone()).or_insert(0) += 1;
        }

        tracing::debug!(
            event_type = %event_type,
            count = self.get_count(&event_type),
            "Event metric recorded"
        );

        // In a real implementation, this would:
        // 1. Send metrics to a monitoring system (e.g., Prometheus, Datadog)
        // 2. Update time-series database
        // 3. Trigger alerts based on thresholds
        // Example:
        // metrics::counter!("kaccy.events.total", 1, "type" => event_type);
        // metrics::histogram!("kaccy.events.processing_time", processing_duration);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::*;
    use chrono::Utc;

    struct TestSubscriber {
        name: String,
    }

    #[async_trait]
    impl EventSubscriber for TestSubscriber {
        fn name(&self) -> &str {
            &self.name
        }

        fn subscribed_to(&self) -> Option<Vec<&'static str>> {
            Some(vec!["user_registered", "token_created"])
        }

        async fn handle(&self, _event: &Event, _payload: &EventPayload) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_subscriber_handle() {
        let handle = SubscriberHandle::new("TestSubscriber".to_string());
        assert_eq!(handle.name(), "TestSubscriber");
        assert!(!handle.id().is_nil());
    }

    #[tokio::test]
    async fn test_subscriber_handle_event() {
        let subscriber = TestSubscriber {
            name: "TestSubscriber".to_string(),
        };

        let event = Event::UserRegistered(UserRegisteredEvent {
            user_id: Uuid::new_v4(),
            username: "test".to_string(),
            email: "test@example.com".to_string(),
            occurred_at: Utc::now(),
        });

        let payload = EventPayload::new(event.clone());

        let result = subscriber.handle(&event, &payload).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_subscriber_subscription() {
        let subscriber = TestSubscriber {
            name: "TestSubscriber".to_string(),
        };

        let types = subscriber.subscribed_to().unwrap();
        assert!(types.contains(&"user_registered"));
        assert!(types.contains(&"token_created"));
    }
}
