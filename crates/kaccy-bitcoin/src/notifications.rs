//! Admin notifications for payment issues
//!
//! This module provides a notification system for alerting administrators
//! about payment discrepancies, RBF replacements, and other important events.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Priority level for notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NotificationPriority {
    /// Low priority - informational
    Low,
    /// Medium priority - requires attention
    Medium,
    /// High priority - requires prompt action
    High,
    /// Critical priority - requires immediate action
    Critical,
}

/// Notification category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationCategory {
    /// Payment amount mismatch
    PaymentMismatch,
    /// Transaction replaced via RBF
    TransactionReplaced,
    /// Transaction dropped from mempool
    TransactionDropped,
    /// Block reorganization detected
    Reorganization,
    /// Large transaction detected
    LargeTransaction,
    /// Suspicious activity detected
    SuspiciousActivity,
    /// System health issue
    SystemHealth,
    /// General information
    Info,
}

/// Admin notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminNotification {
    /// Unique notification ID
    pub id: String,
    /// Notification category
    pub category: NotificationCategory,
    /// Priority level
    pub priority: NotificationPriority,
    /// Short title
    pub title: String,
    /// Detailed message
    pub message: String,
    /// Related order ID (if applicable)
    pub order_id: Option<String>,
    /// Related transaction ID (if applicable)
    pub txid: Option<String>,
    /// Related user ID (if applicable)
    pub user_id: Option<String>,
    /// Additional metadata
    pub metadata: NotificationMetadata,
    /// When the notification was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Whether the notification has been acknowledged
    pub acknowledged: bool,
    /// When the notification was acknowledged
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Who acknowledged the notification
    pub acknowledged_by: Option<String>,
}

/// Additional notification metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationMetadata {
    /// Expected amount in satoshis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sats: Option<u64>,
    /// Received amount in satoshis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_sats: Option<u64>,
    /// Difference in satoshis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difference_sats: Option<i64>,
    /// Payment address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Original transaction ID (for RBF)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_txid: Option<String>,
    /// Replacement transaction ID (for RBF)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_txid: Option<String>,
    /// Fee increase in satoshis (for RBF)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_increase_sats: Option<u64>,
    /// Suggested action
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
    /// Refund address (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_address: Option<String>,
}

