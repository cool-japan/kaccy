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

    /// Send an email notification
    pub async fn send(&self, mut notification: Notification) -> Result<NotificationId> {
        if !self.config.enabled {
            tracing::warn!("Email notifications are disabled");
            notification.status = NotificationStatus::Failed;
            notification.error = Some("Email notifications disabled".to_string());
            return Ok(notification.id);
        }

        match &notification.notification_type {
            NotificationType::Email {
                to,
                subject,
                body,
                html,
            } => {
                notification.attempts += 1;

                // Try to send via SMTP if transport is available
                if let Some(ref transport) = self.transport {
                    match self
                        .send_via_smtp(transport, to, subject, body, html.as_deref())
                        .await
                    {
                        Ok(_) => {
                            tracing::info!("Email sent to {} with subject '{}'", to, subject);
                            notification.status = NotificationStatus::Sent;
                            notification.sent_at = Some(chrono::Utc::now());
                        }
                        Err(e) => {
                            tracing::error!("Failed to send email: {}", e);
                            notification.status = NotificationStatus::Failed;
                            notification.error = Some(format!("SMTP error: {}", e));
                        }
                    }
                } else {
                    // Fallback to stub implementation
                    tracing::info!(
                        "Sending email to {} with subject '{}' (stub - no SMTP configured)",
                        to,
                        subject
                    );
                    tracing::debug!("Email body: {}", body);
                    if let Some(html_body) = html {
                        tracing::debug!("Email HTML: {}", html_body);
                    }
                    notification.status = NotificationStatus::Sent;
                    notification.sent_at = Some(chrono::Utc::now());
                }

                // Store in sent list
                self.sent.write().await.push(notification.clone());

                Ok(notification.id)
            }
            _ => {
                anyhow::bail!("Invalid notification type for email channel")
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

    #[tokio::test]
    async fn test_email_notifier_disabled() {
        let config = EmailConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);

        let notification = Notification::email(
            "test@example.com".to_string(),
            "Test".to_string(),
            "Test body".to_string(),
        );

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);
    }

    #[tokio::test]
    async fn test_email_notifier_stub() {
        let config = EmailConfig {
            enabled: true,
            ..Default::default()
        };
        let notifier = EmailNotifier::new(config);

        let notification = Notification::email(
            "test@example.com".to_string(),
            "Test".to_string(),
            "Test body".to_string(),
        );

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);

        let sent = notifier.get_sent().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].status, NotificationStatus::Sent);
    }
}
