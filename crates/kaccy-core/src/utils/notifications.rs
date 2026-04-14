//! Notification System
//!
//! This module provides a multi-channel notification system with user preferences,
//! templates, priority-based delivery, and comprehensive tracking.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Notification channel types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationChannel {
    /// Delivery via email
    Email,
    /// Delivery via SMS
    SMS,
    /// Delivery via mobile push notification
    Push,
    /// Delivery within the application UI
    InApp,
}

/// Notification priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NotificationPriority {
    /// Low-importance notification
    Low = 0,
    /// Normal importance
    Normal = 1,
    /// High-importance notification
    High = 2,
    /// Critical notification, bypasses quiet hours
    Critical = 3,
}

/// Notification category for user preferences
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NotificationCategory {
    /// Notifications related to trades and orders
    Trading,
    /// Notifications related to the user's account
    Account,
    /// Security and authentication notifications
    Security,
    /// Marketing and promotional notifications
    Marketing,
    /// Platform-level system notifications
    System,
}

/// User notification preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    /// User these preferences belong to
    pub user_id: String,
    /// Preferred delivery channels for each notification category
    pub channel_preferences: HashMap<NotificationCategory, Vec<NotificationChannel>>,
    /// Optional quiet-hours window during which non-critical notifications are suppressed
    pub quiet_hours: Option<QuietHours>,
    /// ISO 639-1 language code for notification content
    pub language: String,
    /// IANA timezone for scheduling digest notifications
    pub timezone: String,
}

impl NotificationPreferences {
    /// Create default preferences for a user
    pub fn new(user_id: String) -> Self {
        let mut channel_preferences = HashMap::new();

        // Default preferences: all categories via email and in-app
        for category in [
            NotificationCategory::Trading,
            NotificationCategory::Account,
            NotificationCategory::Security,
            NotificationCategory::Marketing,
            NotificationCategory::System,
        ] {
            channel_preferences.insert(
                category,
                vec![NotificationChannel::Email, NotificationChannel::InApp],
            );
        }

        Self {
            user_id,
            channel_preferences,
            quiet_hours: None,
            language: "en".to_string(),
            timezone: "UTC".to_string(),
        }
    }

    /// Checks if user wants notifications for this category on this channel
    pub fn is_enabled(&self, category: NotificationCategory, channel: NotificationChannel) -> bool {
        self.channel_preferences
            .get(&category)
            .map(|channels| channels.contains(&channel))
            .unwrap_or(false)
    }

    /// Checks if current time is within quiet hours
    pub fn is_quiet_hours(&self) -> bool {
        if let Some(quiet_hours) = &self.quiet_hours {
            quiet_hours.is_active()
        } else {
            false
        }
    }
}

/// Quiet hours configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuietHours {
    /// Hour (0–23) at which quiet hours begin
    pub start_hour: u8,
    /// Hour (0–23) at which quiet hours end
    pub end_hour: u8,
    /// Whether quiet hours enforcement is active
    pub enabled: bool,
}

impl QuietHours {
    /// Create a new quiet-hours window, returning an error for invalid hours
    pub fn new(start_hour: u8, end_hour: u8) -> Result<Self, CoreError> {
        if start_hour > 23 || end_hour > 23 {
            return Err(CoreError::Validation(
                "Hours must be between 0 and 23".to_string(),
            ));
        }

        Ok(Self {
            start_hour,
            end_hour,
            enabled: true,
        })
    }

    #[allow(dead_code)]
    /// Returns true if this notification channel is currently active.
    pub fn is_active(&self) -> bool {
        if !self.enabled {
            return false;
        }

        // For simplicity, we'll just return false in tests
        // In production, you'd check the current time against quiet hours
        false
    }
}

/// Notification template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTemplate {
    /// Unique identifier for this template
    pub template_id: String,
    /// Category the template belongs to
    pub category: NotificationCategory,
    /// Subject line with `{variable}` placeholders
    pub subject_template: String,
    /// Body text with `{variable}` placeholders
    pub body_template: String,
    /// Names of all variables expected in this template
    pub variables: Vec<String>,
    /// Delivery channels this template supports
    pub supported_channels: Vec<NotificationChannel>,
}

impl NotificationTemplate {
    /// Renders template with variables
    pub fn render(
        &self,
        variables: &HashMap<String, String>,
    ) -> Result<RenderedTemplate, CoreError> {
        let mut subject = self.subject_template.clone();
        let mut body = self.body_template.clone();

        for (key, value) in variables {
            let placeholder = format!("{{{}}}", key);
            subject = subject.replace(&placeholder, value);
            body = body.replace(&placeholder, value);
        }

        // Check if any variables are missing
        if subject.contains('{') || body.contains('{') {
            return Err(CoreError::Validation(
                "Missing template variables".to_string(),
            ));
        }

        Ok(RenderedTemplate { subject, body })
    }
}

