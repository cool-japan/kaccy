//! API key management routes

use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    error::{ApiError, Result},
    middleware::{API_KEY_PREFIX, ApiKeyScope, Claims},
    state::AppState,
};

/// Request to create a new API key
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    /// Name/description for the API key
    pub name: String,
    /// Scopes/permissions for the API key
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Custom rate limit per hour (optional)
    pub rate_limit_per_hour: Option<i32>,
    /// Expiration date (optional)
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response containing the newly created API key
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiKeyResponse {
    /// API key ID
    pub id: Uuid,
    /// The actual API key (only shown once)
    pub api_key: String,
    /// Key prefix for identification
    pub key_prefix: String,
    /// Name/description
    pub name: String,
    /// Scopes
    pub scopes: Vec<String>,
    /// Rate limit per hour
    pub rate_limit_per_hour: Option<i32>,
    /// Expiration date
    pub expires_at: Option<DateTime<Utc>>,
    /// Created at
    pub created_at: DateTime<Utc>,
}

/// API key information (without the actual key)
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyInfo {
    /// API key ID
    pub id: Uuid,
    /// Key prefix for identification
    pub key_prefix: String,
    /// Name/description
    pub name: String,
    /// Scopes
    pub scopes: Vec<String>,
    /// Rate limit per hour
    pub rate_limit_per_hour: Option<i32>,
    /// Last used timestamp
    pub last_used_at: Option<DateTime<Utc>>,
    /// Expiration date
    pub expires_at: Option<DateTime<Utc>>,
    /// Revoked timestamp
    pub revoked_at: Option<DateTime<Utc>>,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

/// Request to update an API key
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateApiKeyRequest {
    /// New name/description
    pub name: Option<String>,
    /// New scopes
    pub scopes: Option<Vec<String>>,
    /// New rate limit per hour
    pub rate_limit_per_hour: Option<i32>,
    /// New expiration date
    pub expires_at: Option<DateTime<Utc>>,
}

/// Generate a secure random API key
fn generate_api_key() -> String {
    let mut rng = rand::rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    let key = hex::encode(random_bytes);
    format!("{}{}", API_KEY_PREFIX, key)
}

/// Hash an API key using Argon2
async fn hash_api_key(api_key: &str) -> Result<String> {
    let api_key = api_key.to_string();
    tokio::task::spawn_blocking(move || {
        use argon2::{
            Argon2,
            password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
        };
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(api_key.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| ApiError::InternalWithMessage(format!("Failed to hash API key: {}", e)))
    })
    .await
    .map_err(|e| ApiError::InternalWithMessage(e.to_string()))?
}

/// Validate scopes
fn validate_scopes(scopes: &[String]) -> Result<()> {
    for scope in scopes {
        if ApiKeyScope::from_str(scope).is_err() {
            return Err(ApiError::Validation(format!("Invalid scope: {}", scope)));
        }
    }
    Ok(())
}

/// Create a new API key
#[utoipa::path(
    post,
    path = "/api/api-keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created successfully", body = CreateApiKeyResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    ),
    security(("jwt" = []))
)]
pub async fn create_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>)> {
    // Validate input
    if req.name.trim().is_empty() {
        return Err(ApiError::Validation("API key name cannot be empty".into()));
    }

    validate_scopes(&req.scopes)?;

    // Validate rate limit
    if let Some(limit) = req.rate_limit_per_hour {
        if limit <= 0 {
            return Err(ApiError::Validation("Rate limit must be positive".into()));
        }
    }

    // Validate expiration
    if let Some(expires_at) = req.expires_at {
        if expires_at <= Utc::now() {
            return Err(ApiError::Validation(
                "Expiration date must be in the future".into(),
            ));
        }
    }

    // Generate API key
    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key).await?;

    // Extract prefix (first 8 chars after API_KEY_PREFIX)
    let key_without_prefix = &api_key.clone()[API_KEY_PREFIX.len()..];
    let key_prefix = key_without_prefix[..8].to_string();

    // Store in database
    let scopes_json = serde_json::to_value(&req.scopes)
        .map_err(|e| ApiError::InternalWithMessage(format!("Failed to serialize scopes: {}", e)))?;

    let record = sqlx::query_as::<_, (Uuid, chrono::NaiveDateTime)>(
        r#"
        INSERT INTO api_keys (user_id, key_hash, key_prefix, name, scopes, rate_limit_per_hour, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, created_at
        "#,
    )
    .bind(claims.sub)
    .bind(key_hash)
    .bind(&key_prefix)
    .bind(&req.name)
    .bind(scopes_json)
    .bind(req.rate_limit_per_hour)
    .bind(req.expires_at)
    .fetch_one(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: record.0,
            api_key,
            key_prefix,
            name: req.name,
            scopes: req.scopes,
            rate_limit_per_hour: req.rate_limit_per_hour,
            expires_at: req.expires_at,
            created_at: record.1.and_utc(),
        }),
    ))
}

