//! Database Size Monitor
//!
//! Tracks database growth over time and provides capacity planning insights.
//! Monitors table sizes, index sizes, and predicts future storage requirements.
//!
//! # Features
//!
//! - Track database and table size growth over time
//! - Predict future storage requirements
//! - Identify rapidly growing tables
//! - Monitor index bloat
//! - Alert on capacity thresholds
//! - Historical growth rate analysis
//! - Capacity planning recommendations
//!
//! # Example
//!
//! ```rust
//! use kaccy_db::database_size_monitor::{DatabaseSizeMonitor, SizeMonitorConfig};
//! use std::time::Duration;
//!
//! let config = SizeMonitorConfig {
//!     warning_threshold_gb: 100.0,
//!     critical_threshold_gb: 150.0,
//!     rapid_growth_threshold: 0.2, // 20% growth
//!     collection_interval: Duration::from_secs(3600),
//!     max_history_points: 1000,
//! };
//!
//! let monitor = DatabaseSizeMonitor::new(config);
//! ```

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info};

/// Configuration for the database size monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeMonitorConfig {
    /// Database size warning threshold (GB)
    pub warning_threshold_gb: f64,

    /// Database size critical threshold (GB)
    pub critical_threshold_gb: f64,

    /// Growth rate threshold for rapid growth alert (0.0-1.0)
    pub rapid_growth_threshold: f64,

    /// How often to collect size metrics
    pub collection_interval: Duration,

    /// Maximum number of historical data points to keep
    pub max_history_points: usize,
}

impl Default for SizeMonitorConfig {
    fn default() -> Self {
        Self {
            warning_threshold_gb: 100.0,
            critical_threshold_gb: 150.0,
            rapid_growth_threshold: 0.2,                    // 20%
            collection_interval: Duration::from_secs(3600), // 1 hour
            max_history_points: 1000,
        }
    }
}

/// Table size information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSize {
    /// Table name (schema.table)
    pub table_name: String,

    /// Table size in bytes
    pub table_bytes: i64,

    /// Index size in bytes
    pub indexes_bytes: i64,

    /// Total size (table + indexes) in bytes
    pub total_bytes: i64,

    /// Number of rows (estimated)
    pub row_count: i64,

    /// Average row size in bytes
    pub avg_row_size: i64,

    /// When this measurement was taken
    pub measured_at: DateTime<Utc>,
}

impl TableSize {
    /// Get table size in GB
    pub fn table_gb(&self) -> f64 {
        self.table_bytes as f64 / 1_073_741_824.0
    }

    /// Get total size in GB
    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / 1_073_741_824.0
    }

    /// Get human-readable table size
    pub fn table_size_formatted(&self) -> String {
        format_bytes(self.table_bytes)
    }

    /// Get human-readable total size
    pub fn total_size_formatted(&self) -> String {
        format_bytes(self.total_bytes)
    }
}

/// Database size snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSnapshot {
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,

    /// Total database size in bytes
    pub total_size_bytes: i64,

    /// Individual table sizes
    pub tables: Vec<TableSize>,

    /// Number of tables
    pub table_count: usize,
}

impl DatabaseSnapshot {
    /// Get total size in GB
    pub fn total_gb(&self) -> f64 {
        self.total_size_bytes as f64 / 1_073_741_824.0
    }

    /// Get largest tables (top N)
    pub fn largest_tables(&self, limit: usize) -> Vec<TableSize> {
        let mut tables = self.tables.clone();
        tables.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
        tables.truncate(limit);
        tables
    }
}

/// Growth statistics for a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthStats {
    /// Table name
    pub table_name: String,

    /// Current size in bytes
    pub current_size_bytes: i64,

    /// Size at start of monitoring period
    pub initial_size_bytes: i64,

    /// Absolute growth in bytes
    pub growth_bytes: i64,

    /// Relative growth (0.0-1.0, can be > 1.0 for >100% growth)
    pub growth_rate: f64,

    /// Average growth per day (bytes)
    pub avg_daily_growth_bytes: i64,

    /// Estimated days until critical threshold
    pub days_until_critical: Option<f64>,
}

/// Size alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeAlert {
    /// Alert type
    pub alert_type: SizeAlertType,

    /// Alert message
    pub message: String,

    /// Current value
    pub current_value: f64,

    /// Threshold value
    pub threshold: f64,

    /// Affected table (if applicable)
    pub affected_table: Option<String>,

    /// When the alert was triggered
    pub triggered_at: DateTime<Utc>,

    /// Recommended action
    pub recommendation: String,
}

/// Type of size alert
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeAlertType {
    /// Database approaching capacity
    DatabaseCapacity,

    /// Table growing rapidly
    RapidGrowth,

    /// Large table detected
    LargeTable,

    /// Index bloat detected
    IndexBloat,
}

