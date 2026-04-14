//! Database connection pool statistics endpoint

use axum::{Json, extract::State};
use serde::Serialize;

use crate::error::Result;
use crate::state::AppState;

/// Connection pool statistics
#[derive(Debug, Serialize)]
pub struct PoolStats {
    /// Current pool size
    pub size: usize,
    /// Number of idle connections
    pub idle: usize,
    /// Number of active connections
    pub active: usize,
    /// Maximum pool size allowed
    pub max_size: u32,
    /// Minimum pool size
    pub min_size: u32,
    /// Pool utilization percentage
    pub utilization_percent: f64,
    /// Average connection wait time (if available)
    pub avg_wait_time_ms: Option<f64>,
    /// Total connections created
    pub total_connections_created: Option<u64>,
}

/// Get detailed pool statistics
pub async fn get_pool_stats(State(state): State<AppState>) -> Result<Json<PoolStats>> {
    let pool = &state.pool;

    let size = pool.size() as usize;
    let idle = pool.num_idle();
    let max_size = pool.options().get_max_connections();
    let min_size = pool.options().get_min_connections();

    let active = size.saturating_sub(idle);
    let utilization_percent = if max_size > 0 {
        (size as f64 / max_size as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(PoolStats {
        size,
        idle,
        active,
        max_size,
        min_size,
        utilization_percent,
        avg_wait_time_ms: None,          // SQLx doesn't expose this metric
        total_connections_created: None, // SQLx doesn't expose this metric
    }))
}

/// Pool health recommendations
#[derive(Debug, Serialize)]
pub struct PoolHealthRecommendation {
    /// Overall pool health status (e.g. "healthy", "degraded")
    pub status: String,
    /// Tuning recommendations for the connection pool
    pub recommendations: Vec<String>,
    /// Active warnings about pool health
    pub warnings: Vec<String>,
}

/// Analyze pool health and provide recommendations
pub async fn get_pool_recommendations(
    State(state): State<AppState>,
) -> Result<Json<PoolHealthRecommendation>> {
    let pool = &state.pool;

    let size = pool.size() as usize;
    let idle = pool.num_idle();
    let max_size = pool.options().get_max_connections();
    let min_size = pool.options().get_min_connections();

    let active = size.saturating_sub(idle);
    let utilization = if max_size > 0 {
        (size as f64 / max_size as f64) * 100.0
    } else {
        0.0
    };

    let mut recommendations = Vec::new();
    let mut warnings = Vec::new();
    let mut status = "healthy";

    // Check if pool is at maximum capacity
    if size >= max_size as usize && max_size > 0 {
        status = "warning";
        warnings.push("Pool has reached maximum capacity".to_string());
        recommendations.push(format!(
            "Consider increasing max_connections (current: {})",
            max_size
        ));
    }

    // Check if utilization is very high
    if utilization > 80.0 {
        status = "warning";
        warnings.push(format!("Pool utilization is high: {:.1}%", utilization));
        recommendations.push("Monitor for connection exhaustion".to_string());
    }

    // Check if there are too many idle connections
    if idle > (max_size as usize) / 2 && active < (max_size as usize) / 4 {
        recommendations.push(format!(
            "Consider reducing max_connections or increasing min_idle (idle: {}, active: {})",
            idle, active
        ));
    }

    // Check if min_connections is set appropriately
    if min_size == 0 {
        recommendations
            .push("Consider setting min_connections > 0 to maintain ready connections".to_string());
    }

    // If no issues found
    if warnings.is_empty() && recommendations.is_empty() {
        recommendations.push("Pool configuration looks good".to_string());
    }

    Ok(Json(PoolHealthRecommendation {
        status: status.to_string(),
        recommendations,
        warnings,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_stats_serialization() {
        let stats = PoolStats {
            size: 10,
            idle: 5,
            active: 5,
            max_size: 20,
            min_size: 2,
            utilization_percent: 50.0,
            avg_wait_time_ms: Some(10.5),
            total_connections_created: Some(100),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"size\":10"));
        assert!(json.contains("\"utilization_percent\":50"));
    }

    #[test]
    fn test_pool_recommendations_serialization() {
        let rec = PoolHealthRecommendation {
            status: "healthy".to_string(),
            recommendations: vec!["All good".to_string()],
            warnings: vec![],
        };

        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("All good"));
    }
}
