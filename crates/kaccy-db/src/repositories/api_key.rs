//! API Key Repository
//!
//! Provides database operations for API key management and authentication.

use crate::error::{DbError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

/// API Key record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiKey {
    /// Unique identifier for the API key.
    pub id: Uuid,
    /// Owner of the API key.
    pub user_id: Uuid,
    /// Bcrypt or Argon2 hash of the full API key value.
    pub key_hash: String,
    /// Short public prefix shown to the user for identification.
    pub key_prefix: String,
    /// Human-readable label for this key.
    pub name: String,
    /// JSON array of permission scopes.
    pub scopes: JsonValue,
    /// Maximum requests allowed per hour, or unlimited if None.
    pub rate_limit_per_hour: Option<i32>,
    /// Timestamp of the most recent use.
    pub last_used_at: Option<DateTime<Utc>>,
    /// Timestamp when the key expires, or None if it never expires.
    pub expires_at: Option<DateTime<Utc>>,
    /// Timestamp when the key was revoked, or None if active.
    pub revoked_at: Option<DateTime<Utc>>,
    /// Record creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Record last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// API Key creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKey {
    /// Owner of the new API key.
    pub user_id: Uuid,
    /// Hash of the API key value.
    pub key_hash: String,
    /// Short prefix shown to the user.
    pub key_prefix: String,
    /// Label for the key.
    pub name: String,
    /// List of permission scope strings.
    pub scopes: Vec<String>,
    /// Optional hourly rate limit.
    pub rate_limit_per_hour: Option<i32>,
    /// Optional expiry timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// API Key summary (without sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeySummary {
    /// Unique identifier for the key.
    pub id: Uuid,
    /// Short public prefix.
    pub key_prefix: String,
    /// Label for the key.
    pub name: String,
    /// Granted permission scopes.
    pub scopes: Vec<String>,
    /// Optional hourly rate limit.
    pub rate_limit_per_hour: Option<i32>,
    /// Timestamp of the most recent use.
    pub last_used_at: Option<DateTime<Utc>>,
    /// Optional expiry timestamp.
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the key is currently active (not revoked and not expired).
    pub is_active: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Repository for API key operations
pub struct ApiKeyRepository {
    pool: PgPool,
}

impl ApiKeyRepository {
    /// Create a new API key repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new API key
    pub async fn create(&self, params: CreateApiKey) -> Result<ApiKey> {
        let scopes_json = serde_json::to_value(&params.scopes)
            .map_err(|e| DbError::Validation(format!("Invalid scopes: {}", e)))?;

        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (
                user_id, key_hash, key_prefix, name, scopes,
                rate_limit_per_hour, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(params.user_id)
        .bind(&params.key_hash)
        .bind(&params.key_prefix)
        .bind(&params.name)
        .bind(&scopes_json)
        .bind(params.rate_limit_per_hour)
        .bind(params.expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(api_key)
    }

    /// Find API key by hash
    pub async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>> {
        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT * FROM api_keys
            WHERE key_hash = $1
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(api_key)
    }

    /// Find API key by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<ApiKey>> {
        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT * FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(api_key)
    }

    /// Get active API keys for a user
    pub async fn get_active_keys(&self, user_id: Uuid) -> Result<Vec<ApiKeySummary>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT * FROM api_keys
            WHERE user_id = $1
              AND revoked_at IS NULL
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(keys.into_iter().map(Self::to_summary).collect())
    }

    /// Get all API keys for a user (including revoked/expired)
    pub async fn get_user_keys(&self, user_id: Uuid) -> Result<Vec<ApiKeySummary>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT * FROM api_keys
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(keys.into_iter().map(Self::to_summary).collect())
    }

    /// Update last used timestamp
    pub async fn update_last_used(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET last_used_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Revoke an API key
    pub async fn revoke(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete an API key (permanent)
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update API key name
    pub async fn update_name(&self, id: Uuid, name: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET name = $1
            WHERE id = $2
            "#,
        )
        .bind(name)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update API key scopes
    pub async fn update_scopes(&self, id: Uuid, scopes: Vec<String>) -> Result<()> {
        let scopes_json = serde_json::to_value(&scopes)
            .map_err(|e| DbError::Validation(format!("Invalid scopes: {}", e)))?;

        sqlx::query(
            r#"
            UPDATE api_keys
            SET scopes = $1
            WHERE id = $2
            "#,
        )
        .bind(&scopes_json)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update rate limit
    pub async fn update_rate_limit(&self, id: Uuid, rate_limit: Option<i32>) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET rate_limit_per_hour = $1
            WHERE id = $2
            "#,
        )
        .bind(rate_limit)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if API key is valid (not revoked, not expired, exists)
    pub async fn is_valid(&self, key_hash: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM api_keys
                WHERE key_hash = $1
                  AND revoked_at IS NULL
                  AND (expires_at IS NULL OR expires_at > NOW())
            )
            "#,
        )
        .bind(key_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get count of active keys for a user
    pub async fn count_active_keys(&self, user_id: Uuid) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM api_keys
            WHERE user_id = $1
              AND revoked_at IS NULL
              AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Clean up expired keys (soft delete by marking as revoked)
    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = NOW()
            WHERE expires_at IS NOT NULL
              AND expires_at <= NOW()
              AND revoked_at IS NULL
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Verify key has required scope
    pub async fn has_scope(&self, id: Uuid, required_scope: &str) -> Result<bool> {
        let api_key = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| DbError::NotFound("API key not found".to_string()))?;

        let scopes: Vec<String> = serde_json::from_value(api_key.scopes)
            .map_err(|e| DbError::Validation(format!("Invalid scopes format: {}", e)))?;

        Ok(scopes.contains(&required_scope.to_string()) || scopes.contains(&"*".to_string()))
    }

    /// Convert ApiKey to ApiKeySummary
    fn to_summary(key: ApiKey) -> ApiKeySummary {
        let scopes: Vec<String> = serde_json::from_value(key.scopes.clone()).unwrap_or_default();
        let is_active = key.revoked_at.is_none()
            && (key.expires_at.is_none() || key.expires_at.expect("expires_at is Some per is_none check") > Utc::now());

        ApiKeySummary {
            id: key.id,
            key_prefix: key.key_prefix,
            name: key.name,
            scopes,
            rate_limit_per_hour: key.rate_limit_per_hour,
            last_used_at: key.last_used_at,
            expires_at: key.expires_at,
            is_active,
            created_at: key.created_at,
        }
    }
}

/// Execute API key operations within a transaction
pub async fn execute_in_transaction<'a, E, F, T>(executor: E, f: F) -> Result<T>
where
    E: Executor<'a, Database = Postgres>,
    F: FnOnce(E) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send + 'a>>,
{
    f(executor).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_create_api_key_params() {
        let params = CreateApiKey {
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Test API Key".to_string(),
            scopes: vec!["tokens:read".to_string(), "orders:write".to_string()],
            rate_limit_per_hour: Some(1000),
            expires_at: None,
        };

        assert_eq!(params.name, "Test API Key");
        assert_eq!(params.scopes.len(), 2);
    }

    #[test]
    fn test_api_key_summary_creation() {
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Test Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: None,
            expires_at: None,
            revoked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let summary = ApiKeyRepository::to_summary(key.clone());
        assert_eq!(summary.id, key.id);
        assert_eq!(summary.key_prefix, key.key_prefix);
        assert!(summary.is_active);
    }

    #[test]
    fn test_expired_key_is_inactive() {
        let expired_at = Utc::now() - Duration::hours(1);
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Expired Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: None,
            expires_at: Some(expired_at),
            revoked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let summary = ApiKeyRepository::to_summary(key);
        assert!(!summary.is_active);
    }

    #[test]
    fn test_revoked_key_is_inactive() {
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Revoked Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: None,
            expires_at: None,
            revoked_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let summary = ApiKeyRepository::to_summary(key);
        assert!(!summary.is_active);
    }

    #[test]
    fn test_scopes_serialization() {
        let scopes = vec!["tokens:read".to_string(), "orders:write".to_string()];
        let json = serde_json::to_value(&scopes).unwrap();
        let deserialized: Vec<String> = serde_json::from_value(json).unwrap();
        assert_eq!(scopes, deserialized);
    }

    #[test]
    fn test_wildcard_scope() {
        let scopes = ["*".to_string()];
        assert!(scopes.contains(&"*".to_string()));
    }

    #[test]
    fn test_multiple_scopes_validation() {
        let scopes = [
            "tokens:read".to_string(),
            "tokens:write".to_string(),
            "orders:read".to_string(),
            "orders:write".to_string(),
            "trades:read".to_string(),
        ];
        assert_eq!(scopes.len(), 5);
        assert!(scopes.contains(&"tokens:read".to_string()));
        assert!(scopes.contains(&"trades:read".to_string()));
    }

    #[test]
    fn test_key_with_no_expiration() {
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Permanent Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: None,
            expires_at: None,
            revoked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let summary = ApiKeyRepository::to_summary(key.clone());
        assert!(summary.is_active);
        assert!(summary.expires_at.is_none());
    }

    #[test]
    fn test_key_with_future_expiration() {
        let future_expiration = Utc::now() + Duration::days(30);
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Future Expiry Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: None,
            expires_at: Some(future_expiration),
            revoked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let summary = ApiKeyRepository::to_summary(key);
        assert!(summary.is_active);
    }

    #[test]
    fn test_key_with_custom_rate_limit() {
        let key = CreateApiKey {
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "High Rate Limit Key".to_string(),
            scopes: vec!["tokens:read".to_string()],
            rate_limit_per_hour: Some(100000),
            expires_at: None,
        };

        assert_eq!(key.rate_limit_per_hour, Some(100000));
    }

    #[test]
    fn test_key_with_no_rate_limit() {
        let key = CreateApiKey {
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Unlimited Key".to_string(),
            scopes: vec!["*".to_string()],
            rate_limit_per_hour: None,
            expires_at: None,
        };

        assert!(key.rate_limit_per_hour.is_none());
    }

    #[test]
    fn test_empty_scopes() {
        let scopes: Vec<String> = vec![];
        let json = serde_json::to_value(&scopes).unwrap();
        let deserialized: Vec<String> = serde_json::from_value(json).unwrap();
        assert_eq!(scopes, deserialized);
        assert!(deserialized.is_empty());
    }

    #[test]
    fn test_key_prefix_format() {
        let key = CreateApiKey {
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_test123".to_string(),
            name: "Test Key".to_string(),
            scopes: vec!["tokens:read".to_string()],
            rate_limit_per_hour: Some(1000),
            expires_at: None,
        };

        assert!(key.key_prefix.starts_with("kaccy_"));
        assert!(key.key_prefix.len() >= 8);
    }

    #[test]
    fn test_last_used_tracking() {
        let now = Utc::now();
        let key = ApiKey {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            key_hash: "hash123".to_string(),
            key_prefix: "kaccy_12".to_string(),
            name: "Used Key".to_string(),
            scopes: serde_json::json!(["tokens:read"]),
            rate_limit_per_hour: Some(1000),
            last_used_at: Some(now),
            expires_at: None,
            revoked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(key.last_used_at.is_some());
        assert_eq!(key.last_used_at.unwrap(), now);
    }
}
