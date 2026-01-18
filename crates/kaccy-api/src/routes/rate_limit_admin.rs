//! Admin routes for rate limit management

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::middleware::DynamicRateLimiter;
use crate::state::AppState;

/// Rate limit status response
#[derive(Debug, Serialize)]
pub struct RateLimitStatus {
    /// Current multiplier
    pub multiplier: f64,
    /// Base max requests per window
    pub base_max_requests: u32,
    /// Current effective max requests
    pub current_max_requests: u32,
    /// Total requests processed
    pub total_requests: u64,
    /// Requests that were rate limited
    pub rate_limited: u64,
    /// Rate limit percentage
    pub rate_limit_percentage: f64,
}

/// Request to adjust rate limit multiplier
#[derive(Debug, Serialize, Deserialize)]
pub struct AdjustMultiplierRequest {
    /// New multiplier value (0.1 to 10.0)
    pub multiplier: f64,
}

/// Get current rate limit status
pub async fn get_rate_limit_status(
    State(limiter): State<DynamicRateLimiter>,
) -> Result<Json<RateLimitStatus>> {
    let multiplier = limiter.get_multiplier();
    let analytics = limiter.analytics();

    // Calculate current effective max requests
    // Note: This is a simplified calculation assuming default config
    let base_max_requests = 100; // Should match your actual base config
    let current_max_requests = (base_max_requests as f64 * multiplier) as u32;

    Ok(Json(RateLimitStatus {
        multiplier,
        base_max_requests,
        current_max_requests,
        total_requests: analytics.total_requests(),
        rate_limited: analytics.rate_limited(),
        rate_limit_percentage: analytics.rate_limit_percentage(),
    }))
}

/// Manually adjust rate limit multiplier
pub async fn adjust_rate_limit_multiplier(
    State(limiter): State<DynamicRateLimiter>,
    Json(req): Json<AdjustMultiplierRequest>,
) -> Result<Json<RateLimitStatus>> {
    limiter.set_multiplier(req.multiplier);

    // Return updated status
    get_rate_limit_status(State(limiter)).await
}

/// Reset rate limit to default configuration
pub async fn reset_rate_limit(
    State(limiter): State<DynamicRateLimiter>,
) -> Result<Json<RateLimitStatus>> {
    limiter.reset();

    // Return updated status
    get_rate_limit_status(State(limiter)).await
}

// ============== AppState-compatible handlers ==============

/// Get current rate limit status (AppState version)
pub async fn get_rate_limit_status_app(
    State(state): State<AppState>,
) -> Result<Json<RateLimitStatus>> {
    get_rate_limit_status(State(state.rate_limiter)).await
}

/// Manually adjust rate limit multiplier (AppState version)
pub async fn adjust_rate_limit_multiplier_app(
    State(state): State<AppState>,
    Json(req): Json<AdjustMultiplierRequest>,
) -> Result<Json<RateLimitStatus>> {
    adjust_rate_limit_multiplier(State(state.rate_limiter), Json(req)).await
}

/// Reset rate limit to default configuration (AppState version)
pub async fn reset_rate_limit_app(State(state): State<AppState>) -> Result<Json<RateLimitStatus>> {
    reset_rate_limit(State(state.rate_limiter)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_multiplier_request_serialization() {
        let req = AdjustMultiplierRequest { multiplier: 1.5 };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("1.5"));
    }

    #[test]
    fn test_rate_limit_status_serialization() {
        let status = RateLimitStatus {
            multiplier: 1.0,
            base_max_requests: 100,
            current_max_requests: 100,
            total_requests: 1000,
            rate_limited: 50,
            rate_limit_percentage: 5.0,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"multiplier\":1"));
        assert!(json.contains("\"total_requests\":1000"));
    }
}