/// List user's API keys
#[utoipa::path(
    get,
    path = "/api/api-keys",
    responses(
        (status = 200, description = "List of API keys", body = Vec<ApiKeyInfo>),
        (status = 401, description = "Unauthorized")
    ),
    security(("jwt" = []))
)]
pub async fn list_api_keys(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<ApiKeyInfo>>> {
    let records = sqlx::query_as::<_, (
        Uuid,
        String,
        String,
        serde_json::Value,
        Option<i32>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        chrono::NaiveDateTime,
        chrono::NaiveDateTime,
    )>(
        r#"
        SELECT id, key_prefix, name, scopes, rate_limit_per_hour, last_used_at, expires_at, revoked_at, created_at, updated_at
        FROM api_keys
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(claims.sub)
    .fetch_all(&state.pool)
    .await?;

    let keys = records
        .into_iter()
        .map(|r| {
            let scopes: Vec<String> = serde_json::from_value(r.3.clone()).unwrap_or_default();
            ApiKeyInfo {
                id: r.0,
                key_prefix: r.1,
                name: r.2,
                scopes,
                rate_limit_per_hour: r.4,
                last_used_at: r.5.map(|t| t.and_utc()),
                expires_at: r.6.map(|t| t.and_utc()),
                revoked_at: r.7.map(|t| t.and_utc()),
                created_at: r.8.and_utc(),
                updated_at: r.9.and_utc(),
            }
        })
        .collect();

    Ok(Json(keys))
}

/// Get a specific API key
#[utoipa::path(
    get,
    path = "/api/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key information", body = ApiKeyInfo),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found")
    ),
    security(("jwt" = []))
)]
pub async fn get_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiKeyInfo>> {
    let record = sqlx::query_as::<_, (
        Uuid,
        String,
        String,
        serde_json::Value,
        Option<i32>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
        chrono::NaiveDateTime,
        chrono::NaiveDateTime,
    )>(
        r#"
        SELECT id, key_prefix, name, scopes, rate_limit_per_hour, last_used_at, expires_at, revoked_at, created_at, updated_at
        FROM api_keys
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(claims.sub)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound("API key not found".into()))?;

    let scopes: Vec<String> = serde_json::from_value(record.3.clone()).unwrap_or_default();

    Ok(Json(ApiKeyInfo {
        id: record.0,
        key_prefix: record.1,
        name: record.2,
        scopes,
        rate_limit_per_hour: record.4,
        last_used_at: record.5.map(|t| t.and_utc()),
        expires_at: record.6.map(|t| t.and_utc()),
        revoked_at: record.7.map(|t| t.and_utc()),
        created_at: record.8.and_utc(),
        updated_at: record.9.and_utc(),
    }))
}

