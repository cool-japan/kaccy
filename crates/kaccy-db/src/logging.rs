//! Enhanced structured logging for database operations.
//!
//! This module provides:
//! - Query context in logs
//! - User context propagation
//! - Performance anomaly detection
//! - Structured log fields for better observability

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Log level for database operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
}

/// Context for database operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbContext {
    /// User ID (if applicable)
    pub user_id: Option<Uuid>,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Additional custom fields
    pub custom_fields: HashMap<String, String>,
}

impl DbContext {
    /// Create a new context with user ID
    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add request ID
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Add session ID
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add IP address
    pub fn with_ip_address(mut self, ip_address: String) -> Self {
        self.ip_address = Some(ip_address);
        self
    }

    /// Add user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Add custom field
    pub fn add_field(mut self, key: String, value: String) -> Self {
        self.custom_fields.insert(key, value);
        self
    }
}

/// Query log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryLogEntry {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Query operation (SELECT, INSERT, UPDATE, DELETE)
    pub operation: String,
    /// Table name
    pub table: String,
    /// Query duration in milliseconds
    pub duration_ms: u64,
    /// Number of rows affected
    pub rows_affected: Option<u64>,
    /// Whether the query succeeded
    pub success: bool,
    /// Error message (if any)
    pub error: Option<String>,
    /// Database context
    pub context: DbContext,
    /// SQL statement (sanitized)
    pub sql: Option<String>,
}

impl QueryLogEntry {
    /// Create a new query log entry
    pub fn new(operation: String, table: String, duration_ms: u64) -> Self {
        Self {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            operation,
            table,
            duration_ms,
            rows_affected: None,
            success: true,
            error: None,
            context: DbContext::default(),
            sql: None,
        }
    }

    /// Set the log level
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Set rows affected
    pub fn with_rows_affected(mut self, rows: u64) -> Self {
        self.rows_affected = Some(rows);
        self
    }

    /// Mark as failed
    pub fn with_error(mut self, error: String) -> Self {
        self.success = false;
        self.error = Some(error);
        self.level = LogLevel::Error;
        self
    }

    /// Set context
    pub fn with_context(mut self, context: DbContext) -> Self {
        self.context = context;
        self
    }

    /// Set SQL statement
    pub fn with_sql(mut self, sql: String) -> Self {
        self.sql = Some(sql);
        self
    }

    /// Log the entry using tracing
    pub fn log(&self) {
        let user_id = self.context.user_id.map(|id| id.to_string());
        let request_id = self.context.request_id.as_deref();

        match self.level {
            LogLevel::Debug => {
                debug!(
                    operation = %self.operation,
                    table = %self.table,
                    duration_ms = self.duration_ms,
                    user_id = ?user_id,
                    request_id = ?request_id,
                    success = self.success,
                    "Database query executed"
                );
            }
            LogLevel::Info => {
                info!(
                    operation = %self.operation,
                    table = %self.table,
                    duration_ms = self.duration_ms,
                    user_id = ?user_id,
                    request_id = ?request_id,
                    success = self.success,
                    "Database query executed"
                );
            }
            LogLevel::Warn => {
                warn!(
                    operation = %self.operation,
                    table = %self.table,
                    duration_ms = self.duration_ms,
                    user_id = ?user_id,
                    request_id = ?request_id,
                    success = self.success,
                    error = ?self.error,
                    "Database query warning"
                );
            }
            LogLevel::Error => {
                error!(
                    operation = %self.operation,
                    table = %self.table,
                    duration_ms = self.duration_ms,
                    user_id = ?user_id,
                    request_id = ?request_id,
                    success = self.success,
                    error = ?self.error,
                    "Database query failed"
                );
            }
        }
    }
}

