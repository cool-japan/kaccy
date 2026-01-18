//! Database event notifications using PostgreSQL LISTEN/NOTIFY
//!
//! This module provides real-time event notification capabilities using PostgreSQL's
//! LISTEN/NOTIFY mechanism. It enables event-driven architectures, cache invalidation,
//! and cross-service communication through the database.
//!
//! # Features
//!
//! - Real-time event notifications from database triggers
//! - Type-safe event payloads with JSON serialization
//! - Automatic reconnection and subscription recovery
//! - Multiple concurrent listeners with different channels
//! - Handler registration for specific event types
//! - Graceful shutdown and cleanup
//!
//! # Example
//!
//! ```rust,no_run
//! use kaccy_db::event_notifications::{EventListener, DatabaseEvent};
//! use sqlx::PgPool;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pool = PgPool::connect("postgresql://localhost/kaccy").await?;
//!
//!     // Create event listener
//!     let listener = EventListener::new(pool.clone()).await?;
//!
//!     // Subscribe to a channel
//!     listener.subscribe("user_events").await?;
//!
//!     // Register event handler
//!     listener.on_event("user_events", |event| {
//!         println!("Received event: {:?}", event);
//!     }).await;
//!
//!     // Listen for events (runs until stopped)
//!     listener.listen().await?;
//!
//!     Ok(())
//! }
//! ```

use crate::error::{DbError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgListener, PgNotification},
    PgPool,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

/// Database event with typed payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseEvent {
    /// Event channel name
    pub channel: String,
    /// Event type (e.g., "user.created", "token.updated")
    pub event_type: String,
    /// Event payload as JSON
    pub payload: serde_json::Value,
    /// Optional correlation ID for distributed tracing
    pub correlation_id: Option<String>,
    /// Event timestamp (ISO 8601)
    pub timestamp: String,
}

impl DatabaseEvent {
    /// Create a new database event
    pub fn new(
        channel: impl Into<String>,
        event_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            channel: channel.into(),
            event_type: event_type.into(),
            payload,
            correlation_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Set correlation ID for distributed tracing
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Parse event from PostgreSQL notification
    pub fn from_notification(notification: &PgNotification) -> Result<Self> {
        let payload_str = notification.payload();
        serde_json::from_str(payload_str)
            .map_err(|e| DbError::Validation(format!("Failed to parse event payload: {}", e)))
    }

    /// Serialize event to JSON string for NOTIFY
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| DbError::Validation(format!("Failed to serialize event: {}", e)))
    }
}

/// Event handler function type
pub type EventHandlerFn = Arc<dyn Fn(DatabaseEvent) + Send + Sync>;

/// Event listener for PostgreSQL LISTEN/NOTIFY
pub struct EventListener {
    /// PostgreSQL connection pool for sending notifications
    pool: PgPool,
    /// PostgreSQL listener for receiving notifications
    listener: Arc<Mutex<PgListener>>,
    /// Registered event handlers per channel
    handlers: Arc<RwLock<HashMap<String, Vec<EventHandlerFn>>>>,
    /// Active subscriptions
    subscriptions: Arc<RwLock<Vec<String>>>,
    /// Listener configuration
    config: ListenerConfig,
}

/// Configuration for event listener
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    /// Maximum reconnection attempts (0 = infinite)
    pub max_reconnect_attempts: usize,
    /// Delay between reconnection attempts in milliseconds
    pub reconnect_delay_ms: u64,
    /// Buffer size for event queue
    pub event_buffer_size: usize,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 0, // Infinite retries
            reconnect_delay_ms: 1000,  // 1 second
            event_buffer_size: 1000,
        }
    }
}

impl EventListener {
    /// Create a new event listener
    pub async fn new(pool: PgPool) -> Result<Self> {
        Self::new_with_config(pool, ListenerConfig::default()).await
    }

    /// Create a new event listener with custom configuration
    pub async fn new_with_config(pool: PgPool, config: ListenerConfig) -> Result<Self> {
        let listener = PgListener::connect_with(&pool).await?;

        info!("Created PostgreSQL event listener");

        Ok(Self {
            pool,
            listener: Arc::new(Mutex::new(listener)),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            config,
        })
    }

