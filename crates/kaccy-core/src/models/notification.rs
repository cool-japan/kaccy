//! Notification and communication models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// User notification
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Notification {
    pub notification_id: Uuid,
    pub user_id: Uuid,
    /// Type of notification
    pub notification_type: NotificationType,
    /// Notification title
    pub title: String,
    /// Notification message
    pub message: String,
    /// Additional data in JSON format
    pub data: Option<JsonValue>,
    /// Whether notification has been read
    pub is_read: bool,
    /// When notification was read
    pub read_at: Option<DateTime<Utc>>,
    /// Priority level
    pub priority: NotificationPriority,
    /// Related resource type (e.g., "order", "token", "commitment")
    pub resource_type: Option<String>,
    /// Related resource ID
    pub resource_id: Option<Uuid>,
    /// Action URL (for clickable notifications)
    pub action_url: Option<String>,
    /// Expiration time (for time-sensitive notifications)
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Type of notification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum NotificationType {
    // Order/Trade notifications
    OrderCreated,
    OrderFilled,
    OrderCancelled,
    OrderExpired,
    TradeExecuted,

    // Payment notifications
    PaymentReceived,
    PaymentConfirmed,
    PaymentExpired,
    PaymentRefund,

    // Token notifications
    TokenCreated,
    TokenUpdated,
    TokenPaused,
    TokenPriceAlert,
    NewTokenHolder,

    // Reputation/Commitment notifications
    CommitmentDeadlineApproaching,
    CommitmentDeadlinePassed,
    CommitmentVerified,
    CommitmentRejected,
    ReputationChanged,

    // KYC notifications
    KycSubmitted,
    KycApproved,
    KycRejected,
    KycDocumentRequired,

    // Governance notifications
    ProposalCreated,
    ProposalVoted,
    ProposalPassed,
    ProposalRejected,

    // Security notifications
    LoginFromNewDevice,
    PasswordChanged,
    TwoFactorEnabled,
    SecurityAlert,
    AccountLocked,

    // System notifications
    MaintenanceScheduled,
    SystemUpdate,
    #[default]
    AnnouncementGeneral,
    AnnouncementImportant,

    // Social notifications
    NewFollower,
    NewMessage,
    MentionedInComment,
}

impl fmt::Display for NotificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Priority level for notifications
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, PartialOrd, Ord,
)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum NotificationPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

impl fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationPriority::Low => write!(f, "low"),
            NotificationPriority::Normal => write!(f, "normal"),
            NotificationPriority::High => write!(f, "high"),
            NotificationPriority::Urgent => write!(f, "urgent"),
        }
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Notification({}, type={}, priority={})",
            self.notification_id, self.notification_type, self.priority
        )
    }
}

impl Notification {
    /// Create a notification builder
    pub fn builder() -> NotificationBuilder {
        NotificationBuilder::new()
    }

    /// Check if notification has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Mark notification as read
    pub fn mark_as_read(&mut self) {
        self.is_read = true;
        self.read_at = Some(Utc::now());
    }

    /// Get age of notification in minutes
    pub fn age_minutes(&self) -> i64 {
        (Utc::now() - self.created_at).num_minutes()
    }

    /// Check if notification is recent (less than 1 hour old)
    pub fn is_recent(&self) -> bool {
        self.age_minutes() < 60
    }
}

/// Builder for creating notifications
pub struct NotificationBuilder {
    user_id: Option<Uuid>,
    notification_type: Option<NotificationType>,
    title: Option<String>,
    message: Option<String>,
    data: Option<JsonValue>,
    priority: NotificationPriority,
    resource_type: Option<String>,
    resource_id: Option<Uuid>,
    action_url: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

impl NotificationBuilder {
    pub fn new() -> Self {
        Self {
            user_id: None,
            notification_type: None,
            title: None,
            message: None,
            data: None,
            priority: NotificationPriority::Normal,
            resource_type: None,
            resource_id: None,
            action_url: None,
            expires_at: None,
        }
    }

    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn notification_type(mut self, notification_type: NotificationType) -> Self {
        self.notification_type = Some(notification_type);
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    pub fn data(mut self, data: JsonValue) -> Self {
        self.data = Some(data);
        self
    }

    pub fn priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn resource(mut self, resource_type: String, resource_id: Uuid) -> Self {
        self.resource_type = Some(resource_type);
        self.resource_id = Some(resource_id);
        self
    }

    pub fn action_url(mut self, url: String) -> Self {
        self.action_url = Some(url);
        self
    }

    pub fn expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn build(self) -> Result<Notification, String> {
        Ok(Notification {
            notification_id: Uuid::new_v4(),
            user_id: self
                .user_id
                .ok_or_else(|| "User ID is required".to_string())?,
            notification_type: self
                .notification_type
                .ok_or_else(|| "Notification type is required".to_string())?,
            title: self.title.ok_or_else(|| "Title is required".to_string())?,
            message: self
                .message
                .ok_or_else(|| "Message is required".to_string())?,
            data: self.data,
            is_read: false,
            read_at: None,
            priority: self.priority,
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            action_url: self.action_url,
            expires_at: self.expires_at,
            created_at: Utc::now(),
        })
    }
}

impl Default for NotificationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// User notification preferences
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationPreferences {
    pub user_id: Uuid,
    /// Receive email notifications
    pub email_enabled: bool,
    /// Receive in-app notifications
    pub in_app_enabled: bool,
    /// Receive push notifications (if mobile app)
    pub push_enabled: bool,
    /// Notification frequency
    pub frequency: NotificationFrequency,
    /// Enabled notification types (JSON array)
    pub enabled_types: Option<JsonValue>,
    /// Quiet hours start (UTC)
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    /// Quiet hours end (UTC)
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub updated_at: DateTime<Utc>,
}

/// Notification delivery frequency
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum NotificationFrequency {
    /// Immediate notifications
    #[default]
    Immediate,
    /// Hourly digest
    Hourly,
    /// Daily digest
    Daily,
    /// Weekly digest
    Weekly,
}

impl fmt::Display for NotificationFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationFrequency::Immediate => write!(f, "immediate"),
            NotificationFrequency::Hourly => write!(f, "hourly"),
            NotificationFrequency::Daily => write!(f, "daily"),
            NotificationFrequency::Weekly => write!(f, "weekly"),
        }
    }
}

