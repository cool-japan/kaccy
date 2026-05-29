//! Connection pool diagnostics and health monitoring utilities
//!
//! This module provides comprehensive diagnostics for database connection pools,
//! helping identify connection issues, pool exhaustion, and performance problems.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Comprehensive connection pool diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolDiagnostics {
    /// Current pool statistics
    pub stats: PoolStats,
    /// Connection health status
    pub health: ConnectionHealth,
    /// Recent connection issues
    pub issues: Vec<ConnectionIssue>,
    /// Recommendations for optimization
    pub recommendations: Vec<String>,
}

/// Current pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Number of connections currently in use
    pub connections_active: u32,
    /// Number of idle connections in the pool
    pub connections_idle: u32,
    /// Maximum number of connections allowed
    pub connections_max: u32,
    /// Current pool utilization percentage
    pub utilization_percent: f64,
    /// Average connection acquisition time (ms)
    pub avg_acquisition_time_ms: Option<f64>,
}

/// Connection health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    /// Whether the pool is healthy
    pub is_healthy: bool,
    /// Health status message
    pub status: String,
    /// Last successful connection timestamp
    pub last_successful_connection: Option<DateTime<Utc>>,
    /// Number of failed connection attempts in the last minute
    pub recent_failures: u32,
}

/// Connection issue record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionIssue {
    /// When the issue occurred
    pub timestamp: DateTime<Utc>,
    /// Issue type
    pub issue_type: IssueType,
    /// Issue description
    pub description: String,
    /// Severity level
    pub severity: Severity,
}

/// Type of connection issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueType {
    /// Pool exhaustion (no available connections)
    PoolExhausted,
    /// Connection timeout
    Timeout,
    /// Connection failure
    ConnectionFailed,
    /// Slow query
    SlowQuery,
    /// High utilization
    HighUtilization,
}

/// Issue severity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational
    Info,
    /// Warning
    Warning,
    /// Critical
    Critical,
}

/// Get comprehensive pool diagnostics
pub async fn get_pool_diagnostics(pool: &PgPool) -> Result<PoolDiagnostics> {
    let stats = get_pool_stats(pool);
    let health = check_connection_health(pool).await?;
    let issues = diagnose_issues(&stats, &health);
    let recommendations = generate_recommendations(&stats, &health, &issues);

    Ok(PoolDiagnostics {
        stats,
        health,
        issues,
        recommendations,
    })
}

/// Get current pool statistics
pub fn get_pool_stats(pool: &PgPool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle() as u32;
    let max = pool.options().get_max_connections();

    let active = size.saturating_sub(idle);
    let utilization = if max > 0 {
        (active as f64 / max as f64) * 100.0
    } else {
        0.0
    };

    PoolStats {
        connections_active: active,
        connections_idle: idle,
        connections_max: max,
        utilization_percent: utilization,
        avg_acquisition_time_ms: None, // Would need tracking
    }
}

/// Check connection health
pub async fn check_connection_health(pool: &PgPool) -> Result<ConnectionHealth> {
    // Try a simple query to check connectivity
    let result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await;

    let is_healthy = result.is_ok();
    let status = if is_healthy {
        "Healthy".to_string()
    } else {
        format!(
            "Unhealthy: {}",
            result.expect_err("is_healthy is false so result is Err")
        )
    };

    Ok(ConnectionHealth {
        is_healthy,
        status,
        last_successful_connection: if is_healthy { Some(Utc::now()) } else { None },
        recent_failures: if is_healthy { 0 } else { 1 },
    })
}

/// Diagnose issues based on stats and health
fn diagnose_issues(stats: &PoolStats, health: &ConnectionHealth) -> Vec<ConnectionIssue> {
    let mut issues = Vec::new();
    let now = Utc::now();

    // Check for high utilization
    if stats.utilization_percent > 90.0 {
        issues.push(ConnectionIssue {
            timestamp: now,
            issue_type: IssueType::HighUtilization,
            description: format!(
                "Pool utilization is {:.1}% - consider increasing max connections",
                stats.utilization_percent
            ),
            severity: Severity::Warning,
        });
    }

    // Check for pool exhaustion
    if stats.connections_idle == 0 && stats.connections_active >= stats.connections_max {
        issues.push(ConnectionIssue {
            timestamp: now,
            issue_type: IssueType::PoolExhausted,
            description: "Connection pool is exhausted - all connections are in use".to_string(),
            severity: Severity::Critical,
        });
    }

    // Check for unhealthy state
    if !health.is_healthy {
        issues.push(ConnectionIssue {
            timestamp: now,
            issue_type: IssueType::ConnectionFailed,
            description: health.status.clone(),
            severity: Severity::Critical,
        });
    }

    issues
}