    /// Subscribe to a notification channel
    pub async fn subscribe(&self, channel: &str) -> Result<()> {
        let mut listener = self.listener.lock().await;
        listener.listen(channel).await?;

        let mut subs = self.subscriptions.write().await;
        if !subs.contains(&channel.to_string()) {
            subs.push(channel.to_string());
        }

        info!("Subscribed to channel: {}", channel);
        Ok(())
    }

    /// Unsubscribe from a notification channel
    pub async fn unsubscribe(&self, channel: &str) -> Result<()> {
        let mut listener = self.listener.lock().await;
        listener.unlisten(channel).await?;

        let mut subs = self.subscriptions.write().await;
        subs.retain(|c| c != channel);

        info!("Unsubscribed from channel: {}", channel);
        Ok(())
    }

    /// Register an event handler for a specific channel
    pub async fn on_event<F>(&self, channel: &str, handler: F)
    where
        F: Fn(DatabaseEvent) + Send + Sync + 'static,
    {
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(channel.to_string())
            .or_insert_with(Vec::new)
            .push(Arc::new(handler));

        debug!("Registered event handler for channel: {}", channel);
    }

    /// Start listening for events (blocking until stopped)
    pub async fn listen(&self) -> Result<()> {
        info!("Started listening for database events");

        loop {
            let notification = {
                let mut listener = self.listener.lock().await;
                match listener.try_recv().await {
                    Ok(Some(notif)) => notif,
                    Ok(None) => {
                        // No notification available, wait a bit
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }
                    Err(e) => {
                        error!("Error receiving notification: {}", e);
                        // Attempt reconnection
                        if let Err(e) = self.reconnect().await {
                            error!("Failed to reconnect: {}", e);
                        }
                        continue;
                    }
                }
            };

            self.handle_notification(notification).await;
        }
    }

    /// Handle a received notification
    async fn handle_notification(&self, notification: PgNotification) {
        let channel = notification.channel();

        // Parse event
        let event = match DatabaseEvent::from_notification(&notification) {
            Ok(event) => event,
            Err(e) => {
                warn!("Failed to parse event from channel {}: {}", channel, e);
                return;
            }
        };

        debug!(
            "Received event: channel={}, type={}",
            event.channel, event.event_type
        );

        // Call registered handlers
        let handlers = self.handlers.read().await;
        if let Some(channel_handlers) = handlers.get(channel) {
            for handler in channel_handlers {
                handler(event.clone());
            }
        }
    }

    /// Reconnect and re-subscribe to all channels
    async fn reconnect(&self) -> Result<()> {
        warn!("Attempting to reconnect to PostgreSQL...");

        let mut attempts = 0;
        loop {
            if self.config.max_reconnect_attempts > 0
                && attempts >= self.config.max_reconnect_attempts
            {
                return Err(DbError::Connection(format!(
                    "Failed to reconnect after {} attempts",
                    attempts
                )));
            }

            // Wait before retrying
            if attempts > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.reconnect_delay_ms,
                ))
                .await;
            }

            // Attempt to create new listener
            match PgListener::connect_with(&self.pool).await {
                Ok(new_listener) => {
                    let mut listener = self.listener.lock().await;
                    *listener = new_listener;

                    // Re-subscribe to all channels
                    let subscriptions = self.subscriptions.read().await;
                    for channel in subscriptions.iter() {
                        if let Err(e) = listener.listen(channel).await {
                            error!("Failed to re-subscribe to {}: {}", channel, e);
                        }
                    }

                    info!("Successfully reconnected and re-subscribed");
                    return Ok(());
                }
                Err(e) => {
                    error!("Reconnection attempt {} failed: {}", attempts + 1, e);
                    attempts += 1;
                }
            }
        }
    }

    /// Publish an event to a channel
    pub async fn notify(&self, event: &DatabaseEvent) -> Result<()> {
        let payload = event.to_json_string()?;

        sqlx::query(&format!("NOTIFY {}, $1", event.channel))
            .bind(&payload)
            .execute(&self.pool)
            .await?;

        debug!("Published event to channel: {}", event.channel);
        Ok(())
    }

    /// Get list of active subscriptions
    pub async fn get_subscriptions(&self) -> Vec<String> {
        self.subscriptions.read().await.clone()
    }

    /// Get number of registered handlers per channel
    pub async fn get_handler_counts(&self) -> HashMap<String, usize> {
        let handlers = self.handlers.read().await;
        handlers
            .iter()
            .map(|(channel, handlers)| (channel.clone(), handlers.len()))
            .collect()
    }
}

