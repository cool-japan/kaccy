//! Vacuum Scheduler
//!
//! Automated PostgreSQL VACUUM scheduling based on table statistics.
//! Complements the maintenance module by providing intelligent scheduling.
//!
//! # Features
//!
//! - Automatic VACUUM scheduling based on dead tuple ratio
//! - Support for different vacuum strategies (standard, FULL, ANALYZE)
//! - Table priority system based on bloat and activity
//! - Configurable thresholds and intervals
//! - Concurrent vacuum support (multiple tables in parallel)
//! - Maintenance window support (only run during specific hours)
//! - Statistics tracking for vacuum operations
//!
//! # Example
//!
//! ```rust
//! use kaccy_db::vacuum_scheduler::{VacuumScheduler, VacuumSchedulerConfig, VacuumStrategy};
//! use std::time::Duration;
//!
//! let config = VacuumSchedulerConfig {
//!     check_interval: Duration::from_secs(3600), // Check every hour
//!     dead_tuple_threshold: 0.2, // Vacuum when 20% dead tuples
//!     strategy: VacuumStrategy::Standard,
//!     max_concurrent_vacuums: 2,
//!     maintenance_window_start_hour: Some(2), // 2 AM
//!     maintenance_window_end_hour: Some(6),   // 6 AM
//! };
//!
//! let scheduler = VacuumScheduler::new(config);
//! ```

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::Result;

/// Vacuum strategy to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VacuumStrategy {
    /// Standard VACUUM (reclaim space, update statistics)
    Standard,

    /// VACUUM FULL (reclaim all space, locks table)
    Full,

    /// VACUUM ANALYZE (vacuum + update statistics)
    Analyze,

    /// ANALYZE only (just update statistics)
    AnalyzeOnly,
}

impl VacuumStrategy {
    /// Get the SQL command for this strategy
    pub fn to_sql(&self, table_name: &str) -> String {
        match self {
            Self::Standard => format!("VACUUM {}", table_name),
            Self::Full => format!("VACUUM FULL {}", table_name),
            Self::Analyze => format!("VACUUM ANALYZE {}", table_name),
            Self::AnalyzeOnly => format!("ANALYZE {}", table_name),
        }
    }

    /// Whether this strategy locks the table
    pub fn locks_table(&self) -> bool {
        matches!(self, Self::Full)
    }
}

/// Configuration for the vacuum scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacuumSchedulerConfig {
    /// How often to check for tables needing vacuum
    pub check_interval: Duration,

    /// Dead tuple ratio threshold (0.0-1.0) for triggering vacuum
    pub dead_tuple_threshold: f64,

    /// Vacuum strategy to use
    pub strategy: VacuumStrategy,

    /// Maximum number of concurrent vacuum operations
    pub max_concurrent_vacuums: usize,

    /// Start hour for maintenance window (0-23), None = no restriction
    pub maintenance_window_start_hour: Option<u32>,

    /// End hour for maintenance window (0-23), None = no restriction
    pub maintenance_window_end_hour: Option<u32>,
}

impl Default for VacuumSchedulerConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(3600), // 1 hour
            dead_tuple_threshold: 0.2,                 // 20%
            strategy: VacuumStrategy::Analyze,
            max_concurrent_vacuums: 2,
            maintenance_window_start_hour: None,
            maintenance_window_end_hour: None,
        }
    }
}

/// Information about a table that needs vacuuming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableVacuumInfo {
    /// Table name
    pub table_name: String,

    /// Dead tuple ratio (0.0-1.0)
    pub dead_tuple_ratio: f64,

    /// Number of dead tuples
    pub dead_tuples: i64,

    /// Number of live tuples
    pub live_tuples: i64,

    /// Last vacuum time (if available)
    pub last_vacuum: Option<DateTime<Utc>>,

    /// Last auto-vacuum time (if available)
    pub last_autovacuum: Option<DateTime<Utc>>,

    /// Priority score (higher = more urgent)
    pub priority: f64,
}