/// Generate recommendations based on diagnostics
fn generate_recommendations(
    stats: &PoolStats,
    health: &ConnectionHealth,
    issues: &[ConnectionIssue],
) -> Vec<String> {
    let mut recommendations = Vec::new();

    // Check if pool is too small
    if stats.utilization_percent > 80.0 {
        recommendations.push(format!(
            "Consider increasing max_connections from {} to {} for better headroom",
            stats.connections_max,
            stats.connections_max * 2
        ));
    }

    // Check if pool is too large
    if stats.utilization_percent < 20.0 && stats.connections_max > 10 {
        recommendations.push(format!(
            "Pool utilization is low ({:.1}%) - consider reducing max_connections to save resources",
            stats.utilization_percent
        ));
    }

    // Check for critical issues
    let has_critical = issues.iter().any(|i| i.severity == Severity::Critical);
    if has_critical {
        recommendations.push(
            "Critical issues detected - investigate immediately to prevent service disruption"
                .to_string(),
        );
    }

    // Connection health recommendations
    if !health.is_healthy {
        recommendations.push("Database connectivity issues detected - check network, credentials, and database status".to_string());
    }

    // If no idle connections but utilization is high
    if stats.connections_idle == 0 && stats.utilization_percent > 70.0 {
        recommendations.push(
            "No idle connections available - increase pool size or optimize query performance"
                .to_string(),
        );
    }

    recommendations
}

/// Get active connections from PostgreSQL
pub async fn get_active_connections(pool: &PgPool) -> Result<Vec<ActiveConnection>> {
    let connections =
        sqlx::query_as::<_, (i32, String, Option<String>, Option<DateTime<Utc>>, String)>(
            r#"
        SELECT
            pid,
            usename,
            application_name,
            query_start,
            state
        FROM pg_stat_activity
        WHERE datname = current_database()
        AND pid != pg_backend_pid()
        ORDER BY query_start DESC NULLS LAST
        "#,
        )
        .fetch_all(pool)
        .await?;

    Ok(connections
        .into_iter()
        .map(|c| ActiveConnection {
            pid: c.0,
            username: c.1,
            application_name: c.2,
            query_start: c.3,
            state: c.4,
            duration: c.3.map(|start| {
                let now = Utc::now();
                now.signed_duration_since(start).num_milliseconds() as u64
            }),
        })
        .collect())
}

/// Information about an active database connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveConnection {
    /// Process ID
    pub pid: i32,
    /// Connected username
    pub username: String,
    /// Application name
    pub application_name: Option<String>,
    /// When the current query started
    pub query_start: Option<DateTime<Utc>>,
    /// Connection state
    pub state: String,
    /// Query duration in milliseconds
    pub duration: Option<u64>,
}

/// Find long-running queries
pub async fn find_long_running_queries(
    pool: &PgPool,
    threshold_ms: u64,
) -> Result<Vec<LongRunningQuery>> {
    let threshold_interval = format!("{} milliseconds", threshold_ms);

    let queries = sqlx::query_as::<_, (i32, String, String, DateTime<Utc>, String)>(
        r#"
        SELECT
            pid,
            usename,
            query,
            query_start,
            state
        FROM pg_stat_activity
        WHERE datname = current_database()
        AND state = 'active'
        AND query_start < NOW() - $1::interval
        AND query NOT LIKE '%pg_stat_activity%'
        ORDER BY query_start ASC
        "#,
    )
    .bind(threshold_interval)
    .fetch_all(pool)
    .await?;

    Ok(queries
        .into_iter()
        .map(|q| {
            let duration = Utc::now().signed_duration_since(q.3).num_milliseconds() as u64;

            LongRunningQuery {
                pid: q.0,
                username: q.1,
                query: q.2,
                started_at: q.3,
                duration_ms: duration,
                state: q.4,
            }
        })
        .collect())
}

