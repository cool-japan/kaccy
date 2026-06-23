use super::types::{Notification, NotificationId, NotificationStatus, NotificationType};
use anyhow::Result;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Email configuration
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP server host
    pub smtp_host: String,
    /// SMTP server port
    pub smtp_port: u16,
    /// SMTP username
    pub smtp_username: String,
    /// SMTP password
    pub smtp_password: String,
    /// From email address
    pub from_address: String,
    /// From name
    pub from_name: String,
    /// Enable TLS
    pub use_tls: bool,
    /// Enable notifications
    pub enabled: bool,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_address: "noreply@kaccy.io".to_string(),
            from_name: "Kaccy".to_string(),
            use_tls: true,
            enabled: false,
        }
    }
}

/// Email notifier
pub struct EmailNotifier {
    config: EmailConfig,
    /// In-memory sent notifications (for testing/demo)
    sent: Arc<RwLock<Vec<Notification>>>,
    /// SMTP transport (optional, only if email is enabled)
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
}

impl EmailNotifier {
    /// Create a new email notifier
    pub fn new(config: EmailConfig) -> Self {
        let transport = if config.enabled && !config.smtp_username.is_empty() {
            let creds =
                Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

            let builder = if config.use_tls {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
            };

            match builder {
                Ok(transport) => Some(transport.credentials(creds).port(config.smtp_port).build()),
                Err(e) => {
                    tracing::error!("Failed to create SMTP transport: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            sent: Arc::new(RwLock::new(Vec::new())),
            transport,
        }
    }

    /// Send an email notification.
    ///
    /// Returns `Ok(id)` only when the message is actually delivered via SMTP.
    /// Returns `Err` (and never marks `Sent`) when:
    ///   - email notifications are disabled in config, or
    ///   - no SMTP transport is available (credentials missing / transport build failed), or
    ///   - the SMTP delivery itself fails.
    pub async fn send(&self, mut notification: Notification) -> Result<NotificationId> {
        if !self.config.enabled {
            tracing::warn!("Email notifications are disabled");
            notification.status = NotificationStatus::Failed;
            notification.error = Some("Email notifications disabled".to_string());
            self.sent.write().await.push(notification.clone());
            anyhow::bail!("email notifications are disabled");
        }

        match &notification.notification_type {
            NotificationType::Email {
                to,
                subject,
                body,
                html,
            } => {
                notification.attempts += 1;

                let transport = self.transport.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "email transport not configured: \
                         set smtp_username/smtp_password and enable the email notifier"
                    )
                })?;

                // Attempt real SMTP delivery — only mark Sent on success.
                match self
                    .send_via_smtp(transport, to, subject, body, html.as_deref())
                    .await
                {
                    Ok(()) => {
                        tracing::info!("Email sent to {} with subject '{}'", to, subject);
                        notification.status = NotificationStatus::Sent;
                        notification.sent_at = Some(chrono::Utc::now());
                        self.sent.write().await.push(notification.clone());
                        Ok(notification.id)
                    }
                    Err(e) => {
                        tracing::error!("Failed to send email to {}: {}", to, e);
                        notification.status = NotificationStatus::Failed;
                        notification.error = Some(format!("SMTP error: {e}"));
                        self.sent.write().await.push(notification.clone());
                        Err(anyhow::anyhow!("SMTP delivery failed: {e}"))
                    }
                }
            }
            _ => {
                anyhow::bail!("invalid notification type for email channel")
            }
        }
    }

