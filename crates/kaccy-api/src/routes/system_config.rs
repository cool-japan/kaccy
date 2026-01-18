//! System configuration management routes
//!
//! Provides endpoints for managing dynamic system configuration (admin only)

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, middleware::Claims, state::AppState};
use kaccy_db::repositories::system_config::{
    ConfigCategorySummary, ConfigHistory, CreateSystemConfig, SystemConfig, SystemConfigRepository,
    UpdateSystemConfig, ValueType,
};

/// Query parameters for listing configs
#[derive(Debug, Deserialize)]
pub struct ListConfigsQuery {
    /// Filter by category
    pub category: Option<String>,
    /// Filter by environment (production, staging, development)
    pub environment: Option<String>,
    /// Include inactive configs
    #[serde(default)]
    pub include_inactive: bool,
}

/// Response for config list
#[derive(Debug, Serialize)]
pub struct ConfigListResponse {
    pub configs: Vec<SystemConfigResponse>,
    pub total: usize,
}

/// System config response (masks sensitive values)
#[derive(Debug, Serialize)]
pub struct SystemConfigResponse {
    pub id: Uuid,
    pub config_key: String,
    pub config_value: String,
    pub value_type: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub environment: String,
    pub version: i32,
    pub is_active: bool,
    pub is_sensitive: bool,
    pub changed_by: Option<Uuid>,
    pub changed_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_until: Option<DateTime<Utc>>,
}

impl From<SystemConfig> for SystemConfigResponse {
    fn from(config: SystemConfig) -> Self {
        let config_value = if config.is_sensitive {
            "***REDACTED***".to_string()
        } else {
            config.config_value
        };

        Self {
            id: config.id,
            config_key: config.config_key,
            config_value,
            value_type: config.value_type,
            description: config.description,
            category: config.category,
            environment: config.environment,
            version: config.version,
            is_active: config.is_active,
            is_sensitive: config.is_sensitive,
            changed_by: config.changed_by,
            changed_reason: config.changed_reason,
            created_at: config.created_at,
            updated_at: config.updated_at,
            effective_from: config.effective_from,
            effective_until: config.effective_until,
        }
    }
}

/// Create config request
#[derive(Debug, Deserialize)]
pub struct CreateConfigRequest {
    pub config_key: String,
    pub config_value: String,
    pub value_type: ValueType,
    pub description: Option<String>,
    pub category: Option<String>,
    pub environment: Option<String>,
    pub validation_rules: Option<serde_json::Value>,
    pub is_sensitive: Option<bool>,
    pub changed_reason: Option<String>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_until: Option<DateTime<Utc>>,
}

/// Update config request
#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub config_value: String,
    pub description: Option<String>,
    pub changed_reason: Option<String>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_until: Option<DateTime<Utc>>,
}

/// Rollback request
#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub to_version: i32,
    pub changed_reason: Option<String>,
}

/// Batch update request
#[derive(Debug, Deserialize)]
pub struct BatchUpdateRequest {
    pub updates: Vec<BatchConfigUpdate>,
    pub changed_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchConfigUpdate {
    pub config_key: String,
    pub config_value: String,
}

/// List system configurations
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn list_configs(
    _claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<ListConfigsQuery>,
) -> Result<Json<ConfigListResponse>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let env = query.environment.as_deref();
    let configs = if let Some(ref category) = query.category {
        repo.get_by_category(category, env).await?
    } else if query.include_inactive {
        repo.get_all(env).await?
    } else {
        // Get only active configs
        repo.get_all(env)
            .await?
            .into_iter()
            .filter(|c| c.is_active)
            .collect()
    };

    // Filter by environment if specified
    let configs = if let Some(ref env) = query.environment {
        configs
            .into_iter()
            .filter(|c| c.environment == *env)
            .collect()
    } else {
        configs
    };

    let total = configs.len();
    let configs: Vec<SystemConfigResponse> = configs.into_iter().map(Into::into).collect();

    Ok(Json(ConfigListResponse { configs, total }))
}

/// Get specific configuration by key
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_config(
    _claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
) -> Result<Json<SystemConfigResponse>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let config = repo
        .get_by_key(&config_key, None)
        .await?
        .ok_or(ApiError::NotFound("Config not found".to_string()))?;

    Ok(Json(config.into()))
}

