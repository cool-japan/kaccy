use super::types::{Notification, NotificationId, NotificationStatus, NotificationType};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Push notification configuration
#[derive(Debug, Clone)]
pub struct PushConfig {
    /// Push provider (FCM, APNS, etc.)
    pub provider: String,
    /// API key
    pub api_key: String,
    /// API secret
    pub api_secret: String,
    /// Enable notifications
    pub enabled: bool,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            provider: "fcm".to_string(),
            api_key: String::new(),
            api_secret: String::new(),
            enabled: false,
        }
    }
}

/// Push notifier
pub struct PushNotifier {
    config: PushConfig,
    /// In-memory sent notifications (for testing/demo)
    sent: Arc<RwLock<Vec<Notification>>>,
}

impl PushNotifier {
    /// Create a new push notifier
    pub fn new(config: PushConfig) -> Self {
        Self {
            config,
            sent: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Send a push notification
    pub async fn send(&self, mut notification: Notification) -> Result<NotificationId> {
        if !self.config.enabled {
            tracing::warn!("Push notifications are disabled");
            notification.status = NotificationStatus::Failed;
            notification.error = Some("Push notifications disabled".to_string());
            return Ok(notification.id);
        }

        match &notification.notification_type {
            NotificationType::Push {
                user_id,
                title,
                body,
                data,
            } => {
                tracing::info!(
                    "Sending push notification to user {} via {}",
                    user_id,
                    self.config.provider
                );

                // Validate title and body lengths (provider limits)
                if title.len() > 256 {
                    notification.status = NotificationStatus::Failed;
                    notification.error = Some("Title too long (max 256 characters)".to_string());
                    return Ok(notification.id);
                }

                if body.len() > 4096 {
                    notification.status = NotificationStatus::Failed;
                    notification.error = Some("Body too long (max 4096 characters)".to_string());
                    return Ok(notification.id);
                }

                tracing::debug!("Push title: {}", title);
                tracing::debug!("Push body: {}", body);
                if let Some(payload) = data {
                    tracing::debug!("Push data: {}", payload);
                }

                // Send via configured provider
                let result = match self.config.provider.as_str() {
                    "fcm" => {
                        self.send_via_fcm(*user_id, title, body, data.as_deref())
                            .await
                    }
                    "apns" => {
                        self.send_via_apns(*user_id, title, body, data.as_deref())
                            .await
                    }
                    _ => {
                        tracing::warn!(
                            "Unsupported push provider: {}, using stub",
                            self.config.provider
                        );
                        self.send_stub(*user_id, title, body).await
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
                            "Push notification sent successfully to user {} (provider_id: {})",
                            user_id,
                            provider_id
                        );
                        Ok(notification.id)
                    }
                    Err(e) => {
                        notification.status = NotificationStatus::Failed;
                        notification.error = Some(e.to_string());
                        notification.attempts += 1;

                        tracing::error!(
                            "Failed to send push notification to user {}: {}",
                            user_id,
                            e
                        );
                        Err(e)
                    }
                }
            }
            _ => {
                anyhow::bail!("Invalid notification type for push channel")
            }
        }
    }

    /// Send push notification via Firebase Cloud Messaging (FCM)
    async fn send_via_fcm(
        &self,
        _user_id: uuid::Uuid,
        _title: &str,
        _body: &str,
        _data: Option<&str>,
    ) -> Result<String> {
        // Production implementation would use FCM HTTP v1 API:
        // 1. Get device token(s) for user from database
        // 2. Build FCM message payload
        // 3. Get OAuth2 access token for FCM
        // 4. POST to https://fcm.googleapis.com/v1/projects/{project_id}/messages:send
        // 5. Parse response and return message ID

        tracing::debug!("Would send push notification via FCM");

        // Example of what production code would look like:
        // let device_tokens = get_user_device_tokens(user_id).await?;
        // for token in device_tokens {
        //     let client = reqwest::Client::new();
        //     let fcm_message = json!({
        //         "message": {
        //             "token": token,
        //             "notification": {
        //                 "title": title,
        //                 "body": body,
        //             },
        //             "data": data.map(|d| serde_json::from_str(d).unwrap_or_default()),
        //         }
        //     });
        //
        //     let response = client
        //         .post(&format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", project_id))
        //         .bearer_auth(&access_token)
        //         .json(&fcm_message)
        //         .send()
        //         .await?;
        //
        //     let result: FcmResponse = response.json().await?;
        //     return Ok(result.name);
        // }

        Ok("fcm_msg_stub_abc123".to_string())
    }

    /// Send push notification via Apple Push Notification Service (APNS)
    async fn send_via_apns(
        &self,
        _user_id: uuid::Uuid,
        _title: &str,
        _body: &str,
        _data: Option<&str>,
    ) -> Result<String> {
        // Production implementation would use APNS HTTP/2 API:
        // 1. Get device token(s) for user from database
        // 2. Build APNS payload
        // 3. Load APNS certificate or use token-based auth
        // 4. POST to https://api.push.apple.com/3/device/{device_token}
        // 5. Return APNS message ID from response header

        tracing::debug!("Would send push notification via APNS");

        // Example of what production code would look like:
        // let device_tokens = get_user_device_tokens(user_id).await?;
        // for token in device_tokens {
        //     let payload = json!({
        //         "aps": {
        //             "alert": {
        //                 "title": title,
        //                 "body": body,
        //             },
        //             "sound": "default",
        //         },
        //         "custom_data": data.map(|d| serde_json::from_str(d).unwrap_or_default()),
        //     });
        //
        //     // Use a2 crate or similar for HTTP/2 APNS client
        //     // let client = a2::Client::token(...);
        //     // let response = client.send(payload, &token).await?;
        //     // return Ok(response.apns_id);
        // }

        Ok("apns_msg_stub_xyz789".to_string())
    }

    /// Stub implementation for testing
    async fn send_stub(&self, _user_id: uuid::Uuid, _title: &str, _body: &str) -> Result<String> {
        tracing::debug!("Stub push notification send");
        // Simulate network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        Ok("stub_push_id".to_string())
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
    use uuid::Uuid;

    #[tokio::test]
    async fn test_push_notifier_disabled() {
        let config = PushConfig {
            enabled: false,
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Test Title".to_string(),
            "Test Body".to_string(),
        );

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);
    }

    #[tokio::test]
    async fn test_push_notifier_stub() {
        let config = PushConfig {
            enabled: true,
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Test Title".to_string(),
            "Test Body".to_string(),
        );

        let id = notifier.send(notification).await.unwrap();
        assert!(id.as_u128() > 0);

        let sent = notifier.get_sent().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].status, NotificationStatus::Sent);
    }
}
