//! Connection Pool Monitor
//!
//! Real-time monitoring and alerting for database connection pools.
//! Provides detailed metrics, health checks, and automatic anomaly detection.
//!
//! # Features
//!
//! - Real-time pool utilization tracking
//! - Connection acquisition time monitoring
//! - Pool saturation detection
//! - Automatic alert generation
//! - Historical metrics collection
//! - Trend analysis for capacity planning
//! - Configurable alert thresholds
//!
//! # Example
//!
//! ```rust
//! use kaccy_db::pool_monitor::{PoolMonitor, MonitorConfig};
//! use sqlx::PgPool;
//! use std::time::Duration;
//!
//! let config = MonitorConfig {
//!     high_utilization_threshold: 0.8,
//!     critical_utilization_threshold: 0.95,
//!     slow_acquisition_threshold_ms: 1000,
//!     collection_interval: Duration::from_secs(30),
//!     max_history_points: 1000,
//! };
//!
//! let monitor = PoolMonitor::new(config);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info};

/// Configuration for the pool monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Utilization threshold for high utilization warning (0.0-1.0)
    pub high_utilization_threshold: f64,

    /// Utilization threshold for critical alert (0.0-1.0)
    pub critical_utilization_threshold: f64,

    /// Connection acquisition time threshold (ms) for slow acquisition warning
    pub slow_acquisition_threshold_ms: u64,

    /// How often to collect metrics
    pub collection_interval: Duration,

    /// Maximum number of historical data points to keep
    pub max_history_points: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            high_utilization_threshold: 0.8,
            critical_utilization_threshold: 0.95,
            slow_acquisition_threshold_ms: 1000,
            collection_interval: Duration::from_secs(30),
            max_history_points: 1000,
        }
    }
}

/// Alert severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    /// Informational alert
    Info,

    /// Warning that requires attention
    Warning,

    /// Critical issue requiring immediate action
    Critical,
}

/// Pool monitoring alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolAlert {
    /// Alert severity level
    pub level: AlertLevel,

    /// Alert message
    pub message: String,

    /// Current metric value that triggered the alert
    pub current_value: f64,

    /// Threshold value
    pub threshold: f64,

    /// When the alert was triggered
    pub triggered_at: DateTime<Utc>,

    /// Recommended action
    pub recommendation: String,
}

/// Pool metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetricsSnapshot {
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,

    /// Total connections in pool
    pub total_connections: u32,

    /// Active connections
    pub active_connections: u32,

    /// Idle connections
    pub idle_connections: u32,

    /// Pool utilization ratio (0.0-1.0)
    pub utilization: f64,

    /// Average connection acquisition time (ms) - estimated
    pub avg_acquisition_time_ms: f64,

    /// Number of connection timeouts
    pub timeouts: u64,
}

impl PoolMetricsSnapshot {
    /// Create a new metrics snapshot from pool
    pub fn from_pool(pool: &PgPool) -> Self {
        let size = pool.size();
        let idle = pool.num_idle() as u32;
        let active = size.saturating_sub(idle);
        let max_size = pool.options().get_max_connections();

        let utilization = if max_size > 0 {
            size as f64 / max_size as f64
        } else {
            0.0
        };

        Self {
            timestamp: Utc::now(),
            total_connections: size,
            active_connections: active,
            idle_connections: idle,
            utilization,
            avg_acquisition_time_ms: 0.0, // Would need tracking layer to measure this
            timeouts: 0,
        }
    }

    /// Check if this snapshot indicates high load
    pub fn is_high_load(&self, threshold: f64) -> bool {
        self.utilization >= threshold
    }

    /// Check if pool is saturated (no idle connections)
    pub fn is_saturated(&self) -> bool {
        self.idle_connections == 0
    }
}

/// Pool health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolHealth {
    /// Pool is healthy
    Healthy,

    /// Pool is experiencing high load
    HighLoad,

    /// Pool is critically loaded
    Critical,

    /// Pool is saturated
    Saturated,
}

impl PoolHealth {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Healthy => "Pool is operating normally",
            Self::HighLoad => "Pool utilization is high",
            Self::Critical => "Pool utilization is critically high",
            Self::Saturated => "Pool is fully saturated",
        }
    }
}

/// Pool monitoring report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,

    /// Current pool health status
    pub health: PoolHealth,

    /// Current metrics snapshot
    pub current_metrics: PoolMetricsSnapshot,

    /// Active alerts
    pub alerts: Vec<PoolAlert>,

    /// Historical metrics (recent)
    pub history: Vec<PoolMetricsSnapshot>,

    /// Capacity statistics
    pub capacity_stats: CapacityStats,
}

/// Capacity statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityStats {
    /// Average utilization over history
    pub avg_utilization: f64,

    /// Peak utilization
    pub peak_utilization: f64,

    /// Time of peak utilization
    pub peak_at: Option<DateTime<Utc>>,

    /// Utilization trend (positive = increasing, negative = decreasing)
    pub trend: f64,

    /// Estimated time until capacity exhaustion (if trend continues)
    pub time_to_exhaustion: Option<Duration>,
}