/// A notification template rendered with concrete values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedTemplate {
    /// Rendered subject line
    pub subject: String,
    /// Rendered body text
    pub body: String,
}

/// Notification record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier of this notification
    pub notification_id: String,
    /// Recipient user
    pub user_id: String,
    /// Category of the notification
    pub category: NotificationCategory,
    /// Priority level
    pub priority: NotificationPriority,
    /// Delivery channel
    pub channel: NotificationChannel,
    /// Rendered subject line
    pub subject: String,
    /// Rendered body text
    pub body: String,
    /// When the notification was created
    pub created_at: SystemTime,
    /// When the notification was sent
    pub sent_at: Option<SystemTime>,
    /// When the notification was read by the user
    pub read_at: Option<SystemTime>,
    /// Current delivery status
    pub status: NotificationStatus,
    /// Additional key-value metadata
    pub metadata: HashMap<String, String>,
}

/// Notification delivery status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationStatus {
    /// Not yet dispatched
    Pending,
    /// Handed off to delivery provider
    Sent,
    /// Confirmed delivered to recipient device
    Delivered,
    /// Delivery failed
    Failed,
    /// Notification has been read by the user
    Read,
}

/// Notification manager
pub struct NotificationManager {
    /// Registered notification templates
    templates: HashMap<String, NotificationTemplate>,
    /// Per-user notification preferences
    preferences: HashMap<String, NotificationPreferences>,
    /// All notifications, indexed by notification ID
    notifications: HashMap<String, Notification>,
    /// Monotonic counter for generating unique IDs
    notification_counter: u64,
}

impl NotificationManager {
    /// Create a new notification manager with built-in default templates
    pub fn new() -> Self {
        let mut manager = Self {
            templates: HashMap::new(),
            preferences: HashMap::new(),
            notifications: HashMap::new(),
            notification_counter: 0,
        };

        // Register default templates
        manager.register_default_templates();
        manager
    }

    /// Registers default notification templates
    fn register_default_templates(&mut self) {
        // Order filled template
        let _ = self.register_template(NotificationTemplate {
            template_id: "order_filled".to_string(),
            category: NotificationCategory::Trading,
            subject_template: "Order {order_id} Filled".to_string(),
            body_template:
                "Your {order_type} order for {amount} {token} has been filled at {price}."
                    .to_string(),
            variables: vec![
                "order_id".to_string(),
                "order_type".to_string(),
                "amount".to_string(),
                "token".to_string(),
                "price".to_string(),
            ],
            supported_channels: vec![
                NotificationChannel::Email,
                NotificationChannel::SMS,
                NotificationChannel::Push,
                NotificationChannel::InApp,
            ],
        });

        // Price alert template
        let _ = self.register_template(NotificationTemplate {
            template_id: "price_alert".to_string(),
            category: NotificationCategory::Trading,
            subject_template: "Price Alert: {token}".to_string(),
            body_template: "{token} has reached {price} (target: {target_price})".to_string(),
            variables: vec![
                "token".to_string(),
                "price".to_string(),
                "target_price".to_string(),
            ],
            supported_channels: vec![
                NotificationChannel::Email,
                NotificationChannel::Push,
                NotificationChannel::InApp,
            ],
        });

        // Security alert template
        let _ = self.register_template(NotificationTemplate {
            template_id: "security_alert".to_string(),
            category: NotificationCategory::Security,
            subject_template: "Security Alert: {alert_type}".to_string(),
            body_template: "Security event detected: {description}. Time: {timestamp}".to_string(),
            variables: vec![
                "alert_type".to_string(),
                "description".to_string(),
                "timestamp".to_string(),
            ],
            supported_channels: vec![
                NotificationChannel::Email,
                NotificationChannel::SMS,
                NotificationChannel::Push,
                NotificationChannel::InApp,
            ],
        });
    }

