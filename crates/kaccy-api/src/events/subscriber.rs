use super::types::{Event, EventType};
use anyhow::Result;
use std::sync::Arc;

/// Event subscription
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Subscription ID
    pub id: uuid::Uuid,
    /// Event types to subscribe to (None = all events)
    pub event_types: Option<Vec<String>>,
}

impl Subscription {
    /// Create a new subscription for all events
    pub fn all() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            event_types: None,
        }
    }

    /// Create a new subscription for specific event types
    pub fn for_types(event_types: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            event_types: Some(event_types),
        }
    }

    /// Check if this subscription matches an event
    pub fn matches(&self, event: &Event) -> bool {
        match &self.event_types {
            None => true, // Subscribe to all events
            Some(types) => {
                let event_name = event_type_name(&event.event_type);
                types.iter().any(|t| t == &event_name)
            }
        }
    }
}

/// Event subscriber
pub struct EventSubscriber {
    subscriptions: Arc<tokio::sync::RwLock<Vec<Subscription>>>,
}

impl EventSubscriber {
    /// Create a new event subscriber
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to events
    pub async fn subscribe(&self, subscription: Subscription) -> uuid::Uuid {
        let id = subscription.id;
        self.subscriptions.write().await.push(subscription);
        id
    }

    /// Unsubscribe from events
    pub async fn unsubscribe(&self, subscription_id: &uuid::Uuid) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(pos) = subscriptions.iter().position(|s| &s.id == subscription_id) {
            subscriptions.remove(pos);
            Ok(())
        } else {
            anyhow::bail!("Subscription not found")
        }
    }

    /// Check if an event matches any subscriptions
    pub async fn should_process(&self, event: &Event) -> bool {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.iter().any(|s| s.matches(event))
    }

    /// Get all subscriptions
    pub async fn get_subscriptions(&self) -> Vec<Subscription> {
        self.subscriptions.read().await.clone()
    }
}

impl Default for EventSubscriber {
    fn default() -> Self {
        Self::new()
    }
}

/// Get event type name as string
fn event_type_name(event_type: &EventType) -> String {
    match event_type {
        EventType::UserRegistered { .. } => "UserRegistered".to_string(),
        EventType::UserUpdated { .. } => "UserUpdated".to_string(),
        EventType::UserDeleted { .. } => "UserDeleted".to_string(),
        EventType::TokenCreated { .. } => "TokenCreated".to_string(),
        EventType::TokenUpdated { .. } => "TokenUpdated".to_string(),
        EventType::OrderCreated { .. } => "OrderCreated".to_string(),
        EventType::OrderCompleted { .. } => "OrderCompleted".to_string(),
        EventType::OrderCancelled { .. } => "OrderCancelled".to_string(),
        EventType::TradeExecuted { .. } => "TradeExecuted".to_string(),
        EventType::CommitmentCreated { .. } => "CommitmentCreated".to_string(),
        EventType::CommitmentEvidenceSubmitted { .. } => "CommitmentEvidenceSubmitted".to_string(),
        EventType::CommitmentVerified { .. } => "CommitmentVerified".to_string(),
        EventType::KycSubmitted { .. } => "KycSubmitted".to_string(),
        EventType::KycVerified { .. } => "KycVerified".to_string(),
        EventType::LoginAttempt { .. } => "LoginAttempt".to_string(),
        EventType::LoginSuccess { .. } => "LoginSuccess".to_string(),
        EventType::LoginFailure { .. } => "LoginFailure".to_string(),
        EventType::AccountLocked { .. } => "AccountLocked".to_string(),
        EventType::HealthCheckFailed { .. } => "HealthCheckFailed".to_string(),
        EventType::RateLimitExceeded { .. } => "RateLimitExceeded".to_string(),
        EventType::Custom { name, .. } => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_subscription_all() {
        let subscription = Subscription::all();
        assert!(subscription.event_types.is_none());

        let event = Event::new(EventType::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
        });

        assert!(subscription.matches(&event));
    }

    #[test]
    fn test_subscription_specific_types() {
        let subscription = Subscription::for_types(vec!["UserRegistered".to_string()]);

        let user_event = Event::new(EventType::UserRegistered {
            user_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
        });

        let token_event = Event::new(EventType::TokenCreated {
            token_id: Uuid::new_v4(),
            symbol: "TEST".to_string(),
            issuer_id: Uuid::new_v4(),
        });

        assert!(subscription.matches(&user_event));
        assert!(!subscription.matches(&token_event));
    }

    #[tokio::test]
    async fn test_subscriber() {
        let subscriber = EventSubscriber::new();

        let subscription = Subscription::all();
        let id = subscriber.subscribe(subscription).await;

        let subscriptions = subscriber.get_subscriptions().await;
        assert_eq!(subscriptions.len(), 1);

        subscriber.unsubscribe(&id).await.unwrap();

        let subscriptions = subscriber.get_subscriptions().await;
        assert_eq!(subscriptions.len(), 0);
    }
}