/// Connection pool monitor
pub struct PoolMonitor {
    config: MonitorConfig,
    history: Arc<Mutex<VecDeque<PoolMetricsSnapshot>>>,
}

impl PoolMonitor {
    /// Create a new pool monitor
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            history: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Create a monitor with default configuration
    pub fn with_defaults() -> Self {
        Self::new(MonitorConfig::default())
    }

    /// Collect current metrics and update history
    pub fn collect_metrics(&self, pool: &PgPool) -> PoolMetricsSnapshot {
        let snapshot = PoolMetricsSnapshot::from_pool(pool);

        if let Ok(mut history) = self.history.lock() {
            history.push_back(snapshot.clone());

            // Keep history within limits
            while history.len() > self.config.max_history_points {
                history.pop_front();
            }
        }

        debug!(
            utilization = snapshot.utilization,
            active = snapshot.active_connections,
            idle = snapshot.idle_connections,
            "Collected pool metrics"
        );

        snapshot
    }

    /// Generate monitoring report with alerts
    pub fn generate_report(&self, pool: &PgPool) -> MonitoringReport {
        let current_metrics = self.collect_metrics(pool);
        let history = self.get_history();

        let health = self.determine_health(&current_metrics);
        let alerts = self.check_alerts(&current_metrics, &history);
        let capacity_stats = self.calculate_capacity_stats(&history);

        info!(
            health = ?health,
            alerts = alerts.len(),
            utilization = current_metrics.utilization,
            "Generated monitoring report"
        );

        MonitoringReport {
            generated_at: Utc::now(),
            health,
            current_metrics,
            alerts,
            history,
            capacity_stats,
        }
    }

    /// Determine pool health status
    fn determine_health(&self, metrics: &PoolMetricsSnapshot) -> PoolHealth {
        if metrics.is_saturated() {
            PoolHealth::Saturated
        } else if metrics.utilization >= self.config.critical_utilization_threshold {
            PoolHealth::Critical
        } else if metrics.utilization >= self.config.high_utilization_threshold {
            PoolHealth::HighLoad
        } else {
            PoolHealth::Healthy
        }
    }

    /// Check for alert conditions
    fn check_alerts(
        &self,
        current: &PoolMetricsSnapshot,
        history: &[PoolMetricsSnapshot],
    ) -> Vec<PoolAlert> {
        let mut alerts = Vec::new();

        // High utilization alert
        if current.utilization >= self.config.high_utilization_threshold {
            let level = if current.utilization >= self.config.critical_utilization_threshold {
                AlertLevel::Critical
            } else {
                AlertLevel::Warning
            };

            alerts.push(PoolAlert {
                level,
                message: "High pool utilization detected".to_string(),
                current_value: current.utilization,
                threshold: self.config.high_utilization_threshold,
                triggered_at: Utc::now(),
                recommendation:
                    "Consider increasing max_connections or optimizing query performance"
                        .to_string(),
            });
        }

        // Pool saturation alert
        if current.is_saturated() {
            alerts.push(PoolAlert {
                level: AlertLevel::Critical,
                message: "Pool is fully saturated - no idle connections available".to_string(),
                current_value: 0.0,
                threshold: 1.0,
                triggered_at: Utc::now(),
                recommendation:
                    "Immediate action required: increase pool size or reduce concurrent connections"
                        .to_string(),
            });
        }

        // Rapid utilization increase alert
        if history.len() >= 3 {
            let recent_avg = history
                .iter()
                .rev()
                .take(3)
                .map(|s| s.utilization)
                .sum::<f64>()
                / 3.0;

            if current.utilization > recent_avg + 0.2 {
                alerts.push(PoolAlert {
                    level: AlertLevel::Warning,
                    message: "Rapid increase in pool utilization detected".to_string(),
                    current_value: current.utilization,
                    threshold: recent_avg,
                    triggered_at: Utc::now(),
                    recommendation: "Monitor for potential traffic spike or connection leak"
                        .to_string(),
                });
            }
        }

        alerts
    }

    /// Calculate capacity statistics from history
    fn calculate_capacity_stats(&self, history: &[PoolMetricsSnapshot]) -> CapacityStats {
        if history.is_empty() {
            return CapacityStats {
                avg_utilization: 0.0,
                peak_utilization: 0.0,
                peak_at: None,
                trend: 0.0,
                time_to_exhaustion: None,
            };
        }

        let avg_utilization =
            history.iter().map(|s| s.utilization).sum::<f64>() / history.len() as f64;

        let peak = history
            .iter()
            .max_by(|a, b| a.utilization.partial_cmp(&b.utilization).unwrap_or(std::cmp::Ordering::Equal))
            .expect("history is non-empty per len check above");

        let peak_utilization = peak.utilization;
        let peak_at = Some(peak.timestamp);

        // Calculate trend using simple linear regression
        let trend = self.calculate_trend(history);

        // Estimate time to exhaustion if trend is positive
        let time_to_exhaustion = if trend > 0.0 {
            let current_utilization = history.last().map(|s| s.utilization).unwrap_or(0.0);
            let remaining = 1.0 - current_utilization;
            if remaining > 0.0 {
                let hours = remaining / (trend * 24.0); // trend is per day, convert to hours
                Some(Duration::from_secs((hours * 3600.0) as u64))
            } else {
                None
            }
        } else {
            None
        };

        CapacityStats {
            avg_utilization,
            peak_utilization,
            peak_at,
            trend,
            time_to_exhaustion,
        }
    }