/// Event notification statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationStats {
    /// Total notifications received
    pub notifications_received: u64,
    /// Total notifications sent
    pub notifications_sent: u64,
    /// Active subscriptions
    pub active_subscriptions: usize,
    /// Registered handlers count per channel
    pub handlers_per_channel: HashMap<String, usize>,
}

/// Helper trait for creating database triggers that send notifications
pub trait NotificationTriggers {
    /// Create a trigger that sends a notification on INSERT
    fn create_insert_trigger_sql(table: &str, channel: &str) -> String;

    /// Create a trigger that sends a notification on UPDATE
    fn create_update_trigger_sql(table: &str, channel: &str) -> String;

    /// Create a trigger that sends a notification on DELETE
    fn create_delete_trigger_sql(table: &str, channel: &str) -> String;
}

pub struct PostgresNotificationTriggers;

impl NotificationTriggers for PostgresNotificationTriggers {
    fn create_insert_trigger_sql(table: &str, channel: &str) -> String {
        format!(
            r#"
CREATE OR REPLACE FUNCTION notify_{table}_insert()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        '{channel}',
        json_build_object(
            'channel', '{channel}',
            'event_type', '{table}.created',
            'payload', row_to_json(NEW),
            'timestamp', to_char(NOW(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER {table}_insert_notify
AFTER INSERT ON {table}
FOR EACH ROW
EXECUTE FUNCTION notify_{table}_insert();
            "#,
            table = table,
            channel = channel
        )
    }

    fn create_update_trigger_sql(table: &str, channel: &str) -> String {
        format!(
            r#"
CREATE OR REPLACE FUNCTION notify_{table}_update()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        '{channel}',
        json_build_object(
            'channel', '{channel}',
            'event_type', '{table}.updated',
            'payload', json_build_object('old', row_to_json(OLD), 'new', row_to_json(NEW)),
            'timestamp', to_char(NOW(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER {table}_update_notify
AFTER UPDATE ON {table}
FOR EACH ROW
EXECUTE FUNCTION notify_{table}_update();
            "#,
            table = table,
            channel = channel
        )
    }

    fn create_delete_trigger_sql(table: &str, channel: &str) -> String {
        format!(
            r#"
CREATE OR REPLACE FUNCTION notify_{table}_delete()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify(
        '{channel}',
        json_build_object(
            'channel', '{channel}',
            'event_type', '{table}.deleted',
            'payload', row_to_json(OLD),
            'timestamp', to_char(NOW(), 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
        )::text
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER {table}_delete_notify
AFTER DELETE ON {table}
FOR EACH ROW
EXECUTE FUNCTION notify_{table}_delete();
            "#,
            table = table,
            channel = channel
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_event_creation() {
        let payload = serde_json::json!({"user_id": "123", "action": "login"});
        let event = DatabaseEvent::new("user_events", "user.login", payload.clone());

        assert_eq!(event.channel, "user_events");
        assert_eq!(event.event_type, "user.login");
        assert_eq!(event.payload, payload);
        assert!(event.correlation_id.is_none());
        assert!(!event.timestamp.is_empty());
    }

    #[test]
    fn test_database_event_with_correlation_id() {
        let payload = serde_json::json!({"order_id": "456"});
        let event = DatabaseEvent::new("order_events", "order.created", payload)
            .with_correlation_id("trace-123-456");

        assert_eq!(event.correlation_id, Some("trace-123-456".to_string()));
    }

    #[test]
    fn test_database_event_serialization() {
        let payload = serde_json::json!({"token_id": "789", "amount": 1000});
        let event = DatabaseEvent::new("token_events", "token.transfer", payload);

        let json_str = event.to_json_string().unwrap();
        assert!(json_str.contains("token_events"));
        assert!(json_str.contains("token.transfer"));
        assert!(json_str.contains("token_id"));
    }

    #[test]
    fn test_database_event_deserialization() {
        let json_str = r#"{
            "channel": "trade_events",
            "event_type": "trade.executed",
            "payload": {"trade_id": "999", "price": 50000},
            "correlation_id": null,
            "timestamp": "2024-01-01T12:00:00Z"
        }"#;

        let event: DatabaseEvent = serde_json::from_str(json_str).unwrap();
        assert_eq!(event.channel, "trade_events");
        assert_eq!(event.event_type, "trade.executed");
        assert_eq!(event.payload["trade_id"], "999");
    }

    #[test]
    fn test_listener_config_default() {
        let config = ListenerConfig::default();
        assert_eq!(config.max_reconnect_attempts, 0); // Infinite
        assert_eq!(config.reconnect_delay_ms, 1000);
        assert_eq!(config.event_buffer_size, 1000);
    }

    #[test]
    fn test_listener_config_custom() {
        let config = ListenerConfig {
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 2000,
            event_buffer_size: 500,
        };

        assert_eq!(config.max_reconnect_attempts, 5);
        assert_eq!(config.reconnect_delay_ms, 2000);
        assert_eq!(config.event_buffer_size, 500);
    }

    #[test]
    fn test_create_insert_trigger_sql() {
        let sql = PostgresNotificationTriggers::create_insert_trigger_sql("users", "user_events");

        assert!(sql.contains("CREATE OR REPLACE FUNCTION notify_users_insert()"));
        assert!(sql.contains("pg_notify"));
        assert!(sql.contains("user_events"));
        assert!(sql.contains("users.created"));
        assert!(sql.contains("CREATE TRIGGER users_insert_notify"));
    }

    #[test]
    fn test_create_update_trigger_sql() {
        let sql = PostgresNotificationTriggers::create_update_trigger_sql("tokens", "token_events");

        assert!(sql.contains("CREATE OR REPLACE FUNCTION notify_tokens_update()"));
        assert!(sql.contains("pg_notify"));
        assert!(sql.contains("token_events"));
        assert!(sql.contains("tokens.updated"));
        assert!(sql.contains("CREATE TRIGGER tokens_update_notify"));
    }

    #[test]
    fn test_create_delete_trigger_sql() {
        let sql = PostgresNotificationTriggers::create_delete_trigger_sql("orders", "order_events");

        assert!(sql.contains("CREATE OR REPLACE FUNCTION notify_orders_delete()"));
        assert!(sql.contains("pg_notify"));
        assert!(sql.contains("order_events"));
        assert!(sql.contains("orders.deleted"));
        assert!(sql.contains("CREATE TRIGGER orders_delete_notify"));
    }

    #[test]
    fn test_notification_stats_serialization() {
        let stats = NotificationStats {
            notifications_received: 1000,
            notifications_sent: 500,
            active_subscriptions: 5,
            handlers_per_channel: [("users".to_string(), 3), ("tokens".to_string(), 2)]
                .iter()
                .cloned()
                .collect(),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("notifications_received"));
        assert!(json.contains("1000"));
        assert!(json.contains("handlers_per_channel"));
    }

    #[test]
    fn test_event_payload_types() {
        // Test with different payload types
        let string_payload = serde_json::json!("simple string");
        let event1 = DatabaseEvent::new("test", "test.string", string_payload);
        assert!(event1.to_json_string().is_ok());

        let object_payload = serde_json::json!({"key": "value", "number": 42});
        let event2 = DatabaseEvent::new("test", "test.object", object_payload);
        assert!(event2.to_json_string().is_ok());

        let array_payload = serde_json::json!([1, 2, 3, 4, 5]);
        let event3 = DatabaseEvent::new("test", "test.array", array_payload);
        assert!(event3.to_json_string().is_ok());
    }

    #[test]
    fn test_event_timestamp_format() {
        let event = DatabaseEvent::new("test", "test.event", serde_json::json!({"test": true}));

        // Verify timestamp is in RFC3339 format
        assert!(chrono::DateTime::parse_from_rfc3339(&event.timestamp).is_ok());
    }
}
