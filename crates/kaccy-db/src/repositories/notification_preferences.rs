//! Notification preferences repository

use chrono::{DateTime, Utc};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::error::Result;

/// Notification preferences data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotificationPreferences {
    pub user_id: Uuid,

    // Email preferences
    pub email_enabled: bool,
    pub email_order_updates: bool,
    pub email_trade_notifications: bool,
    pub email_commitment_updates: bool,
    pub email_kyc_updates: bool,
    pub email_marketing: bool,

    // SMS preferences
    pub sms_enabled: bool,
    pub sms_order_updates: bool,
    pub sms_trade_notifications: bool,
    pub sms_security_alerts: bool,

    // Push notification preferences
    pub push_enabled: bool,
    pub push_order_updates: bool,
    pub push_trade_notifications: bool,
    pub push_price_alerts: bool,

    // In-app notification preferences
    pub inapp_enabled: bool,

    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating notification preferences
#[derive(Debug, Clone)]
pub struct CreateNotificationPreferences {
    pub user_id: Uuid,

    // Email preferences (optional, defaults to true/false per migration)
    pub email_enabled: Option<bool>,
    pub email_order_updates: Option<bool>,
    pub email_trade_notifications: Option<bool>,
    pub email_commitment_updates: Option<bool>,
    pub email_kyc_updates: Option<bool>,
    pub email_marketing: Option<bool>,

    // SMS preferences (optional)
    pub sms_enabled: Option<bool>,
    pub sms_order_updates: Option<bool>,
    pub sms_trade_notifications: Option<bool>,
    pub sms_security_alerts: Option<bool>,

    // Push notification preferences (optional)
    pub push_enabled: Option<bool>,
    pub push_order_updates: Option<bool>,
    pub push_trade_notifications: Option<bool>,
    pub push_price_alerts: Option<bool>,

    // In-app notification preferences (optional)
    pub inapp_enabled: Option<bool>,
}

/// Input for updating notification preferences
#[derive(Debug, Clone, Default)]
pub struct UpdateNotificationPreferences {
    // Email preferences
    pub email_enabled: Option<bool>,
    pub email_order_updates: Option<bool>,
    pub email_trade_notifications: Option<bool>,
    pub email_commitment_updates: Option<bool>,
    pub email_kyc_updates: Option<bool>,
    pub email_marketing: Option<bool>,

    // SMS preferences
    pub sms_enabled: Option<bool>,
    pub sms_order_updates: Option<bool>,
    pub sms_trade_notifications: Option<bool>,
    pub sms_security_alerts: Option<bool>,

    // Push notification preferences
    pub push_enabled: Option<bool>,
    pub push_order_updates: Option<bool>,
    pub push_trade_notifications: Option<bool>,
    pub push_price_alerts: Option<bool>,

    // In-app notification preferences
    pub inapp_enabled: Option<bool>,
}

/// Repository for notification preferences operations
pub struct NotificationPreferencesRepository {
    pool: PgPool,
}

