//! Pool event hooks for monitoring connection lifecycle events.
//!
//! This module provides:
//! - Connection acquire/release callbacks
//! - Connection timeout notifications
//! - Pool exhaustion alerts

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Event types for pool lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoolEventType {
    /// Connection acquired from pool
    ConnectionAcquired,
    /// Connection released back to pool
    ConnectionReleased,
    /// Connection acquisition timed out
    AcquisitionTimeout,
    /// Pool is exhausted (no available connections)
    PoolExhausted,
    /// Connection created
    ConnectionCreated,
    /// Connection closed
    ConnectionClosed,
    /// Connection validation failed
    ValidationFailed,
}

/// Pool event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEvent {
    /// Unique event ID
    pub event_id: Uuid,
    /// Event type
    pub event_type: PoolEventType,
    /// Timestamp when event occurred
    pub timestamp: DateTime<Utc>,
    /// Pool size at time of event
    pub pool_size: u32,
    /// Number of idle connections
    pub idle_connections: u32,
    /// Wait time in milliseconds (for acquire events)
    pub wait_time_ms: Option<u64>,
    /// Error message (if applicable)
    pub error: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl PoolEvent {
    /// Create a new pool event
    pub fn new(event_type: PoolEventType, pool_size: u32, idle_connections: u32) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            pool_size,
            idle_connections,
            wait_time_ms: None,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Add wait time
    pub fn with_wait_time(mut self, wait_time: Duration) -> Self {
        self.wait_time_ms = Some(wait_time.as_millis() as u64);
        self
    }

    /// Add error message
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Add metadata
    pub fn add_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Trait for handling pool events
#[async_trait]
pub trait PoolEventHandler: Send + Sync {
    /// Handle a pool event
    async fn handle_event(&self, event: PoolEvent);
}

/// Logging event handler that logs events using tracing
#[derive(Debug, Clone)]
pub struct LoggingEventHandler {
    /// Log level for different event types
    pub log_levels: HashMap<PoolEventType, LogLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LoggingEventHandler {
    fn default() -> Self {
        let mut log_levels = HashMap::new();
        log_levels.insert(PoolEventType::ConnectionAcquired, LogLevel::Debug);
        log_levels.insert(PoolEventType::ConnectionReleased, LogLevel::Debug);
        log_levels.insert(PoolEventType::AcquisitionTimeout, LogLevel::Warn);
        log_levels.insert(PoolEventType::PoolExhausted, LogLevel::Error);
        log_levels.insert(PoolEventType::ConnectionCreated, LogLevel::Info);
        log_levels.insert(PoolEventType::ConnectionClosed, LogLevel::Info);
        log_levels.insert(PoolEventType::ValidationFailed, LogLevel::Warn);

        Self { log_levels }
    }
}

#[async_trait]
impl PoolEventHandler for LoggingEventHandler {
    async fn handle_event(&self, event: PoolEvent) {
        let level = self
            .log_levels
            .get(&event.event_type)
            .copied()
            .unwrap_or(LogLevel::Info);

        let message = format!(
            "Pool event: {:?}, size: {}, idle: {}",
            event.event_type, event.pool_size, event.idle_connections
        );

        match level {
            LogLevel::Debug => debug!(
                event_id = %event.event_id,
                event_type = ?event.event_type,
                pool_size = event.pool_size,
                idle = event.idle_connections,
                wait_time_ms = ?event.wait_time_ms,
                "{}", message
            ),
            LogLevel::Info => info!(
                event_id = %event.event_id,
                event_type = ?event.event_type,
                pool_size = event.pool_size,
                idle = event.idle_connections,
                wait_time_ms = ?event.wait_time_ms,
                "{}", message
            ),
            LogLevel::Warn => warn!(
                event_id = %event.event_id,
                event_type = ?event.event_type,
                pool_size = event.pool_size,
                idle = event.idle_connections,
                wait_time_ms = ?event.wait_time_ms,
                error = ?event.error,
                "{}", message
            ),
            LogLevel::Error => error!(
                event_id = %event.event_id,
                event_type = ?event.event_type,
                pool_size = event.pool_size,
                idle = event.idle_connections,
                wait_time_ms = ?event.wait_time_ms,
                error = ?event.error,
                "{}", message
            ),
        }
    }
}

/// Metrics event handler that tracks pool metrics
#[derive(Debug, Clone)]
pub struct MetricsEventHandler {
    /// Event counts by type
    counts: Arc<RwLock<HashMap<PoolEventType, u64>>>,
    /// Total wait time in milliseconds
    total_wait_time_ms: Arc<RwLock<u64>>,
    /// Number of wait time samples
    wait_time_samples: Arc<RwLock<u64>>,
}

impl Default for MetricsEventHandler {
    fn default() -> Self {
        Self {
            counts: Arc::new(RwLock::new(HashMap::new())),
            total_wait_time_ms: Arc::new(RwLock::new(0)),
            wait_time_samples: Arc::new(RwLock::new(0)),
        }
    }
}

impl MetricsEventHandler {
    /// Get event count for a specific type
    pub fn get_count(&self, event_type: &PoolEventType) -> u64 {
        self.counts.read().get(event_type).copied().unwrap_or(0)
    }

    /// Get average wait time in milliseconds
    pub fn get_avg_wait_time_ms(&self) -> f64 {
        let total = *self.total_wait_time_ms.read();
        let samples = *self.wait_time_samples.read();

        if samples == 0 {
            0.0
        } else {
            total as f64 / samples as f64
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.counts.write().clear();
        *self.total_wait_time_ms.write() = 0;
        *self.wait_time_samples.write() = 0;
    }
}

#[async_trait]
impl PoolEventHandler for MetricsEventHandler {
    async fn handle_event(&self, event: PoolEvent) {
        // Increment count
        let mut counts = self.counts.write();
        *counts.entry(event.event_type.clone()).or_insert(0) += 1;
        drop(counts);

        // Track wait time
        if let Some(wait_time_ms) = event.wait_time_ms {
            *self.total_wait_time_ms.write() += wait_time_ms;
            *self.wait_time_samples.write() += 1;
        }
    }
}

/// Alert event handler that triggers alerts for critical events
#[derive(Clone)]
pub struct AlertEventHandler {
    /// Callback function for alerts
    alert_callback: Arc<dyn Fn(PoolEvent) + Send + Sync>,
}

impl AlertEventHandler {
    /// Create a new alert handler with a callback
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(PoolEvent) + Send + Sync + 'static,
    {
        Self {
            alert_callback: Arc::new(callback),
        }
    }
}

#[async_trait]
impl PoolEventHandler for AlertEventHandler {
    async fn handle_event(&self, event: PoolEvent) {
        // Trigger alert for critical events
        match event.event_type {
            PoolEventType::AcquisitionTimeout
            | PoolEventType::PoolExhausted
            | PoolEventType::ValidationFailed => {
                (self.alert_callback)(event);
            }
            _ => {}
        }
    }
}

/// Pool event manager that coordinates multiple event handlers
#[derive(Clone)]
pub struct PoolEventManager {
    handlers: Arc<RwLock<Vec<Arc<dyn PoolEventHandler>>>>,
}

impl Default for PoolEventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolEventManager {
    /// Create a new event manager
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add an event handler
    pub fn add_handler(&self, handler: Arc<dyn PoolEventHandler>) {
        self.handlers.write().push(handler);
    }

    /// Emit an event to all handlers
    pub async fn emit(&self, event: PoolEvent) {
        let handlers = self.handlers.read().clone();

        for handler in handlers {
            handler.handle_event(event.clone()).await;
        }
    }

    /// Convenience method to emit connection acquired event
    pub async fn emit_acquired(&self, pool_size: u32, idle: u32, wait_time: Duration) {
        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, pool_size, idle)
            .with_wait_time(wait_time);
        self.emit(event).await;
    }

    /// Convenience method to emit connection released event
    pub async fn emit_released(&self, pool_size: u32, idle: u32) {
        let event = PoolEvent::new(PoolEventType::ConnectionReleased, pool_size, idle);
        self.emit(event).await;
    }

    /// Convenience method to emit timeout event
    pub async fn emit_timeout(&self, pool_size: u32, idle: u32, timeout: Duration) {
        let event = PoolEvent::new(PoolEventType::AcquisitionTimeout, pool_size, idle)
            .with_wait_time(timeout)
            .with_error(format!(
                "Connection acquisition timed out after {}ms",
                timeout.as_millis()
            ));
        self.emit(event).await;
    }

    /// Convenience method to emit pool exhausted event
    pub async fn emit_exhausted(&self, pool_size: u32) {
        let event = PoolEvent::new(PoolEventType::PoolExhausted, pool_size, 0)
            .with_error("Pool has no available connections".to_string());
        self.emit(event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_event_creation() {
        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5);

        assert_eq!(event.event_type, PoolEventType::ConnectionAcquired);
        assert_eq!(event.pool_size, 10);
        assert_eq!(event.idle_connections, 5);
        assert!(event.wait_time_ms.is_none());
        assert!(event.error.is_none());
    }

    #[test]
    fn test_pool_event_with_wait_time() {
        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5)
            .with_wait_time(Duration::from_millis(150));

        assert_eq!(event.wait_time_ms, Some(150));
    }

    #[test]
    fn test_pool_event_with_error() {
        let event = PoolEvent::new(PoolEventType::AcquisitionTimeout, 10, 0)
            .with_error("Timeout occurred".to_string());

        assert_eq!(event.error, Some("Timeout occurred".to_string()));
    }

    #[tokio::test]
    async fn test_logging_event_handler() {
        let handler = LoggingEventHandler::default();
        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5);

        // Should not panic
        handler.handle_event(event).await;
    }

    #[tokio::test]
    async fn test_metrics_event_handler() {
        let handler = MetricsEventHandler::default();

        // Emit several events
        for _ in 0..5 {
            let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5)
                .with_wait_time(Duration::from_millis(100));
            handler.handle_event(event).await;
        }

        assert_eq!(handler.get_count(&PoolEventType::ConnectionAcquired), 5);
        assert!((handler.get_avg_wait_time_ms() - 100.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_metrics_event_handler_reset() {
        let handler = MetricsEventHandler::default();

        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5)
            .with_wait_time(Duration::from_millis(100));
        handler.handle_event(event).await;

        handler.reset();

        assert_eq!(handler.get_count(&PoolEventType::ConnectionAcquired), 0);
        assert_eq!(handler.get_avg_wait_time_ms(), 0.0);
    }

    #[tokio::test]
    async fn test_alert_event_handler() {
        let alert_triggered = Arc::new(RwLock::new(false));
        let alert_triggered_clone = alert_triggered.clone();

        let handler = AlertEventHandler::new(move |_event| {
            *alert_triggered_clone.write() = true;
        });

        // Non-critical event should not trigger alert
        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5);
        handler.handle_event(event).await;
        assert!(!*alert_triggered.read());

        // Critical event should trigger alert
        let event = PoolEvent::new(PoolEventType::PoolExhausted, 10, 0);
        handler.handle_event(event).await;
        assert!(*alert_triggered.read());
    }

    #[tokio::test]
    async fn test_pool_event_manager() {
        let manager = PoolEventManager::new();
        let metrics = Arc::new(MetricsEventHandler::default());

        manager.add_handler(metrics.clone());

        manager
            .emit_acquired(10, 5, Duration::from_millis(100))
            .await;
        manager.emit_released(10, 6).await;

        assert_eq!(metrics.get_count(&PoolEventType::ConnectionAcquired), 1);
        assert_eq!(metrics.get_count(&PoolEventType::ConnectionReleased), 1);
    }

    #[tokio::test]
    async fn test_pool_event_manager_multiple_handlers() {
        let manager = PoolEventManager::new();
        let metrics1 = Arc::new(MetricsEventHandler::default());
        let metrics2 = Arc::new(MetricsEventHandler::default());

        manager.add_handler(metrics1.clone());
        manager.add_handler(metrics2.clone());

        let event = PoolEvent::new(PoolEventType::ConnectionAcquired, 10, 5);
        manager.emit(event).await;

        // Both handlers should receive the event
        assert_eq!(metrics1.get_count(&PoolEventType::ConnectionAcquired), 1);
        assert_eq!(metrics2.get_count(&PoolEventType::ConnectionAcquired), 1);
    }

    #[tokio::test]
    async fn test_emit_timeout() {
        let manager = PoolEventManager::new();
        let metrics = Arc::new(MetricsEventHandler::default());

        manager.add_handler(metrics.clone());
        manager.emit_timeout(10, 0, Duration::from_secs(5)).await;

        assert_eq!(metrics.get_count(&PoolEventType::AcquisitionTimeout), 1);
    }

    #[tokio::test]
    async fn test_emit_exhausted() {
        let manager = PoolEventManager::new();
        let metrics = Arc::new(MetricsEventHandler::default());

        manager.add_handler(metrics.clone());
        manager.emit_exhausted(10).await;

        assert_eq!(metrics.get_count(&PoolEventType::PoolExhausted), 1);
    }
}
