//! Comprehensive health check system for database monitoring
//!
//! This module provides a unified health check system that aggregates information
//! from various diagnostic sources including connection pools, table statistics,
//! lock monitoring, query performance, and cache health.

use crate::connection_diagnostics::get_pool_diagnostics;
use crate::error::Result;
use crate::lock_monitoring::{detect_deadlocks, get_blocking_queries, get_lock_wait_stats};
use crate::query_plan::get_expensive_queries;
use crate::table_stats::{
    get_database_size, get_high_seq_scan_tables, get_table_cache_hit_ratio, get_unused_indexes,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Overall health status of the database system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// System is healthy and operating normally
    Healthy,
    /// System has minor issues but is operational
    Degraded,
    /// System has serious issues affecting performance
    Unhealthy,
    /// System is in critical state and may fail soon
    Critical,
}

/// Health check category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCategory {
    /// Connection pool health
    ConnectionPool,
    /// Query performance health
    QueryPerformance,
    /// Database locks and deadlocks
    Locks,
    /// Table and index health
    Tables,
    /// Cache performance
    Cache,
    /// Database size and growth
    Storage,
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Category of this health check
    pub category: HealthCategory,
    /// Overall status for this category
    pub status: HealthStatus,
    /// Human-readable message
    pub message: String,
    /// Detailed findings (optional)
    pub details: Option<serde_json::Value>,
    /// Timestamp of the check
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// Comprehensive system health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    /// Overall system health status
    pub overall_status: HealthStatus,
    /// Individual health check results
    pub checks: Vec<HealthCheckResult>,
    /// Total number of issues found
    pub total_issues: usize,
    /// Number of critical issues
    pub critical_issues: usize,
    /// Number of warnings
    pub warnings: usize,
    /// Report generation timestamp
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Time taken to generate report
    pub generation_time_ms: u64,
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Pool utilization threshold for warnings (0.0-1.0)
    pub pool_utilization_warning: f64,
    /// Pool utilization threshold for critical (0.0-1.0)
    pub pool_utilization_critical: f64,
    /// Cache hit ratio threshold for warnings (0.0-1.0)
    pub cache_hit_ratio_warning: f64,
    /// Cache hit ratio threshold for critical (0.0-1.0)
    pub cache_hit_ratio_critical: f64,
    /// Slow query threshold in milliseconds
    pub slow_query_threshold_ms: u64,
    /// Maximum acceptable number of blocking queries
    pub max_blocking_queries: usize,
    /// Maximum acceptable lock wait time in seconds
    pub max_lock_wait_seconds: i64,
    /// Sequential scan ratio threshold (scans/total_queries)
    pub seq_scan_ratio_warning: f64,
    /// Maximum database size in GB before warning
    pub max_database_size_gb: f64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            pool_utilization_warning: 0.8,
            pool_utilization_critical: 0.95,
            cache_hit_ratio_warning: 0.95,
            cache_hit_ratio_critical: 0.85,
            slow_query_threshold_ms: 1000,
            max_blocking_queries: 5,
            max_lock_wait_seconds: 10,
            seq_scan_ratio_warning: 0.5,
            max_database_size_gb: 100.0,
        }
    }
}

/// Performs a comprehensive health check of the database system
pub async fn perform_health_check(
    pool: &PgPool,
    config: &HealthCheckConfig,
) -> Result<SystemHealthReport> {
    let start = std::time::Instant::now();
    let mut checks = Vec::new();

    // Check connection pool health
    checks.push(check_pool_health(pool, config).await?);

    // Check query performance
    checks.push(check_query_performance(pool, config).await?);

    // Check locks and deadlocks
    checks.push(check_locks_health(pool, config).await?);

    // Check table and index health
    checks.push(check_tables_health(pool, config).await?);

    // Check cache performance
    checks.push(check_cache_health(pool, config).await?);

    // Check storage health
    checks.push(check_storage_health(pool, config).await?);

    // Calculate overall status
    let overall_status = calculate_overall_status(&checks);
    let (total_issues, critical_issues, warnings) = count_issues(&checks);

    let generation_time_ms = start.elapsed().as_millis() as u64;

    Ok(SystemHealthReport {
        overall_status,
        checks,
        total_issues,
        critical_issues,
        warnings,
        generated_at: chrono::Utc::now(),
        generation_time_ms,
    })
}

