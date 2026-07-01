//! Database maintenance utilities for PostgreSQL
//!
//! This module provides utilities for routine database maintenance operations
//! including VACUUM, ANALYZE, REINDEX, and general health checks.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Database maintenance operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaintenanceOperation {
    /// Standard VACUUM to reclaim storage
    Vacuum,
    /// VACUUM FULL to reclaim maximum storage (locks table)
    VacuumFull,
    /// VACUUM ANALYZE to update statistics and reclaim storage
    VacuumAnalyze,
    /// ANALYZE only to update query planner statistics
    Analyze,
    /// REINDEX to rebuild indexes
    Reindex,
}

/// Result of a maintenance operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceResult {
    /// Operation that was performed
    pub operation: MaintenanceOperation,
    /// Table name (None for database-wide operations)
    pub table_name: Option<String>,
    /// Whether the operation succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Any error message if failed
    pub error_message: Option<String>,
    /// When the operation was performed
    pub timestamp: DateTime<Utc>,
}

/// VACUUM a specific table
///
/// Reclaims storage occupied by dead tuples. Does not lock the table for reads/writes.
pub async fn vacuum_table(pool: &PgPool, table_name: &str) -> Result<MaintenanceResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    // Sanitize table name to prevent SQL injection
    let sanitized_table = crate::helpers::sanitize_table_name(table_name)?;

    let query = format!("VACUUM {}", sanitized_table);
    let result = sqlx::query(sqlx::AssertSqlSafe(query)).execute(pool).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Vacuum,
            table_name: Some(table_name.to_string()),
            success: true,
            duration_ms,
            error_message: None,
            timestamp,
        }),
        Err(e) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Vacuum,
            table_name: Some(table_name.to_string()),
            success: false,
            duration_ms,
            error_message: Some(e.to_string()),
            timestamp,
        }),
    }
}

/// VACUUM ANALYZE a specific table
///
/// Reclaims storage and updates statistics for the query planner.
pub async fn vacuum_analyze_table(pool: &PgPool, table_name: &str) -> Result<MaintenanceResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    let sanitized_table = crate::helpers::sanitize_table_name(table_name)?;

    let query = format!("VACUUM ANALYZE {}", sanitized_table);
    let result = sqlx::query(sqlx::AssertSqlSafe(query)).execute(pool).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::VacuumAnalyze,
            table_name: Some(table_name.to_string()),
            success: true,
            duration_ms,
            error_message: None,
            timestamp,
        }),
        Err(e) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::VacuumAnalyze,
            table_name: Some(table_name.to_string()),
            success: false,
            duration_ms,
            error_message: Some(e.to_string()),
            timestamp,
        }),
    }
}

/// ANALYZE a specific table
///
/// Updates statistics used by the query planner without reclaiming storage.
pub async fn analyze_table(pool: &PgPool, table_name: &str) -> Result<MaintenanceResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    let sanitized_table = crate::helpers::sanitize_table_name(table_name)?;

    let query = format!("ANALYZE {}", sanitized_table);
    let result = sqlx::query(sqlx::AssertSqlSafe(query)).execute(pool).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Analyze,
            table_name: Some(table_name.to_string()),
            success: true,
            duration_ms,
            error_message: None,
            timestamp,
        }),
        Err(e) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Analyze,
            table_name: Some(table_name.to_string()),
            success: false,
            duration_ms,
            error_message: Some(e.to_string()),
            timestamp,
        }),
    }
}

/// REINDEX a specific table
///
/// Rebuilds all indexes on the table. This can fix index corruption and
/// improve performance for bloated indexes.
pub async fn reindex_table(pool: &PgPool, table_name: &str) -> Result<MaintenanceResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    let sanitized_table = crate::helpers::sanitize_table_name(table_name)?;

    let query = format!("REINDEX TABLE {}", sanitized_table);
    let result = sqlx::query(sqlx::AssertSqlSafe(query)).execute(pool).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(_) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Reindex,
            table_name: Some(table_name.to_string()),
            success: true,
            duration_ms,
            error_message: None,
            timestamp,
        }),
        Err(e) => Ok(MaintenanceResult {
            operation: MaintenanceOperation::Reindex,
            table_name: Some(table_name.to_string()),
            success: false,
            duration_ms,
            error_message: Some(e.to_string()),
            timestamp,
        }),
    }
}

/// Get tables that need vacuuming
///
/// Returns tables with high dead tuple ratio that would benefit from VACUUM.
pub async fn get_tables_needing_vacuum(
    pool: &PgPool,
    threshold_percent: f64,
) -> Result<Vec<TableVacuumInfo>> {
    let tables = sqlx::query_as::<_, (String, i64, i64, f64)>(
        r#"
        SELECT
            schemaname || '.' || relname as table_name,
            n_live_tup,
            n_dead_tup,
            CASE
                WHEN n_live_tup > 0
                THEN (n_dead_tup::float / n_live_tup::float) * 100
                ELSE 0
            END as dead_tuple_percent
        FROM pg_stat_user_tables
        WHERE n_dead_tup > 0
        AND (
            CASE
                WHEN n_live_tup > 0
                THEN (n_dead_tup::float / n_live_tup::float) * 100
                ELSE 0
            END
        ) > $1
        ORDER BY dead_tuple_percent DESC
        "#,
    )
    .bind(threshold_percent)
    .fetch_all(pool)
    .await?;

    Ok(tables
        .into_iter()
        .map(|t| TableVacuumInfo {
            table_name: t.0,
            live_tuples: t.1,
            dead_tuples: t.2,
            dead_tuple_percent: t.3,
        })
        .collect())
}

