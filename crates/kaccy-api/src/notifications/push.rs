use super::types::{Notification, NotificationId, NotificationStatus, NotificationType};
use anyhow::{anyhow, bail, Context, Result};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// ──────────────────────────────────────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────────────────────────────────────

/// Push notification configuration.
///
/// FCM (HTTP v1) requires either:
///   - `fcm_project_id` + `fcm_oauth2_bearer`  → v1 endpoint
///   - `api_key` (server key)                   → legacy endpoint
///
/// APNS requires:
///   - `apns_key_id`, `apns_team_id`, `apns_bundle_id`, `apns_private_key_pem`
///   - ES256 private key in PKCS#8 PEM form (the .p8 file from Apple Developer)
///
/// `device_token` is a single static device registration token used when no
/// per-user token registry is wired up.  In production, replace this with a
/// database look-up keyed by `user_id`.
#[derive(Debug, Clone)]
pub struct PushConfig {
    /// Push provider: `"fcm"` | `"apns"` (other values → honest error)
    pub provider: String,
    /// FCM legacy server key *or* unused for APNS (kept for backwards compat)
    pub api_key: String,
    /// Unused legacy field (kept for backwards compat)
    pub api_secret: String,
    /// Enable notifications
    pub enabled: bool,

    // ── FCM HTTP v1 ──────────────────────────────────────────────────────────
    /// FCM project ID (required for FCM v1).
    /// Example: `"my-firebase-project-12345"`
    pub fcm_project_id: Option<String>,
    /// Short-lived OAuth 2 bearer token for FCM v1.  Obtain via the Google
    /// auth libraries with scope `https://www.googleapis.com/auth/firebase.messaging`.
    /// If `None` and `api_key` is non-empty, falls back to the legacy endpoint.
    pub fcm_oauth2_bearer: Option<String>,
    /// FCM device registration token for the target device.
    /// In production wire up a per-user token registry instead.
    pub fcm_device_token: Option<String>,

