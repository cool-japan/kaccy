use super::types::{Notification, NotificationId};
use anyhow::Result;
use async_trait::async_trait;

/// Notification channel trait
#[async_trait]
pub trait NotificationChannel: Send + Sync {
    /// Send a notification
    async fn send(&self, notification: Notification) -> Result<NotificationId>;

    /// Get the channel name
    fn name(&self) -> &str;

    /// Check if the channel is enabled
    fn is_enabled(&self) -> bool;
}