/// Information about table vacuum needs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableVacuumInfo {
    /// Fully qualified table name
    pub table_name: String,
    /// Number of live tuples
    pub live_tuples: i64,
    /// Number of dead tuples
    pub dead_tuples: i64,
    /// Percentage of dead tuples
    pub dead_tuple_percent: f64,
}

/// Get bloated tables
///
/// Identifies tables with significant bloat that might benefit from VACUUM FULL
/// or table reorganization.
pub async fn get_bloated_tables(pool: &PgPool, min_wasted_mb: i64) -> Result<Vec<TableBloatInfo>> {
    // This is a simplified bloat estimate
    let tables = sqlx::query_as::<_, (String, i64, i64, f64)>(
        r#"
        SELECT
            schemaname || '.' || tablename as table_name,
            pg_total_relation_size(schemaname || '.' || tablename) / 1024 / 1024 as size_mb,
            (pg_total_relation_size(schemaname || '.' || tablename) -
             pg_relation_size(schemaname || '.' || tablename)) / 1024 / 1024 as wasted_mb,
            CASE
                WHEN pg_total_relation_size(schemaname || '.' || tablename) > 0
                THEN ((pg_total_relation_size(schemaname || '.' || tablename) -
                       pg_relation_size(schemaname || '.' || tablename))::float /
                      pg_total_relation_size(schemaname || '.' || tablename)::float) * 100
                ELSE 0
            END as bloat_percent
        FROM pg_tables
        WHERE schemaname = 'public'
        AND (pg_total_relation_size(schemaname || '.' || tablename) -
             pg_relation_size(schemaname || '.' || tablename)) / 1024 / 1024 > $1
        ORDER BY wasted_mb DESC
        "#,
    )
    .bind(min_wasted_mb)
    .fetch_all(pool)
    .await?;

    Ok(tables
        .into_iter()
        .map(|t| TableBloatInfo {
            table_name: t.0,
            total_size_mb: t.1,
            wasted_mb: t.2,
            bloat_percent: t.3,
        })
        .collect())
}

/// Information about table bloat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBloatInfo {
    /// Fully qualified table name
    pub table_name: String,
    /// Total table size in MB
    pub total_size_mb: i64,
    /// Wasted space in MB
    pub wasted_mb: i64,
    /// Bloat percentage
    pub bloat_percent: f64,
}

/// Run automatic maintenance on tables that need it
///
/// Analyzes the database and runs VACUUM ANALYZE on tables that exceed
/// the dead tuple threshold.
pub async fn auto_vacuum_tables(
    pool: &PgPool,
    threshold_percent: f64,
) -> Result<Vec<MaintenanceResult>> {
    let tables = get_tables_needing_vacuum(pool, threshold_percent).await?;

    let mut results = Vec::new();
    for table_info in tables {
        // Extract just the table name without schema
        let table_name = table_info
            .table_name
            .split('.')
            .next_back()
            .unwrap_or(&table_info.table_name);

        let result = vacuum_analyze_table(pool, table_name).await?;
        results.push(result);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintenance_operation_enum() {
        let op = MaintenanceOperation::Vacuum;
        assert_eq!(op, MaintenanceOperation::Vacuum);
    }

    #[test]
    fn test_maintenance_result_structure() {
        let result = MaintenanceResult {
            operation: MaintenanceOperation::Vacuum,
            table_name: Some("users".to_string()),
            success: true,
            duration_ms: 100,
            error_message: None,
            timestamp: Utc::now(),
        };

        assert!(result.success);
        assert_eq!(result.table_name, Some("users".to_string()));
    }

    #[test]
    fn test_table_vacuum_info_structure() {
        let info = TableVacuumInfo {
            table_name: "public.users".to_string(),
            live_tuples: 10000,
            dead_tuples: 500,
            dead_tuple_percent: 5.0,
        };

        assert_eq!(info.live_tuples, 10000);
        assert_eq!(info.dead_tuple_percent, 5.0);
    }

    #[test]
    fn test_table_bloat_info_structure() {
        let info = TableBloatInfo {
            table_name: "public.orders".to_string(),
            total_size_mb: 1000,
            wasted_mb: 200,
            bloat_percent: 20.0,
        };

        assert_eq!(info.wasted_mb, 200);
        assert_eq!(info.bloat_percent, 20.0);
    }

    #[test]
    fn test_maintenance_result_serialization() {
        let result = MaintenanceResult {
            operation: MaintenanceOperation::Analyze,
            table_name: None,
            success: true,
            duration_ms: 50,
            error_message: None,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: MaintenanceResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, result.success);
        assert_eq!(deserialized.duration_ms, result.duration_ms);
    }

    #[test]
    fn test_maintenance_operation_serialization() {
        let op = MaintenanceOperation::VacuumFull;
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: MaintenanceOperation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, op);
    }
}