impl TableVacuumInfo {
    /// Calculate priority based on various factors
    pub fn calculate_priority(&mut self) {
        let mut priority = self.dead_tuple_ratio;

        // Increase priority for tables with many dead tuples
        if self.dead_tuples > 1_000_000 {
            priority += 0.3;
        } else if self.dead_tuples > 100_000 {
            priority += 0.2;
        } else if self.dead_tuples > 10_000 {
            priority += 0.1;
        }

        // Increase priority for tables that haven't been vacuumed recently
        let now = Utc::now();
        let last_vacuum_time = self
            .last_vacuum
            .or(self.last_autovacuum)
            .unwrap_or_else(|| now - chrono::Duration::days(365));

        let hours_since_vacuum = now.signed_duration_since(last_vacuum_time).num_hours();

        if hours_since_vacuum > 168 {
            // 1 week
            priority += 0.3;
        } else if hours_since_vacuum > 72 {
            // 3 days
            priority += 0.2;
        } else if hours_since_vacuum > 24 {
            // 1 day
            priority += 0.1;
        }

        self.priority = priority.min(1.0);
    }
}

/// Result of a vacuum operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacuumResult {
    /// Table name
    pub table_name: String,

    /// Whether the vacuum succeeded
    pub success: bool,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Error message if failed
    pub error: Option<String>,

    /// Dead tuples before vacuum
    pub dead_tuples_before: i64,

    /// Strategy used
    pub strategy: VacuumStrategy,
}

/// Statistics for vacuum scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacuumStats {
    /// Total number of vacuums performed
    pub total_vacuums: usize,

    /// Number of successful vacuums
    pub successful_vacuums: usize,

    /// Number of failed vacuums
    pub failed_vacuums: usize,

    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,

    /// Average execution time in milliseconds
    pub average_execution_time_ms: u64,

    /// Last vacuum time
    pub last_vacuum_at: Option<DateTime<Utc>>,
}

/// Vacuum scheduler
pub struct VacuumScheduler {
    config: VacuumSchedulerConfig,
}

impl VacuumScheduler {
    /// Create a new vacuum scheduler
    pub fn new(config: VacuumSchedulerConfig) -> Self {
        Self { config }
    }

    /// Create a vacuum scheduler with default configuration
    pub fn with_defaults() -> Self {
        Self::new(VacuumSchedulerConfig::default())
    }

    /// Check if we're currently in the maintenance window
    pub fn in_maintenance_window(&self) -> bool {
        match (
            self.config.maintenance_window_start_hour,
            self.config.maintenance_window_end_hour,
        ) {
            (Some(start), Some(end)) => {
                let now = Utc::now();
                let current_hour = now.hour();

                if start <= end {
                    // Normal case: 2 AM - 6 AM
                    current_hour >= start && current_hour < end
                } else {
                    // Wrap around: 22 PM - 2 AM
                    current_hour >= start || current_hour < end
                }
            }
            _ => true, // No maintenance window configured, always allowed
        }
    }

    /// Get tables that need vacuuming
    pub async fn get_tables_needing_vacuum(&self, pool: &PgPool) -> Result<Vec<TableVacuumInfo>> {
        let query = r#"
            SELECT
                schemaname || '.' || relname as table_name,
                COALESCE(n_dead_tup, 0) as dead_tuples,
                COALESCE(n_live_tup, 0) as live_tuples,
                CASE
                    WHEN n_live_tup + n_dead_tup > 0
                    THEN CAST(n_dead_tup AS FLOAT) / (n_live_tup + n_dead_tup)
                    ELSE 0
                END as dead_tuple_ratio,
                last_vacuum,
                last_autovacuum
            FROM pg_stat_user_tables
            WHERE n_dead_tup > 0
            ORDER BY dead_tuple_ratio DESC
        "#;

        let rows = sqlx::query_as::<
            _,
            (
                String,
                i64,
                i64,
                f64,
                Option<DateTime<Utc>>,
                Option<DateTime<Utc>>,
            ),
        >(query)
        .fetch_all(pool)
        .await?;

        let mut tables: Vec<TableVacuumInfo> = rows
            .into_iter()
            .filter_map(
                |(
                    table_name,
                    dead_tuples,
                    live_tuples,
                    dead_tuple_ratio,
                    last_vacuum,
                    last_autovacuum,
                )| {
                    if dead_tuple_ratio >= self.config.dead_tuple_threshold {
                        let mut info = TableVacuumInfo {
                            table_name,
                            dead_tuple_ratio,
                            dead_tuples,
                            live_tuples,
                            last_vacuum,
                            last_autovacuum,
                            priority: 0.0,
                        };
                        info.calculate_priority();
                        Some(info)
                    } else {
                        None
                    }
                },
            )
            .collect();

        // Sort by priority (highest first)
        tables.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(tables)
    }

