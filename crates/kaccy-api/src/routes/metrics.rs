//! JSON metrics endpoints for monitoring dashboards
//!
//! Provides structured metrics data for consumption by dashboards and monitoring tools

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use crate::{error::ApiError, state::AppState};

/// System metrics response
#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub database: DatabaseMetrics,
    pub application: ApplicationMetrics,
    pub resources: ResourceMetrics,
}

/// Database metrics
#[derive(Debug, Serialize)]
pub struct DatabaseMetrics {
    pub pool_size: u32,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub max_connections: u32,
    pub min_connections: u32,
}

/// Application metrics
#[derive(Debug, Serialize)]
pub struct ApplicationMetrics {
    pub total_users: i64,
    pub total_tokens: i64,
    pub total_orders: i64,
    pub total_trades: i64,
    pub pending_orders: i64,
    pub active_commitments: i64,
}

/// Resource metrics
#[derive(Debug, Serialize)]
pub struct ResourceMetrics {
    pub event_bus_configured: bool,
    pub job_processor_configured: bool,
    pub redis_configured: bool,
    pub bitcoin_configured: bool,
}

/// Service health status
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Service health details
#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub service: String,
    pub status: ServiceStatus,
    pub message: Option<String>,
    pub latency_ms: Option<u64>,
}

/// Comprehensive health check response
#[derive(Debug, Serialize)]
pub struct ComprehensiveHealthResponse {
    pub overall_status: ServiceStatus,
    pub timestamp: u64,
    pub services: Vec<ServiceHealth>,
}

/// Get system metrics in JSON format
///
/// Provides metrics for dashboards and monitoring tools (admin only)
#[allow(dead_code)]
pub async fn get_system_metrics(
    State(state): State<AppState>,
) -> Result<Json<SystemMetrics>, ApiError> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Calculate uptime from application start time
    let uptime_seconds = state.uptime_seconds();

    // Database metrics
    let pool = &state.pool;
    let database = DatabaseMetrics {
        pool_size: pool.size(),
        active_connections: pool.num_idle(),
        idle_connections: pool.num_idle(),
        max_connections: pool.options().get_max_connections(),
        min_connections: pool.options().get_min_connections(),
    };

    // Application metrics
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let total_tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tokens")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let total_orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let total_trades: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trades")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let pending_orders: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE status = 'pending'")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let active_commitments: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM commitments WHERE status IN ('pending', 'submitted')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let application = ApplicationMetrics {
        total_users,
        total_tokens,
        total_orders,
        total_trades,
        pending_orders,
        active_commitments,
    };

    // Resource metrics
    let resources = ResourceMetrics {
        event_bus_configured: state.has_event_bus(),
        job_processor_configured: state.has_job_processor(),
        redis_configured: state.has_redis(),
        bitcoin_configured: state.has_bitcoin(),
    };

    Ok(Json(SystemMetrics {
        timestamp,
        uptime_seconds,
        database,
        application,
        resources,
    }))
}

/// Get comprehensive health check with service-level details
///
/// Provides detailed health status for each service component
#[allow(dead_code)]
pub async fn get_comprehensive_health(
    State(state): State<AppState>,
) -> Result<Json<ComprehensiveHealthResponse>, ApiError> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut services = Vec::new();
    let mut overall_healthy = true;

    // Check database
    let db_start = std::time::Instant::now();
    let db_result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await;
    let db_latency = db_start.elapsed().as_millis() as u64;

    let db_status = if db_result.is_ok() && db_latency < 100 {
        ServiceStatus::Healthy
    } else if db_result.is_ok() {
        overall_healthy = false;
        ServiceStatus::Degraded
    } else {
        overall_healthy = false;
        ServiceStatus::Unhealthy
    };

    services.push(ServiceHealth {
        service: "database".to_string(),
        status: db_status,
        message: db_result.err().map(|e| e.to_string()),
        latency_ms: Some(db_latency),
    });

    // Check Redis (if configured)
    if state.has_redis() {
        services.push(ServiceHealth {
            service: "redis".to_string(),
            status: ServiceStatus::Healthy,
            message: Some("configured".to_string()),
            latency_ms: None,
        });
    }

    // Check Bitcoin RPC (if configured)
    if state.has_bitcoin() {
        services.push(ServiceHealth {
            service: "bitcoin_rpc".to_string(),
            status: ServiceStatus::Healthy,
            message: Some("configured".to_string()),
            latency_ms: None,
        });
    }

    // Check Event Bus
    if state.has_event_bus() {
        services.push(ServiceHealth {
            service: "event_bus".to_string(),
            status: ServiceStatus::Healthy,
            message: Some("configured".to_string()),
            latency_ms: None,
        });
    }

    // Check Job Processor
    if state.has_job_processor() {
        services.push(ServiceHealth {
            service: "job_processor".to_string(),
            status: ServiceStatus::Healthy,
            message: Some("configured".to_string()),
            latency_ms: None,
        });
    }

    let overall_status = if overall_healthy {
        ServiceStatus::Healthy
    } else {
        ServiceStatus::Degraded
    };

    Ok(Json(ComprehensiveHealthResponse {
        overall_status,
        timestamp,
        services,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_status_serialization() {
        let status = ServiceStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"healthy\"");
    }

    #[test]
    fn test_service_status_deserialization() {
        let json = "\"degraded\"";
        let status: ServiceStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, ServiceStatus::Degraded);
    }
}
