//! API key authentication middleware

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

/// API key header name
pub const API_KEY_HEADER: &str = "x-api-key";

/// API key prefix for identification (e.g., "kaccy_pk_")
pub const API_KEY_PREFIX: &str = "kaccy_pk_";

/// API key authentication result
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    /// Unique API key ID
    pub key_id: Uuid,
    /// User ID who owns this key
    pub user_id: Uuid,
    /// Granted scopes for this key
    pub scopes: Vec<String>,
    /// Optional rate limit (requests per hour)
    pub rate_limit_per_hour: Option<i32>,
}

/// API key scope for permission checking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyScope {
    /// Read access to tokens
    #[serde(rename = "tokens:read")]
    TokensRead,
    /// Write access to tokens
    #[serde(rename = "tokens:write")]
    TokensWrite,
    /// Read access to orders
    #[serde(rename = "orders:read")]
    OrdersRead,
    /// Write access to orders
    #[serde(rename = "orders:write")]
    OrdersWrite,
    /// Read access to trades
    #[serde(rename = "trades:read")]
    TradesRead,
    /// Read access to portfolio
    #[serde(rename = "portfolio:read")]
    PortfolioRead,
    /// Read access to commitments
    #[serde(rename = "commitments:read")]
    CommitmentsRead,
    /// Write access to commitments
    #[serde(rename = "commitments:write")]
    CommitmentsWrite,
    /// Full access (all scopes)
    #[serde(rename = "*")]
    All,
}

impl FromStr for ApiKeyScope {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tokens:read" => Ok(Self::TokensRead),
            "tokens:write" => Ok(Self::TokensWrite),
            "orders:read" => Ok(Self::OrdersRead),
            "orders:write" => Ok(Self::OrdersWrite),
            "trades:read" => Ok(Self::TradesRead),
            "portfolio:read" => Ok(Self::PortfolioRead),
            "commitments:read" => Ok(Self::CommitmentsRead),
            "commitments:write" => Ok(Self::CommitmentsWrite),
            "*" => Ok(Self::All),
            _ => Err(()),
        }
    }
}

impl ApiKeyScope {
    /// Convert scope to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TokensRead => "tokens:read",
            Self::TokensWrite => "tokens:write",
            Self::OrdersRead => "orders:read",
            Self::OrdersWrite => "orders:write",
            Self::TradesRead => "trades:read",
            Self::PortfolioRead => "portfolio:read",
            Self::CommitmentsRead => "commitments:read",
            Self::CommitmentsWrite => "commitments:write",
            Self::All => "*",
        }
    }
}

impl ApiKeyAuth {
    /// Check if the API key has the required scope
    pub fn has_scope(&self, required_scope: &ApiKeyScope) -> bool {
        // Check for wildcard permission
        if self.scopes.contains(&"*".to_string()) {
            return true;
        }

        // Check for exact scope match
        self.scopes.contains(&required_scope.as_str().to_string())
    }

    /// Verify API key from database
    pub async fn verify(pool: &PgPool, api_key: &str) -> Option<Self> {
        // Validate key format
        if !api_key.starts_with(API_KEY_PREFIX) {
            return None;
        }

        // Extract key prefix (first 8 chars after prefix)
        let key_without_prefix = &api_key[API_KEY_PREFIX.len()..];
        if key_without_prefix.len() < 8 {
            return None;
        }
        let key_prefix = &key_without_prefix[..8];

        // Query database for key with matching prefix
        let record = sqlx::query_as::<
            _,
            (
                uuid::Uuid,
                uuid::Uuid,
                String,
                serde_json::Value,
                Option<i32>,
                Option<chrono::NaiveDateTime>,
                Option<chrono::NaiveDateTime>,
            ),
        >(
            r#"
            SELECT id, user_id, key_hash, scopes, rate_limit_per_hour, expires_at, revoked_at
            FROM api_keys
            WHERE key_prefix = $1
                AND revoked_at IS NULL
                AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(key_prefix)
        .fetch_optional(pool)
        .await
        .ok()??;

        // Verify the full key using Argon2
        let key_hash = record.2.clone();
        let api_key_str = api_key.to_string();
        tokio::task::spawn_blocking(move || {
            use argon2::{Argon2, PasswordHash, PasswordVerifier};
            let parsed_hash = PasswordHash::new(&key_hash).ok()?;
            Argon2::default()
                .verify_password(api_key_str.as_bytes(), &parsed_hash)
                .ok()
        })
        .await
        .ok()??;

        // Update last_used_at
        let key_id = record.0;
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(key_id)
            .execute(pool)
            .await;

        // Parse scopes from JSONB
        let scopes: Vec<String> = serde_json::from_value(record.3.clone()).unwrap_or_default();

        Some(Self {
            key_id,
            user_id: record.1,
            scopes,
            rate_limit_per_hour: record.4,
        })
    }
}

/// Middleware to check for API key authentication
pub async fn api_key_middleware(
    State(pool): State<Arc<PgPool>>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    // Check for API key in header
    let api_key = req
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|v| v.to_str().ok());

    if let Some(api_key) = api_key {
        // Verify API key
        if let Some(auth) = ApiKeyAuth::verify(&pool, api_key).await {
            // Store API key auth in request extensions
            req.extensions_mut().insert(auth);
        } else {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(req).await)
}

/// Require API key authentication and specific scope
pub fn require_api_key_scope(
    required_scope: ApiKeyScope,
) -> impl Fn(
    Request,
    Next,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = std::result::Result<Response, StatusCode>> + Send>,
> + Clone {
    move |req: Request, next: Next| {
        let scope = required_scope.clone();
        Box::pin(async move {
            let auth = req
                .extensions()
                .get::<ApiKeyAuth>()
                .ok_or(StatusCode::UNAUTHORIZED)?;

            if !auth.has_scope(&scope) {
                return Err(StatusCode::FORBIDDEN);
            }

            Ok(next.run(req).await)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_parsing() {
        assert_eq!(
            ApiKeyScope::from_str("tokens:read").ok(),
            Some(ApiKeyScope::TokensRead)
        );
        assert_eq!(
            ApiKeyScope::from_str("orders:write").ok(),
            Some(ApiKeyScope::OrdersWrite)
        );
        assert_eq!(ApiKeyScope::from_str("*").ok(), Some(ApiKeyScope::All));
        assert_eq!(ApiKeyScope::from_str("invalid").ok(), None);
    }

    #[test]
    fn test_scope_has_permission() {
        let auth = ApiKeyAuth {
            key_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec!["tokens:read".to_string(), "orders:read".to_string()],
            rate_limit_per_hour: None,
        };

        assert!(auth.has_scope(&ApiKeyScope::TokensRead));
        assert!(auth.has_scope(&ApiKeyScope::OrdersRead));
        assert!(!auth.has_scope(&ApiKeyScope::TokensWrite));
    }

    #[test]
    fn test_wildcard_scope() {
        let auth = ApiKeyAuth {
            key_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            scopes: vec!["*".to_string()],
            rate_limit_per_hour: None,
        };

        assert!(auth.has_scope(&ApiKeyScope::TokensRead));
        assert!(auth.has_scope(&ApiKeyScope::TokensWrite));
        assert!(auth.has_scope(&ApiKeyScope::OrdersRead));
        assert!(auth.has_scope(&ApiKeyScope::All));
    }

    #[test]
    fn test_api_key_prefix() {
        assert!(format!("{}abc123", API_KEY_PREFIX).starts_with(API_KEY_PREFIX));
    }
}