impl NotificationPreferencesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get notification preferences for a user
    pub async fn get(&self, user_id: Uuid) -> Result<Option<NotificationPreferences>> {
        let prefs = sqlx::query_as::<_, NotificationPreferences>(
            r#"
            SELECT * FROM notification_preferences WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(prefs)
    }

    /// Get notification preferences for a user, creating default if none exist
    pub async fn get_or_create_default(&self, user_id: Uuid) -> Result<NotificationPreferences> {
        if let Some(prefs) = self.get(user_id).await? {
            return Ok(prefs);
        }

        // Create default preferences
        self.create(&CreateNotificationPreferences {
            user_id,
            email_enabled: None, // Use database defaults
            email_order_updates: None,
            email_trade_notifications: None,
            email_commitment_updates: None,
            email_kyc_updates: None,
            email_marketing: None,
            sms_enabled: None,
            sms_order_updates: None,
            sms_trade_notifications: None,
            sms_security_alerts: None,
            push_enabled: None,
            push_order_updates: None,
            push_trade_notifications: None,
            push_price_alerts: None,
            inapp_enabled: None,
        })
        .await
    }

    /// Create notification preferences for a user
    pub async fn create(
        &self,
        input: &CreateNotificationPreferences,
    ) -> Result<NotificationPreferences> {
        let prefs = sqlx::query_as::<_, NotificationPreferences>(
            r#"
            INSERT INTO notification_preferences (
                user_id,
                email_enabled, email_order_updates, email_trade_notifications,
                email_commitment_updates, email_kyc_updates, email_marketing,
                sms_enabled, sms_order_updates, sms_trade_notifications, sms_security_alerts,
                push_enabled, push_order_updates, push_trade_notifications, push_price_alerts,
                inapp_enabled
            )
            VALUES (
                $1,
                COALESCE($2, DEFAULT), COALESCE($3, DEFAULT), COALESCE($4, DEFAULT),
                COALESCE($5, DEFAULT), COALESCE($6, DEFAULT), COALESCE($7, DEFAULT),
                COALESCE($8, DEFAULT), COALESCE($9, DEFAULT), COALESCE($10, DEFAULT), COALESCE($11, DEFAULT),
                COALESCE($12, DEFAULT), COALESCE($13, DEFAULT), COALESCE($14, DEFAULT), COALESCE($15, DEFAULT),
                COALESCE($16, DEFAULT)
            )
            RETURNING *
            "#,
        )
        .bind(input.user_id)
        .bind(input.email_enabled)
        .bind(input.email_order_updates)
        .bind(input.email_trade_notifications)
        .bind(input.email_commitment_updates)
        .bind(input.email_kyc_updates)
        .bind(input.email_marketing)
        .bind(input.sms_enabled)
        .bind(input.sms_order_updates)
        .bind(input.sms_trade_notifications)
        .bind(input.sms_security_alerts)
        .bind(input.push_enabled)
        .bind(input.push_order_updates)
        .bind(input.push_trade_notifications)
        .bind(input.push_price_alerts)
        .bind(input.inapp_enabled)
        .fetch_one(&self.pool)
        .await?;

        Ok(prefs)
    }

    /// Update notification preferences for a user
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        user_id: Uuid,
        input: &UpdateNotificationPreferences,
    ) -> Result<NotificationPreferences> {
        let prefs = sqlx::query_as::<_, NotificationPreferences>(
            r#"
            UPDATE notification_preferences
            SET
                email_enabled = COALESCE($2, email_enabled),
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
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(input.email_enabled)
        .bind(input.email_order_updates)
        .bind(input.email_trade_notifications)
        .bind(input.email_commitment_updates)
        .bind(input.email_kyc_updates)
        .bind(input.email_marketing)
        .bind(input.sms_enabled)
        .bind(input.sms_order_updates)
        .bind(input.sms_trade_notifications)
        .bind(input.sms_security_alerts)
        .bind(input.push_enabled)
        .bind(input.push_order_updates)
        .bind(input.push_trade_notifications)
        .bind(input.push_price_alerts)
        .bind(input.inapp_enabled)
        .fetch_one(&self.pool)
        .await?;

        Ok(prefs)
    }

    /// Delete notification preferences for a user
    pub async fn delete(&self, user_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM notification_preferences WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if user should receive email notifications for a specific type
    pub async fn should_send_email(
        &self,
        user_id: Uuid,
        notification_type: EmailNotificationType,
    ) -> Result<bool> {
        let prefs = self.get_or_create_default(user_id).await?;

        if !prefs.email_enabled {
            return Ok(false);
        }

        Ok(match notification_type {
            EmailNotificationType::OrderUpdate => prefs.email_order_updates,
            EmailNotificationType::TradeNotification => prefs.email_trade_notifications,
            EmailNotificationType::CommitmentUpdate => prefs.email_commitment_updates,
            EmailNotificationType::KycUpdate => prefs.email_kyc_updates,
            EmailNotificationType::Marketing => prefs.email_marketing,
        })
    }

    /// Check if user should receive SMS notifications for a specific type
    pub async fn should_send_sms(
        &self,
        user_id: Uuid,
        notification_type: SmsNotificationType,
    ) -> Result<bool> {
        let prefs = self.get_or_create_default(user_id).await?;

        if !prefs.sms_enabled {
            return Ok(false);
        }

        Ok(match notification_type {
            SmsNotificationType::OrderUpdate => prefs.sms_order_updates,
            SmsNotificationType::TradeNotification => prefs.sms_trade_notifications,
            SmsNotificationType::SecurityAlert => prefs.sms_security_alerts,
        })
    }

    /// Check if user should receive push notifications for a specific type
    pub async fn should_send_push(
        &self,
        user_id: Uuid,
        notification_type: PushNotificationType,
    ) -> Result<bool> {
        let prefs = self.get_or_create_default(user_id).await?;

        if !prefs.push_enabled {
            return Ok(false);
        }

        Ok(match notification_type {
            PushNotificationType::OrderUpdate => prefs.push_order_updates,
            PushNotificationType::TradeNotification => prefs.push_trade_notifications,
            PushNotificationType::PriceAlert => prefs.push_price_alerts,
        })
    }

    /// Check if user should receive in-app notifications
    pub async fn should_send_inapp(&self, user_id: Uuid) -> Result<bool> {
        let prefs = self.get_or_create_default(user_id).await?;
        Ok(prefs.inapp_enabled)
    }

    /// Disable all notifications for a user (e.g., during account suspension)
    pub async fn disable_all(&self, user_id: Uuid) -> Result<NotificationPreferences> {
        let prefs = sqlx::query_as::<_, NotificationPreferences>(
            r#"
            UPDATE notification_preferences
            SET
                email_enabled = false,
                sms_enabled = false,
                push_enabled = false,
                inapp_enabled = false,
                updated_at = NOW()
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(prefs)
    }

    /// Enable default notifications for a user (e.g., after account reactivation)
    pub async fn enable_defaults(&self, user_id: Uuid) -> Result<NotificationPreferences> {
        let prefs = sqlx::query_as::<_, NotificationPreferences>(
            r#"
            UPDATE notification_preferences
            SET
                email_enabled = true,
                email_order_updates = true,
                email_trade_notifications = true,
                email_commitment_updates = true,
                email_kyc_updates = true,
                email_marketing = false,
                sms_enabled = false,
                push_enabled = true,
                push_order_updates = true,
                push_trade_notifications = true,
                inapp_enabled = true,
                updated_at = NOW()
            WHERE user_id = $1
            RETURNING *
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(prefs)
    }

    /// Batch update email preferences
    #[allow(clippy::too_many_arguments)]
    pub async fn update_email_preferences(
        &self,
        user_id: Uuid,
        enabled: Option<bool>,
        order_updates: Option<bool>,
        trade_notifications: Option<bool>,
        commitment_updates: Option<bool>,
        kyc_updates: Option<bool>,
        marketing: Option<bool>,
    ) -> Result<NotificationPreferences> {
        self.update(
            user_id,
            &UpdateNotificationPreferences {
                email_enabled: enabled,
                email_order_updates: order_updates,
                email_trade_notifications: trade_notifications,
                email_commitment_updates: commitment_updates,
                email_kyc_updates: kyc_updates,
                email_marketing: marketing,
                ..Default::default()
            },
        )
        .await
    }

    /// Batch update SMS preferences
    #[allow(clippy::too_many_arguments)]
    pub async fn update_sms_preferences(
        &self,
        user_id: Uuid,
        enabled: Option<bool>,
        order_updates: Option<bool>,
        trade_notifications: Option<bool>,
        security_alerts: Option<bool>,
    ) -> Result<NotificationPreferences> {
        self.update(
            user_id,
            &UpdateNotificationPreferences {
                sms_enabled: enabled,
                sms_order_updates: order_updates,
                sms_trade_notifications: trade_notifications,
                sms_security_alerts: security_alerts,
                ..Default::default()
            },
        )
        .await
    }

    /// Batch update push notification preferences
    #[allow(clippy::too_many_arguments)]
    pub async fn update_push_preferences(
        &self,
        user_id: Uuid,
        enabled: Option<bool>,
        order_updates: Option<bool>,
        trade_notifications: Option<bool>,
        price_alerts: Option<bool>,
    ) -> Result<NotificationPreferences> {
        self.update(
            user_id,
            &UpdateNotificationPreferences {
                push_enabled: enabled,
                push_order_updates: order_updates,
                push_trade_notifications: trade_notifications,
                push_price_alerts: price_alerts,
                ..Default::default()
            },
        )
        .await
    }

    /// Get count of users with marketing emails enabled
    pub async fn count_marketing_enabled(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM notification_preferences
            WHERE email_enabled = true AND email_marketing = true
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Get all users with marketing emails enabled (for bulk campaigns)
    pub async fn get_marketing_enabled_users(&self, limit: i64, offset: i64) -> Result<Vec<Uuid>> {
        let users: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT user_id FROM notification_preferences
            WHERE email_enabled = true AND email_marketing = true
            ORDER BY user_id
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(users.into_iter().map(|row| row.0).collect())
    }

    /// Bulk create default preferences for new users (batch operation)
    pub async fn batch_create_defaults<'a, E>(&self, executor: E, user_ids: &[Uuid]) -> Result<u64>
    where
        E: PgExecutor<'a>,
    {
        if user_ids.is_empty() {
            return Ok(0);
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            INSERT INTO notification_preferences (user_id)
            "#,
        );

        query_builder.push_values(user_ids, |mut b, user_id| {
            b.push_bind(user_id);
        });

        query_builder.push(" ON CONFLICT (user_id) DO NOTHING");

        let result = query_builder.build().execute(executor).await?;

        Ok(result.rows_affected())
    }
}

/// Email notification types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailNotificationType {
    OrderUpdate,
    TradeNotification,
    CommitmentUpdate,
    KycUpdate,
    Marketing,
}

/// SMS notification types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsNotificationType {
    OrderUpdate,
    TradeNotification,
    SecurityAlert,
}

/// Push notification types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushNotificationType {
    OrderUpdate,
    TradeNotification,
    PriceAlert,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_preferences_struct() {
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let prefs = NotificationPreferences {
            user_id,
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
            created_at: now,
            updated_at: now,
        };

        assert_eq!(prefs.user_id, user_id);
        assert!(prefs.email_enabled);
        assert!(!prefs.email_marketing);
        assert!(!prefs.sms_enabled);
        assert!(prefs.push_enabled);
        assert!(prefs.inapp_enabled);
    }

    #[test]
    fn test_create_notification_preferences_struct() {
        let user_id = Uuid::new_v4();

        let input = CreateNotificationPreferences {
            user_id,
            email_enabled: Some(true),
            email_order_updates: Some(false),
            email_trade_notifications: None,
            email_commitment_updates: None,
            email_kyc_updates: None,
            email_marketing: Some(true),
            sms_enabled: Some(true),
            sms_order_updates: None,
            sms_trade_notifications: None,
            sms_security_alerts: None,
            push_enabled: None,
            push_order_updates: None,
            push_trade_notifications: None,
            push_price_alerts: Some(true),
            inapp_enabled: None,
        };

        assert_eq!(input.user_id, user_id);
        assert_eq!(input.email_enabled, Some(true));
        assert_eq!(input.email_order_updates, Some(false));
        assert_eq!(input.email_trade_notifications, None);
        assert_eq!(input.email_marketing, Some(true));
    }

    #[test]
    fn test_update_notification_preferences_struct() {
        let input = UpdateNotificationPreferences {
            email_enabled: Some(false),
            email_order_updates: Some(true),
            ..Default::default()
        };

        assert_eq!(input.email_enabled, Some(false));
        assert_eq!(input.email_order_updates, Some(true));
        assert_eq!(input.sms_enabled, None);
        assert_eq!(input.push_enabled, None);
    }

    #[test]
    fn test_email_notification_type_enum() {
        let notification = EmailNotificationType::OrderUpdate;
        assert_eq!(notification, EmailNotificationType::OrderUpdate);
        assert_ne!(notification, EmailNotificationType::TradeNotification);

        let marketing = EmailNotificationType::Marketing;
        assert_eq!(marketing, EmailNotificationType::Marketing);
    }

    #[test]
    fn test_sms_notification_type_enum() {
        let notification = SmsNotificationType::SecurityAlert;
        assert_eq!(notification, SmsNotificationType::SecurityAlert);
        assert_ne!(notification, SmsNotificationType::OrderUpdate);
    }

    #[test]
    fn test_push_notification_type_enum() {
        let notification = PushNotificationType::PriceAlert;
        assert_eq!(notification, PushNotificationType::PriceAlert);
        assert_ne!(notification, PushNotificationType::TradeNotification);
    }

    #[test]
    fn test_default_update_preferences() {
        let defaults = UpdateNotificationPreferences::default();
        assert_eq!(defaults.email_enabled, None);
        assert_eq!(defaults.sms_enabled, None);
        assert_eq!(defaults.push_enabled, None);
        assert_eq!(defaults.inapp_enabled, None);
    }
}
