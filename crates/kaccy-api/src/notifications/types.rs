use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a notification
pub type NotificationId = Uuid;

/// Notification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationStatus {
    /// Notification is pending
    Pending,
    /// Notification was sent successfully
    Sent,
    /// Notification failed to send
    Failed,
    /// Notification was read (for in-app notifications)
    Read,
}

/// Notification types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationType {
    /// Email notification
    Email {
        /// Recipient email address
        to: String,
        /// Email subject line
        subject: String,
        /// Plain text email body
        body: String,
        /// Optional HTML email body
        html: Option<String>,
    },
    /// SMS notification
    Sms {
        /// Recipient phone number
        to: String,
        /// SMS message text
        message: String,
    },
    /// Push notification
    Push {
        /// Target user ID
        user_id: Uuid,
        /// Notification title
        title: String,
        /// Notification body text
        body: String,
        /// Optional JSON payload data
        data: Option<String>,
    },
    /// In-app notification
    InApp {
        /// Target user ID
        user_id: Uuid,
        /// Notification title
        title: String,
        /// Notification message body
        message: String,
        /// Optional URL to navigate to when the notification is clicked
        action_url: Option<String>,
    },
}

/// Notification definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique notification identifier
    pub id: NotificationId,
    /// Notification type and parameters
    pub notification_type: NotificationType,
    /// Notification status
    pub status: NotificationStatus,
    /// Number of retry attempts
    pub attempts: u32,
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Priority (higher = more urgent)
    pub priority: i32,
    /// When the notification was created
    pub created_at: DateTime<Utc>,
    /// When the notification was sent
    pub sent_at: Option<DateTime<Utc>>,
    /// Error message if failed
    pub error: Option<String>,
}

impl Notification {
    /// Create a new notification
    pub fn new(notification_type: NotificationType) -> Self {
        Self {
            id: Uuid::new_v4(),
            notification_type,
            status: NotificationStatus::Pending,
            attempts: 0,
            max_attempts: 3,
            priority: 0,
            created_at: Utc::now(),
            sent_at: None,
            error: None,
        }
    }

    /// Create an email notification
    pub fn email(to: String, subject: String, body: String) -> Self {
        Self::new(NotificationType::Email {
            to,
            subject,
            body,
            html: None,
        })
    }

    /// Create an email notification with HTML
    pub fn email_html(to: String, subject: String, body: String, html: String) -> Self {
        Self::new(NotificationType::Email {
            to,
            subject,
            body,
            html: Some(html),
        })
    }

    /// Create an SMS notification
    pub fn sms(to: String, message: String) -> Self {
        Self::new(NotificationType::Sms { to, message })
    }

    /// Create a push notification
    pub fn push(user_id: Uuid, title: String, body: String) -> Self {
        Self::new(NotificationType::Push {
            user_id,
            title,
            body,
            data: None,
        })
    }

    /// Create a push notification with data
    pub fn push_with_data(user_id: Uuid, title: String, body: String, data: String) -> Self {
        Self::new(NotificationType::Push {
            user_id,
            title,
            body,
            data: Some(data),
        })
    }

    /// Create an in-app notification
    pub fn in_app(user_id: Uuid, title: String, message: String) -> Self {
        Self::new(NotificationType::InApp {
            user_id,
            title,
            message,
            action_url: None,
        })
    }

    /// Create an in-app notification with action URL
    pub fn in_app_with_action(
        user_id: Uuid,
        title: String,
        message: String,
        action_url: String,
    ) -> Self {
        Self::new(NotificationType::InApp {
            user_id,
            title,
            message,
            action_url: Some(action_url),
        })
    }

    /// Set notification priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retry attempts
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Check if notification can be retried
    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_notification_creation() {
        let notif = Notification::email(
            "test@example.com".to_string(),
            "Test Subject".to_string(),
            "Test Body".to_string(),
        );

        assert_eq!(notif.status, NotificationStatus::Pending);
        assert_eq!(notif.attempts, 0);
        assert!(matches!(
            notif.notification_type,
            NotificationType::Email { .. }
        ));
    }

    #[test]
    fn test_sms_notification_creation() {
        let notif = Notification::sms("+1234567890".to_string(), "Test message".to_string());

        assert_eq!(notif.status, NotificationStatus::Pending);
        assert!(matches!(
            notif.notification_type,
            NotificationType::Sms { .. }
        ));
    }

    #[test]
    fn test_push_notification_creation() {
        let user_id = Uuid::new_v4();
        let notif = Notification::push(user_id, "Test Title".to_string(), "Test Body".to_string());

        assert_eq!(notif.status, NotificationStatus::Pending);
        assert!(matches!(
            notif.notification_type,
            NotificationType::Push { .. }
        ));
    }

    #[test]
    fn test_in_app_notification_creation() {
        let user_id = Uuid::new_v4();
        let notif = Notification::in_app(
            user_id,
            "Test Title".to_string(),
            "Test Message".to_string(),
        );

        assert_eq!(notif.status, NotificationStatus::Pending);
        assert!(matches!(
            notif.notification_type,
            NotificationType::InApp { .. }
        ));
    }

    #[test]
    fn test_notification_retry_logic() {
        let mut notif = Notification::email(
            "test@example.com".to_string(),
            "Test".to_string(),
            "Test".to_string(),
        )
        .with_max_attempts(3);

        assert!(notif.can_retry());

        notif.attempts = 2;
        assert!(notif.can_retry());

        notif.attempts = 3;
        assert!(!notif.can_retry());
    }
}