impl AdminNotification {
    /// Create a new notification
    pub fn new(
        category: NotificationCategory,
        priority: NotificationPriority,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            category,
            priority,
            title: title.into(),
            message: message.into(),
            order_id: None,
            txid: None,
            user_id: None,
            metadata: NotificationMetadata::default(),
            created_at: chrono::Utc::now(),
            acknowledged: false,
            acknowledged_at: None,
            acknowledged_by: None,
        }
    }

    /// Set order ID
    pub fn with_order(mut self, order_id: impl Into<String>) -> Self {
        self.order_id = Some(order_id.into());
        self
    }

    /// Set transaction ID
    pub fn with_txid(mut self, txid: impl Into<String>) -> Self {
        self.txid = Some(txid.into());
        self
    }

    /// Set user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: NotificationMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Create an underpayment notification
    pub fn underpayment(
        order_id: &str,
        txid: &str,
        expected_sats: u64,
        received_sats: u64,
        address: &str,
        refund_address: Option<String>,
    ) -> Self {
        let shortfall = expected_sats.saturating_sub(received_sats);
        let percentage = (shortfall as f64 / expected_sats as f64 * 100.0) as u32;

        let priority = if percentage > 50 {
            NotificationPriority::High
        } else if percentage > 20 {
            NotificationPriority::Medium
        } else {
            NotificationPriority::Low
        };

        Self::new(
            NotificationCategory::PaymentMismatch,
            priority,
            format!("Underpayment: {} sats short", shortfall),
            format!(
                "Order {} received {} sats but expected {} sats ({}% short)",
                order_id, received_sats, expected_sats, percentage
            ),
        )
        .with_order(order_id)
        .with_txid(txid)
        .with_metadata(NotificationMetadata {
            expected_sats: Some(expected_sats),
            received_sats: Some(received_sats),
            difference_sats: Some(-(shortfall as i64)),
            address: Some(address.to_string()),
            suggested_action: Some(if percentage > 10 {
                "Contact user for additional payment or cancel order".to_string()
            } else {
                "Consider accepting as minor discrepancy".to_string()
            }),
            refund_address,
            ..Default::default()
        })
    }

    /// Create an overpayment notification
    pub fn overpayment(
        order_id: &str,
        txid: &str,
        expected_sats: u64,
        received_sats: u64,
        address: &str,
        refund_address: Option<String>,
    ) -> Self {
        let excess = received_sats.saturating_sub(expected_sats);
        let percentage = (excess as f64 / expected_sats as f64 * 100.0) as u32;

        let priority = if excess > 100_000 {
            // > 0.001 BTC
            NotificationPriority::High
        } else if percentage > 20 {
            NotificationPriority::Medium
        } else {
            NotificationPriority::Low
        };

        let suggested_action = if let Some(ref refund_addr) = refund_address {
            format!("Process refund of {} sats to {}", excess, refund_addr)
        } else {
            "Unable to determine refund address - manual intervention required".to_string()
        };

        Self::new(
            NotificationCategory::PaymentMismatch,
            priority,
            format!("Overpayment: {} sats excess", excess),
            format!(
                "Order {} received {} sats but expected {} sats ({}% over)",
                order_id, received_sats, expected_sats, percentage
            ),
        )
        .with_order(order_id)
        .with_txid(txid)
        .with_metadata(NotificationMetadata {
            expected_sats: Some(expected_sats),
            received_sats: Some(received_sats),
            difference_sats: Some(excess as i64),
            address: Some(address.to_string()),
            suggested_action: Some(suggested_action),
            refund_address,
            ..Default::default()
        })
    }

    /// Create an RBF replacement notification
    pub fn rbf_replacement(
        order_id: Option<&str>,
        original_txid: &str,
        replacement_txid: &str,
        fee_increase: Option<u64>,
    ) -> Self {
        let mut notification = Self::new(
            NotificationCategory::TransactionReplaced,
            NotificationPriority::Medium,
            "Transaction Replaced (RBF)",
            format!(
                "Transaction {} was replaced by {}",
                original_txid, replacement_txid
            ),
        )
        .with_metadata(NotificationMetadata {
            original_txid: Some(original_txid.to_string()),
            replacement_txid: Some(replacement_txid.to_string()),
            fee_increase_sats: fee_increase,
            suggested_action: Some(
                "Verify replacement transaction is valid and update order tracking".to_string(),
            ),
            ..Default::default()
        });

        if let Some(order) = order_id {
            notification = notification.with_order(order);
        }

        notification
    }

    /// Create a large transaction notification
    pub fn large_transaction(
        txid: &str,
        amount_sats: u64,
        address: &str,
        order_id: Option<&str>,
    ) -> Self {
        let btc_amount = amount_sats as f64 / 100_000_000.0;

        let mut notification = Self::new(
            NotificationCategory::LargeTransaction,
            if amount_sats > 10_000_000_000 {
                // > 100 BTC
                NotificationPriority::Critical
            } else if amount_sats > 1_000_000_000 {
                // > 10 BTC
                NotificationPriority::High
            } else {
                NotificationPriority::Medium
            },
            format!("Large Transaction: {:.4} BTC", btc_amount),
            format!(
                "Received {} sats ({:.4} BTC) at address {}",
                amount_sats, btc_amount, address
            ),
        )
        .with_txid(txid)
        .with_metadata(NotificationMetadata {
            received_sats: Some(amount_sats),
            address: Some(address.to_string()),
            suggested_action: Some("Review and verify legitimacy".to_string()),
            ..Default::default()
        });

        if let Some(order) = order_id {
            notification = notification.with_order(order);
        }

        notification
    }
}

/// Admin notification service
pub struct AdminNotificationService {
    /// Notification channel
    notification_tx: broadcast::Sender<AdminNotification>,
    /// Recent notifications (ring buffer)
    recent: Arc<RwLock<VecDeque<AdminNotification>>>,
    /// Maximum recent notifications to keep
    max_recent: usize,
    /// Large transaction threshold in satoshis
    large_tx_threshold: u64,
}