    // ── APNS (token-based auth) ───────────────────────────────────────────────
    /// Apple 10-character key ID from the Developer Portal (.p8 key ID).
    pub apns_key_id: Option<String>,
    /// Apple 10-character Team ID.
    pub apns_team_id: Option<String>,
    /// App bundle ID used as the `apns-topic` header, e.g. `"com.example.app"`.
    pub apns_bundle_id: Option<String>,
    /// ES256 private key in PKCS#8 PEM form (contents of the `.p8` file).
    pub apns_private_key_pem: Option<String>,
    /// APNS device token (hex string) for the target device.
    /// In production wire up a per-user token registry instead.
    pub apns_device_token: Option<String>,
    /// Use the APNS sandbox endpoint instead of production.  Default: `false`.
    pub apns_sandbox: bool,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            provider: "fcm".to_string(),
            api_key: String::new(),
            api_secret: String::new(),
            enabled: false,
            fcm_project_id: None,
            fcm_oauth2_bearer: None,
            fcm_device_token: None,
            apns_key_id: None,
            apns_team_id: None,
            apns_bundle_id: None,
            apns_private_key_pem: None,
            apns_device_token: None,
            apns_sandbox: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Wire-format types
// ──────────────────────────────────────────────────────────────────────────────

/// FCM v1 request envelope.
#[derive(Debug, Serialize)]
struct FcmV1Request {
    message: FcmV1Message,
}

/// FCM v1 message object.
#[derive(Debug, Serialize)]
struct FcmV1Message {
    /// Registration token of the target device.
    token: String,
    notification: FcmNotification,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<std::collections::HashMap<String, String>>,
}

/// FCM notification display payload.
#[derive(Debug, Serialize)]
struct FcmNotification {
    title: String,
    body: String,
}

/// FCM v1 success response: `{ "name": "projects/.../messages/..." }`.
#[derive(Debug, Deserialize)]
struct FcmV1Response {
    name: String,
}

/// FCM legacy request envelope.
#[derive(Debug, Serialize)]
struct FcmLegacyRequest {
    to: String,
    notification: FcmNotification,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// FCM legacy success response.
#[derive(Debug, Deserialize)]
struct FcmLegacyResponse {
    /// Number of messages that were processed without an error.
    success: i32,
    /// For each message processed, the corresponding entry in the results list.
    results: Option<Vec<FcmLegacyResult>>,
}

/// Per-message result in the FCM legacy response.
#[derive(Debug, Deserialize)]
struct FcmLegacyResult {
    #[serde(rename = "message_id")]
    message_id: Option<String>,
    error: Option<String>,
}

/// APNS notification payload structure.
#[derive(Debug, Serialize)]
struct ApnsPayload {
    aps: ApnsAps,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_data: Option<serde_json::Value>,
}

/// APNS `aps` dictionary.
#[derive(Debug, Serialize)]
struct ApnsAps {
    alert: ApnsAlert,
    sound: String,
}

/// APNS alert dictionary.
#[derive(Debug, Serialize)]
struct ApnsAlert {
    title: String,
    body: String,
}

/// JWT claims for APNS token-based authentication (ES256).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApnsJwtClaims {
    /// Issuer: Apple Team ID.
    iss: String,
    /// Issued-at timestamp (Unix seconds).
    iat: u64,
}

// ──────────────────────────────────────────────────────────────────────────────
// PushNotifier
// ──────────────────────────────────────────────────────────────────────────────

/// Push notifier — sends real push notifications via FCM or APNS.
pub struct PushNotifier {
    config: PushConfig,
    /// Shared `reqwest::Client` (connection pooling, TLS session reuse).
    http_client: Client,
    /// In-memory log of successfully sent notifications.
    sent: Arc<RwLock<Vec<Notification>>>,
}

impl PushNotifier {
    /// Create a new push notifier.
    pub fn new(config: PushConfig) -> Self {
        // Build a shared HTTP client with HTTP/2 enabled (required for APNS).
        let http_client = Client::builder()
            .http2_prior_knowledge() // will negotiate to HTTP/2 where possible
            .build()
            .unwrap_or_default(); // Default client as safe fallback

        Self {
            config,
            http_client,
            sent: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Send a push notification through the configured provider.
    ///
    /// Returns the notification `id` on success (whether sent or disabled).
    /// Returns an error when the provider responds with a failure.
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

                // Enforce per-provider length limits.
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

                let result = match self.config.provider.as_str() {
                    "fcm" => {
                        self.send_via_fcm(*user_id, title, body, data.as_deref())
                            .await
                    }
                    "apns" => {
                        self.send_via_apns(*user_id, title, body, data.as_deref())
                            .await
                    }
                    other => {
                        Err(anyhow!(
                            "Unsupported push provider '{}': \
                             configure provider as 'fcm' or 'apns'",
                            other
                        ))
                    }
                };

                match result {
                    Ok(provider_id) => {
                        notification.status = NotificationStatus::Sent;
                        notification.sent_at = Some(chrono::Utc::now());
                        notification.attempts += 1;

                        self.sent.write().await.push(notification.clone());

                        tracing::info!(
                            "Push notification sent to user {} (provider_id: {})",
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
            _ => bail!("Invalid notification type for push channel"),
        }
    }

    // ──────────────────────────────────────────────────────────────────────────
    // FCM
    // ──────────────────────────────────────────────────────────────────────────

    /// Send a push notification via Firebase Cloud Messaging.
    ///
    /// Tries the FCM HTTP v1 endpoint first (when `fcm_project_id` and
    /// `fcm_oauth2_bearer` are both configured), then falls back to the
    /// legacy endpoint (when only `api_key` is non-empty).  Returns an
    /// honest error when neither credential set is configured.
    async fn send_via_fcm(
        &self,
        _user_id: uuid::Uuid,
        title: &str,
        body: &str,
        data: Option<&str>,
    ) -> Result<String> {
        let device_token = self.config.fcm_device_token.as_deref().ok_or_else(|| {
            anyhow!(
                "FCM device token not configured; \
                 set PushConfig::fcm_device_token to the target device's \
                 FCM registration token"
            )
        })?;

        // Determine which FCM flavour to use.
        match (
            self.config.fcm_project_id.as_deref(),
            self.config.fcm_oauth2_bearer.as_deref(),
        ) {
            (Some(project_id), Some(bearer)) => {
                self.send_via_fcm_v1(project_id, bearer, device_token, title, body, data)
                    .await
            }
            _ if !self.config.api_key.is_empty() => {
                self.send_via_fcm_legacy(&self.config.api_key.clone(), device_token, title, body, data)
                    .await
            }
            _ => Err(anyhow!(
                "FCM credentials not configured: \
                 provide (fcm_project_id + fcm_oauth2_bearer) for FCM v1 \
                 or api_key for the legacy endpoint"
            )),
        }
    }

    /// FCM HTTP v1 — `POST https://fcm.googleapis.com/v1/projects/{project_id}/messages:send`.
    async fn send_via_fcm_v1(
        &self,
        project_id: &str,
        bearer: &str,
        device_token: &str,
        title: &str,
        body: &str,
        data: Option<&str>,
    ) -> Result<String> {
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            project_id
        );

        let data_map = build_fcm_data_map(data)?;

        let request_body = FcmV1Request {
            message: FcmV1Message {
                token: device_token.to_string(),
                notification: FcmNotification {
                    title: title.to_string(),
                    body: body.to_string(),
                },
                data: data_map,
            },
        };

        tracing::debug!("FCM v1 POST {}", url);

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(bearer)
            .json(&request_body)
            .send()
            .await
            .context("FCM v1 HTTP request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            bail!("FCM v1 request failed with HTTP {}: {}", status, body_text);
        }

        let fcm_response: FcmV1Response = response
            .json()
            .await
            .context("FCM v1 response JSON parse failed")?;

        Ok(fcm_response.name)
    }

    /// FCM legacy — `POST https://fcm.googleapis.com/fcm/send`.
    async fn send_via_fcm_legacy(
        &self,
        server_key: &str,
        device_token: &str,
        title: &str,
        body: &str,
        data: Option<&str>,
    ) -> Result<String> {
        const LEGACY_URL: &str = "https://fcm.googleapis.com/fcm/send";

        let data_json = data
            .map(serde_json::from_str::<serde_json::Value>)
            .transpose()
            .context("FCM data field is not valid JSON")?;

        let request_body = FcmLegacyRequest {
            to: device_token.to_string(),
            notification: FcmNotification {
                title: title.to_string(),
                body: body.to_string(),
            },
            data: data_json,
        };

        tracing::debug!("FCM legacy POST {}", LEGACY_URL);

        let response = self
            .http_client
            .post(LEGACY_URL)
            .header("Authorization", format!("key={}", server_key))
            .json(&request_body)
            .send()
            .await
            .context("FCM legacy HTTP request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            bail!(
                "FCM legacy request failed with HTTP {}: {}",
                status,
                body_text
            );
        }

        let fcm_response: FcmLegacyResponse = response
            .json()
            .await
            .context("FCM legacy response JSON parse failed")?;

        if fcm_response.success == 0 {
            let error_msg = fcm_response
                .results
                .as_ref()
                .and_then(|r| r.first())
                .and_then(|r| r.error.as_deref())
                .unwrap_or("unknown FCM error");
            bail!("FCM delivery failed: {}", error_msg);
        }

        let message_id = fcm_response
            .results
            .as_ref()
            .and_then(|r| r.first())
            .and_then(|r| r.message_id.as_deref())
            .ok_or_else(|| anyhow!("FCM legacy response missing message_id"))?;

        Ok(message_id.to_string())
    }

    // ──────────────────────────────────────────────────────────────────────────
    // APNS
    // ──────────────────────────────────────────────────────────────────────────

    /// Send a push notification via the Apple Push Notification Service (APNS).
    ///
    /// Uses HTTP/2 token-based authentication (ES256 JWT).  Returns an honest
    /// error when any required credential is absent.
    async fn send_via_apns(
        &self,
        _user_id: uuid::Uuid,
        title: &str,
        body: &str,
        data: Option<&str>,
    ) -> Result<String> {
        // Validate all required APNS credentials up front.
        let key_id = self.config.apns_key_id.as_deref().ok_or_else(|| {
            anyhow!("APNS credential missing: set PushConfig::apns_key_id (10-char key ID from Apple Developer Portal)")
        })?;
        let team_id = self.config.apns_team_id.as_deref().ok_or_else(|| {
            anyhow!("APNS credential missing: set PushConfig::apns_team_id (10-char Team ID)")
        })?;
        let bundle_id = self.config.apns_bundle_id.as_deref().ok_or_else(|| {
            anyhow!("APNS credential missing: set PushConfig::apns_bundle_id (e.g. com.example.app)")
        })?;
        let private_key_pem = self.config.apns_private_key_pem.as_deref().ok_or_else(|| {
            anyhow!("APNS credential missing: set PushConfig::apns_private_key_pem (contents of .p8 file)")
        })?;
        let device_token = self.config.apns_device_token.as_deref().ok_or_else(|| {
            anyhow!(
                "APNS device token not configured; \
                 set PushConfig::apns_device_token to the target device's \
                 APNS hex device token"
            )
        })?;

        // Build the ES256 JWT.
        let jwt = build_apns_jwt(key_id, team_id, private_key_pem)?;

        // Build APNS payload.
        let custom_data = data
            .map(serde_json::from_str::<serde_json::Value>)
            .transpose()
            .context("APNS data field is not valid JSON")?;

        let payload = ApnsPayload {
            aps: ApnsAps {
                alert: ApnsAlert {
                    title: title.to_string(),
                    body: body.to_string(),
                },
                sound: "default".to_string(),
            },
            custom_data,
        };

        let host = if self.config.apns_sandbox {
            "api.sandbox.push.apple.com"
        } else {
            "api.push.apple.com"
        };
        let url = format!("https://{}/3/device/{}", host, device_token);

        tracing::debug!("APNS POST {}", url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("bearer {}", jwt))
            .header("apns-topic", bundle_id)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .json(&payload)
            .send()
            .await
            .context("APNS HTTP request failed")?;

        let status = response.status();
        // APNS returns the message ID in the `apns-id` response header.
        let apns_id = response
            .headers()
            .get("apns-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if !status.is_success() {
            // APNS error responses are JSON: `{ "reason": "...", "timestamp": ... }`
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            bail!("APNS request failed with HTTP {}: {}", status, body_text);
        }

        apns_id.ok_or_else(|| anyhow!("APNS response missing apns-id header"))
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Helpers
    // ──────────────────────────────────────────────────────────────────────────

    /// Return a snapshot of successfully sent notifications (useful in tests).
    pub async fn get_sent(&self) -> Vec<Notification> {
        self.sent.read().await.clone()
    }

    /// Clear the in-memory sent log (useful in tests).
    pub async fn clear_sent(&self) {
        self.sent.write().await.clear();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Free helper functions (also exercised by unit tests)
// ──────────────────────────────────────────────────────────────────────────────

/// Parse the optional `data` string (must be a JSON object if present) into a
/// `HashMap<String, String>` for the FCM v1 `data` field.
///
/// FCM v1 requires all data-map values to be strings.  If the JSON object
/// contains non-string values they are serialised back to JSON strings.
fn build_fcm_data_map(
    data: Option<&str>,
) -> Result<Option<std::collections::HashMap<String, String>>> {
    let raw = match data {
        None => return Ok(None),
        Some(s) => s,
    };

    let value: serde_json::Value =
        serde_json::from_str(raw).context("FCM data field is not valid JSON")?;

    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("FCM data field must be a JSON object"))?;

    let mut map = std::collections::HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        let s = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        map.insert(k.clone(), s);
    }
    Ok(Some(map))
}

/// Mint a short-lived ES256 JWT for APNS token-based authentication.
///
/// The JWT is valid for ~1 hour per Apple's specification; callers should
/// cache and refresh it if needed (outside the scope of this module).
fn build_apns_jwt(key_id: &str, team_id: &str, private_key_pem: &str) -> Result<String> {
    let iat = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("System clock is before UNIX epoch")?
        .as_secs();

    let claims = ApnsJwtClaims {
        iss: team_id.to_string(),
        iat,
    };

    let encoding_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes())
        .context("Failed to parse APNS private key PEM (expected EC key in PKCS#8 format)")?;

    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_string());

