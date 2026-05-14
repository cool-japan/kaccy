use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Configuration value type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    /// A UTF-8 string value.
    String,
    /// A numeric value (stored as string, parsed as f64).
    Number,
    /// A boolean true/false value.
    Boolean,
    /// An arbitrary JSON value.
    Json,
}

impl ValueType {
    /// Return the lowercase string representation of the value type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ValueType::String => "string",
            ValueType::Number => "number",
            ValueType::Boolean => "boolean",
            ValueType::Json => "json",
        }
    }
}

/// System configuration model
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Unique identifier for the config entry.
    pub id: Uuid,
    /// Configuration key (e.g., "feature.dark_mode").
    pub config_key: String,
    /// Serialized configuration value.
    pub config_value: String,
    /// Value type string (e.g., "string", "number", "boolean", "json").
    pub value_type: String,
    /// Human-readable description of the configuration.
    pub description: Option<String>,
    /// Optional grouping category.
    pub category: Option<String>,
    /// Environment this config applies to (e.g., "production", "staging").
    pub environment: String,
    /// Monotonically increasing version number.
    pub version: i32,
    /// Whether this configuration entry is currently active.
    pub is_active: bool,
    /// JSON schema or rules used to validate the config value.
    pub validation_rules: Option<JsonValue>,
    /// Whether the config value should be treated as sensitive.
    pub is_sensitive: bool,
    /// User who last changed this config, if known.
    pub changed_by: Option<Uuid>,
    /// Reason for the most recent change.
    pub changed_reason: Option<String>,
    /// Previous value before the most recent change.
    pub previous_value: Option<String>,
    /// Timestamp when this entry was first created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent update.
    pub updated_at: DateTime<Utc>,
    /// Timestamp from which this config becomes effective.
    pub effective_from: Option<DateTime<Utc>>,
    /// Timestamp after which this config is no longer effective.
    pub effective_until: Option<DateTime<Utc>>,
}

/// Parameters for creating a new config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSystemConfig {
    /// Configuration key.
    pub config_key: String,
    /// Initial configuration value.
    pub config_value: String,
    /// Type of the configuration value.
    pub value_type: ValueType,
    /// Optional description.
    pub description: Option<String>,
    /// Optional category grouping.
    pub category: Option<String>,
    /// Target environment; defaults to "production" if not specified.
    pub environment: Option<String>,
    /// JSON validation rules for the value.
    pub validation_rules: Option<JsonValue>,
    /// Whether the value is sensitive; defaults to false.
    pub is_sensitive: Option<bool>,
    /// User creating the config.
    pub changed_by: Option<Uuid>,
    /// Reason for creation.
    pub changed_reason: Option<String>,
    /// When the config becomes effective.
    pub effective_from: Option<DateTime<Utc>>,
    /// When the config expires.
    pub effective_until: Option<DateTime<Utc>>,
}

/// Parameters for updating a config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSystemConfig {
    /// New configuration value.
    pub config_value: String,
    /// Updated description.
    pub description: Option<String>,
    /// User making the update.
    pub changed_by: Option<Uuid>,
    /// Reason for the update.
    pub changed_reason: Option<String>,
    /// New effective-from timestamp.
    pub effective_from: Option<DateTime<Utc>>,
    /// New expiry timestamp.
    pub effective_until: Option<DateTime<Utc>>,
}

/// Configuration change history
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ConfigHistory {
    /// Unique identifier for this history record.
    pub id: Uuid,
    /// Config entry that was changed.
    pub config_id: Uuid,
    /// Key of the config entry.
    pub config_key: String,
    /// Previous value before the change.
    pub old_value: Option<String>,
    /// New value after the change.
    pub new_value: String,
    /// Value type of the config.
    pub value_type: String,
    /// Type of change (e.g., "create", "update", "delete").
    pub change_type: String,
    /// User who made the change.
    pub changed_by: Option<Uuid>,
    /// Reason provided for the change.
    pub changed_reason: Option<String>,
    /// Environment the change applies to.
    pub environment: String,
    /// Timestamp when the change was recorded.
    pub created_at: DateTime<Utc>,
}

/// Configuration summary by category
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ConfigCategorySummary {
    /// Category name.
    pub category: String,
    /// Total number of configs in this category.
    pub total_configs: i64,
    /// Number of currently active configs.
    pub active_configs: i64,
}

/// System configuration repository
pub struct SystemConfigRepository {
    pool: PgPool,
}