    /// Calculate utilization trend (change per day)
    fn calculate_trend(&self, history: &[PoolMetricsSnapshot]) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let first = &history[0];
        let last = history.last().expect("history has >= 2 elements per len check above");

        let time_diff = last.timestamp.signed_duration_since(first.timestamp);
        let days = time_diff.num_seconds() as f64 / 86400.0;

        if days > 0.0 {
            (last.utilization - first.utilization) / days
        } else {
            0.0
        }
    }

    /// Get historical metrics
    pub fn get_history(&self) -> Vec<PoolMetricsSnapshot> {
        self.history
            .lock()
            .ok()
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear historical data
    pub fn clear_history(&self) {
        if let Ok(mut history) = self.history.lock() {
            history.clear();
        }
    }

    /// Get the number of historical data points
    pub fn history_count(&self) -> usize {
        self.history.lock().ok().map(|h| h.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_config_default() {
        let config = MonitorConfig::default();
        assert_eq!(config.high_utilization_threshold, 0.8);
        assert_eq!(config.critical_utilization_threshold, 0.95);
        assert_eq!(config.slow_acquisition_threshold_ms, 1000);
    }

    #[test]
    fn test_alert_level_ordering() {
        assert_ne!(AlertLevel::Info, AlertLevel::Warning);
        assert_ne!(AlertLevel::Warning, AlertLevel::Critical);
    }

    #[test]
    fn test_pool_health_description() {
        assert_eq!(
            PoolHealth::Healthy.description(),
            "Pool is operating normally"
        );
        assert_eq!(
            PoolHealth::Critical.description(),
            "Pool utilization is critically high"
        );
    }

    #[test]
    fn test_metrics_snapshot_high_load() {
        let snapshot = PoolMetricsSnapshot {
            timestamp: Utc::now(),
            total_connections: 8,
            active_connections: 7,
            idle_connections: 1,
            utilization: 0.85,
            avg_acquisition_time_ms: 100.0,
            timeouts: 0,
        };

        assert!(snapshot.is_high_load(0.8));
        assert!(!snapshot.is_high_load(0.9));
    }

    #[test]
    fn test_metrics_snapshot_saturated() {
        let snapshot = PoolMetricsSnapshot {
            timestamp: Utc::now(),
            total_connections: 10,
            active_connections: 10,
            idle_connections: 0,
            utilization: 1.0,
            avg_acquisition_time_ms: 500.0,
            timeouts: 5,
        };

        assert!(snapshot.is_saturated());
    }

    #[test]
    fn test_pool_alert_serialization() {
        let alert = PoolAlert {
            level: AlertLevel::Critical,
            message: "Pool exhausted".to_string(),
            current_value: 1.0,
            threshold: 0.95,
            triggered_at: Utc::now(),
            recommendation: "Increase pool size".to_string(),
        };

        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("Critical"));
        assert!(json.contains("Pool exhausted"));
    }

    #[test]
    fn test_monitor_with_defaults() {
        let monitor = PoolMonitor::with_defaults();
        assert_eq!(monitor.history_count(), 0);
    }

    #[test]
    fn test_monitor_clear_history() {
        let monitor = PoolMonitor::with_defaults();
        monitor.clear_history();
        assert_eq!(monitor.history_count(), 0);
    }

    #[test]
    fn test_capacity_stats_serialization() {
        let stats = CapacityStats {
            avg_utilization: 0.7,
            peak_utilization: 0.95,
            peak_at: Some(Utc::now()),
            trend: 0.05,
            time_to_exhaustion: Some(Duration::from_secs(3600)),
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("avg_utilization"));
        assert!(json.contains("peak_utilization"));
    }

    #[test]
    fn test_monitoring_report_structure() {
        let snapshot = PoolMetricsSnapshot {
            timestamp: Utc::now(),
            total_connections: 5,
            active_connections: 3,
            idle_connections: 2,
            utilization: 0.5,
            avg_acquisition_time_ms: 50.0,
            timeouts: 0,
        };

        let report = MonitoringReport {
            generated_at: Utc::now(),
            health: PoolHealth::Healthy,
            current_metrics: snapshot,
            alerts: vec![],
            history: vec![],
            capacity_stats: CapacityStats {
                avg_utilization: 0.5,
                peak_utilization: 0.7,
                peak_at: None,
                trend: 0.0,
                time_to_exhaustion: None,
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("Healthy"));
    }
}