impl AdminNotificationService {
    /// Create a new notification service
    pub fn new() -> Self {
        let (notification_tx, _) = broadcast::channel(100);
        Self {
            notification_tx,
            recent: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            max_recent: 1000,
            large_tx_threshold: 100_000_000, // 1 BTC default
        }
    }

    /// Set the large transaction threshold
    pub fn with_large_tx_threshold(mut self, threshold_sats: u64) -> Self {
        self.large_tx_threshold = threshold_sats;
        self
    }

    /// Subscribe to notifications
    pub fn subscribe(&self) -> broadcast::Receiver<AdminNotification> {
        self.notification_tx.subscribe()
    }

    /// Send a notification
    pub async fn notify(&self, notification: AdminNotification) {
        // Add to recent notifications
        {
            let mut recent = self.recent.write().await;
            if recent.len() >= self.max_recent {
                recent.pop_front();
            }
            recent.push_back(notification.clone());
        }

        // Broadcast to subscribers
        let _ = self.notification_tx.send(notification.clone());

        // Log based on priority
        match notification.priority {
            NotificationPriority::Critical => {
                tracing::error!(
                    category = ?notification.category,
                    title = %notification.title,
                    "CRITICAL: {}", notification.message
                );
            }
            NotificationPriority::High => {
                tracing::warn!(
                    category = ?notification.category,
                    title = %notification.title,
                    "{}", notification.message
                );
            }
            NotificationPriority::Medium => {
                tracing::info!(
                    category = ?notification.category,
                    title = %notification.title,
                    "{}", notification.message
                );
            }
            NotificationPriority::Low => {
                tracing::debug!(
                    category = ?notification.category,
                    title = %notification.title,
                    "{}", notification.message
                );
            }
        }
    }

    /// Notify about underpayment
    pub async fn notify_underpayment(
        &self,
        order_id: &str,
        txid: &str,
        expected_sats: u64,
        received_sats: u64,
        address: &str,
        refund_address: Option<String>,
    ) {
        let notification = AdminNotification::underpayment(
            order_id,
            txid,
            expected_sats,
            received_sats,
            address,
            refund_address,
        );
        self.notify(notification).await;
    }

    /// Notify about overpayment
    pub async fn notify_overpayment(
        &self,
        order_id: &str,
        txid: &str,
        expected_sats: u64,
        received_sats: u64,
        address: &str,
        refund_address: Option<String>,
    ) {
        let notification = AdminNotification::overpayment(
            order_id,
            txid,
            expected_sats,
            received_sats,
            address,
            refund_address,
        );
        self.notify(notification).await;
    }

    /// Notify about RBF replacement
    pub async fn notify_rbf_replacement(
        &self,
        order_id: Option<&str>,
        original_txid: &str,
        replacement_txid: &str,
        fee_increase: Option<u64>,
    ) {
        let notification = AdminNotification::rbf_replacement(
            order_id,
            original_txid,
            replacement_txid,
            fee_increase,
        );
        self.notify(notification).await;
    }

    /// Notify about large transaction if above threshold
    pub async fn check_large_transaction(
        &self,
        txid: &str,
        amount_sats: u64,
        address: &str,
        order_id: Option<&str>,
    ) {
        if amount_sats >= self.large_tx_threshold {
            let notification =
                AdminNotification::large_transaction(txid, amount_sats, address, order_id);
            self.notify(notification).await;
        }
    }

    /// Get recent notifications
    pub async fn get_recent(&self, limit: usize) -> Vec<AdminNotification> {
        let recent = self.recent.read().await;
        recent.iter().rev().take(limit).cloned().collect()
    }

    /// Get unacknowledged notifications
    pub async fn get_unacknowledged(&self) -> Vec<AdminNotification> {
        let recent = self.recent.read().await;
        recent.iter().filter(|n| !n.acknowledged).cloned().collect()
    }

