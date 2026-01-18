//! Notification preferences routes
//!
//! Allows users to manage their notification preferences

use axum::{Extension, Json, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// Notification preferences for a user
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationPreferences {
    pub user_id: Uuid,
    pub email_enabled: bool,
    pub email_order_updates: bool,
    pub email_trade_notifications: bool,
    pub email_commitment_updates: bool,
    pub email_kyc_updates: bool,
    pub email_marketing: bool,
    pub sms_enabled: bool,
    pub sms_order_updates: bool,
    pub sms_trade_notifications: bool,
    pub sms_security_alerts: bool,
    pub push_enabled: bool,
    pub push_order_updates: bool,
    pub push_trade_notifications: bool,
    pub push_price_alerts: bool,
    pub inapp_enabled: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            user_id: Uuid::nil(),
            email_enabled: true,
            email_order_updates: true,
            email_trade_notifications: true,
            email_commitment_updates: true,
            email_kyc_updates: true,
            email_marketing: false,
            sms_enabled: false,
            sms_order_updates: false,
            sms_trade_notifications: false,
            sms_security_alerts: true,
            push_enabled: true,
            push_order_updates: true,
            push_trade_notifications: true,
            push_price_alerts: false,
            inapp_enabled: true,
        }
    }
}

/// Update notification preferences request
#[derive(Debug, Deserialize)]
pub struct UpdateNotificationPreferencesRequest {
    pub email_enabled: Option<bool>,
    pub email_order_updates: Option<bool>,
    pub email_trade_notifications: Option<bool>,
    pub email_commitment_updates: Option<bool>,
    pub email_kyc_updates: Option<bool>,
    pub email_marketing: Option<bool>,
    pub sms_enabled: Option<bool>,
    pub sms_order_updates: Option<bool>,
    pub sms_trade_notifications: Option<bool>,
    pub sms_security_alerts: Option<bool>,
    pub push_enabled: Option<bool>,
    pub push_order_updates: Option<bool>,
    pub push_trade_notifications: Option<bool>,
    pub push_price_alerts: Option<bool>,
    pub inapp_enabled: Option<bool>,
}

/// Get user's notification preferences
#[allow(dead_code)]
pub async fn get_notification_preferences(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<NotificationPreferences>, ApiError> {
    // Try to fetch existing preferences
    let preferences: Option<NotificationPreferences> =
        sqlx::query_as("SELECT * FROM notification_preferences WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(ApiError::Database)?;

    // If no preferences exist, return defaults
    let prefs = preferences.unwrap_or_else(|| NotificationPreferences {
        user_id,
        ..Default::default()
    });

    Ok(Json(prefs))
}

/// Update user's notification preferences
#[allow(dead_code)]
pub async fn update_notification_preferences(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(req): Json<UpdateNotificationPreferencesRequest>,
) -> Result<Json<NotificationPreferences>, ApiError> {
    // First, ensure the preferences record exists
    sqlx::query(
        "INSERT INTO notification_preferences (user_id)
         VALUES ($1)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    // Update preferences using COALESCE to only update provided fields
    // This approach is simpler and more maintainable than dynamic query building
    sqlx::query(
        "UPDATE notification_preferences
         SET email_enabled = COALESCE($2, email_enabled),
             email_order_updates = COALESCE($3, email_order_updates),
             email_trade_notifications = COALESCE($4, email_trade_notifications),
             email_commitment_updates = COALESCE($5, email_commitment_updates),
             email_kyc_updates = COALESCE($6, email_kyc_updates),
             email_marketing = COALESCE($7, email_marketing),
             sms_enabled = COALESCE($8, sms_enabled),
             sms_order_updates = COALESCE($9, sms_order_updates),
             sms_trade_notifications = COALESCE($10, sms_trade_notifications),
             sms_security_alerts = COALESCE($11, sms_security_alerts),
             push_enabled = COALESCE($12, push_enabled),
             push_order_updates = COALESCE($13, push_order_updates),
             push_trade_notifications = COALESCE($14, push_trade_notifications),
             push_price_alerts = COALESCE($15, push_price_alerts),
             inapp_enabled = COALESCE($16, inapp_enabled),
             updated_at = NOW()
         WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(req.email_enabled)
    .bind(req.email_order_updates)
    .bind(req.email_trade_notifications)
    .bind(req.email_commitment_updates)
    .bind(req.email_kyc_updates)
    .bind(req.email_marketing)
    .bind(req.sms_enabled)
    .bind(req.sms_order_updates)
    .bind(req.sms_trade_notifications)
    .bind(req.sms_security_alerts)
    .bind(req.push_enabled)
    .bind(req.push_order_updates)
    .bind(req.push_trade_notifications)
    .bind(req.push_price_alerts)
    .bind(req.inapp_enabled)
    .execute(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    // Fetch updated preferences
    let preferences: NotificationPreferences =
        sqlx::query_as("SELECT * FROM notification_preferences WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::Database)?;

    Ok(Json(preferences))
}

/// Unsubscribe from all marketing emails (public endpoint, no auth required)
/// This is typically accessed via a link in marketing emails
#[allow(dead_code)]
pub async fn unsubscribe_marketing(
    State(state): State<AppState>,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query(
        "UPDATE notification_preferences
         SET email_marketing = false
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(serde_json::json!({
        "message": "Successfully unsubscribed from marketing emails"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_notification_preferences() {
        let prefs = NotificationPreferences::default();
        assert!(prefs.email_enabled);
        assert!(prefs.email_order_updates);
        assert!(!prefs.email_marketing);
        assert!(!prefs.sms_enabled);
        assert!(prefs.push_enabled);
        assert!(prefs.inapp_enabled);
    }
}
