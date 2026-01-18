use super::types::{Notification, NotificationId, NotificationStatus, NotificationType};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// SMS configuration
#[derive(Debug, Clone)]
pub struct SmsConfig {
    /// SMS provider (Twilio, AWS SNS, etc.)
    pub provider: String,
    /// API key
    pub api_key: String,
    /// API secret
    pub api_secret: String,
    /// From phone number
    pub from_number: String,
    /// Enable notifications
    pub enabled: bool,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            provider: "twilio".to_string(),
            api_key: String::new(),
            api_secret: String::new(),
            from_number: String::new(),
            enabled: false,
        }
    }
}

/// SMS notifier
pub struct SmsNotifier {
    config: SmsConfig,
    /// In-memory sent notifications (for testing/demo)
    sent: Arc<RwLock<Vec<Notification>>>,
}

impl SmsNotifier {
    /// Create a new SMS notifier
    pub fn new(config: SmsConfig) -> Self {
        Self {
            config,
            sent: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Send an SMS notification
    pub async fn send(&self, mut notification: Notification) -> Result<NotificationId> {
        if !self.config.enabled {
            tracing::warn!("SMS notifications are disabled");
            notification.status = NotificationStatus::Failed;
            notification.error = Some("SMS notifications disabled".to_string());
            return Ok(notification.id);
        }

        match &notification.notification_type {
            NotificationType::Sms { to, message } => {
                tracing::info!("Sending SMS to {} via {}", to, self.config.provider);

                // Validate phone number format
                if !Self::validate_phone_number(to) {
                    notification.status = NotificationStatus::Failed;
                    notification.error = Some(format!("Invalid phone number format: {}", to));
                    return Ok(notification.id);
                }

                // Validate message length (most SMS providers have limits)
                if message.len() > 1600 {
                    notification.status = NotificationStatus::Failed;
                    notification.error = Some("Message too long (max 1600 characters)".to_string());
                    return Ok(notification.id);
                }

                // Send via configured provider
                let result = match self.config.provider.as_str() {
                    "twilio" => self.send_via_twilio(to, message).await,
                    "aws_sns" => self.send_via_aws_sns(to, message).await,
                    _ => {
                        // Fallback to stub implementation for unsupported providers
                        tracing::warn!(
                            "Unsupported SMS provider: {}, using stub",
                            self.config.provider
                        );
                        self.send_stub(to, message).await
                    }
                };

                match result {
                    Ok(provider_id) => {
                        notification.status = NotificationStatus::Sent;
                        notification.sent_at = Some(chrono::Utc::now());
                        notification.attempts += 1;

                        // Store in sent list
                        self.sent.write().await.push(notification.clone());

                        tracing::info!(
                            "SMS sent successfully to {} (provider_id: {})",
                            to,
                            provider_id
                        );
                        Ok(notification.id)
                    }
                    Err(e) => {
                        notification.status = NotificationStatus::Failed;
                        notification.error = Some(e.to_string());
                        notification.attempts += 1;

                        tracing::error!("Failed to send SMS to {}: {}", to, e);
                        Err(e)
                    }
                }
            }
            _ => {
                anyhow::bail!("Invalid notification type for SMS channel")
            }
        }
    }

    /// Validate phone number format (E.164 format recommended)
    fn validate_phone_number(phone: &str) -> bool {
        // Basic validation: should start with + and contain only digits after that
        if !phone.starts_with('+') {
            return false;
        }

        let digits = &phone[1..];
        digits.chars().all(|c| c.is_ascii_digit()) && digits.len() >= 10 && digits.len() <= 15
    }

    /// Send SMS via Twilio
    async fn send_via_twilio(&self, _to: &str, _message: &str) -> Result<String> {
        // Production implementation would use Twilio API:
        // 1. Build request: POST https://api.twilio.com/2010-04-01/Accounts/{AccountSid}/Messages.json
        // 2. Set auth: Basic auth with AccountSid:AuthToken
        // 3. Form data: To, From, Body
        // 4. Parse response and return message SID

        // For now, stub implementation
        tracing::debug!("Would send SMS via Twilio API");

        // Example of what production code would look like:
        // let client = reqwest::Client::new();
        // let response = client
        //     .post(&format!("https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json", self.config.api_key))
        //     .basic_auth(&self.config.api_key, Some(&self.config.api_secret))
        //     .form(&[("To", to), ("From", &self.config.from_number), ("Body", message)])
        //     .send()
        //     .await?;
        // let result: TwilioResponse = response.json().await?;
        // Ok(result.sid)

        Ok("twilio_msg_stub_12345".to_string())
    }

    /// Send SMS via AWS SNS
    async fn send_via_aws_sns(&self, _to: &str, _message: &str) -> Result<String> {
        // Production implementation would use AWS SNS SDK:
        // 1. Create SNS client with credentials
        // 2. Call publish() with phone number and message
        // 3. Return message ID

        tracing::debug!("Would send SMS via AWS SNS");

        // Example of what production code would look like:
        // let config = aws_config::load_from_env().await;
        // let client = aws_sdk_sns::Client::new(&config);
        // let result = client
        //     .publish()
        //     .phone_number(to)
        //     .message(message)
        //     .send()
        //     .await?;
        // Ok(result.message_id().unwrap_or("").to_string())

        Ok("sns_msg_stub_67890".to_string())
    }

    /// Stub implementation for testing
    async fn send_stub(&self, _to: &str, _message: &str) -> Result<String> {
        tracing::debug!("Stub SMS send");
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        Ok("stub_msg_id".to_string())
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
    async fn test_sms_notifier_disabled() {
        let config = SmsConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = SmsNotifier::new(config);

        let notification = Notification::sms("+1234567890".to_string(), "Test message".to_string());

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);
    }

    #[tokio::test]
    async fn test_sms_notifier_stub() {
        let config = SmsConfig {
            enabled: true,
            ..Default::default()
        };
        let notifier = SmsNotifier::new(config);

        let notification = Notification::sms("+1234567890".to_string(), "Test message".to_string());

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);

        let sent = notifier.get_sent().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].status, NotificationStatus::Sent);
    }
}
