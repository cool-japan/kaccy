//! Webhook System
//!
//! This module provides a comprehensive webhook system for real-time event notifications,
//! including registration, delivery, retry logic, and security features.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    /// Unique identifier of this webhook
    pub webhook_id: String,
    /// User who owns this webhook
    pub user_id: String,
    /// Endpoint URL to deliver events to
    pub url: String,
    /// HMAC signing secret for payload verification
    pub secret: String,
    /// Event types this webhook subscribes to
    pub events: Vec<WebhookEventType>,
    /// Whether this webhook is currently active
    pub enabled: bool,
    /// When the webhook was created
    pub created_at: SystemTime,
    /// When the webhook configuration was last modified
    pub updated_at: SystemTime,
    /// Arbitrary key-value metadata
    pub metadata: HashMap<String, String>,
}

/// Types of events that can trigger webhooks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WebhookEventType {
    /// An order was placed
    OrderCreated,
    /// An order was fully filled
    OrderFilled,
    /// An order was cancelled
    OrderCancelled,
    /// A trade was executed
    TradeExecuted,

    /// A balance changed
    BalanceUpdated,
    /// A deposit was received
    Deposit,
    /// A withdrawal was processed
    Withdrawal,

    /// A price alert was triggered
    PriceAlert,
    /// A significant price movement occurred
    PriceChange,

    /// A margin call was issued
    MarginCall,
    /// A position is approaching liquidation
    LiquidationWarning,
    /// A position was closed
    PositionClosed,

    /// Scheduled maintenance announcement
    MaintenanceScheduled,
    /// A system-level alert
    SystemAlert,

    /// Subscribe to all event types
    All,
}

/// Webhook delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    /// Unique identifier of this delivery attempt
    pub delivery_id: String,
    /// Webhook this delivery belongs to
    pub webhook_id: String,
    /// Event type that triggered the delivery
    pub event_type: WebhookEventType,
    /// JSON-encoded event payload
    pub payload: String,
    /// Number of delivery attempts so far
    pub attempt_count: u32,
    /// Maximum allowed delivery attempts before giving up
    pub max_attempts: u32,
    /// Current delivery status
    pub status: DeliveryStatus,
    /// When the delivery was first created
    pub created_at: SystemTime,
    /// When the next retry is scheduled
    pub next_retry_at: Option<SystemTime>,
    /// When the delivery reached a terminal state
    pub completed_at: Option<SystemTime>,
    /// HTTP response code from the last attempt
    pub response_code: Option<u16>,
    /// Error message from the last failed attempt
    pub error_message: Option<String>,
}

/// Status of a webhook delivery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Queued but not yet dispatched
    Pending,
    /// Currently being delivered
    InProgress,
    /// Delivered successfully
    Success,
    /// All attempts exhausted without success
    Failed,
    /// Failed and scheduled for a retry
    Retrying,
}

/// Webhook rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRateLimit {
    /// Maximum number of deliveries allowed per minute
    pub max_deliveries_per_minute: usize,
    /// Maximum number of deliveries allowed per hour
    pub max_deliveries_per_hour: usize,
    /// Number of deliveries made in the current minute window
    pub current_minute_count: usize,
    /// Number of deliveries made in the current hour window
    pub current_hour_count: usize,
    /// When the per-minute counter resets
    pub minute_reset_at: SystemTime,
    /// When the per-hour counter resets
    pub hour_reset_at: SystemTime,
}

impl Default for WebhookRateLimit {
    fn default() -> Self {
        let now = SystemTime::now();
        Self {
            max_deliveries_per_minute: 60,
            max_deliveries_per_hour: 1000,
            current_minute_count: 0,
            current_hour_count: 0,
            minute_reset_at: now + Duration::from_secs(60),
            hour_reset_at: now + Duration::from_secs(3600),
        }
    }
}

impl WebhookRateLimit {
    /// Checks if webhook can be delivered within rate limits
    pub fn can_deliver(&mut self) -> bool {
        let now = SystemTime::now();

        // Reset minute counter if needed
        if now >= self.minute_reset_at {
            self.current_minute_count = 0;
            self.minute_reset_at = now + Duration::from_secs(60);
        }

        // Reset hour counter if needed
        if now >= self.hour_reset_at {
            self.current_hour_count = 0;
            self.hour_reset_at = now + Duration::from_secs(3600);
        }

        self.current_minute_count < self.max_deliveries_per_minute
            && self.current_hour_count < self.max_deliveries_per_hour
    }

    /// Records a delivery
    pub fn record_delivery(&mut self) {
        self.current_minute_count += 1;
        self.current_hour_count += 1;
    }
}

