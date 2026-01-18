//! HTTP response caching middleware using Redis

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::state::AppState;

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Default TTL for cached responses (in seconds)
    pub default_ttl: Duration,
    /// Enable caching
    pub enabled: bool,
    /// Cache key prefix
    pub key_prefix: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5 minutes
            enabled: true,
            key_prefix: "http_cache:".to_string(),
        }
    }
}

/// Cached response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// Generate cache key from request path and query
fn generate_cache_key(config: &CacheConfig, path: &str, query: Option<&str>) -> String {
    match query {
        Some(q) => format!("{}{}?{}", config.key_prefix, path, q),
        None => format!("{}{}", config.key_prefix, path),
    }
}

/// Response caching middleware
pub async fn cache_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // Only cache GET requests
    if req.method() != Method::GET {
        return next.run(req).await;
    }

    // Skip if Redis is not configured
    let cache = match state.redis_cache.as_ref() {
        Some(c) => c,
        None => return next.run(req).await,
    };

    // Generate cache key
    let config = CacheConfig::default();
    let path = req.uri().path();
    let query = req.uri().query();
    let cache_key = generate_cache_key(&config, path, query);

    // Try to get cached response
    if let Ok(Some(cached_response)) = cache.get::<CachedResponse>(&cache_key).await {
        // Build response from cache
        let mut response = Response::builder()
            .status(StatusCode::from_u16(cached_response.status).unwrap_or(StatusCode::OK));

        // Add headers
        for (key, value) in &cached_response.headers {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                response = response.header(key, header_value);
            }
        }

        // Add cache status header
        response = response.header("X-Cache-Status", "HIT");

        return response.body(Body::from(cached_response.body)).unwrap();
    }

    // Cache miss - execute request
    let response = next.run(req).await;

    // Only cache successful responses (2xx)
    if !response.status().is_success() {
        return response;
    }

    // Clone status and headers before consuming the body
    let status = response.status();
    let headers = response.headers().clone();

    // Extract body
    let (parts, body) = response.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            // If we can't read the body, just return the original response
            let response = Response::from_parts(parts, Body::default());
            return response;
        }
    };

    // Prepare cached response
    let cached_response = CachedResponse {
        status: status.as_u16(),
        headers: headers
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v_str| (k.to_string(), v_str.to_string()))
            })
            .collect(),
        body: body_bytes.to_vec(),
    };

    // Store in cache (fire and forget)
    let cache_clone = cache.clone();
    let key = cache_key.clone();
    let ttl = config.default_ttl.as_secs();
    tokio::spawn(async move {
        let _ = cache_clone.set(&key, &cached_response, ttl).await;
    });

    // Build response with cache miss header
    let mut response = Response::builder()
        .status(status)
        .header("X-Cache-Status", "MISS");

    for (key, value) in headers.iter() {
        response = response.header(key, value);
    }

    response.body(Body::from(body_bytes)).unwrap()
}

/// Cache invalidation helper
pub struct CacheInvalidator {
    state: AppState,
}

impl CacheInvalidator {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Invalidate cache for a specific path
    pub async fn invalidate_path(&self, path: &str) -> Result<(), String> {
        let cache = self
            .state
            .redis_cache
            .as_ref()
            .ok_or("Redis not configured")?;

        let config = CacheConfig::default();
        let cache_key = generate_cache_key(&config, path, None);

        cache
            .delete(&cache_key)
            .await
            .map_err(|e| format!("Failed to invalidate cache: {}", e))?;

        Ok(())
    }

    /// Invalidate cache for a path pattern (e.g., "/api/tokens*")
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<u64, String> {
        let cache = self
            .state
            .redis_cache
            .as_ref()
            .ok_or("Redis not configured")?;

        let config = CacheConfig::default();
        let search_pattern = format!("{}{}", config.key_prefix, pattern);

        cache
            .delete_pattern(&search_pattern)
            .await
            .map_err(|e| format!("Failed to invalidate cache pattern: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_generation() {
        let config = CacheConfig::default();

        let key1 = generate_cache_key(&config, "/api/tokens", None);
        assert_eq!(key1, "http_cache:/api/tokens");

        let key2 = generate_cache_key(&config, "/api/tokens", Some("page=1&limit=10"));
        assert_eq!(key2, "http_cache:/api/tokens?page=1&limit=10");
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_ttl, Duration::from_secs(300));
        assert_eq!(config.key_prefix, "http_cache:");
    }
}
