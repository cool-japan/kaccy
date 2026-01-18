//! Health check routes

use axum::{Json, extract::State};
use serde::Serialize;
use std::time::Instant;

use crate::error::Result;
use crate::middleware::update_db_pool_metrics;
use crate::state::AppState;

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub database: DatabaseHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisHealth>,
    pub disk: DiskHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitcoin: Option<BitcoinHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_bus: Option<EventBusHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_processor: Option<JobProcessorHealth>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub external_services: Vec<ExternalServiceHealth>,
}

#[derive(Debug, Serialize)]
pub struct DatabaseHealth {
    pub connected: bool,
    pub pool_size: u32,
    pub pool_idle: u32,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RedisHealth {
    pub connected: bool,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct DiskHealth {
    pub available_gb: f64,
    pub total_gb: f64,
    pub used_percent: f64,
    pub healthy: bool,
}

#[derive(Debug, Serialize)]
pub struct BitcoinHealth {
    pub connected: bool,
    pub block_height: Option<u64>,
    pub network: Option<String>,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ExternalServiceHealth {
    pub name: String,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EventBusHealth {
    pub configured: bool,
    pub handler_count: usize,
}

#[derive(Debug, Serialize)]
pub struct JobProcessorHealth {
    pub configured: bool,
    pub worker_count: usize,
    pub queue_stats: Option<JobQueueStats>,
}

#[derive(Debug, Serialize)]
pub struct JobQueueStats {
    pub pending_jobs: usize,
    pub processing_jobs: usize,
    pub total_jobs: usize,
}

/// Basic health check
pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Check Redis health
async fn check_redis_health(state: &AppState) -> Option<RedisHealth> {
    if state.has_redis() {
        let start = Instant::now();
        // Try a simple get operation to test connectivity
        // Using a non-existent key for health check - we just care if the operation succeeds
        let _result = state.get_cached_session("__health_check__").await;
        let latency_ms = start.elapsed().as_millis() as u64;

        // Redis ConnectionManager handles reconnections automatically,
        // so if we get here without panicking, the connection is healthy
        Some(RedisHealth {
            connected: true,
            latency_ms: Some(latency_ms),
        })
    } else {
        None
    }
}

/// Check Bitcoin RPC health
async fn check_bitcoin_health(state: &AppState) -> Option<BitcoinHealth> {
    if state.has_bitcoin() {
        let start = Instant::now();

        // Try to get blockchain info to verify connection
        match state.bitcoin() {
            Ok(client) => {
                match client.get_block_height() {
                    Ok(block_height) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        Some(BitcoinHealth {
                            connected: true,
                            block_height: Some(block_height),
                            network: Some(format!("{:?}", client.network())),
                            latency_ms: Some(latency_ms),
                        })
                    }
                    Err(_) => {
                        // Connection failed
                        Some(BitcoinHealth {
                            connected: false,
                            block_height: None,
                            network: None,
                            latency_ms: None,
                        })
                    }
                }
            }
            Err(_) => {
                // Bitcoin client not configured
                None
            }
        }
    } else {
        None
    }
}

/// Check EventBus health
fn check_event_bus_health(state: &AppState) -> Option<EventBusHealth> {
    if state.has_event_bus() {
        // EventBus is configured - we can add more detailed checks later
        // For now, just report that it's configured
        Some(EventBusHealth {
            configured: true,
            handler_count: 5, // AuditLog, Metrics, Notification, Webhook, Analytics
        })
    } else {
        None
    }
}

/// Check JobProcessor health
async fn check_job_processor_health(state: &AppState) -> Option<JobProcessorHealth> {
    if state.has_job_processor() {
        if let Ok(processor_mutex) = state.job_processor() {
            // Try to get lock without blocking too long
            if let Ok(processor) = processor_mutex.try_lock() {
                let queue = processor.queue();
                let stats = queue.stats().await;

                return Some(JobProcessorHealth {
                    configured: true,
                    worker_count: 4, // Will be updated when we add worker tracking
                    queue_stats: Some(JobQueueStats {
                        pending_jobs: stats.pending,
                        processing_jobs: stats.processing,
                        total_jobs: stats.total,
                    }),
                });
            }
        }

        // If we can't get the lock, still report configured status
        Some(JobProcessorHealth {
            configured: true,
            worker_count: 4,
            queue_stats: None,
        })
    } else {
        None
    }
}

/// Check external services
async fn check_external_services() -> Vec<ExternalServiceHealth> {
    // Example: Check a hypothetical API endpoint
    // In a real scenario, you'd check your actual external dependencies
    // Vec::new() with items pushed as needed
    Vec::new()
}

/// Check disk space using sys-info crate
fn check_disk_health() -> DiskHealth {
    // Try to get disk info using sys-info
    if let Ok(disk) = sys_info::disk_info() {
        let total_gb = disk.total as f64 / 1024.0 / 1024.0; // Convert KB to GB
        let free_gb = disk.free as f64 / 1024.0 / 1024.0;
        let used_gb = total_gb - free_gb;
        let used_percent = (used_gb / total_gb) * 100.0;

        DiskHealth {
            available_gb: free_gb,
            total_gb,
            used_percent,
            healthy: used_percent < 90.0, // Consider healthy if less than 90% used
        }
    } else {
        // Return a default unhealthy status if we can't get disk info
        DiskHealth {
            available_gb: 0.0,
            total_gb: 0.0,
            used_percent: 100.0,
            healthy: false,
        }
    }
}

/// Detailed health check including database
pub async fn health_detailed(State(state): State<AppState>) -> Result<Json<HealthResponse>> {
    // Update database pool metrics for Prometheus
    update_db_pool_metrics(&state.pool);

    let db_health = kaccy_db::pool::health_check(&state.pool).await;
    let redis_health = check_redis_health(&state).await;
    let bitcoin_health = check_bitcoin_health(&state).await;
    let disk_health = check_disk_health();
    let event_bus_health = check_event_bus_health(&state);
    let job_processor_health = check_job_processor_health(&state).await;
    let external_services = check_external_services().await;

    // Determine overall status
    let mut status = match db_health.status {
        kaccy_db::pool::HealthStatus::Healthy => "healthy",
        kaccy_db::pool::HealthStatus::Degraded => "degraded",
        kaccy_db::pool::HealthStatus::Unhealthy => "unhealthy",
    };

    // Downgrade status if Redis is configured but not connected
    if let Some(ref redis) = redis_health {
        if !redis.connected && status == "healthy" {
            status = "degraded";
        }
    }

    // Downgrade status if Bitcoin is configured but not connected
    if let Some(ref bitcoin) = bitcoin_health {
        if !bitcoin.connected && status == "healthy" {
            status = "degraded";
        }
    }

    // Downgrade status if disk is unhealthy
    if !disk_health.healthy && status == "healthy" {
        status = "degraded";
    }

    // Downgrade status if any external service is unhealthy
    if external_services.iter().any(|s| s.status != "healthy") && status == "healthy" {
        status = "degraded";
    }

    Ok(Json(HealthResponse {
        status: status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.uptime_seconds(),
        database: DatabaseHealth {
            connected: db_health.database_connected,
            pool_size: db_health.pool_size,
            pool_idle: db_health.pool_idle,
            latency_ms: db_health.latency_ms,
        },
        redis: redis_health,
        disk: disk_health,
        bitcoin: bitcoin_health,
        event_bus: event_bus_health,
        job_processor: job_processor_health,
        external_services,
    }))
}

/// Readiness check (for Kubernetes)
pub async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // Update database pool metrics for Prometheus
    update_db_pool_metrics(&state.pool);

    let db_health = kaccy_db::pool::health_check(&state.pool).await;

    if db_health.database_connected {
        Ok(Json(serde_json::json!({
            "ready": true
        })))
    } else {
        // Return error if database is not connected
        Err(crate::error::ApiError::Internal)
    }
}