/// Performance anomaly detector
#[derive(Debug, Clone)]
pub struct AnomalyDetector {
    /// Baseline average duration in milliseconds
    baseline_avg_ms: f64,
    /// Standard deviation threshold for anomaly detection
    std_dev_threshold: f64,
    /// Minimum samples required for detection
    min_samples: usize,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self {
            baseline_avg_ms: 100.0,
            std_dev_threshold: 3.0,
            min_samples: 10,
        }
    }
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(baseline_avg_ms: f64, std_dev_threshold: f64) -> Self {
        Self {
            baseline_avg_ms,
            std_dev_threshold,
            min_samples: 10,
        }
    }

    /// Check if a duration is anomalous
    pub fn is_anomalous(&self, duration_ms: u64, samples: &[u64]) -> bool {
        if samples.len() < self.min_samples {
            // Not enough samples for statistical analysis
            return duration_ms as f64 > self.baseline_avg_ms * 2.0;
        }

        let mean = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
        let variance = samples
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / samples.len() as f64;
        let std_dev = variance.sqrt();

        duration_ms as f64 > mean + (self.std_dev_threshold * std_dev)
    }

    /// Update baseline with new samples
    pub fn update_baseline(&mut self, samples: &[u64]) {
        if samples.is_empty() {
            return;
        }

        self.baseline_avg_ms = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
    }
}

/// Performance statistics tracker
#[derive(Debug)]
pub struct PerformanceTracker {
    /// Query duration samples by operation and table
    samples: Arc<RwLock<HashMap<String, Vec<u64>>>>,
    /// Anomaly detector
    detector: AnomalyDetector,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new(AnomalyDetector::default())
    }
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new(detector: AnomalyDetector) -> Self {
        Self {
            samples: Arc::new(RwLock::new(HashMap::new())),
            detector,
        }
    }

    /// Record a query duration
    pub fn record(&self, operation: &str, table: &str, duration_ms: u64) {
        let key = format!("{}:{}", operation, table);
        let mut samples = self.samples.write();
        let entry = samples.entry(key).or_default();

        // Keep only last 100 samples to prevent unbounded growth
        if entry.len() >= 100 {
            entry.remove(0);
        }

        entry.push(duration_ms);
    }

    /// Check if a query is anomalous
    pub fn check_anomaly(&self, operation: &str, table: &str, duration_ms: u64) -> bool {
        let key = format!("{}:{}", operation, table);
        let samples = self.samples.read();

        if let Some(history) = samples.get(&key) {
            self.detector.is_anomalous(duration_ms, history)
        } else {
            false
        }
    }

    /// Get statistics for an operation
    pub fn get_stats(&self, operation: &str, table: &str) -> Option<QueryStats> {
        let key = format!("{}:{}", operation, table);
        let samples = self.samples.read();

        samples.get(&key).map(|history| {
            let count = history.len();
            let sum: u64 = history.iter().sum();
            let avg = if count > 0 { sum / count as u64 } else { 0 };
            let min = history.iter().min().copied().unwrap_or(0);
            let max = history.iter().max().copied().unwrap_or(0);

            QueryStats {
                operation: operation.to_string(),
                table: table.to_string(),
                count,
                avg_duration_ms: avg,
                min_duration_ms: min,
                max_duration_ms: max,
            }
        })
    }
}