/// Information about a long-running query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongRunningQuery {
    /// Process ID
    pub pid: i32,
    /// Username
    pub username: String,
    /// Query text
    pub query: String,
    /// When the query started
    pub started_at: DateTime<Utc>,
    /// Query duration in milliseconds
    pub duration_ms: u64,
    /// Query state
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_stats_structure() {
        let stats = PoolStats {
            connections_active: 5,
            connections_idle: 15,
            connections_max: 20,
            utilization_percent: 25.0,
            avg_acquisition_time_ms: Some(1.5),
        };

        assert_eq!(stats.connections_active, 5);
        assert_eq!(stats.utilization_percent, 25.0);
    }

    #[test]
    fn test_connection_health_structure() {
        let health = ConnectionHealth {
            is_healthy: true,
            status: "OK".to_string(),
            last_successful_connection: Some(Utc::now()),
            recent_failures: 0,
        };

        assert!(health.is_healthy);
        assert_eq!(health.recent_failures, 0);
    }

    #[test]
    fn test_connection_issue_structure() {
        let issue = ConnectionIssue {
            timestamp: Utc::now(),
            issue_type: IssueType::PoolExhausted,
            description: "Pool full".to_string(),
            severity: Severity::Critical,
        };

        assert_eq!(issue.issue_type, IssueType::PoolExhausted);
        assert_eq!(issue.severity, Severity::Critical);
    }

    #[test]
    fn test_diagnose_high_utilization() {
        let stats = PoolStats {
            connections_active: 19,
            connections_idle: 1,
            connections_max: 20,
            utilization_percent: 95.0,
            avg_acquisition_time_ms: None,
        };

        let health = ConnectionHealth {
            is_healthy: true,
            status: "OK".to_string(),
            last_successful_connection: Some(Utc::now()),
            recent_failures: 0,
        };

        let issues = diagnose_issues(&stats, &health);
        assert!(issues
            .iter()
            .any(|i| i.issue_type == IssueType::HighUtilization));
    }

    #[test]
    fn test_diagnose_pool_exhaustion() {
        let stats = PoolStats {
            connections_active: 20,
            connections_idle: 0,
            connections_max: 20,
            utilization_percent: 100.0,
            avg_acquisition_time_ms: None,
        };

        let health = ConnectionHealth {
            is_healthy: true,
            status: "OK".to_string(),
            last_successful_connection: Some(Utc::now()),
            recent_failures: 0,
        };

        let issues = diagnose_issues(&stats, &health);
        assert!(issues
            .iter()
            .any(|i| i.issue_type == IssueType::PoolExhausted));
        assert!(issues.iter().any(|i| i.severity == Severity::Critical));
    }

    #[test]
    fn test_generate_recommendations_high_utilization() {
        let stats = PoolStats {
            connections_active: 18,
            connections_idle: 2,
            connections_max: 20,
            utilization_percent: 90.0,
            avg_acquisition_time_ms: None,
        };

        let health = ConnectionHealth {
            is_healthy: true,
            status: "OK".to_string(),
            last_successful_connection: Some(Utc::now()),
            recent_failures: 0,
        };

        let issues = vec![];
        let recommendations = generate_recommendations(&stats, &health, &issues);

        assert!(!recommendations.is_empty());
        assert!(recommendations
            .iter()
            .any(|r| r.contains("increasing max_connections")));
    }

    #[test]
    fn test_generate_recommendations_low_utilization() {
        let stats = PoolStats {
            connections_active: 2,
            connections_idle: 18,
            connections_max: 20,
            utilization_percent: 10.0,
            avg_acquisition_time_ms: None,
        };

        let health = ConnectionHealth {
            is_healthy: true,
            status: "OK".to_string(),
            last_successful_connection: Some(Utc::now()),
            recent_failures: 0,
        };

        let issues = vec![];
        let recommendations = generate_recommendations(&stats, &health, &issues);

        assert!(recommendations
            .iter()
            .any(|r| r.contains("reducing max_connections")));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Critical);
    }

    #[test]
    fn test_pool_diagnostics_serialization() {
        let diagnostics = PoolDiagnostics {
            stats: PoolStats {
                connections_active: 5,
                connections_idle: 5,
                connections_max: 10,
                utilization_percent: 50.0,
                avg_acquisition_time_ms: None,
            },
            health: ConnectionHealth {
                is_healthy: true,
                status: "OK".to_string(),
                last_successful_connection: None,
                recent_failures: 0,
            },
            issues: vec![],
            recommendations: vec![],
        };

        let json = serde_json::to_string(&diagnostics).unwrap();
        let deserialized: PoolDiagnostics = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.stats.connections_active, 5);
        assert!(deserialized.health.is_healthy);
    }
}
