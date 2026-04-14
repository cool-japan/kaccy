/// Notification system for kaccy-api
///
/// This module provides a comprehensive notification infrastructure with multi-channel support:
///
/// ## Supported Channels
///
/// - **Email**: Full SMTP support via `lettre` library with HTML/plain text, attachments
/// - **SMS**: Multi-provider support (Twilio, AWS SNS) with E.164 phone validation
/// - **Push**: FCM (Android) and APNS (iOS) with payload validation and delivery tracking
/// - **In-app**: Database-persisted notifications with read/unread status tracking
///
/// ## Features
///
/// - Automatic retry logic with configurable max attempts
/// - Provider fallback and error handling
/// - Message validation (length limits, format checks)
/// - Delivery tracking and status management
/// - Production-ready stubs for testing and development
///
/// ## Usage Example
///
/// ```rust,ignore
/// use kaccy_api::notifications::{NotificationManager, Notification};
///
/// let manager = NotificationManager::new(email, sms, push, in_app);
///
/// // Send email
/// let notification = Notification::email(
///     "user@example.com".to_string(),
///     "Welcome!".to_string(),
///     "Welcome to Kaccy Protocol".to_string(),
/// );
/// manager.send(notification).await?;
/// ```
/// Notification channel abstractions and trait definitions
pub mod channels;
/// Email notification sending via SMTP
pub mod email;
/// In-application notification storage and delivery
pub mod in_app;
/// Push notification delivery to mobile devices
pub mod push;
/// SMS notification delivery
pub mod sms;
/// Notification type definitions and core data structures
pub mod types;

pub use channels::NotificationChannel;
pub use email::{EmailConfig, EmailNotifier};
pub use in_app::{InAppConfig, InAppNotifier};
pub use push::{PushConfig, PushNotifier};
pub use sms::{SmsConfig, SmsNotifier};
pub use types::{Notification, NotificationId, NotificationStatus, NotificationType};

use anyhow::Result;
use std::sync::Arc;

/// Main notification manager that coordinates all notification channels
pub struct NotificationManager {
    email: Arc<EmailNotifier>,
    sms: Arc<SmsNotifier>,
    push: Arc<PushNotifier>,
    in_app: Arc<InAppNotifier>,
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new(
        email: EmailNotifier,
        sms: SmsNotifier,
        push: PushNotifier,
        in_app: InAppNotifier,
    ) -> Self {
        Self {
            email: Arc::new(email),
            sms: Arc::new(sms),
            push: Arc::new(push),
            in_app: Arc::new(in_app),
        }
    }

    /// Send a notification through the appropriate channel
    pub async fn send(&self, notification: Notification) -> Result<NotificationId> {
        match notification.notification_type {
            NotificationType::Email { .. } => self.email.send(notification).await,
            NotificationType::Sms { .. } => self.sms.send(notification).await,
            NotificationType::Push { .. } => self.push.send(notification).await,
            NotificationType::InApp { .. } => self.in_app.send(notification).await,
        }
    }

    /// Get email notifier
    pub fn email(&self) -> Arc<EmailNotifier> {
        self.email.clone()
    }

    /// Get SMS notifier
    pub fn sms(&self) -> Arc<SmsNotifier> {
        self.sms.clone()
    }

    /// Get push notifier
    pub fn push(&self) -> Arc<PushNotifier> {
        self.push.clone()
    }

    /// Get in-app notifier
    pub fn in_app(&self) -> Arc<InAppNotifier> {
        self.in_app.clone()
    }
}