/// Query statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStats {
    /// Operation type
    pub operation: String,
    /// Table name
    pub table: String,
    /// Number of queries
    pub count: usize,
    /// Average duration in milliseconds
    pub avg_duration_ms: u64,
    /// Minimum duration in milliseconds
    pub min_duration_ms: u64,
    /// Maximum duration in milliseconds
    pub max_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_context_builder() {
        let user_id = Uuid::new_v4();
        let context = DbContext::default()
            .with_user_id(user_id)
            .with_request_id("req-123".to_string())
            .with_session_id("sess-456".to_string())
            .with_ip_address("127.0.0.1".to_string())
            .with_user_agent("test-agent".to_string())
            .add_field("custom".to_string(), "value".to_string());

        assert_eq!(context.user_id, Some(user_id));
        assert_eq!(context.request_id, Some("req-123".to_string()));
        assert_eq!(context.session_id, Some("sess-456".to_string()));
        assert_eq!(context.ip_address, Some("127.0.0.1".to_string()));
        assert_eq!(context.user_agent, Some("test-agent".to_string()));
        assert_eq!(
            context.custom_fields.get("custom"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_query_log_entry() {
        let entry = QueryLogEntry::new("SELECT".to_string(), "users".to_string(), 50)
            .with_rows_affected(10)
            .with_sql("SELECT * FROM users".to_string());

        assert_eq!(entry.operation, "SELECT");
        assert_eq!(entry.table, "users");
        assert_eq!(entry.duration_ms, 50);
        assert_eq!(entry.rows_affected, Some(10));
        assert!(entry.success);
        assert_eq!(entry.sql, Some("SELECT * FROM users".to_string()));
    }

    #[test]
    fn test_query_log_entry_with_error() {
        let entry = QueryLogEntry::new("INSERT".to_string(), "users".to_string(), 100)
            .with_error("Constraint violation".to_string());

        assert!(!entry.success);
        assert_eq!(entry.error, Some("Constraint violation".to_string()));
        assert_eq!(entry.level, LogLevel::Error);
    }

    #[test]
    fn test_anomaly_detector_with_insufficient_samples() {
        let detector = AnomalyDetector::default();
        let samples = vec![100, 110, 105];

        // With few samples, uses simple threshold
        assert!(!detector.is_anomalous(150, &samples));
        assert!(detector.is_anomalous(300, &samples));
    }

    #[test]
    fn test_anomaly_detector_with_sufficient_samples() {
        let detector = AnomalyDetector::new(100.0, 3.0);
        let samples: Vec<u64> = vec![95, 100, 105, 98, 102, 99, 101, 103, 97, 100, 104, 96];

        // Normal value
        assert!(!detector.is_anomalous(105, &samples));

        // Anomalous value (more than 3 std devs from mean)
        assert!(detector.is_anomalous(500, &samples));
    }

    #[test]
    fn test_anomaly_detector_update_baseline() {
        let mut detector = AnomalyDetector::default();
        let samples = vec![200, 210, 205, 215];

        detector.update_baseline(&samples);
        assert!((detector.baseline_avg_ms - 207.5).abs() < 0.1);
    }

    #[test]
    fn test_performance_tracker_record() {
        let tracker = PerformanceTracker::default();

        tracker.record("SELECT", "users", 100);
        tracker.record("SELECT", "users", 110);
        tracker.record("SELECT", "users", 105);

        let stats = tracker.get_stats("SELECT", "users").unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_duration_ms, 100);
        assert_eq!(stats.max_duration_ms, 110);
    }

    #[test]
    fn test_performance_tracker_check_anomaly() {
        let tracker = PerformanceTracker::default();

        // Record baseline samples with some variance
        for i in 0..20 {
            tracker.record("SELECT", "users", 95 + (i % 10) as u64);
        }

        // Normal query (within 3 std devs)
        assert!(!tracker.check_anomaly("SELECT", "users", 105));

        // Anomalous query (far beyond 3 std devs)
        assert!(tracker.check_anomaly("SELECT", "users", 1000));
    }

    #[test]
    fn test_performance_tracker_max_samples() {
        let tracker = PerformanceTracker::default();

        // Record more than 100 samples
        for i in 0..150 {
            tracker.record("SELECT", "users", i);
        }

        let stats = tracker.get_stats("SELECT", "users").unwrap();
        assert_eq!(stats.count, 100); // Should only keep last 100
    }

    #[test]
    fn test_query_log_entry_log() {
        let entry = QueryLogEntry::new("SELECT".to_string(), "users".to_string(), 50);

        // Should not panic
        entry.log();
    }
}