impl SystemConfigRepository {
    /// Create a new system config repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new configuration
    pub async fn create(&self, params: CreateSystemConfig) -> Result<SystemConfig> {
        let config = sqlx::query_as::<_, SystemConfig>(
            r#"
            INSERT INTO system_config (
                config_key, config_value, value_type, description, category,
                environment, validation_rules, is_sensitive, changed_by, changed_reason,
                effective_from, effective_until
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(&params.config_key)
        .bind(&params.config_value)
        .bind(params.value_type.as_str())
        .bind(&params.description)
        .bind(&params.category)
        .bind(
            params
                .environment
                .unwrap_or_else(|| "production".to_string()),
        )
        .bind(&params.validation_rules)
        .bind(params.is_sensitive.unwrap_or(false))
        .bind(params.changed_by)
        .bind(&params.changed_reason)
        .bind(params.effective_from)
        .bind(params.effective_until)
        .fetch_one(&self.pool)
        .await?;

        // Record in history
        self.record_history(
            config.id,
            &config.config_key,
            None,
            &config.config_value,
            &config.value_type,
            "created",
            params.changed_by,
            params.changed_reason.as_deref(),
            &config.environment,
        )
        .await?;

        Ok(config)
    }

    /// Get configuration by key and environment
    pub async fn get_by_key(
        &self,
        key: &str,
        environment: Option<&str>,
    ) -> Result<Option<SystemConfig>> {
        let env = environment.unwrap_or("production");

        let config = sqlx::query_as::<_, SystemConfig>(
            r#"
            SELECT * FROM system_config
            WHERE config_key = $1
                AND (environment = $2 OR environment = 'all')
                AND is_active = true
                AND (effective_from IS NULL OR effective_from <= NOW())
                AND (effective_until IS NULL OR effective_until > NOW())
            ORDER BY
                CASE WHEN environment = $2 THEN 0 ELSE 1 END,
                version DESC
            LIMIT 1
            "#,
        )
        .bind(key)
        .bind(env)
        .fetch_optional(&self.pool)
        .await?;

        Ok(config)
    }

    /// Get all active configurations for an environment
    pub async fn get_all(&self, environment: Option<&str>) -> Result<Vec<SystemConfig>> {
        let env = environment.unwrap_or("production");

        let configs = sqlx::query_as::<_, SystemConfig>(
            r#"
            SELECT DISTINCT ON (config_key) *
            FROM system_config
            WHERE (environment = $1 OR environment = 'all')
                AND is_active = true
                AND (effective_from IS NULL OR effective_from <= NOW())
                AND (effective_until IS NULL OR effective_until > NOW())
            ORDER BY config_key,
                CASE WHEN environment = $1 THEN 0 ELSE 1 END,
                version DESC
            "#,
        )
        .bind(env)
        .fetch_all(&self.pool)
        .await?;

        Ok(configs)
    }

    /// Get configurations by category
    pub async fn get_by_category(
        &self,
        category: &str,
        environment: Option<&str>,
    ) -> Result<Vec<SystemConfig>> {
        let env = environment.unwrap_or("production");

        let configs = sqlx::query_as::<_, SystemConfig>(
            r#"
            SELECT DISTINCT ON (config_key) *
            FROM system_config
            WHERE category = $1
                AND (environment = $2 OR environment = 'all')
                AND is_active = true
                AND (effective_from IS NULL OR effective_from <= NOW())
                AND (effective_until IS NULL OR effective_until > NOW())
            ORDER BY config_key,
                CASE WHEN environment = $2 THEN 0 ELSE 1 END,
                version DESC
            "#,
        )
        .bind(category)
        .bind(env)
        .fetch_all(&self.pool)
        .await?;

        Ok(configs)
    }

    /// Update configuration value (creates new version)
    pub async fn update(
        &self,
        key: &str,
        environment: Option<&str>,
        params: UpdateSystemConfig,
    ) -> Result<SystemConfig> {
        let env = environment.unwrap_or("production");

        // Get current config
        let current = self.get_by_key(key, Some(env)).await?;
        if current.is_none() {
            return Err(crate::error::DbError::NotFound(format!(
                "Configuration not found: {}",
                key
            )));
        }
        let current = current.expect("current is Some per is_none check above");

        // Deactivate old version
        sqlx::query(
            r#"
            UPDATE system_config
            SET is_active = false, updated_at = NOW()
            WHERE config_key = $1 AND environment = $2 AND is_active = true
            "#,
        )
        .bind(key)
        .bind(env)
        .execute(&self.pool)
        .await?;

        // Create new version
        let new_config = sqlx::query_as::<_, SystemConfig>(
            r#"
            INSERT INTO system_config (
                config_key, config_value, value_type, description, category,
                environment, version, validation_rules, is_sensitive,
                changed_by, changed_reason, previous_value,
                effective_from, effective_until
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING *
            "#,
        )
        .bind(key)
        .bind(&params.config_value)
        .bind(&current.value_type)
        .bind(params.description.or(current.description))
        .bind(&current.category)
        .bind(env)
        .bind(current.version + 1)
        .bind(&current.validation_rules)
        .bind(current.is_sensitive)
        .bind(params.changed_by)
        .bind(&params.changed_reason)
        .bind(&current.config_value)
        .bind(params.effective_from)
        .bind(params.effective_until)
        .fetch_one(&self.pool)
        .await?;

        // Record in history
        self.record_history(
            new_config.id,
            key,
            Some(&current.config_value),
            &params.config_value,
            &current.value_type,
            "updated",
            params.changed_by,
            params.changed_reason.as_deref(),
            env,
        )
        .await?;

        Ok(new_config)
    }

    /// Delete configuration (soft delete by deactivating)
    pub async fn delete(
        &self,
        key: &str,
        environment: Option<&str>,
        changed_by: Option<Uuid>,
        reason: Option<&str>,
    ) -> Result<()> {
        let env = environment.unwrap_or("production");

        // Get current config for history
        let current = self.get_by_key(key, Some(env)).await?;
        if let Some(config) = current {
            sqlx::query(
                r#"
                UPDATE system_config
                SET is_active = false, updated_at = NOW()
                WHERE config_key = $1 AND environment = $2 AND is_active = true
                "#,
            )
            .bind(key)
            .bind(env)
            .execute(&self.pool)
            .await?;

            // Record in history
            self.record_history(
                config.id,
                key,
                Some(&config.config_value),
                "",
                &config.value_type,
                "deleted",
                changed_by,
                reason,
                env,
            )
            .await?;
        }

        Ok(())
    }

    /// Get configuration history
    pub async fn get_history(&self, key: &str, limit: i64) -> Result<Vec<ConfigHistory>> {
        let history = sqlx::query_as::<_, ConfigHistory>(
            r#"
            SELECT * FROM system_config_history
            WHERE config_key = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Get all versions of a configuration
    pub async fn get_versions(
        &self,
        key: &str,
        environment: Option<&str>,
    ) -> Result<Vec<SystemConfig>> {
        let env = environment.unwrap_or("production");

        let versions = sqlx::query_as::<_, SystemConfig>(
            r#"
            SELECT * FROM system_config
            WHERE config_key = $1 AND environment = $2
            ORDER BY version DESC
            "#,
        )
        .bind(key)
        .bind(env)
        .fetch_all(&self.pool)
        .await?;

        Ok(versions)
    }

    /// Rollback to previous version
    pub async fn rollback(
        &self,
        key: &str,
        environment: Option<&str>,
        changed_by: Option<Uuid>,
        reason: Option<&str>,
    ) -> Result<SystemConfig> {
        let env = environment.unwrap_or("production");

        // Get current and previous versions
        let versions = self.get_versions(key, Some(env)).await?;
        if versions.len() < 2 {
            return Err(crate::error::DbError::Other(
                "No previous version available for rollback".to_string(),
            ));
        }

        let previous = &versions[1];

        // Update using previous value
        self.update(
            key,
            Some(env),
            UpdateSystemConfig {
                config_value: previous.config_value.clone(),
                description: previous.description.clone(),
                changed_by,
                changed_reason: Some(format!(
                    "Rollback to version {}: {}",
                    previous.version,
                    reason.unwrap_or("No reason provided")
                )),
                effective_from: None,
                effective_until: None,
            },
        )
        .await
    }

    /// Batch update configurations
    pub async fn batch_update(
        &self,
        updates: Vec<(String, String, Option<Uuid>, Option<String>)>,
    ) -> Result<i64> {
        let mut count = 0i64;

        for (key, value, changed_by, reason) in updates {
            let update_params = UpdateSystemConfig {
                config_value: value,
                description: None,
                changed_by,
                changed_reason: reason,
                effective_from: None,
                effective_until: None,
            };

            if self.update(&key, None, update_params).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get configuration summary by category
    pub async fn get_category_summary(&self) -> Result<Vec<ConfigCategorySummary>> {
        let summary = sqlx::query_as::<_, ConfigCategorySummary>(
            r#"
            SELECT
                COALESCE(category, 'uncategorized') as category,
                COUNT(*) as total_configs,
                COUNT(*) FILTER (WHERE is_active = true) as active_configs
            FROM system_config
            GROUP BY category
            ORDER BY active_configs DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Count active configurations
    pub async fn count_active(&self, environment: Option<&str>) -> Result<i64> {
        let env = environment.unwrap_or("production");

        let row = sqlx::query(
            r#"
            SELECT COUNT(DISTINCT config_key) as count
            FROM system_config
            WHERE (environment = $1 OR environment = 'all')
                AND is_active = true
            "#,
        )
        .bind(env)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }

    /// Check if a feature flag is enabled
    pub async fn is_feature_enabled(
        &self,
        feature_key: &str,
        environment: Option<&str>,
    ) -> Result<bool> {
        if let Some(config) = self.get_by_key(feature_key, environment).await? {
            Ok(config.config_value.eq_ignore_ascii_case("true") || config.config_value == "1")
        } else {
            Ok(false)
        }
    }

    /// Get configuration as typed value
    pub async fn get_string(&self, key: &str, environment: Option<&str>) -> Result<Option<String>> {
        Ok(self
            .get_by_key(key, environment)
            .await?
            .map(|c| c.config_value))
    }

    /// Retrieve a configuration value parsed as a floating-point number.
    pub async fn get_number(&self, key: &str, environment: Option<&str>) -> Result<Option<f64>> {
        Ok(self
            .get_by_key(key, environment)
            .await?
            .and_then(|c| c.config_value.parse::<f64>().ok()))
    }

    /// Retrieve a configuration value parsed as a boolean.
    pub async fn get_bool(&self, key: &str, environment: Option<&str>) -> Result<Option<bool>> {
        Ok(self
            .get_by_key(key, environment)
            .await?
            .map(|c| c.config_value.eq_ignore_ascii_case("true") || c.config_value == "1"))
    }

    /// Retrieve a configuration value parsed as a JSON value.
    pub async fn get_json(
        &self,
        key: &str,
        environment: Option<&str>,
    ) -> Result<Option<JsonValue>> {
        Ok(self
            .get_by_key(key, environment)
            .await?
            .and_then(|c| serde_json::from_str(&c.config_value).ok()))
    }

    /// Record configuration change in history
    #[allow(clippy::too_many_arguments)]
    async fn record_history(
        &self,
        config_id: Uuid,
        config_key: &str,
        old_value: Option<&str>,
        new_value: &str,
        value_type: &str,
        change_type: &str,
        changed_by: Option<Uuid>,
        changed_reason: Option<&str>,
        environment: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_config_history (
                config_id, config_key, old_value, new_value, value_type,
                change_type, changed_by, changed_reason, environment
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(config_id)
        .bind(config_key)
        .bind(old_value)
        .bind(new_value)
        .bind(value_type)
        .bind(change_type)
        .bind(changed_by)
        .bind(changed_reason)
        .bind(environment)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_type_as_str() {
        assert_eq!(ValueType::String.as_str(), "string");
        assert_eq!(ValueType::Number.as_str(), "number");
        assert_eq!(ValueType::Boolean.as_str(), "boolean");
        assert_eq!(ValueType::Json.as_str(), "json");
    }

    #[test]
    fn test_value_type_serialization() {
        let vtype = ValueType::String;
        let json = serde_json::to_string(&vtype).unwrap();
        assert_eq!(json, r#""string""#);

        let deserialized: ValueType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ValueType::String);
    }

    #[test]
    fn test_create_config_params() {
        let params = CreateSystemConfig {
            config_key: "max_api_requests".to_string(),
            config_value: "1000".to_string(),
            value_type: ValueType::Number,
            description: Some("Maximum API requests per hour".to_string()),
            category: Some("api_limits".to_string()),
            environment: Some("production".to_string()),
            validation_rules: None,
            is_sensitive: Some(false),
            changed_by: Some(Uuid::new_v4()),
            changed_reason: Some("Initial configuration".to_string()),
            effective_from: None,
            effective_until: None,
        };

        assert_eq!(params.config_key, "max_api_requests");
        assert_eq!(params.value_type, ValueType::Number);
    }

    #[test]
    fn test_update_config_params() {
        let params = UpdateSystemConfig {
            config_value: "2000".to_string(),
            description: Some("Updated limit".to_string()),
            changed_by: Some(Uuid::new_v4()),
            changed_reason: Some("Increased for peak hours".to_string()),
            effective_from: None,
            effective_until: None,
        };

        assert_eq!(params.config_value, "2000");
        assert!(params.changed_by.is_some());
    }

    #[test]
    fn test_config_category_summary_structure() {
        let summary = ConfigCategorySummary {
            category: "feature_flags".to_string(),
            total_configs: 10,
            active_configs: 8,
        };

        assert_eq!(summary.category, "feature_flags");
        assert_eq!(summary.active_configs, 8);
    }
}