/// Update an API key
#[utoipa::path(
    put,
    path = "/api/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    request_body = UpdateApiKeyRequest,
    responses(
        (status = 200, description = "API key updated successfully", body = ApiKeyInfo),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found")
    ),
    security(("jwt" = []))
)]
pub async fn update_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyInfo>> {
    // Verify ownership
    let exists =
        sqlx::query_as::<_, (Uuid,)>("SELECT id FROM api_keys WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(&claims.sub)
            .fetch_optional(&state.pool)
            .await?
            .is_some();

    if !exists {
        return Err(ApiError::NotFound("API key not found".into()));
    }

    // Validate scopes if provided
    if let Some(ref scopes) = req.scopes {
        validate_scopes(scopes)?;
    }

    // Build dynamic update query
    let mut query = "UPDATE api_keys SET ".to_string();
    let mut updates = Vec::new();
    let mut param_count = 1;

    if let Some(ref name) = req.name {
        if name.trim().is_empty() {
            return Err(ApiError::Validation("API key name cannot be empty".into()));
        }
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }

    if req.scopes.is_some() {
        updates.push(format!("scopes = ${}", param_count));
        param_count += 1;
    }

    if req.rate_limit_per_hour.is_some() {
        updates.push(format!("rate_limit_per_hour = ${}", param_count));
        param_count += 1;
    }

    if req.expires_at.is_some() {
        updates.push(format!("expires_at = ${}", param_count));
        param_count += 1;
    }

    if updates.is_empty() {
        return Err(ApiError::Validation("No fields to update".into()));
    }

    query.push_str(&updates.join(", "));
    query.push_str(&format!(" WHERE id = ${} RETURNING id", param_count));

    // Execute update (simplified - in production, use sqlx's query builder)
    if let Some(name) = req.name {
        sqlx::query("UPDATE api_keys SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    if let Some(scopes) = req.scopes {
        let scopes_json = serde_json::to_value(&scopes).map_err(|e| {
            ApiError::InternalWithMessage(format!("Failed to serialize scopes: {}", e))
        })?;
        sqlx::query("UPDATE api_keys SET scopes = $1 WHERE id = $2")
            .bind(scopes_json)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    if let Some(rate_limit) = req.rate_limit_per_hour {
        sqlx::query("UPDATE api_keys SET rate_limit_per_hour = $1 WHERE id = $2")
            .bind(rate_limit)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    if let Some(expires_at) = req.expires_at {
        sqlx::query("UPDATE api_keys SET expires_at = $1 WHERE id = $2")
            .bind(expires_at)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

    // Fetch and return updated key
    get_api_key(State(state), claims, Path(id)).await
}

/// Revoke an API key
#[utoipa::path(
    post,
    path = "/api/api-keys/{id}/revoke",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key revoked successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found")
    ),
    security(("jwt" = []))
)]
pub async fn revoke_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let result = sqlx::query(
        "UPDATE api_keys SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(claims.sub)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "API key not found or already revoked".into(),
        ));
    }

    Ok(StatusCode::OK)
}

/// Delete an API key
#[utoipa::path(
    delete,
    path = "/api/api-keys/{id}",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 204, description = "API key deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found")
    ),
    security(("jwt" = []))
)]
pub async fn delete_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(claims.sub)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound("API key not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Rotate an API key (creates a new key and revokes the old one)
#[utoipa::path(
    post,
    path = "/api/api-keys/{id}/rotate",
    params(
        ("id" = Uuid, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key rotated successfully", body = CreateApiKeyResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "API key not found")
    ),
    security(("jwt" = []))
)]
pub async fn rotate_api_key(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<CreateApiKeyResponse>> {
    // Get existing key details
    let existing = sqlx::query_as::<
        _,
        (
            String,
            serde_json::Value,
            Option<i32>,
            Option<chrono::NaiveDateTime>,
        ),
    >(
        r#"
        SELECT name, scopes, rate_limit_per_hour, expires_at
        FROM api_keys
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&claims.sub)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound(
        "API key not found or already revoked".into(),
    ))?;

    // Generate new API key
    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key).await?;
    let key_without_prefix = &api_key.clone()[API_KEY_PREFIX.len()..];
    let key_prefix = key_without_prefix[..8].to_string();

    let scopes: Vec<String> = serde_json::from_value(existing.1.clone()).unwrap_or_default();
    let scopes_json = serde_json::to_value(&scopes)
        .map_err(|e| ApiError::InternalWithMessage(format!("Failed to serialize scopes: {}", e)))?;

    // Start transaction
    let mut tx = state.pool.begin().await?;

    // Revoke old key
    sqlx::query("UPDATE api_keys SET revoked_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Create new key
    let record = sqlx::query_as::<_, (Uuid, chrono::NaiveDateTime)>(
        r#"
        INSERT INTO api_keys (user_id, key_hash, key_prefix, name, scopes, rate_limit_per_hour, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, created_at
        "#,
    )
    .bind(&claims.sub)
    .bind(key_hash)
    .bind(&key_prefix)
    .bind(&existing.0)
    .bind(scopes_json)
    .bind(existing.2)
    .bind(existing.3)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(CreateApiKeyResponse {
        id: record.0,
        api_key,
        key_prefix,
        name: existing.0,
        scopes,
        rate_limit_per_hour: existing.2,
        expires_at: existing.3.map(|t| t.and_utc()),
        created_at: record.1.and_utc(),
    }))
}