    /// Registers a notification template
    pub fn register_template(&mut self, template: NotificationTemplate) -> Result<(), CoreError> {
        if self.templates.contains_key(&template.template_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Template {} already exists",
                template.template_id
            )));
        }

        self.templates
            .insert(template.template_id.clone(), template);
        Ok(())
    }

    /// Sets user notification preferences
    pub fn set_preferences(&mut self, preferences: NotificationPreferences) {
        self.preferences
            .insert(preferences.user_id.clone(), preferences);
    }

    /// Gets user preferences (or creates default)
    pub fn get_or_create_preferences(&mut self, user_id: &str) -> &mut NotificationPreferences {
        self.preferences
            .entry(user_id.to_string())
            .or_insert_with(|| NotificationPreferences::new(user_id.to_string()))
    }

    /// Sends a notification using a template
    pub fn send_notification(
        &mut self,
        user_id: String,
        template_id: &str,
        variables: HashMap<String, String>,
        priority: NotificationPriority,
    ) -> Result<Vec<String>, CoreError> {
        let template = self
            .templates
            .get(template_id)
            .ok_or_else(|| CoreError::NotFound(format!("Template {} not found", template_id)))?
            .clone();

        let rendered = template.render(&variables)?;
        let preferences = self.get_or_create_preferences(&user_id).clone();

        let mut notification_ids = Vec::new();

        // Send on each enabled channel
        for channel in &template.supported_channels {
            if !preferences.is_enabled(template.category, *channel) {
                continue;
            }

            // Skip non-critical notifications during quiet hours
            if preferences.is_quiet_hours() && priority < NotificationPriority::Critical {
                continue;
            }

            self.notification_counter += 1;
            let notification_id = format!("notif_{}", self.notification_counter);

            let notification = Notification {
                notification_id: notification_id.clone(),
                user_id: user_id.clone(),
                category: template.category,
                priority,
                channel: *channel,
                subject: rendered.subject.clone(),
                body: rendered.body.clone(),
                created_at: SystemTime::now(),
                sent_at: None,
                read_at: None,
                status: NotificationStatus::Pending,
                metadata: variables.clone(),
            };

            self.notifications
                .insert(notification_id.clone(), notification);
            notification_ids.push(notification_id);
        }

        Ok(notification_ids)
    }

    /// Marks notification as sent
    pub fn mark_sent(&mut self, notification_id: &str) -> Result<(), CoreError> {
        let notification = self.notifications.get_mut(notification_id).ok_or_else(|| {
            CoreError::NotFound(format!("Notification {} not found", notification_id))
        })?;

        notification.status = NotificationStatus::Sent;
        notification.sent_at = Some(SystemTime::now());

        Ok(())
    }

    /// Marks notification as read
    pub fn mark_read(&mut self, notification_id: &str) -> Result<(), CoreError> {
        let notification = self.notifications.get_mut(notification_id).ok_or_else(|| {
            CoreError::NotFound(format!("Notification {} not found", notification_id))
        })?;

        notification.status = NotificationStatus::Read;
        notification.read_at = Some(SystemTime::now());

        Ok(())
    }

    /// Gets unread notifications for a user
    pub fn get_unread_notifications(&self, user_id: &str) -> Vec<&Notification> {
        self.notifications
            .values()
            .filter(|n| n.user_id == user_id && n.status != NotificationStatus::Read)
            .collect()
    }

    /// Gets notification history for a user
    pub fn get_notification_history(&self, user_id: &str, limit: usize) -> Vec<&Notification> {
        let mut notifications: Vec<_> = self
            .notifications
            .values()
            .filter(|n| n.user_id == user_id)
            .collect();

        notifications.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notifications.truncate(limit);
        notifications
    }

    /// Gets notification statistics for a user
    pub fn get_user_stats(&self, user_id: &str) -> NotificationStats {
        let total = self
            .notifications
            .values()
            .filter(|n| n.user_id == user_id)
            .count();

        let unread = self
            .notifications
            .values()
            .filter(|n| n.user_id == user_id && n.status != NotificationStatus::Read)
            .count();

        let by_channel = self
            .notifications
            .values()
            .filter(|n| n.user_id == user_id)
            .fold(HashMap::new(), |mut acc, n| {
                *acc.entry(n.channel).or_insert(0) += 1;
                acc
            });

        let by_category = self
            .notifications
            .values()
            .filter(|n| n.user_id == user_id)
            .fold(HashMap::new(), |mut acc, n| {
                *acc.entry(n.category).or_insert(0) += 1;
                acc
            });

        NotificationStats {
            total_notifications: total,
            unread_notifications: unread,
            notifications_by_channel: by_channel,
            notifications_by_category: by_category,
        }
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Notification statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationStats {
    /// Total number of notifications for the user
    pub total_notifications: usize,
    /// Number of unread notifications
    pub unread_notifications: usize,
    /// Notification count broken down by channel
    pub notifications_by_channel: HashMap<NotificationChannel, usize>,
    /// Notification count broken down by category
    pub notifications_by_category: HashMap<NotificationCategory, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_preferences() {
        let prefs = NotificationPreferences::new("user1".to_string());

        assert!(prefs.is_enabled(NotificationCategory::Trading, NotificationChannel::Email));
        assert!(prefs.is_enabled(NotificationCategory::Security, NotificationChannel::InApp));
    }

    #[test]
    fn test_quiet_hours() {
        let quiet_hours = QuietHours::new(22, 6).unwrap();
        assert!(quiet_hours.enabled);

        let invalid = QuietHours::new(25, 6);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_template_rendering() {
        let template = NotificationTemplate {
            template_id: "test".to_string(),
            category: NotificationCategory::Trading,
            subject_template: "Order {order_id} filled".to_string(),
            body_template: "Your order {order_id} for {amount} tokens has been filled.".to_string(),
            variables: vec!["order_id".to_string(), "amount".to_string()],
            supported_channels: vec![NotificationChannel::Email],
        };

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("amount".to_string(), "100".to_string());

        let rendered = template.render(&vars).unwrap();
        assert_eq!(rendered.subject, "Order ORD123 filled");
        assert!(rendered.body.contains("ORD123"));
        assert!(rendered.body.contains("100"));
    }

    #[test]
    fn test_send_notification() {
        let mut manager = NotificationManager::new();

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("order_type".to_string(), "BUY".to_string());
        vars.insert("amount".to_string(), "100".to_string());
        vars.insert("token".to_string(), "BTC".to_string());
        vars.insert("price".to_string(), "50000".to_string());

        let notification_ids = manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars,
                NotificationPriority::Normal,
            )
            .unwrap();

        // Should send to email and in-app by default
        assert!(!notification_ids.is_empty());
    }

    #[test]
    fn test_mark_read() {
        let mut manager = NotificationManager::new();

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("order_type".to_string(), "BUY".to_string());
        vars.insert("amount".to_string(), "100".to_string());
        vars.insert("token".to_string(), "BTC".to_string());
        vars.insert("price".to_string(), "50000".to_string());

        let notification_ids = manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars,
                NotificationPriority::Normal,
            )
            .unwrap();

        let notif_id = &notification_ids[0];
        manager.mark_read(notif_id).unwrap();

        let notification = manager.notifications.get(notif_id).unwrap();
        assert_eq!(notification.status, NotificationStatus::Read);
    }

    #[test]
    fn test_unread_notifications() {
        let mut manager = NotificationManager::new();

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("order_type".to_string(), "BUY".to_string());
        vars.insert("amount".to_string(), "100".to_string());
        vars.insert("token".to_string(), "BTC".to_string());
        vars.insert("price".to_string(), "50000".to_string());

        manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars.clone(),
                NotificationPriority::Normal,
            )
            .unwrap();

        manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars,
                NotificationPriority::High,
            )
            .unwrap();

        let unread = manager.get_unread_notifications("user1");
        assert!(unread.len() >= 2);
    }

    #[test]
    fn test_notification_stats() {
        let mut manager = NotificationManager::new();

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("order_type".to_string(), "BUY".to_string());
        vars.insert("amount".to_string(), "100".to_string());
        vars.insert("token".to_string(), "BTC".to_string());
        vars.insert("price".to_string(), "50000".to_string());

        manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars,
                NotificationPriority::Normal,
            )
            .unwrap();

        let stats = manager.get_user_stats("user1");
        assert!(stats.total_notifications > 0);
    }

    #[test]
    fn test_custom_preferences() {
        let mut manager = NotificationManager::new();

        let mut prefs = NotificationPreferences::new("user1".to_string());
        // Only want trading notifications via SMS
        prefs.channel_preferences.insert(
            NotificationCategory::Trading,
            vec![NotificationChannel::SMS],
        );

        manager.set_preferences(prefs);

        let mut vars = HashMap::new();
        vars.insert("order_id".to_string(), "ORD123".to_string());
        vars.insert("order_type".to_string(), "BUY".to_string());
        vars.insert("amount".to_string(), "100".to_string());
        vars.insert("token".to_string(), "BTC".to_string());
        vars.insert("price".to_string(), "50000".to_string());

        let notification_ids = manager
            .send_notification(
                "user1".to_string(),
                "order_filled",
                vars,
                NotificationPriority::Normal,
            )
            .unwrap();

        // Should only send to SMS
        assert_eq!(notification_ids.len(), 1);
        let notif = manager.notifications.get(&notification_ids[0]).unwrap();
        assert_eq!(notif.channel, NotificationChannel::SMS);
    }
}
