//! Webhook subscription routes

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{ApiError, Result, ValidationBuilder},
    middleware::Claims,
    state::AppState,
};

/// Webhook event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Token created
    TokenCreated,
    /// Token price updated
    TokenPriceUpdated,
    /// Order created
    OrderCreated,
    /// Order completed
    OrderCompleted,
    /// Order cancelled
    OrderCancelled,
    /// Trade executed
    TradeExecuted,
    /// Commitment created
    CommitmentCreated,
    /// Commitment verified
    CommitmentVerified,
    /// KYC submitted
    KycSubmitted,
    /// KYC verified
    KycVerified,
}

impl std::fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenCreated => write!(f, "token.created"),
            Self::TokenPriceUpdated => write!(f, "token.price_updated"),
            Self::OrderCreated => write!(f, "order.created"),
            Self::OrderCompleted => write!(f, "order.completed"),
            Self::OrderCancelled => write!(f, "order.cancelled"),
            Self::TradeExecuted => write!(f, "trade.executed"),
            Self::CommitmentCreated => write!(f, "commitment.created"),
            Self::CommitmentVerified => write!(f, "commitment.verified"),
            Self::KycSubmitted => write!(f, "kyc.submitted"),
            Self::KycVerified => write!(f, "kyc.verified"),
        }
    }
}

/// Webhook subscription status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebhookStatus {
    Active,
    Inactive,
    Failed,
}

/// Create webhook request
#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    /// Webhook URL (must be HTTPS)
    pub url: String,
    /// Events to subscribe to
    pub events: Vec<WebhookEventType>,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
}

/// Update webhook request
#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    /// Webhook URL (must be HTTPS)
    #[serde(default)]
    pub url: Option<String>,
    /// Events to subscribe to
    #[serde(default)]
    pub events: Option<Vec<WebhookEventType>>,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Status
    #[serde(default)]
    pub status: Option<WebhookStatus>,
}

/// Webhook response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub events: Vec<WebhookEventType>,
    pub description: Option<String>,
    pub status: WebhookStatus,
    pub secret: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Webhook delivery response
#[derive(Debug, Serialize)]
pub struct WebhookDeliveryResponse {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: WebhookEventType,
    pub payload: serde_json::Value,
    pub status: String,
    pub response_code: Option<i32>,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

/// List webhooks response
#[derive(Debug, Serialize)]
pub struct ListWebhooksResponse {
    pub webhooks: Vec<WebhookResponse>,
    pub total: i64,
}

/// Create webhook
pub async fn create_webhook(
    State(_state): State<AppState>,
    claims: Claims,
    Json(req): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookResponse>> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // Validation
    let mut errors = ValidationBuilder::new();

    if req.url.is_empty() {
        errors = errors.field("url", "URL is required");
    } else if !req.url.starts_with("https://") {
        errors = errors.field("url", "URL must use HTTPS");
    }

    if req.events.is_empty() {
        errors = errors.field("events", "At least one event type is required");
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    // In production: store webhook in database
    // Generate a webhook secret for signature verification
    let webhook_id = Uuid::new_v4();
    let secret = generate_webhook_secret();

    Ok(Json(WebhookResponse {
        id: webhook_id,
        user_id,
        url: req.url,
        events: req.events,
        description: req.description,
        status: WebhookStatus::Active,
        secret,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }))
}

/// Get webhook by ID
pub async fn get_webhook(
    State(_state): State<AppState>,
    claims: Claims,
    Path(webhook_id): Path<Uuid>,
) -> Result<Json<WebhookResponse>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production: fetch webhook from database
    // Verify user owns the webhook

    Err(ApiError::NotFound(format!(
        "Webhook {} not found",
        webhook_id
    )))
}

/// List webhooks for user
pub async fn list_webhooks(
    State(_state): State<AppState>,
    claims: Claims,
) -> Result<Json<ListWebhooksResponse>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production: fetch webhooks from database

    Ok(Json(ListWebhooksResponse {
        webhooks: vec![],
        total: 0,
    }))
}

/// Update webhook
pub async fn update_webhook(
    State(_state): State<AppState>,
    claims: Claims,
    Path(webhook_id): Path<Uuid>,
    Json(req): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // Validation
    let mut errors = ValidationBuilder::new();

    if let Some(ref url) = req.url {
        if url.is_empty() {
            errors = errors.field("url", "URL cannot be empty");
        } else if !url.starts_with("https://") {
            errors = errors.field("url", "URL must use HTTPS");
        }
    }

    if let Some(ref events) = req.events {
        if events.is_empty() {
            errors = errors.field("events", "At least one event type is required");
        }
    }

    if errors.has_errors() {
        return errors.into_result();
    }

    // In production: update webhook in database
    // Verify user owns the webhook

    Err(ApiError::NotFound(format!(
        "Webhook {} not found",
        webhook_id
    )))
}

/// Delete webhook
pub async fn delete_webhook(
    State(_state): State<AppState>,
    claims: Claims,
    Path(_webhook_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production: delete webhook from database
    // Verify user owns the webhook

    Ok((StatusCode::OK, "Webhook deleted"))
}

/// List webhook deliveries
pub async fn list_webhook_deliveries(
    State(_state): State<AppState>,
    claims: Claims,
    Path(_webhook_id): Path<Uuid>,
) -> Result<Json<Vec<WebhookDeliveryResponse>>> {
    let _user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ApiError::Auth("Invalid user ID in token".to_string()))?;

    // In production: fetch webhook deliveries from database
    // Verify user owns the webhook

    Ok(Json(vec![]))
}

/// Generate webhook secret (HMAC key)
fn generate_webhook_secret() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random_range(0..256) as u8).collect();
    hex::encode(bytes)
}

/// Webhook payload structure
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub id: Uuid,
    pub event: String,
    pub created_at: DateTime<Utc>,
    pub data: serde_json::Value,
}

/// Generate webhook signature
#[allow(dead_code)]
fn generate_signature(secret: &str, payload: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(payload.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_type_display() {
        assert_eq!(WebhookEventType::TokenCreated.to_string(), "token.created");
        assert_eq!(
            WebhookEventType::OrderCompleted.to_string(),
            "order.completed"
        );
    }

    #[test]
    fn test_generate_webhook_secret() {
        let secret1 = generate_webhook_secret();
        let secret2 = generate_webhook_secret();

        assert_eq!(secret1.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(secret2.len(), 64);
        assert_ne!(secret1, secret2); // Should be unique
    }

    #[test]
    fn test_generate_signature() {
        let secret = "test_secret";
        let payload = r#"{"event":"test"}"#;

        let sig1 = generate_signature(secret, payload);
        let sig2 = generate_signature(secret, payload);

        assert_eq!(sig1, sig2); // Same input should produce same signature
        assert!(!sig1.is_empty());

        // Different payload should produce different signature
        let sig3 = generate_signature(secret, r#"{"event":"different"}"#);
        assert_ne!(sig1, sig3);
    }
}