/// Create a new configuration
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn create_config(
    claims: Claims,
    State(state): State<AppState>,
    Json(request): Json<CreateConfigRequest>,
) -> Result<Json<SystemConfigResponse>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    // Check if config already exists
    if let Some(_existing) = repo.get_by_key(&request.config_key, None).await? {
        return Err(ApiError::Validation(format!(
            "Configuration with key '{}' already exists",
            request.config_key
        )));
    }

    let user_id = Uuid::parse_str(&claims.sub).ok();

    let config = repo
        .create(CreateSystemConfig {
            config_key: request.config_key,
            config_value: request.config_value,
            value_type: request.value_type,
            description: request.description,
            category: request.category,
            environment: request.environment,
            validation_rules: request.validation_rules,
            is_sensitive: request.is_sensitive,
            changed_by: user_id,
            changed_reason: request.changed_reason,
            effective_from: request.effective_from,
            effective_until: request.effective_until,
        })
        .await?;

    Ok(Json(config.into()))
}

/// Update a configuration
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn update_config(
    claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
    Json(request): Json<UpdateConfigRequest>,
) -> Result<Json<SystemConfigResponse>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let user_id = Uuid::parse_str(&claims.sub).ok();

    let config = repo
        .update(
            &config_key,
            None, // environment
            UpdateSystemConfig {
                config_value: request.config_value,
                description: request.description,
                changed_by: user_id,
                changed_reason: request.changed_reason,
                effective_from: request.effective_from,
                effective_until: request.effective_until,
            },
        )
        .await?;

    Ok(Json(config.into()))
}

/// Delete a configuration
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn delete_config(
    claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let user_id = Uuid::parse_str(&claims.sub).ok();

    repo.delete(&config_key, None, user_id, None).await?;

    Ok(Json(serde_json::json!({
        "message": "Configuration deleted successfully"
    })))
}

/// Get configuration change history
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_config_history(
    _claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
) -> Result<Json<Vec<ConfigHistory>>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let history = repo
        .get_history(&config_key, 100) // Default limit of 100 entries
        .await?;

    Ok(Json(history))
}

/// Get all versions of a configuration
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_config_versions(
    _claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
) -> Result<Json<Vec<SystemConfig>>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let versions = repo.get_versions(&config_key, None).await?;

    Ok(Json(versions))
}

/// Rollback configuration to a previous version
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn rollback_config(
    claims: Claims,
    State(state): State<AppState>,
    Path(config_key): Path<String>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<SystemConfigResponse>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let user_id = Uuid::parse_str(&claims.sub).ok();
    let reason = request.changed_reason.as_deref();

    // Note: to_version is currently not used by the repository - it rolls back to immediate previous version
    let _ = request.to_version;

    let config = repo.rollback(&config_key, None, user_id, reason).await?;

    Ok(Json(config.into()))
}

/// Batch update multiple configurations
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn batch_update_configs(
    claims: Claims,
    State(state): State<AppState>,
    Json(request): Json<BatchUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let user_id = Uuid::parse_str(&claims.sub).ok();

    let updates: Vec<(String, String, Option<Uuid>, Option<String>)> = request
        .updates
        .into_iter()
        .map(|u| {
            (
                u.config_key,
                u.config_value,
                user_id,
                request.changed_reason.clone(),
            )
        })
        .collect();

    let updated_count = repo.batch_update(updates).await?;

    Ok(Json(serde_json::json!({
        "message": format!("Updated {} configurations", updated_count),
        "updated_count": updated_count
    })))
}

/// Get configuration summary by category
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_category_summary(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<ConfigCategorySummary>>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let summary = repo.get_category_summary().await?;

    Ok(Json(summary))
}

/// Get total active configurations count
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_active_config_count(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let count = repo.count_active(None).await?;

    Ok(Json(serde_json::json!({
        "active_configs": count
    })))
}

/// Check if a feature is enabled
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn is_feature_enabled(
    _claims: Claims,
    State(state): State<AppState>,
    Path(feature_key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = SystemConfigRepository::new(state.pool.clone());

    let enabled = repo.is_feature_enabled(&feature_key, None).await?;

    Ok(Json(serde_json::json!({
        "feature_key": feature_key,
        "enabled": enabled
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_response_masks_sensitive() {
        let config = SystemConfig {
            id: Uuid::new_v4(),
            config_key: "api_key".to_string(),
            config_value: "secret123".to_string(),
            value_type: "string".to_string(),
            description: None,
            category: None,
            environment: "production".to_string(),
            version: 1,
            is_active: true,
            validation_rules: None,
            is_sensitive: true,
            changed_by: None,
            changed_reason: None,
            previous_value: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            effective_from: None,
            effective_until: None,
        };

        let response: SystemConfigResponse = config.into();
        assert_eq!(response.config_value, "***REDACTED***");
    }

    #[test]
    fn test_list_configs_query_defaults() {
        let query: ListConfigsQuery = serde_json::from_str("{}").unwrap();
        assert!(!query.include_inactive);
        assert!(query.category.is_none());
    }
}