/// Check connection pool health
async fn check_pool_health(pool: &PgPool, config: &HealthCheckConfig) -> Result<HealthCheckResult> {
    let diagnostics = get_pool_diagnostics(pool).await?;

    let utilization = diagnostics.stats.utilization_percent / 100.0;

    let status = if utilization >= config.pool_utilization_critical {
        HealthStatus::Critical
    } else if utilization >= config.pool_utilization_warning || !diagnostics.issues.is_empty() {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = format!(
        "Pool utilization: {:.1}%, Active: {}, Idle: {}",
        diagnostics.stats.utilization_percent,
        diagnostics.stats.connections_active,
        diagnostics.stats.connections_idle
    );

    Ok(HealthCheckResult {
        category: HealthCategory::ConnectionPool,
        status,
        message,
        details: Some(serde_json::to_value(&diagnostics).unwrap()),
        checked_at: chrono::Utc::now(),
    })
}

/// Check query performance health
async fn check_query_performance(
    pool: &PgPool,
    config: &HealthCheckConfig,
) -> Result<HealthCheckResult> {
    let expensive_queries = get_expensive_queries(pool, 5).await?;

    let slow_queries: Vec<_> = expensive_queries
        .iter()
        .filter(|q| q.mean_time > config.slow_query_threshold_ms as f64)
        .collect();

    let status = if slow_queries.len() >= 10 {
        HealthStatus::Critical
    } else if slow_queries.len() >= 5 {
        HealthStatus::Unhealthy
    } else if !slow_queries.is_empty() {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = if slow_queries.is_empty() {
        "No slow queries detected".to_string()
    } else {
        format!("{} slow queries detected", slow_queries.len())
    };

    Ok(HealthCheckResult {
        category: HealthCategory::QueryPerformance,
        status,
        message,
        details: Some(serde_json::json!({
            "slow_queries_count": slow_queries.len(),
            "total_queries_analyzed": expensive_queries.len(),
            "threshold_ms": config.slow_query_threshold_ms,
        })),
        checked_at: chrono::Utc::now(),
    })
}

/// Check locks and deadlocks health
async fn check_locks_health(
    pool: &PgPool,
    config: &HealthCheckConfig,
) -> Result<HealthCheckResult> {
    let blocking_queries = get_blocking_queries(pool).await?;
    let deadlocks = detect_deadlocks(pool).await?;
    let lock_stats = get_lock_wait_stats(pool).await?;

    let max_wait_time_seconds = lock_stats.max_wait_time_ms / 1000;

    let status = if !deadlocks.is_empty() {
        HealthStatus::Critical
    } else if blocking_queries.len() > config.max_blocking_queries
        || max_wait_time_seconds > config.max_lock_wait_seconds
    {
        HealthStatus::Unhealthy
    } else if !blocking_queries.is_empty() {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = format!(
        "Blocking queries: {}, Deadlocks: {}, Max wait: {}s",
        blocking_queries.len(),
        deadlocks.len(),
        max_wait_time_seconds
    );

    Ok(HealthCheckResult {
        category: HealthCategory::Locks,
        status,
        message,
        details: Some(serde_json::json!({
            "blocking_queries": blocking_queries.len(),
            "deadlocks": deadlocks.len(),
            "max_wait_seconds": max_wait_time_seconds,
        })),
        checked_at: chrono::Utc::now(),
    })
}

/// Check table and index health
async fn check_tables_health(
    pool: &PgPool,
    _config: &HealthCheckConfig,
) -> Result<HealthCheckResult> {
    let high_seq_scan_tables = get_high_seq_scan_tables(pool, 5).await?;
    let unused_indexes = get_unused_indexes(pool).await?;

    let status = if high_seq_scan_tables.len() >= 10 {
        HealthStatus::Unhealthy
    } else if high_seq_scan_tables.len() >= 5 || unused_indexes.len() >= 10 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = format!(
        "High seq scan tables: {}, Unused indexes: {}",
        high_seq_scan_tables.len(),
        unused_indexes.len()
    );

    Ok(HealthCheckResult {
        category: HealthCategory::Tables,
        status,
        message,
        details: Some(serde_json::json!({
            "high_seq_scan_tables": high_seq_scan_tables.len(),
            "unused_indexes": unused_indexes.len(),
        })),
        checked_at: chrono::Utc::now(),
    })
}

/// Check cache performance health
async fn check_cache_health(
    pool: &PgPool,
    config: &HealthCheckConfig,
) -> Result<HealthCheckResult> {
    let cache_hit_ratio = get_table_cache_hit_ratio(pool).await?;

    let avg_hit_ratio = cache_hit_ratio
        .iter()
        .map(|c| c.hit_ratio_percent / 100.0)
        .sum::<f64>()
        / cache_hit_ratio.len().max(1) as f64;

    let status = if avg_hit_ratio < config.cache_hit_ratio_critical {
        HealthStatus::Critical
    } else if avg_hit_ratio < config.cache_hit_ratio_warning {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = format!("Average cache hit ratio: {:.2}%", avg_hit_ratio * 100.0);

    Ok(HealthCheckResult {
        category: HealthCategory::Cache,
        status,
        message,
        details: Some(serde_json::json!({
            "average_hit_ratio": avg_hit_ratio,
            "tables_analyzed": cache_hit_ratio.len(),
        })),
        checked_at: chrono::Utc::now(),
    })
}

/// Check storage health
async fn check_storage_health(
    pool: &PgPool,
    config: &HealthCheckConfig,
) -> Result<HealthCheckResult> {
    let db_size = get_database_size(pool).await?;
    let size_gb = db_size.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    let status = if size_gb > config.max_database_size_gb * 0.95 {
        HealthStatus::Critical
    } else if size_gb > config.max_database_size_gb * 0.8 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };

    let message = format!(
        "Database size: {:.2} GB / {:.0} GB limit",
        size_gb, config.max_database_size_gb
    );

    Ok(HealthCheckResult {
        category: HealthCategory::Storage,
        status,
        message,
        details: Some(serde_json::json!({
            "size_gb": size_gb,
            "limit_gb": config.max_database_size_gb,
            "utilization": size_gb / config.max_database_size_gb,
        })),
        checked_at: chrono::Utc::now(),
    })
}

/// Calculate overall system status from individual checks
fn calculate_overall_status(checks: &[HealthCheckResult]) -> HealthStatus {
    let mut has_critical = false;
    let mut has_unhealthy = false;
    let mut has_degraded = false;

    for check in checks {
        match check.status {
            HealthStatus::Critical => has_critical = true,
            HealthStatus::Unhealthy => has_unhealthy = true,
            HealthStatus::Degraded => has_degraded = true,
            HealthStatus::Healthy => {}
        }
    }

    if has_critical {
        HealthStatus::Critical
    } else if has_unhealthy {
        HealthStatus::Unhealthy
    } else if has_degraded {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    }
}

/// Count issues by severity
fn count_issues(checks: &[HealthCheckResult]) -> (usize, usize, usize) {
    let mut total_issues = 0;
    let mut critical_issues = 0;
    let mut warnings = 0;

    for check in checks {
        match check.status {
            HealthStatus::Critical => {
                total_issues += 1;
                critical_issues += 1;
            }
            HealthStatus::Unhealthy => {
                total_issues += 1;
            }
            HealthStatus::Degraded => {
                total_issues += 1;
                warnings += 1;
            }
            HealthStatus::Healthy => {}
        }
    }

    (total_issues, critical_issues, warnings)
}

/// Quick health check that only checks critical components (faster)
pub async fn quick_health_check(pool: &PgPool) -> Result<HealthStatus> {
    let config = HealthCheckConfig::default();

    // Quick pool check
    let pool_health = check_pool_health(pool, &config).await?;
    if matches!(
        pool_health.status,
        HealthStatus::Critical | HealthStatus::Unhealthy
    ) {
        return Ok(pool_health.status);
    }

    // Quick deadlock check
    let deadlocks = detect_deadlocks(pool).await?;
    if !deadlocks.is_empty() {
        return Ok(HealthStatus::Critical);
    }

    // If pool is degraded, return degraded
    if pool_health.status == HealthStatus::Degraded {
        return Ok(HealthStatus::Degraded);
    }

    Ok(HealthStatus::Healthy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_config_default() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.pool_utilization_warning, 0.8);
        assert_eq!(config.pool_utilization_critical, 0.95);
        assert_eq!(config.cache_hit_ratio_warning, 0.95);
        assert_eq!(config.slow_query_threshold_ms, 1000);
    }

    #[test]
    fn test_calculate_overall_status_all_healthy() {
        let checks = vec![
            HealthCheckResult {
                category: HealthCategory::ConnectionPool,
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::QueryPerformance,
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
        ];

        assert_eq!(calculate_overall_status(&checks), HealthStatus::Healthy);
    }

    #[test]
    fn test_calculate_overall_status_with_degraded() {
        let checks = vec![
            HealthCheckResult {
                category: HealthCategory::ConnectionPool,
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::QueryPerformance,
                status: HealthStatus::Degraded,
                message: "Slow queries".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
        ];

        assert_eq!(calculate_overall_status(&checks), HealthStatus::Degraded);
    }

    #[test]
    fn test_calculate_overall_status_with_unhealthy() {
        let checks = vec![
            HealthCheckResult {
                category: HealthCategory::ConnectionPool,
                status: HealthStatus::Degraded,
                message: "High load".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::Locks,
                status: HealthStatus::Unhealthy,
                message: "Many blocking queries".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
        ];

        assert_eq!(calculate_overall_status(&checks), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_calculate_overall_status_with_critical() {
        let checks = vec![
            HealthCheckResult {
                category: HealthCategory::ConnectionPool,
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::Locks,
                status: HealthStatus::Critical,
                message: "Deadlock detected".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
        ];

        assert_eq!(calculate_overall_status(&checks), HealthStatus::Critical);
    }

    #[test]
    fn test_count_issues_no_issues() {
        let checks = vec![HealthCheckResult {
            category: HealthCategory::ConnectionPool,
            status: HealthStatus::Healthy,
            message: "OK".to_string(),
            details: None,
            checked_at: chrono::Utc::now(),
        }];

        let (total, critical, warnings) = count_issues(&checks);
        assert_eq!(total, 0);
        assert_eq!(critical, 0);
        assert_eq!(warnings, 0);
    }

    #[test]
    fn test_count_issues_with_mixed_statuses() {
        let checks = vec![
            HealthCheckResult {
                category: HealthCategory::ConnectionPool,
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::QueryPerformance,
                status: HealthStatus::Degraded,
                message: "Slow".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::Locks,
                status: HealthStatus::Unhealthy,
                message: "Blocking".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
            HealthCheckResult {
                category: HealthCategory::Cache,
                status: HealthStatus::Critical,
                message: "Failed".to_string(),
                details: None,
                checked_at: chrono::Utc::now(),
            },
        ];

        let (total, critical, warnings) = count_issues(&checks);
        assert_eq!(total, 3);
        assert_eq!(critical, 1);
        assert_eq!(warnings, 1);
    }

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_health_category_serialization() {
        let category = HealthCategory::ConnectionPool;
        let json = serde_json::to_string(&category).unwrap();
        let deserialized: HealthCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(category, deserialized);
    }

    #[test]
    fn test_health_check_result_serialization() {
        let result = HealthCheckResult {
            category: HealthCategory::QueryPerformance,
            status: HealthStatus::Degraded,
            message: "Test message".to_string(),
            details: Some(serde_json::json!({"key": "value"})),
            checked_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: HealthCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.category, deserialized.category);
        assert_eq!(result.status, deserialized.status);
        assert_eq!(result.message, deserialized.message);
    }

    #[test]
    fn test_system_health_report_serialization() {
        let report = SystemHealthReport {
            overall_status: HealthStatus::Healthy,
            checks: vec![],
            total_issues: 0,
            critical_issues: 0,
            warnings: 0,
            generated_at: chrono::Utc::now(),
            generation_time_ms: 150,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SystemHealthReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.overall_status, deserialized.overall_status);
        assert_eq!(report.total_issues, deserialized.total_issues);
        assert_eq!(report.generation_time_ms, deserialized.generation_time_ms);
    }
}