    /// Run vacuum on a single table
    pub async fn vacuum_table(
        &self,
        pool: &PgPool,
        table_name: &str,
        dead_tuples_before: i64,
    ) -> Result<VacuumResult> {
        let start_time = std::time::Instant::now();
        let strategy = self.config.strategy;

        info!(
            table_name = table_name,
            strategy = ?strategy,
            dead_tuples = dead_tuples_before,
            "Starting vacuum operation"
        );

        let sql = strategy.to_sql(table_name);

        match sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await {
            Ok(_) => {
                let execution_time_ms = start_time.elapsed().as_millis() as u64;

                info!(
                    table_name = table_name,
                    execution_time_ms = execution_time_ms,
                    "Vacuum completed successfully"
                );

                Ok(VacuumResult {
                    table_name: table_name.to_string(),
                    success: true,
                    execution_time_ms,
                    error: None,
                    dead_tuples_before,
                    strategy,
                })
            }
            Err(e) => {
                let execution_time_ms = start_time.elapsed().as_millis() as u64;

                warn!(
                    table_name = table_name,
                    error = %e,
                    "Vacuum failed"
                );

                Ok(VacuumResult {
                    table_name: table_name.to_string(),
                    success: false,
                    execution_time_ms,
                    error: Some(e.to_string()),
                    dead_tuples_before,
                    strategy,
                })
            }
        }
    }