    /// Send email via SMTP using lettre
    async fn send_via_smtp(
        &self,
        transport: &AsyncSmtpTransport<Tokio1Executor>,
        to: &str,
        subject: &str,
        body: &str,
        html: Option<&str>,
    ) -> Result<()> {
        let mut email_builder = Message::builder()
            .from(
                format!("{} <{}>", self.config.from_name, self.config.from_address)
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid from address: {}", e))?,
            )
            .to(to
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid to address: {}", e))?)
            .subject(subject);

        let message = if let Some(html_content) = html {
            // Send multipart email with both plain text and HTML
            email_builder = email_builder.header(ContentType::TEXT_HTML);
            email_builder
                .body(html_content.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to build email: {}", e))?
        } else {
            // Send plain text email
            email_builder = email_builder.header(ContentType::TEXT_PLAIN);
            email_builder
                .body(body.to_string())
                .map_err(|e| anyhow::anyhow!("Failed to build email: {}", e))?
        };

        transport
            .send(message)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send email: {}", e))?;

        Ok(())
    }

    /// Get sent notifications (for testing)
    pub async fn get_sent(&self) -> Vec<Notification> {
        self.sent.read().await.clone()
    }

    /// Clear sent notifications (for testing)
    pub async fn clear_sent(&self) {
        self.sent.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_email_notification() -> Notification {
        Notification::email(
            "test@example.com".to_string(),
            "Test Subject".to_string(),
            "Test body".to_string(),
        )
    }

    // -----------------------------------------------------------------------
    // Disabled config: send must return Err (never Sent)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_fails_when_email_disabled() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        let notification = make_email_notification();

        let result = notifier.send(notification).await;

        assert!(
            result.is_err(),
            "send must return Err when email notifications are disabled"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("disabled"),
            "error message should mention 'disabled', got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_disabled_records_failed_status_not_sent() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        let notification = make_email_notification();

        let _ = notifier.send(notification).await;

        let stored = notifier.get_sent().await;
        assert_eq!(stored.len(), 1, "failed attempt must still be recorded");
        assert_eq!(
            stored[0].status,
            NotificationStatus::Failed,
            "disabled-send must record Failed, not Sent"
        );
        assert!(
            stored[0].sent_at.is_none(),
            "sent_at must not be set when no delivery occurred"
        );
    }

    // -----------------------------------------------------------------------
    // Enabled but no SMTP credentials / transport: send must return Err (never Sent)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_fails_when_no_smtp_transport_configured() {
        // enabled=true but smtp_username is empty → transport stays None
        let config = EmailConfig {
            enabled: true,
            smtp_username: String::new(),
            smtp_password: String::new(),
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        // Verify the notifier was indeed built without a transport
        assert!(
            notifier.transport.is_none(),
            "transport must be None when credentials are absent"
        );

        let notification = make_email_notification();
        let result = notifier.send(notification).await;

        assert!(
            result.is_err(),
            "send must return Err when SMTP transport is not configured"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not configured"),
            "error message should mention 'not configured', got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_no_transport_records_failed_status_not_sent() {
        let config = EmailConfig {
            enabled: true,
            smtp_username: String::new(),
            smtp_password: String::new(),
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        let notification = make_email_notification();

        let _ = notifier.send(notification).await;

        // The notification must NOT be in the sent list as Sent
        let stored = notifier.get_sent().await;
        // No transport ⇒ bail before push, so list is empty
        assert_eq!(
            stored.len(),
            0,
            "notification must not be stored when transport is absent (bail before push)"
        );
    }

    // -----------------------------------------------------------------------
    // Wrong notification type: send must return Err
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_fails_for_wrong_notification_type() {
        let config = EmailConfig {
            enabled: true,
            smtp_username: "user".to_string(),
            smtp_password: "pass".to_string(),
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);

        // Use an SMS notification — not an Email — to trigger the wrong-type branch.
        let notification = Notification::new(NotificationType::Sms {
            to: "+1-555-0100".to_string(),
            message: "hello".to_string(),
        });

        let result = notifier.send(notification).await;
        assert!(
            result.is_err(),
            "send with wrong notification type must return Err"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("invalid notification type"),
            "error should describe the type mismatch, got: {err_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Invariant: Sent status is NEVER set without a successful SMTP round-trip
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sent_status_never_set_without_real_smtp() {
        // Both disabled and no-transport paths must not produce Sent entries.
        for (label, config) in [
            (
                "disabled",
                EmailConfig {
                    enabled: false,
                    ..Default::default()
                },
            ),
            (
                "no-transport",
                EmailConfig {
                    enabled: true,
                    smtp_username: String::new(),
                    ..Default::default()
                },
            ),
        ] {
            let notifier = EmailNotifier::new(config);
            let _ = notifier.send(make_email_notification()).await;

            let stored = notifier.get_sent().await;
            for entry in &stored {
                assert_ne!(
                    entry.status,
                    NotificationStatus::Sent,
                    "[{label}] found Sent status in stored notifications without a real SMTP send"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Config validation helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config_has_email_disabled() {
        let config = EmailConfig::default();
        assert!(
            !config.enabled,
            "default EmailConfig must have enabled=false to prevent accidental sends"
        );
    }

    #[test]
    fn test_new_notifier_has_no_transport_when_disabled() {
        let config = EmailConfig {
            enabled: false,
            smtp_username: "user".to_string(),
            smtp_password: "pass".to_string(),
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        assert!(
            notifier.transport.is_none(),
            "disabled config must not build a transport"
        );
    }

    #[test]
    fn test_new_notifier_has_no_transport_when_credentials_empty() {
        let config = EmailConfig {
            enabled: true,
            smtp_username: String::new(),
            smtp_password: String::new(),
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        assert!(
            notifier.transport.is_none(),
            "empty credentials must not build a transport"
        );
    }

    // -----------------------------------------------------------------------
    // get_sent / clear_sent housekeeping
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_clear_sent_empties_the_store() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);
        let _ = notifier.send(make_email_notification()).await;
        assert_eq!(notifier.get_sent().await.len(), 1);

        notifier.clear_sent().await;
        assert_eq!(
            notifier.get_sent().await.len(),
            0,
            "clear_sent must empty the store"
        );
    }
}