impl NotificationPreferences {
    /// Check if user should receive notification now
    pub fn should_receive_now(&self, notification_type: NotificationType) -> bool {
        // Check if any delivery method is enabled
        if !self.email_enabled && !self.in_app_enabled && !self.push_enabled {
            return false;
        }

        // Check if this notification type is enabled
        if let Some(ref enabled) = self.enabled_types {
            if let Some(types) = enabled.as_array() {
                let type_str = format!("{:?}", notification_type);
                if !types.iter().any(|t| t.as_str() == Some(&type_str)) {
                    return false;
                }
            }
        }

        // Check quiet hours
        if let (Some(start), Some(end)) = (self.quiet_hours_start, self.quiet_hours_end) {
            let now = Utc::now().time();
            if start < end {
                // Normal case: quiet hours don't cross midnight
                if now >= start && now < end {
                    return false;
                }
            } else {
                // Quiet hours cross midnight
                if now >= start || now < end {
                    return false;
                }
            }
        }

        true
    }

    /// Check if using digest mode
    pub fn is_digest_mode(&self) -> bool {
        !matches!(self.frequency, NotificationFrequency::Immediate)
    }
}

/// Email template for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTemplate {
    pub template_name: String,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    /// Variables to replace in template (e.g., {{username}}, {{amount}})
    pub variables: Vec<String>,
}

/// Notification statistics
#[derive(Debug, Serialize)]
pub struct NotificationStats {
    pub user_id: Uuid,
    pub total_notifications: i64,
    pub unread_notifications: i64,
    pub notifications_last_24h: i64,
    pub most_common_type: Option<NotificationType>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_builder() {
        let notification = Notification::builder()
            .user_id(Uuid::new_v4())
            .notification_type(NotificationType::OrderFilled)
            .title("Order Filled".to_string())
            .message("Your order has been filled".to_string())
            .priority(NotificationPriority::High)
            .build()
            .unwrap();

        assert_eq!(
            notification.notification_type,
            NotificationType::OrderFilled
        );
        assert_eq!(notification.priority, NotificationPriority::High);
        assert!(!notification.is_read);
    }

    #[test]
    fn test_notification_expiration() {
        let mut notification = Notification::builder()
            .user_id(Uuid::new_v4())
            .notification_type(NotificationType::OrderExpired)
            .title("Test".to_string())
            .message("Test message".to_string())
            .expires_at(Utc::now() - chrono::Duration::hours(1))
            .build()
            .unwrap();

        assert!(notification.is_expired());

        notification.expires_at = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!notification.is_expired());
    }

    #[test]
    fn test_mark_as_read() {
        let mut notification = Notification::builder()
            .user_id(Uuid::new_v4())
            .notification_type(NotificationType::TradeExecuted)
            .title("Test".to_string())
            .message("Test message".to_string())
            .build()
            .unwrap();

        assert!(!notification.is_read);
        assert!(notification.read_at.is_none());

        notification.mark_as_read();

        assert!(notification.is_read);
        assert!(notification.read_at.is_some());
    }

    #[test]
    fn test_notification_preferences() {
        let prefs = NotificationPreferences {
            user_id: Uuid::new_v4(),
            email_enabled: true,
            in_app_enabled: true,
            push_enabled: false,
            frequency: NotificationFrequency::Immediate,
            enabled_types: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            updated_at: Utc::now(),
        };

        assert!(!prefs.is_digest_mode());

        // Test that notifications are enabled
        assert!(prefs.should_receive_now(NotificationType::OrderFilled));
    }

    #[test]
    fn test_digest_mode() {
        let prefs = NotificationPreferences {
            user_id: Uuid::new_v4(),
            email_enabled: true,
            in_app_enabled: true,
            push_enabled: false,
            frequency: NotificationFrequency::Daily,
            enabled_types: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            updated_at: Utc::now(),
        };

        assert!(prefs.is_digest_mode());
    }
}
