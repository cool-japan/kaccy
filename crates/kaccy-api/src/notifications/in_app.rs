use super::types::{Notification, NotificationId, NotificationStatus, NotificationType};
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// In-app notification configuration
#[derive(Debug, Clone)]
pub struct InAppConfig {
    /// Maximum notifications to keep per user
    pub max_notifications_per_user: usize,
    /// Enable notifications
    pub enabled: bool,
}

impl Default for InAppConfig {
    fn default() -> Self {
        Self {
            max_notifications_per_user: 100,
            enabled: true,
        }
    }
}

/// In-app notifier
pub struct InAppNotifier {
    config: InAppConfig,
    /// In-memory notification storage by user ID
    notifications: Arc<DashMap<Uuid, Vec<Notification>>>,
}

impl InAppNotifier {
    /// Create a new in-app notifier
    pub fn new(config: InAppConfig) -> Self {
        Self {
            config,
            notifications: Arc::new(DashMap::new()),
        }
    }

    /// Send an in-app notification
    pub async fn send(&self, mut notification: Notification) -> Result<NotificationId> {
        if !self.config.enabled {
            tracing::warn!("In-app notifications are disabled");
            notification.status = NotificationStatus::Failed;
            notification.error = Some("In-app notifications disabled".to_string());
            return Ok(notification.id);
        }

        match &notification.notification_type {
            NotificationType::InApp {
                user_id,
                title,
                message,
                action_url,
            } => {
                tracing::info!(
                    "Creating in-app notification for user {} with title '{}'",
                    user_id,
                    title
                );

                // For now, just log the details
                tracing::debug!("In-app message: {}", message);
                if let Some(url) = action_url {
                    tracing::debug!("In-app action URL: {}", url);
                }

                notification.status = NotificationStatus::Sent;
                notification.sent_at = Some(chrono::Utc::now());
                notification.attempts += 1;

                // Store in user's notification list
                let mut entry = self.notifications.entry(*user_id).or_default();
                entry.push(notification.clone());

                // Enforce max notifications per user
                let current_len = entry.len();
                if current_len > self.config.max_notifications_per_user {
                    let to_remove = current_len - self.config.max_notifications_per_user;
                    entry.drain(0..to_remove);
                }

                Ok(notification.id)
            }
            _ => {
                anyhow::bail!("Invalid notification type for in-app channel")
            }
        }
    }

    /// Get notifications for a user
    pub async fn get_notifications(&self, user_id: &Uuid) -> Vec<Notification> {
        self.notifications
            .get(user_id)
            .map(|entry| entry.value().clone())
            .unwrap_or_default()
    }

    /// Get unread notifications for a user
    pub async fn get_unread_notifications(&self, user_id: &Uuid) -> Vec<Notification> {
        self.get_notifications(user_id)
            .await
            .into_iter()
            .filter(|n| n.status == NotificationStatus::Sent)
            .collect()
    }

    /// Mark a notification as read
    pub async fn mark_as_read(
        &self,
        user_id: &Uuid,
        notification_id: &NotificationId,
    ) -> Result<()> {
        if let Some(mut entry) = self.notifications.get_mut(user_id) {
            if let Some(notification) = entry.iter_mut().find(|n| n.id == *notification_id) {
                notification.status = NotificationStatus::Read;
                return Ok(());
            }
        }
        anyhow::bail!("Notification not found")
    }

    /// Mark all notifications as read for a user
    pub async fn mark_all_as_read(&self, user_id: &Uuid) -> Result<usize> {
        if let Some(mut entry) = self.notifications.get_mut(user_id) {
            let mut count = 0;
            for notification in entry.iter_mut() {
                if notification.status == NotificationStatus::Sent {
                    notification.status = NotificationStatus::Read;
                    count += 1;
                }
            }
            return Ok(count);
        }
        Ok(0)
    }

    /// Delete a notification
    pub async fn delete(&self, user_id: &Uuid, notification_id: &NotificationId) -> Result<()> {
        if let Some(mut entry) = self.notifications.get_mut(user_id) {
            if let Some(index) = entry.iter().position(|n| n.id == *notification_id) {
                entry.remove(index);
                return Ok(());
            }
        }
        anyhow::bail!("Notification not found")
    }

    /// Clear all notifications for a user
    pub async fn clear_all(&self, user_id: &Uuid) -> Result<usize> {
        if let Some(entry) = self.notifications.remove(user_id) {
            let count = entry.1.len();
            Ok(count)
        } else {
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_app_notifier() {
        let config = InAppConfig::default();
        let notifier = InAppNotifier::new(config);

        let user_id = Uuid::new_v4();
        let notification = Notification::in_app(
            user_id,
            "Test Title".to_string(),
            "Test Message".to_string(),
        );

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);

        let notifications = notifier.get_notifications(&user_id).await;
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].status, NotificationStatus::Sent);
    }

    #[tokio::test]
    async fn test_mark_as_read() {
        let config = InAppConfig::default();
        let notifier = InAppNotifier::new(config);

        let user_id = Uuid::new_v4();
        let notification = Notification::in_app(
            user_id,
            "Test Title".to_string(),
            "Test Message".to_string(),
        );

        let notification_id = notifier.send(notification).await.unwrap();

        let unread = notifier.get_unread_notifications(&user_id).await;
        assert_eq!(unread.len(), 1);

        notifier
            .mark_as_read(&user_id, &notification_id)
            .await
            .unwrap();

        let unread = notifier.get_unread_notifications(&user_id).await;
        assert_eq!(unread.len(), 0);
    }

    #[tokio::test]
    async fn test_max_notifications_per_user() {
        let config = InAppConfig {
            max_notifications_per_user: 5,
            enabled: true,
        };
        let notifier = InAppNotifier::new(config);

        let user_id = Uuid::new_v4();

        // Send 10 notifications
        for i in 0..10 {
            let notification =
                Notification::in_app(user_id, format!("Title {}", i), format!("Message {}", i));
            notifier.send(notification).await.unwrap();
        }

        let notifications = notifier.get_notifications(&user_id).await;
        // Should only keep the last 5
        assert_eq!(notifications.len(), 5);
    }

    #[tokio::test]
    async fn test_delete_notification() {
        let config = InAppConfig::default();
        let notifier = InAppNotifier::new(config);

        let user_id = Uuid::new_v4();
        let notification = Notification::in_app(
            user_id,
            "Test Title".to_string(),
            "Test Message".to_string(),
        );

        let notification_id = notifier.send(notification).await.unwrap();

        let notifications = notifier.get_notifications(&user_id).await;
        assert_eq!(notifications.len(), 1);

        notifier.delete(&user_id, &notification_id).await.unwrap();

        let notifications = notifier.get_notifications(&user_id).await;
        assert_eq!(notifications.len(), 0);
    }
}