/// Webhook manager
pub struct WebhookManager {
    /// Registered webhooks indexed by webhook ID
    webhooks: HashMap<String, Webhook>,
    /// Delivery records indexed by delivery ID
    deliveries: HashMap<String, WebhookDelivery>,
    /// Delivery IDs waiting for retry
    retry_queue: VecDeque<String>,
    /// Per-webhook rate limit state
    rate_limits: HashMap<String, WebhookRateLimit>,
    /// Monotonic counter for generating unique delivery IDs
    delivery_counter: u64,
}

impl WebhookManager {
    /// Create a new webhook manager
    pub fn new() -> Self {
        Self {
            webhooks: HashMap::new(),
            deliveries: HashMap::new(),
            retry_queue: VecDeque::new(),
            rate_limits: HashMap::new(),
            delivery_counter: 0,
        }
    }

    /// Registers a new webhook
    pub fn register_webhook(
        &mut self,
        webhook_id: String,
        user_id: String,
        url: String,
        secret: String,
        events: Vec<WebhookEventType>,
    ) -> Result<(), CoreError> {
        // Validate URL format
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(CoreError::Validation(
                "Webhook URL must start with http:// or https://".to_string(),
            ));
        }

        if self.webhooks.contains_key(&webhook_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Webhook {} already exists",
                webhook_id
            )));
        }

        let now = SystemTime::now();
        let webhook = Webhook {
            webhook_id: webhook_id.clone(),
            user_id,
            url,
            secret,
            events,
            enabled: true,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };

        self.webhooks.insert(webhook_id.clone(), webhook);
        self.rate_limits
            .insert(webhook_id, WebhookRateLimit::default());

        Ok(())
    }

    /// Updates webhook configuration
    pub fn update_webhook(
        &mut self,
        webhook_id: &str,
        url: Option<String>,
        events: Option<Vec<WebhookEventType>>,
        enabled: Option<bool>,
    ) -> Result<(), CoreError> {
        let webhook = self
            .webhooks
            .get_mut(webhook_id)
            .ok_or_else(|| CoreError::NotFound(format!("Webhook {} not found", webhook_id)))?;

        if let Some(new_url) = url {
            if !new_url.starts_with("http://") && !new_url.starts_with("https://") {
                return Err(CoreError::Validation(
                    "Webhook URL must start with http:// or https://".to_string(),
                ));
            }
            webhook.url = new_url;
        }

        if let Some(new_events) = events {
            webhook.events = new_events;
        }

        if let Some(new_enabled) = enabled {
            webhook.enabled = new_enabled;
        }

        webhook.updated_at = SystemTime::now();

        Ok(())
    }

    /// Deletes a webhook
    pub fn delete_webhook(&mut self, webhook_id: &str) -> Result<(), CoreError> {
        if !self.webhooks.contains_key(webhook_id) {
            return Err(CoreError::NotFound(format!(
                "Webhook {} not found",
                webhook_id
            )));
        }

        self.webhooks.remove(webhook_id);
        self.rate_limits.remove(webhook_id);

        Ok(())
    }

    /// Triggers webhooks for an event
    pub fn trigger_event(
        &mut self,
        event_type: WebhookEventType,
        payload: String,
    ) -> Result<Vec<String>, CoreError> {
        let mut delivery_ids = Vec::new();

        // Find all webhooks that should receive this event
        let matching_webhooks: Vec<_> = self
            .webhooks
            .values()
            .filter(|wh| wh.enabled)
            .filter(|wh| {
                wh.events.contains(&event_type) || wh.events.contains(&WebhookEventType::All)
            })
            .cloned()
            .collect();

        for webhook in matching_webhooks {
            // Check rate limits
            let rate_limit = self.rate_limits.get_mut(&webhook.webhook_id).expect("webhook_id rate_limit entry was created when webhook was registered");
            if !rate_limit.can_deliver() {
                continue; // Skip if rate limited
            }

            // Create delivery
            self.delivery_counter += 1;
            let delivery_id = format!("delivery_{}", self.delivery_counter);

            let delivery = WebhookDelivery {
                delivery_id: delivery_id.clone(),
                webhook_id: webhook.webhook_id.clone(),
                event_type,
                payload: payload.clone(),
                attempt_count: 0,
                max_attempts: 5,
                status: DeliveryStatus::Pending,
                created_at: SystemTime::now(),
                next_retry_at: Some(SystemTime::now()),
                completed_at: None,
                response_code: None,
                error_message: None,
            };

            rate_limit.record_delivery();
            self.deliveries.insert(delivery_id.clone(), delivery);
            self.retry_queue.push_back(delivery_id.clone());
            delivery_ids.push(delivery_id);
        }

        Ok(delivery_ids)
    }

    /// Marks a delivery attempt
    pub fn mark_delivery_attempt(
        &mut self,
        delivery_id: &str,
        success: bool,
        response_code: Option<u16>,
        error_message: Option<String>,
    ) -> Result<(), CoreError> {
        let delivery = self
            .deliveries
            .get_mut(delivery_id)
            .ok_or_else(|| CoreError::NotFound(format!("Delivery {} not found", delivery_id)))?;

        delivery.attempt_count += 1;
        delivery.response_code = response_code;
        delivery.error_message = error_message;

        if success {
            delivery.status = DeliveryStatus::Success;
            delivery.completed_at = Some(SystemTime::now());
            delivery.next_retry_at = None;
        } else if delivery.attempt_count >= delivery.max_attempts {
            delivery.status = DeliveryStatus::Failed;
            delivery.completed_at = Some(SystemTime::now());
            delivery.next_retry_at = None;
        } else {
            delivery.status = DeliveryStatus::Retrying;
            // Exponential backoff: 2^attempt_count minutes
            let backoff_minutes = 2_u64.pow(delivery.attempt_count);
            delivery.next_retry_at =
                Some(SystemTime::now() + Duration::from_secs(backoff_minutes * 60));
            self.retry_queue.push_back(delivery_id.to_string());
        }

        Ok(())
    }

    /// Gets deliveries ready for retry
    pub fn get_pending_deliveries(&mut self) -> Vec<WebhookDelivery> {
        let now = SystemTime::now();
        let mut ready = Vec::new();

        // Process retry queue
        let mut temp_queue = VecDeque::new();
        while let Some(delivery_id) = self.retry_queue.pop_front() {
            if let Some(delivery) = self.deliveries.get(&delivery_id) {
                if let Some(retry_at) = delivery.next_retry_at {
                    if now >= retry_at {
                        ready.push(delivery.clone());
                    } else {
                        temp_queue.push_back(delivery_id);
                    }
                }
            }
        }

        self.retry_queue = temp_queue;
        ready
    }

    /// Gets webhook statistics
    pub fn get_webhook_stats(&self, webhook_id: &str) -> Option<WebhookStats> {
        if !self.webhooks.contains_key(webhook_id) {
            return None;
        }

        let total_deliveries = self
            .deliveries
            .values()
            .filter(|d| d.webhook_id == webhook_id)
            .count();

        let successful = self
            .deliveries
            .values()
            .filter(|d| d.webhook_id == webhook_id && d.status == DeliveryStatus::Success)
            .count();

        let failed = self
            .deliveries
            .values()
            .filter(|d| d.webhook_id == webhook_id && d.status == DeliveryStatus::Failed)
            .count();

        let pending = self
            .deliveries
            .values()
            .filter(|d| {
                d.webhook_id == webhook_id
                    && matches!(d.status, DeliveryStatus::Pending | DeliveryStatus::Retrying)
            })
            .count();

        let success_rate = if total_deliveries > 0 {
            Decimal::from(successful) / Decimal::from(total_deliveries) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        };

        Some(WebhookStats {
            webhook_id: webhook_id.to_string(),
            total_deliveries,
            successful_deliveries: successful,
            failed_deliveries: failed,
            pending_deliveries: pending,
            success_rate,
        })
    }

    /// Gets webhook by ID
    pub fn get_webhook(&self, webhook_id: &str) -> Option<&Webhook> {
        self.webhooks.get(webhook_id)
    }

    /// Lists all webhooks for a user
    pub fn list_user_webhooks(&self, user_id: &str) -> Vec<&Webhook> {
        self.webhooks
            .values()
            .filter(|wh| wh.user_id == user_id)
            .collect()
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Webhook statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookStats {
    /// Webhook these statistics relate to
    pub webhook_id: String,
    /// Total number of delivery attempts
    pub total_deliveries: usize,
    /// Number of successful deliveries
    pub successful_deliveries: usize,
    /// Number of permanently failed deliveries
    pub failed_deliveries: usize,
    /// Number of deliveries still pending or retrying
    pub pending_deliveries: usize,
    /// Success rate as a percentage (0–100)
    pub success_rate: Decimal,
}

/// Webhook signature generator for security
pub struct WebhookSignature;

impl WebhookSignature {
    /// Generates HMAC-SHA256 signature for webhook payload
    pub fn generate(secret: &str, payload: &str) -> String {
        use sha2::{Digest, Sha256};

        let mut mac = Sha256::new();
        mac.update(secret.as_bytes());
        mac.update(payload.as_bytes());
        let result = mac.finalize();

        format!("sha256={}", hex::encode(result))
    }

    /// Verifies webhook signature
    pub fn verify(secret: &str, payload: &str, signature: &str) -> bool {
        let expected = Self::generate(secret, payload);
        expected == signature
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_registration() {
        let mut manager = WebhookManager::new();

        let result = manager.register_webhook(
            "wh1".to_string(),
            "user1".to_string(),
            "https://example.com/webhook".to_string(),
            "secret123".to_string(),
            vec![
                WebhookEventType::OrderCreated,
                WebhookEventType::OrderFilled,
            ],
        );

        assert!(result.is_ok());
        assert!(manager.webhooks.contains_key("wh1"));
    }

    #[test]
    fn test_webhook_invalid_url() {
        let mut manager = WebhookManager::new();

        let result = manager.register_webhook(
            "wh1".to_string(),
            "user1".to_string(),
            "invalid-url".to_string(),
            "secret123".to_string(),
            vec![WebhookEventType::OrderCreated],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_webhook_update() {
        let mut manager = WebhookManager::new();

        manager
            .register_webhook(
                "wh1".to_string(),
                "user1".to_string(),
                "https://example.com/webhook".to_string(),
                "secret123".to_string(),
                vec![WebhookEventType::OrderCreated],
            )
            .unwrap();

        manager
            .update_webhook(
                "wh1",
                None,
                Some(vec![WebhookEventType::OrderFilled]),
                Some(false),
            )
            .unwrap();

        let webhook = manager.get_webhook("wh1").unwrap();
        assert!(!webhook.enabled);
        assert_eq!(webhook.events.len(), 1);
    }

    #[test]
    fn test_trigger_event() {
        let mut manager = WebhookManager::new();

        manager
            .register_webhook(
                "wh1".to_string(),
                "user1".to_string(),
                "https://example.com/webhook".to_string(),
                "secret123".to_string(),
                vec![WebhookEventType::OrderCreated],
            )
            .unwrap();

        let delivery_ids = manager
            .trigger_event(
                WebhookEventType::OrderCreated,
                r#"{"order_id": "order123"}"#.to_string(),
            )
            .unwrap();

        assert_eq!(delivery_ids.len(), 1);
        assert!(manager.deliveries.contains_key(&delivery_ids[0]));
    }

    #[test]
    fn test_rate_limiting() {
        let mut rate_limit = WebhookRateLimit {
            max_deliveries_per_minute: 2,
            max_deliveries_per_hour: 100,
            current_minute_count: 0,
            current_hour_count: 0,
            minute_reset_at: SystemTime::now() + Duration::from_secs(60),
            hour_reset_at: SystemTime::now() + Duration::from_secs(3600),
        };

        assert!(rate_limit.can_deliver());
        rate_limit.record_delivery();

        assert!(rate_limit.can_deliver());
        rate_limit.record_delivery();

        assert!(!rate_limit.can_deliver()); // Exceeded minute limit
    }

    #[test]
    fn test_delivery_retry() {
        let mut manager = WebhookManager::new();

        manager
            .register_webhook(
                "wh1".to_string(),
                "user1".to_string(),
                "https://example.com/webhook".to_string(),
                "secret123".to_string(),
                vec![WebhookEventType::OrderCreated],
            )
            .unwrap();

        let delivery_ids = manager
            .trigger_event(
                WebhookEventType::OrderCreated,
                r#"{"order_id": "order123"}"#.to_string(),
            )
            .unwrap();

        let delivery_id = &delivery_ids[0];

        // Mark first attempt as failed
        manager
            .mark_delivery_attempt(
                delivery_id,
                false,
                Some(500),
                Some("Internal Server Error".to_string()),
            )
            .unwrap();

        let delivery = manager.deliveries.get(delivery_id).unwrap();
        assert_eq!(delivery.status, DeliveryStatus::Retrying);
        assert_eq!(delivery.attempt_count, 1);
    }

    #[test]
    fn test_webhook_stats() {
        let mut manager = WebhookManager::new();

        manager
            .register_webhook(
                "wh1".to_string(),
                "user1".to_string(),
                "https://example.com/webhook".to_string(),
                "secret123".to_string(),
                vec![WebhookEventType::OrderCreated],
            )
            .unwrap();

        // Trigger some events
        manager
            .trigger_event(
                WebhookEventType::OrderCreated,
                r#"{"order_id": "order1"}"#.to_string(),
            )
            .unwrap();

        manager
            .trigger_event(
                WebhookEventType::OrderCreated,
                r#"{"order_id": "order2"}"#.to_string(),
            )
            .unwrap();

        let stats = manager.get_webhook_stats("wh1").unwrap();
        assert_eq!(stats.total_deliveries, 2);
    }

    #[test]
    fn test_signature_generation() {
        let signature = WebhookSignature::generate("secret123", r#"{"test": "data"}"#);
        assert!(signature.starts_with("sha256="));

        let is_valid = WebhookSignature::verify("secret123", r#"{"test": "data"}"#, &signature);
        assert!(is_valid);

        let is_invalid =
            WebhookSignature::verify("wrong_secret", r#"{"test": "data"}"#, &signature);
        assert!(!is_invalid);
    }
}