    /// Get notifications by category
    pub async fn get_by_category(&self, category: NotificationCategory) -> Vec<AdminNotification> {
        let recent = self.recent.read().await;
        recent
            .iter()
            .filter(|n| n.category == category)
            .cloned()
            .collect()
    }

    /// Get notifications by priority
    pub async fn get_by_priority(
        &self,
        min_priority: NotificationPriority,
    ) -> Vec<AdminNotification> {
        let recent = self.recent.read().await;
        recent
            .iter()
            .filter(|n| n.priority >= min_priority)
            .cloned()
            .collect()
    }

    /// Acknowledge a notification
    pub async fn acknowledge(&self, notification_id: &str, admin_id: &str) -> bool {
        let mut recent = self.recent.write().await;
        for notification in recent.iter_mut() {
            if notification.id == notification_id {
                notification.acknowledged = true;
                notification.acknowledged_at = Some(chrono::Utc::now());
                notification.acknowledged_by = Some(admin_id.to_string());
                return true;
            }
        }
        false
    }

    /// Get notification statistics
    pub async fn get_stats(&self) -> NotificationStats {
        let recent = self.recent.read().await;

        let mut by_category: std::collections::HashMap<NotificationCategory, usize> =
            std::collections::HashMap::new();
        let mut by_priority: std::collections::HashMap<NotificationPriority, usize> =
            std::collections::HashMap::new();
        let mut unacknowledged = 0;

        for notification in recent.iter() {
            *by_category.entry(notification.category).or_insert(0) += 1;
            *by_priority.entry(notification.priority).or_insert(0) += 1;
            if !notification.acknowledged {
                unacknowledged += 1;
            }
        }

        NotificationStats {
            total: recent.len(),
            unacknowledged,
            by_category,
            by_priority,
        }
    }
}

impl Default for AdminNotificationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Notification statistics
#[derive(Debug, Clone, Serialize)]
pub struct NotificationStats {
    /// Total notifications
    pub total: usize,
    /// Unacknowledged notifications
    pub unacknowledged: usize,
    /// Count by category
    pub by_category: std::collections::HashMap<NotificationCategory, usize>,
    /// Count by priority
    pub by_priority: std::collections::HashMap<NotificationPriority, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_underpayment_notification() {
        let notification = AdminNotification::underpayment(
            "order-123",
            "txid-abc",
            100_000,
            80_000,
            "bc1qtest",
            Some("bc1qrefund".to_string()),
        );

        assert_eq!(notification.category, NotificationCategory::PaymentMismatch);
        assert!(notification.title.contains("20000"));
        assert_eq!(notification.metadata.expected_sats, Some(100_000));
        assert_eq!(notification.metadata.received_sats, Some(80_000));
        assert_eq!(notification.metadata.difference_sats, Some(-20000));
    }

    #[test]
    fn test_overpayment_notification() {
        let notification = AdminNotification::overpayment(
            "order-456",
            "txid-def",
            100_000,
            150_000,
            "bc1qtest",
            None,
        );

        assert_eq!(notification.category, NotificationCategory::PaymentMismatch);
        assert!(notification.title.contains("50000"));
        assert_eq!(notification.metadata.difference_sats, Some(50000));
    }

    #[test]
    fn test_large_transaction_priority() {
        // > 100 BTC should be critical
        let notification = AdminNotification::large_transaction(
            "txid",
            15_000_000_000, // 150 BTC
            "bc1qtest",
            None,
        );
        assert_eq!(notification.priority, NotificationPriority::Critical);

        // > 10 BTC should be high
        let notification = AdminNotification::large_transaction(
            "txid",
            5_000_000_000, // 50 BTC
            "bc1qtest",
            None,
        );
        assert_eq!(notification.priority, NotificationPriority::High);
    }

    #[tokio::test]
    async fn test_notification_service() {
        let service = AdminNotificationService::new();

        let notification = AdminNotification::new(
            NotificationCategory::Info,
            NotificationPriority::Low,
            "Test",
            "Test notification",
        );

        service.notify(notification).await;

        let recent = service.get_recent(10).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].title, "Test");
    }
}