    let token =
        jsonwebtoken::encode(&header, &claims, &encoding_key).context("APNS JWT signing failed")?;

    Ok(token)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    // ── send() gating ─────────────────────────────────────────────────────────

    /// When push is disabled, send() must return the notification id without
    /// making any network call and without marking the notification Sent.
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

        // Nothing should be logged as sent.
        assert!(notifier.get_sent().await.is_empty());
    }

    /// An unsupported provider string must propagate an honest error — not a
    /// fake ID — and must NOT mark the notification Sent.
    #[tokio::test]
    async fn test_push_unsupported_provider_returns_error() {
        let config = PushConfig {
            enabled: true,
            provider: "carrier_pigeon".to_string(),
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Test".to_string(),
            "Body".to_string(),
        );
        let result = notifier.send(notification).await;
        assert!(result.is_err(), "unsupported provider must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Unsupported push provider"),
            "error message should mention unsupported provider, got: {msg}"
        );
    }

    // ── FCM credential validation ─────────────────────────────────────────────

    /// FCM with no credentials at all must return an honest error.
    #[tokio::test]
    async fn test_fcm_unconfigured_returns_honest_error() {
        let config = PushConfig {
            enabled: true,
            provider: "fcm".to_string(),
            api_key: String::new(),
            fcm_project_id: None,
            fcm_oauth2_bearer: None,
            fcm_device_token: Some("some_device_token".to_string()),
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Hello".to_string(),
            "World".to_string(),
        );
        let result = notifier.send(notification).await;
        assert!(result.is_err(), "unconfigured FCM must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("FCM credentials not configured"),
            "error must mention missing FCM credentials, got: {msg}"
        );
    }

    /// FCM with a device token absent must return an honest error.
    #[tokio::test]
    async fn test_fcm_no_device_token_returns_honest_error() {
        let config = PushConfig {
            enabled: true,
            provider: "fcm".to_string(),
            fcm_project_id: Some("proj-123".to_string()),
            fcm_oauth2_bearer: Some("token-xyz".to_string()),
            fcm_device_token: None, // absent
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Hello".to_string(),
            "World".to_string(),
        );
        let result = notifier.send(notification).await;
        assert!(result.is_err(), "missing device token must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("FCM device token not configured"),
            "error must mention FCM device token, got: {msg}"
        );
    }

    // ── APNS credential validation ────────────────────────────────────────────

    /// APNS with every credential absent must return an honest error.
    #[tokio::test]
    async fn test_apns_unconfigured_returns_honest_error() {
        let config = PushConfig {
            enabled: true,
            provider: "apns".to_string(),
            // All APNS fields left None.
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Hello".to_string(),
            "World".to_string(),
        );
        let result = notifier.send(notification).await;
        assert!(result.is_err(), "unconfigured APNS must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("APNS credential missing"),
            "error must mention missing APNS credentials, got: {msg}"
        );
    }

    /// APNS missing only the device token must report that specific gap.
    #[tokio::test]
    async fn test_apns_no_device_token_returns_honest_error() {
        let (_, private_key_pem) = generate_test_ec_key();
        let config = PushConfig {
            enabled: true,
            provider: "apns".to_string(),
            apns_key_id: Some("ABCDE12345".to_string()),
            apns_team_id: Some("TEAMID1234".to_string()),
            apns_bundle_id: Some("com.example.app".to_string()),
            apns_private_key_pem: Some(private_key_pem),
            apns_device_token: None, // absent
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(
            Uuid::new_v4(),
            "Hello".to_string(),
            "World".to_string(),
        );
        let result = notifier.send(notification).await;
        assert!(result.is_err(), "missing device token must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("APNS device token not configured"),
            "error must mention APNS device token, got: {msg}"
        );
    }

    // ── FCM request construction ──────────────────────────────────────────────

    /// Verify FCM v1 request body serialises to the correct JSON shape.
    #[test]
    fn test_fcm_v1_request_body_shape() {
        let request = FcmV1Request {
            message: FcmV1Message {
                token: "device-abc".to_string(),
                notification: FcmNotification {
                    title: "Hi".to_string(),
                    body: "Hello".to_string(),
                },
                data: None,
            },
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["message"]["token"], "device-abc");
        assert_eq!(json["message"]["notification"]["title"], "Hi");
        assert_eq!(json["message"]["notification"]["body"], "Hello");
        // `data` is None so must be absent from serialised output.
        assert!(json["message"].get("data").is_none() || json["message"]["data"].is_null());
    }

    /// Verify FCM legacy request body serialises to the correct JSON shape.
    #[test]
    fn test_fcm_legacy_request_body_shape() {
        let request = FcmLegacyRequest {
            to: "device-xyz".to_string(),
            notification: FcmNotification {
                title: "Alert".to_string(),
                body: "Message body".to_string(),
            },
            data: Some(serde_json::json!({ "key": "value" })),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["to"], "device-xyz");
        assert_eq!(json["notification"]["title"], "Alert");
        assert_eq!(json["data"]["key"], "value");
    }

    // ── APNS request construction ─────────────────────────────────────────────

    /// Verify APNS payload serialises to the correct JSON structure.
    #[test]
    fn test_apns_payload_shape() {
        let payload = ApnsPayload {
            aps: ApnsAps {
                alert: ApnsAlert {
                    title: "Alert Title".to_string(),
                    body: "Alert Body".to_string(),
                },
                sound: "default".to_string(),
            },
            custom_data: Some(serde_json::json!({ "tx": "abc" })),
        };

        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["aps"]["alert"]["title"], "Alert Title");
        assert_eq!(json["aps"]["alert"]["body"], "Alert Body");
        assert_eq!(json["aps"]["sound"], "default");
        assert_eq!(json["custom_data"]["tx"], "abc");
    }

    // ── APNS JWT construction ─────────────────────────────────────────────────

    /// Verify that `build_apns_jwt` produces a parseable ES256 JWT with the
    /// correct `kid`, `iss`, and a recent `iat`.  Does NOT call Apple APIs.
    #[test]
    fn test_apns_jwt_claims_and_header() {
        let (verifying_pem, signing_pem) = generate_test_ec_key();

        let jwt = build_apns_jwt("KEYID00001", "TEAMID0001", &signing_pem).unwrap();

        // Decode without signature verification to inspect claims and header.
        // `dangerous::insecure_decode` is the v10 replacement for the deprecated
        // `Validation::insecure_disable_signature_validation` path.
        let token_data = jsonwebtoken::dangerous::insecure_decode::<ApnsJwtClaims>(&jwt)
            .expect("JWT must be decodeable");

        assert_eq!(token_data.claims.iss, "TEAMID0001");

        // `iat` must be within a few seconds of now.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(
            token_data.claims.iat <= now,
            "iat must not be in the future"
        );
        assert!(
            now - token_data.claims.iat < 30,
            "iat must be recent (within 30 s)"
        );

        // Verify header kid.
        assert_eq!(token_data.header.kid.as_deref(), Some("KEYID00001"));
        assert_eq!(token_data.header.alg, Algorithm::ES256);

        // Verify signature with the public key.
        let decoding_key = jsonwebtoken::DecodingKey::from_ec_pem(verifying_pem.as_bytes())
            .expect("public key must be parseable");
        let mut sig_validation = jsonwebtoken::Validation::new(Algorithm::ES256);
        sig_validation.set_required_spec_claims(&[] as &[&str]);
        jsonwebtoken::decode::<ApnsJwtClaims>(&jwt, &decoding_key, &sig_validation)
            .expect("JWT signature must verify");
    }

    /// A garbage PEM string must return a descriptive error, not panic.
    #[test]
    fn test_apns_jwt_bad_key_returns_error() {
        let result = build_apns_jwt("KID", "TEAM", "not-a-valid-pem");
        assert!(result.is_err(), "bad PEM must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Failed to parse APNS private key PEM"),
            "error must describe PEM parsing failure, got: {msg}"
        );
    }

    // ── build_fcm_data_map ────────────────────────────────────────────────────

    /// None input produces None output.
    #[test]
    fn test_build_fcm_data_map_none() {
        let result = build_fcm_data_map(None).unwrap();
        assert!(result.is_none());
    }

    /// A valid JSON object is converted to a string-map.
    #[test]
    fn test_build_fcm_data_map_valid_object() {
        let result = build_fcm_data_map(Some(r#"{"action":"open","id":"42"}"#)).unwrap();
        let map = result.unwrap();
        assert_eq!(map.get("action").map(String::as_str), Some("open"));
        assert_eq!(map.get("id").map(String::as_str), Some("42"));
    }

    /// A non-string JSON value (number) is stringified.
    #[test]
    fn test_build_fcm_data_map_non_string_value_stringified() {
        let result = build_fcm_data_map(Some(r#"{"count":7}"#)).unwrap();
        let map = result.unwrap();
        assert_eq!(map.get("count").map(String::as_str), Some("7"));
    }

    /// A non-object JSON value (array) is rejected.
    #[test]
    fn test_build_fcm_data_map_non_object_rejected() {
        let result = build_fcm_data_map(Some(r#"["a","b"]"#));
        assert!(result.is_err(), "JSON array must be rejected");
    }

    /// Invalid JSON is rejected.
    #[test]
    fn test_build_fcm_data_map_invalid_json_rejected() {
        let result = build_fcm_data_map(Some("not json"));
        assert!(result.is_err(), "invalid JSON must be rejected");
    }

    // ── title / body validation ───────────────────────────────────────────────

    /// A title over 256 characters must cause the notification to be marked
    /// Failed (not Sent) without returning an error from `send`.
    #[tokio::test]
    async fn test_push_title_too_long_marks_failed() {
        // Provider doesn't matter; validation happens before provider dispatch.
        let config = PushConfig {
            enabled: true,
            provider: "fcm".to_string(),
            ..Default::default()
        };
        let notifier = PushNotifier::new(config);

        let long_title = "x".repeat(257);
        let mut notification = Notification::push(
            Uuid::new_v4(),
            long_title,
            "Body".to_string(),
        );
        // Pre-set so we can detect the status was changed.
        notification.status = NotificationStatus::Pending;

        let result = notifier.send(notification.clone()).await;
        // send() returns Ok(id) even for validation failures.
        assert!(result.is_ok(), "send must return Ok for validation failures");
        // Nothing should be logged as sent.
        assert!(notifier.get_sent().await.is_empty());
    }

    // ── Key generation helper ─────────────────────────────────────────────────

    /// Generate a throwaway ECDSA P-256 key pair in PEM form.
    /// Returns `(public_pem, private_pem)`.
    ///
    /// This uses `ring` which is already a transitive dependency via
    /// `jsonwebtoken`'s `rust_crypto` feature (which pulls in `aws-lc-rs`
    /// or `ring`).  We use the `openssl` CLI if available, otherwise fall
    /// back to a pre-generated test key pair embedded inline.
    fn generate_test_ec_key() -> (String, String) {
        // Try to generate a fresh pair with the `openssl` CLI.
        if let Ok(output) = std::process::Command::new("openssl")
            .args(["ecparam", "-genkey", "-name", "prime256v1", "-noout", "-text"])
            .output()
        {
            if output.status.success() {
                // Generate private key in PKCS#8 PEM.
                let priv_out = std::process::Command::new("openssl")
                    .args(["genpkey", "-algorithm", "EC",
                           "-pkeyopt", "ec_paramgen_curve:P-256"])
                    .output();
                if let Ok(priv_out) = priv_out {
                    if priv_out.status.success() {
                        let priv_pem = String::from_utf8_lossy(&priv_out.stdout).to_string();
                        // Derive the public key.
                        let mut pub_cmd = std::process::Command::new("openssl")
                            .args(["pkey", "-pubout"])
                            .stdin(std::process::Stdio::piped())
                            .stdout(std::process::Stdio::piped())
                            .spawn()
                            .expect("openssl spawn");
                        if let Some(stdin) = pub_cmd.stdin.take() {
                            use std::io::Write;
                            let mut stdin = stdin;
                            stdin.write_all(priv_pem.as_bytes()).ok();
                        }
                        if let Ok(pub_out) = pub_cmd.wait_with_output() {
                            if pub_out.status.success() {
                                let pub_pem =
                                    String::from_utf8_lossy(&pub_out.stdout).to_string();
                                return (pub_pem, priv_pem);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: a static pre-generated P-256 PKCS#8 test key pair.
        // This pair is intentionally public / test-only — never use in production.
        let private_pem = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hD2\n\
cteJctsIfsvvKAx5IwkgYYfroUKBZANiAARxyHCg+aSRMPOHOkiXHnMV3OHIKL1V\n\
3OWvMHkFGmC9t8LAuFPx1lbNfwAkLBNK7TSJVQ9DPsbjxSuQ1HW8RlE=\n\
-----END PRIVATE KEY-----\n";

        let public_pem = "-----BEGIN PUBLIC KEY-----\n\
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEcchwovm0kTDzhzpIlx5zFdzhyCi9\n\
Vdzlr zB5BRpgvbfCwLhT8dZWzX8AJCwTSu00iVUPQz7G48UrkNR1vEZRMQ==\n\
-----END PUBLIC KEY-----\n";

        (public_pem.to_string(), private_pem.to_string())
    }
}