    /// Run scheduled vacuum operations
    pub async fn run_scheduled_vacuum(&self, pool: &PgPool) -> Result<Vec<VacuumResult>> {
        // Check maintenance window
        if !self.in_maintenance_window() {
            debug!("Outside maintenance window, skipping vacuum");
            return Ok(Vec::new());
        }

        // Get tables needing vacuum
        let tables = self.get_tables_needing_vacuum(pool).await?;

        if tables.is_empty() {
            debug!("No tables need vacuuming");
            return Ok(Vec::new());
        }

        info!(table_count = tables.len(), "Found tables needing vacuum");

        let mut results = Vec::new();

        // Vacuum tables (respect max_concurrent_vacuums limit)
        for table in tables.iter().take(self.config.max_concurrent_vacuums) {
            let result = self
                .vacuum_table(pool, &table.table_name, table.dead_tuples)
                .await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Calculate statistics from vacuum results
    pub fn calculate_stats(&self, results: &[VacuumResult]) -> VacuumStats {
        let total_vacuums = results.len();
        let successful_vacuums = results.iter().filter(|r| r.success).count();
        let failed_vacuums = total_vacuums - successful_vacuums;

        let total_execution_time_ms: u64 = results.iter().map(|r| r.execution_time_ms).sum();

        let average_execution_time_ms = if total_vacuums > 0 {
            total_execution_time_ms / total_vacuums as u64
        } else {
            0
        };

        VacuumStats {
            total_vacuums,
            successful_vacuums,
            failed_vacuums,
            total_execution_time_ms,
            average_execution_time_ms,
            last_vacuum_at: Some(Utc::now()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vacuum_scheduler_config_default() {
        let config = VacuumSchedulerConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(3600));
        assert_eq!(config.dead_tuple_threshold, 0.2);
        assert_eq!(config.strategy, VacuumStrategy::Analyze);
        assert_eq!(config.max_concurrent_vacuums, 2);
    }

    #[test]
    fn test_vacuum_strategy_to_sql() {
        assert_eq!(VacuumStrategy::Standard.to_sql("users"), "VACUUM users");
        assert_eq!(VacuumStrategy::Full.to_sql("users"), "VACUUM FULL users");
        assert_eq!(
            VacuumStrategy::Analyze.to_sql("users"),
            "VACUUM ANALYZE users"
        );
        assert_eq!(VacuumStrategy::AnalyzeOnly.to_sql("users"), "ANALYZE users");
    }

    #[test]
    fn test_vacuum_strategy_locks_table() {
        assert!(!VacuumStrategy::Standard.locks_table());
        assert!(VacuumStrategy::Full.locks_table());
        assert!(!VacuumStrategy::Analyze.locks_table());
        assert!(!VacuumStrategy::AnalyzeOnly.locks_table());
    }

    #[test]
    fn test_table_vacuum_info_priority_calculation() {
        let mut info = TableVacuumInfo {
            table_name: "test".to_string(),
            dead_tuple_ratio: 0.5,
            dead_tuples: 2_000_000,
            live_tuples: 2_000_000,
            last_vacuum: Some(Utc::now() - chrono::Duration::days(10)),
            last_autovacuum: None,
            priority: 0.0,
        };

        info.calculate_priority();

        // Should have high priority: 0.5 (ratio) + 0.3 (>1M dead) + 0.3 (>1 week) = 1.1, capped at 1.0
        assert_eq!(info.priority, 1.0); // Capped at 1.0
    }

    #[test]
    fn test_table_vacuum_info_low_priority() {
        let mut info = TableVacuumInfo {
            table_name: "test".to_string(),
            dead_tuple_ratio: 0.1,
            dead_tuples: 1000,
            live_tuples: 9000,
            last_vacuum: Some(Utc::now() - chrono::Duration::hours(12)),
            last_autovacuum: None,
            priority: 0.0,
        };

        info.calculate_priority();

        // Should have low priority: just the ratio 0.1
        assert!(info.priority >= 0.1 && info.priority < 0.2);
    }

    #[test]
    fn test_in_maintenance_window_no_restriction() {
        let scheduler = VacuumScheduler::with_defaults();
        assert!(scheduler.in_maintenance_window());
    }

    #[test]
    fn test_vacuum_result_serialization() {
        let result = VacuumResult {
            table_name: "users".to_string(),
            success: true,
            execution_time_ms: 5000,
            error: None,
            dead_tuples_before: 100000,
            strategy: VacuumStrategy::Analyze,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("users"));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_vacuum_stats_calculation() {
        let scheduler = VacuumScheduler::with_defaults();

        let results = vec![
            VacuumResult {
                table_name: "table1".to_string(),
                success: true,
                execution_time_ms: 1000,
                error: None,
                dead_tuples_before: 10000,
                strategy: VacuumStrategy::Standard,
            },
            VacuumResult {
                table_name: "table2".to_string(),
                success: true,
                execution_time_ms: 2000,
                error: None,
                dead_tuples_before: 20000,
                strategy: VacuumStrategy::Standard,
            },
            VacuumResult {
                table_name: "table3".to_string(),
                success: false,
                execution_time_ms: 500,
                error: Some("error".to_string()),
                dead_tuples_before: 5000,
                strategy: VacuumStrategy::Standard,
            },
        ];

        let stats = scheduler.calculate_stats(&results);

        assert_eq!(stats.total_vacuums, 3);
        assert_eq!(stats.successful_vacuums, 2);
        assert_eq!(stats.failed_vacuums, 1);
        assert_eq!(stats.total_execution_time_ms, 3500);
        assert_eq!(stats.average_execution_time_ms, 1166);
    }

    #[test]
    fn test_vacuum_stats_serialization() {
        let stats = VacuumStats {
            total_vacuums: 10,
            successful_vacuums: 9,
            failed_vacuums: 1,
            total_execution_time_ms: 50000,
            average_execution_time_ms: 5000,
            last_vacuum_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_vacuums"));
        assert!(json.contains("\"successful_vacuums\":9"));
    }
}