/// Size monitoring report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeReport {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,

    /// Current database snapshot
    pub current_snapshot: DatabaseSnapshot,

    /// Growth statistics
    pub growth_stats: Vec<GrowthStats>,

    /// Active alerts
    pub alerts: Vec<SizeAlert>,

    /// Capacity forecast
    pub forecast: CapacityForecast,
}

/// Capacity forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityForecast {
    /// Current database size in GB
    pub current_size_gb: f64,

    /// Projected size in 30 days (GB)
    pub projected_30d_gb: f64,

    /// Projected size in 90 days (GB)
    pub projected_90d_gb: f64,

    /// Average daily growth rate (GB/day)
    pub avg_growth_gb_per_day: f64,

    /// Days until warning threshold
    pub days_until_warning: Option<f64>,

    /// Days until critical threshold
    pub days_until_critical: Option<f64>,
}

/// Database size monitor
pub struct DatabaseSizeMonitor {
    config: SizeMonitorConfig,
    history: Arc<Mutex<VecDeque<DatabaseSnapshot>>>,
    table_history: Arc<Mutex<HashMap<String, VecDeque<TableSize>>>>,
}

impl DatabaseSizeMonitor {
    /// Create a new database size monitor
    pub fn new(config: SizeMonitorConfig) -> Self {
        Self {
            config,
            history: Arc::new(Mutex::new(VecDeque::new())),
            table_history: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(SizeMonitorConfig::default())
    }

    /// Collect current size metrics
    pub async fn collect_metrics(&self, pool: &PgPool) -> Result<DatabaseSnapshot> {
        info!("Collecting database size metrics");

        let total_size_bytes = self.get_database_size(pool).await?;
        let tables = self.get_table_sizes(pool).await?;

        let snapshot = DatabaseSnapshot {
            timestamp: Utc::now(),
            total_size_bytes,
            table_count: tables.len(),
            tables,
        };

        // Store in history
        if let Ok(mut history) = self.history.lock() {
            history.push_back(snapshot.clone());
            while history.len() > self.config.max_history_points {
                history.pop_front();
            }
        }

        // Store table history
        if let Ok(mut table_history) = self.table_history.lock() {
            for table in &snapshot.tables {
                let entry = table_history
                    .entry(table.table_name.clone())
                    .or_insert_with(VecDeque::new);
                entry.push_back(table.clone());
                while entry.len() > self.config.max_history_points {
                    entry.pop_front();
                }
            }
        }

        debug!(
            size_gb = snapshot.total_gb(),
            tables = snapshot.table_count,
            "Collected size metrics"
        );

        Ok(snapshot)
    }

    /// Generate size monitoring report
    pub async fn generate_report(&self, pool: &PgPool) -> Result<SizeReport> {
        let current_snapshot = self.collect_metrics(pool).await?;
        let growth_stats = self.calculate_growth_stats();
        let alerts = self.check_alerts(&current_snapshot, &growth_stats);
        let forecast = self.generate_forecast(&current_snapshot);

        info!(
            size_gb = current_snapshot.total_gb(),
            alerts = alerts.len(),
            "Generated size report"
        );

        Ok(SizeReport {
            generated_at: Utc::now(),
            current_snapshot,
            growth_stats,
            alerts,
            forecast,
        })
    }

    /// Get database size in bytes
    async fn get_database_size(&self, pool: &PgPool) -> Result<i64> {
        let size = sqlx::query_scalar::<_, i64>("SELECT pg_database_size(current_database())")
            .fetch_one(pool)
            .await?;

        Ok(size)
    }

    /// Get table sizes
    async fn get_table_sizes(&self, pool: &PgPool) -> Result<Vec<TableSize>> {
        let query = r#"
            SELECT
                schemaname || '.' || tablename as table_name,
                pg_table_size(schemaname || '.' || tablename) as table_bytes,
                pg_indexes_size(schemaname || '.' || tablename) as indexes_bytes,
                pg_total_relation_size(schemaname || '.' || tablename) as total_bytes,
                COALESCE(n_live_tup, 0) as row_count
            FROM pg_tables
            LEFT JOIN pg_stat_user_tables ON
                pg_tables.schemaname = pg_stat_user_tables.schemaname AND
                pg_tables.tablename = pg_stat_user_tables.relname
            WHERE pg_tables.schemaname NOT IN ('pg_catalog', 'information_schema')
            ORDER BY total_bytes DESC
        "#;

        let rows = sqlx::query_as::<_, (String, i64, i64, i64, i64)>(query)
            .fetch_all(pool)
            .await?;

        let tables = rows
            .into_iter()
            .map(
                |(table_name, table_bytes, indexes_bytes, total_bytes, row_count)| {
                    let avg_row_size = if row_count > 0 {
                        table_bytes / row_count
                    } else {
                        0
                    };

                    TableSize {
                        table_name,
                        table_bytes,
                        indexes_bytes,
                        total_bytes,
                        row_count,
                        avg_row_size,
                        measured_at: Utc::now(),
                    }
                },
            )
            .collect();

        Ok(tables)
    }

    /// Calculate growth statistics
    fn calculate_growth_stats(&self) -> Vec<GrowthStats> {
        let table_history = match self.table_history.lock() {
            Ok(hist) => hist,
            Err(_) => return Vec::new(),
        };

        let mut stats = Vec::new();

        for (table_name, history) in table_history.iter() {
            if history.len() < 2 {
                continue;
            }

            let first = &history[0];
            let Some(last) = history.back() else {
                continue;
            };

            let growth_bytes = last.total_bytes - first.total_bytes;
            let growth_rate = if first.total_bytes > 0 {
                growth_bytes as f64 / first.total_bytes as f64
            } else {
                0.0
            };

            let time_diff = last.measured_at.signed_duration_since(first.measured_at);
            let days = time_diff.num_seconds() as f64 / 86400.0;

            let avg_daily_growth_bytes = if days > 0.0 {
                (growth_bytes as f64 / days) as i64
            } else {
                0
            };

            stats.push(GrowthStats {
                table_name: table_name.clone(),
                current_size_bytes: last.total_bytes,
                initial_size_bytes: first.total_bytes,
                growth_bytes,
                growth_rate,
                avg_daily_growth_bytes,
                days_until_critical: None,
            });
        }

        stats.sort_by(|a, b| {
            b.growth_rate
                .partial_cmp(&a.growth_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        stats
    }

    /// Check for alert conditions
    fn check_alerts(
        &self,
        snapshot: &DatabaseSnapshot,
        growth_stats: &[GrowthStats],
    ) -> Vec<SizeAlert> {
        let mut alerts = Vec::new();

        let size_gb = snapshot.total_gb();

        // Database capacity alerts
        if size_gb >= self.config.critical_threshold_gb {
            alerts.push(SizeAlert {
                alert_type: SizeAlertType::DatabaseCapacity,
                message: "Database size exceeds critical threshold".to_string(),
                current_value: size_gb,
                threshold: self.config.critical_threshold_gb,
                affected_table: None,
                triggered_at: Utc::now(),
                recommendation: "Urgent: Archive old data or increase storage capacity".to_string(),
            });
        } else if size_gb >= self.config.warning_threshold_gb {
            alerts.push(SizeAlert {
                alert_type: SizeAlertType::DatabaseCapacity,
                message: "Database size exceeds warning threshold".to_string(),
                current_value: size_gb,
                threshold: self.config.warning_threshold_gb,
                affected_table: None,
                triggered_at: Utc::now(),
                recommendation: "Plan for storage expansion or data archival".to_string(),
            });
        }

        // Rapid growth alerts
        for stat in growth_stats {
            if stat.growth_rate >= self.config.rapid_growth_threshold {
                alerts.push(SizeAlert {
                    alert_type: SizeAlertType::RapidGrowth,
                    message: format!("Table '{}' is growing rapidly", stat.table_name),
                    current_value: stat.growth_rate,
                    threshold: self.config.rapid_growth_threshold,
                    affected_table: Some(stat.table_name.clone()),
                    triggered_at: Utc::now(),
                    recommendation: "Investigate data retention policy for this table".to_string(),
                });
            }
        }

        alerts
    }

    /// Generate capacity forecast
    fn generate_forecast(&self, snapshot: &DatabaseSnapshot) -> CapacityForecast {
        let history = match self.history.lock() {
            Ok(hist) => hist.iter().cloned().collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        let current_size_gb = snapshot.total_gb();

        let avg_growth_gb_per_day = if history.len() >= 2 {
            let first = &history[0];
            // history.last() is Some here (guarded by len() >= 2); map_or avoids unwrap
            history.last().map_or(0.0, |last| {
                let growth_bytes = last.total_size_bytes - first.total_size_bytes;
                let time_diff = last.timestamp.signed_duration_since(first.timestamp);
                let days = time_diff.num_seconds() as f64 / 86400.0;

                if days > 0.0 {
                    (growth_bytes as f64 / days) / 1_073_741_824.0
                } else {
                    0.0
                }
            })
        } else {
            0.0
        };

        let projected_30d_gb = current_size_gb + (avg_growth_gb_per_day * 30.0);
        let projected_90d_gb = current_size_gb + (avg_growth_gb_per_day * 90.0);

        let days_until_warning = if avg_growth_gb_per_day > 0.0 {
            let remaining = self.config.warning_threshold_gb - current_size_gb;
            if remaining > 0.0 {
                Some(remaining / avg_growth_gb_per_day)
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        let days_until_critical = if avg_growth_gb_per_day > 0.0 {
            let remaining = self.config.critical_threshold_gb - current_size_gb;
            if remaining > 0.0 {
                Some(remaining / avg_growth_gb_per_day)
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        CapacityForecast {
            current_size_gb,
            projected_30d_gb,
            projected_90d_gb,
            avg_growth_gb_per_day,
            days_until_warning,
            days_until_critical,
        }
    }

    /// Get historical snapshots
    pub fn get_history(&self) -> Vec<DatabaseSnapshot> {
        self.history
            .lock()
            .ok()
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear all historical data
    pub fn clear_history(&self) {
        if let Ok(mut history) = self.history.lock() {
            history.clear();
        }
        if let Ok(mut table_history) = self.table_history.lock() {
            table_history.clear();
        }
    }
}

/// Format bytes to human-readable string
fn format_bytes(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_monitor_config_default() {
        let config = SizeMonitorConfig::default();
        assert_eq!(config.warning_threshold_gb, 100.0);
        assert_eq!(config.critical_threshold_gb, 150.0);
        assert_eq!(config.rapid_growth_threshold, 0.2);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_table_size_formatting() {
        let table_size = TableSize {
            table_name: "users".to_string(),
            table_bytes: 1_073_741_824,
            indexes_bytes: 536_870_912,
            total_bytes: 1_610_612_736,
            row_count: 1000,
            avg_row_size: 1_073_741,
            measured_at: Utc::now(),
        };

        assert_eq!(table_size.table_gb(), 1.0);
        assert_eq!(table_size.table_size_formatted(), "1.00 GB");
    }

    #[test]
    fn test_database_snapshot_largest_tables() {
        let snapshot = DatabaseSnapshot {
            timestamp: Utc::now(),
            total_size_bytes: 10_737_418_240,
            table_count: 3,
            tables: vec![
                TableSize {
                    table_name: "small".to_string(),
                    table_bytes: 1_048_576,
                    indexes_bytes: 0,
                    total_bytes: 1_048_576,
                    row_count: 10,
                    avg_row_size: 104_857,
                    measured_at: Utc::now(),
                },
                TableSize {
                    table_name: "large".to_string(),
                    table_bytes: 5_368_709_120,
                    indexes_bytes: 0,
                    total_bytes: 5_368_709_120,
                    row_count: 1000,
                    avg_row_size: 5_368_709,
                    measured_at: Utc::now(),
                },
                TableSize {
                    table_name: "medium".to_string(),
                    table_bytes: 1_073_741_824,
                    indexes_bytes: 0,
                    total_bytes: 1_073_741_824,
                    row_count: 100,
                    avg_row_size: 10_737_418,
                    measured_at: Utc::now(),
                },
            ],
        };

        let largest = snapshot.largest_tables(2);
        assert_eq!(largest.len(), 2);
        assert_eq!(largest[0].table_name, "large");
        assert_eq!(largest[1].table_name, "medium");
    }

    #[test]
    fn test_growth_stats_serialization() {
        let stats = GrowthStats {
            table_name: "users".to_string(),
            current_size_bytes: 2_147_483_648,
            initial_size_bytes: 1_073_741_824,
            growth_bytes: 1_073_741_824,
            growth_rate: 1.0,
            avg_daily_growth_bytes: 10_737_418,
            days_until_critical: Some(100.0),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("users"));
        assert!(json.contains("growth_rate"));
    }

    #[test]
    fn test_size_alert_serialization() {
        let alert = SizeAlert {
            alert_type: SizeAlertType::DatabaseCapacity,
            message: "Database is full".to_string(),
            current_value: 150.0,
            threshold: 100.0,
            affected_table: None,
            triggered_at: Utc::now(),
            recommendation: "Archive data".to_string(),
        };

        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("DatabaseCapacity"));
    }

    #[test]
    fn test_capacity_forecast_serialization() {
        let forecast = CapacityForecast {
            current_size_gb: 80.0,
            projected_30d_gb: 90.0,
            projected_90d_gb: 110.0,
            avg_growth_gb_per_day: 0.5,
            days_until_warning: Some(40.0),
            days_until_critical: Some(140.0),
        };

        let json = serde_json::to_string(&forecast).unwrap();
        assert!(json.contains("projected_30d_gb"));
    }

    #[test]
    fn test_monitor_with_defaults() {
        let monitor = DatabaseSizeMonitor::with_defaults();
        assert_eq!(monitor.get_history().len(), 0);
    }

    #[test]
    fn test_monitor_clear_history() {
        let monitor = DatabaseSizeMonitor::with_defaults();
        monitor.clear_history();
        assert_eq!(monitor.get_history().len(), 0);
    }
}
