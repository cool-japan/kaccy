//! Connection Leak Detector
//!
//! Monitors database connections to detect and report potential connection leaks.
//! Connection leaks occur when connections are acquired but not properly returned
//! to the pool, leading to pool exhaustion and application failures.
//!
//! # Features
//!
//! - Track connection acquisition and release
//! - Detect long-held connections (potential leaks)
//! - Generate leak reports with stack traces
//! - Automatic cleanup of stale connections
//! - Configurable thresholds and monitoring intervals
//!
//! # Example
//!
//! ```rust
//! use kaccy_db::connection_leak_detector::{LeakDetector, LeakDetectorConfig};
//! use std::time::Duration;
//!
//! let config = LeakDetectorConfig {
//!     leak_threshold: Duration::from_secs(300), // 5 minutes
//!     warning_threshold: Duration::from_secs(60), // 1 minute
//!     check_interval: Duration::from_secs(30),
//!     max_tracked_connections: 10000,
//! };
//!
//! let detector = LeakDetector::new(config);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, warn};

/// Configuration for the connection leak detector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakDetectorConfig {
    /// Duration after which a connection is considered leaked
    pub leak_threshold: Duration,

    /// Duration after which a connection generates a warning
    pub warning_threshold: Duration,

    /// Interval between leak detection checks
    pub check_interval: Duration,

    /// Maximum number of connections to track (prevents memory issues)
    pub max_tracked_connections: usize,
}

impl Default for LeakDetectorConfig {
    fn default() -> Self {
        Self {
            leak_threshold: Duration::from_secs(300),   // 5 minutes
            warning_threshold: Duration::from_secs(60), // 1 minute
            check_interval: Duration::from_secs(30),    // 30 seconds
            max_tracked_connections: 10000,
        }
    }
}

/// Information about a tracked connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedConnection {
    /// Unique connection ID
    pub connection_id: u64,

    /// When the connection was acquired
    pub acquired_at: DateTime<Utc>,

    /// Caller information (file, line, function)
    pub caller_info: String,

    /// Thread that acquired the connection
    pub thread_id: String,

    /// Optional operation description
    pub operation: Option<String>,
}

impl TrackedConnection {
    /// Get the age of this connection
    pub fn age(&self) -> Duration {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.acquired_at);
        Duration::from_millis(elapsed.num_milliseconds().max(0) as u64)
    }

    /// Check if this connection has exceeded the leak threshold
    pub fn is_leaked(&self, threshold: Duration) -> bool {
        self.age() >= threshold
    }

    /// Check if this connection should generate a warning
    pub fn is_warning(&self, threshold: Duration) -> bool {
        self.age() >= threshold
    }
}

/// Report of detected connection leaks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakReport {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,

    /// Connections that are considered leaked
    pub leaked_connections: Vec<TrackedConnection>,

    /// Connections that generated warnings
    pub warning_connections: Vec<TrackedConnection>,

    /// Total number of connections currently tracked
    pub total_tracked: usize,

    /// Summary statistics
    pub stats: LeakStats,
}

/// Statistics about connection leaks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakStats {
    /// Number of leaked connections detected
    pub leak_count: usize,

    /// Number of warnings generated
    pub warning_count: usize,

    /// Oldest connection age in seconds
    pub oldest_connection_age_secs: u64,

    /// Average connection age in seconds
    pub average_connection_age_secs: u64,
}

/// Connection leak detector
pub struct LeakDetector {
    config: LeakDetectorConfig,
    connections: Arc<Mutex<HashMap<u64, TrackedConnection>>>,
    next_id: Arc<Mutex<u64>>,
}

impl LeakDetector {
    /// Create a new leak detector with the given configuration
    pub fn new(config: LeakDetectorConfig) -> Self {
        Self {
            config,
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Create a leak detector with default configuration
    pub fn with_defaults() -> Self {
        Self::new(LeakDetectorConfig::default())
    }

    /// Track a new connection acquisition
    ///
    /// Returns a connection ID that should be used to release the connection
    pub fn track_acquisition(&self, caller_info: String, operation: Option<String>) -> Option<u64> {
        let mut connections = self.connections.lock().ok()?;

        // Check if we've hit the tracking limit
        if connections.len() >= self.config.max_tracked_connections {
            warn!(
                "Connection tracking limit reached ({}), not tracking new connection",
                self.config.max_tracked_connections
            );
            return None;
        }

        let mut next_id = self.next_id.lock().ok()?;
        let connection_id = *next_id;
        *next_id += 1;

        let thread_id = format!("{:?}", std::thread::current().id());

        let tracked = TrackedConnection {
            connection_id,
            acquired_at: Utc::now(),
            caller_info,
            thread_id,
            operation,
        };

        debug!(
            connection_id = connection_id,
            caller = tracked.caller_info,
            "Tracking connection acquisition"
        );

        connections.insert(connection_id, tracked);

        Some(connection_id)
    }

    /// Release a tracked connection
    pub fn release_connection(&self, connection_id: u64) -> bool {
        if let Ok(mut connections) = self.connections.lock() {
            if let Some(tracked) = connections.remove(&connection_id) {
                debug!(
                    connection_id = connection_id,
                    age_secs = tracked.age().as_secs(),
                    "Released connection"
                );
                return true;
            }
        }
        false
    }

    /// Check for connection leaks and generate a report
    pub fn check_for_leaks(&self) -> Option<LeakReport> {
        let connections = self.connections.lock().ok()?;

        let mut leaked_connections = Vec::new();
        let mut warning_connections = Vec::new();
        let mut total_age_secs = 0u64;
        let mut oldest_age_secs = 0u64;

        for conn in connections.values() {
            let age = conn.age();
            let age_secs = age.as_secs();

            total_age_secs += age_secs;
            oldest_age_secs = oldest_age_secs.max(age_secs);

            if conn.is_leaked(self.config.leak_threshold) {
                warn!(
                    connection_id = conn.connection_id,
                    age_secs = age_secs,
                    caller = conn.caller_info,
                    "Connection leak detected"
                );
                leaked_connections.push(conn.clone());
            } else if conn.is_warning(self.config.warning_threshold) {
                warn!(
                    connection_id = conn.connection_id,
                    age_secs = age_secs,
                    caller = conn.caller_info,
                    "Long-held connection detected"
                );
                warning_connections.push(conn.clone());
            }
        }

        let total_tracked = connections.len();
        let average_age_secs = if total_tracked > 0 {
            total_age_secs / total_tracked as u64
        } else {
            0
        };

        let leak_count = leaked_connections.len();
        let warning_count = warning_connections.len();

        Some(LeakReport {
            generated_at: Utc::now(),
            leaked_connections,
            warning_connections,
            total_tracked,
            stats: LeakStats {
                leak_count,
                warning_count,
                oldest_connection_age_secs: oldest_age_secs,
                average_connection_age_secs: average_age_secs,
            },
        })
    }

    /// Get all currently tracked connections
    pub fn get_tracked_connections(&self) -> Vec<TrackedConnection> {
        self.connections
            .lock()
            .ok()
            .map(|conns| conns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of currently tracked connections
    pub fn connection_count(&self) -> usize {
        self.connections
            .lock()
            .ok()
            .map(|conns| conns.len())
            .unwrap_or(0)
    }

    /// Clear all tracked connections (use with caution)
    pub fn clear_all(&self) {
        if let Ok(mut connections) = self.connections.lock() {
            connections.clear();
        }
    }

    /// Force cleanup of connections older than the leak threshold
    ///
    /// Returns the number of connections cleaned up
    pub fn cleanup_leaked_connections(&self) -> usize {
        let mut connections = match self.connections.lock() {
            Ok(conns) => conns,
            Err(_) => return 0,
        };

        let leaked_ids: Vec<u64> = connections
            .iter()
            .filter(|(_, conn)| conn.is_leaked(self.config.leak_threshold))
            .map(|(id, _)| *id)
            .collect();

        let count = leaked_ids.len();

        for id in leaked_ids {
            connections.remove(&id);
            warn!(
                connection_id = id,
                "Forcefully cleaned up leaked connection"
            );
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_leak_detector_config_default() {
        let config = LeakDetectorConfig::default();
        assert_eq!(config.leak_threshold, Duration::from_secs(300));
        assert_eq!(config.warning_threshold, Duration::from_secs(60));
        assert_eq!(config.check_interval, Duration::from_secs(30));
        assert_eq!(config.max_tracked_connections, 10000);
    }

    #[test]
    fn test_track_and_release_connection() {
        let detector = LeakDetector::with_defaults();

        let conn_id = detector
            .track_acquisition("test.rs:123".to_string(), Some("SELECT".to_string()))
            .unwrap();

        assert_eq!(detector.connection_count(), 1);

        let released = detector.release_connection(conn_id);
        assert!(released);
        assert_eq!(detector.connection_count(), 0);
    }

    #[test]
    fn test_connection_age() {
        let conn = TrackedConnection {
            connection_id: 1,
            acquired_at: Utc::now() - chrono::Duration::seconds(120),
            caller_info: "test.rs:1".to_string(),
            thread_id: "1".to_string(),
            operation: None,
        };

        let age = conn.age();
        assert!(age.as_secs() >= 120);
    }

    #[test]
    fn test_leak_detection() {
        let config = LeakDetectorConfig {
            leak_threshold: Duration::from_millis(50),
            warning_threshold: Duration::from_millis(25),
            ..Default::default()
        };

        let detector = LeakDetector::new(config);

        detector.track_acquisition("test.rs:1".to_string(), None);

        // Wait for leak threshold
        thread::sleep(Duration::from_millis(100));

        let report = detector.check_for_leaks().unwrap();
        assert_eq!(report.stats.leak_count, 1);
    }

    #[test]
    fn test_warning_detection() {
        let config = LeakDetectorConfig {
            leak_threshold: Duration::from_secs(1000),
            warning_threshold: Duration::from_millis(50),
            ..Default::default()
        };

        let detector = LeakDetector::new(config);

        detector.track_acquisition("test.rs:1".to_string(), None);

        // Wait for warning threshold but not leak threshold
        thread::sleep(Duration::from_millis(100));

        let report = detector.check_for_leaks().unwrap();
        assert_eq!(report.stats.leak_count, 0);
        assert_eq!(report.stats.warning_count, 1);
    }

    #[test]
    fn test_cleanup_leaked_connections() {
        let config = LeakDetectorConfig {
            leak_threshold: Duration::from_millis(50),
            ..Default::default()
        };

        let detector = LeakDetector::new(config);

        detector.track_acquisition("test.rs:1".to_string(), None);
        detector.track_acquisition("test.rs:2".to_string(), None);

        assert_eq!(detector.connection_count(), 2);

        // Wait for leak threshold
        thread::sleep(Duration::from_millis(100));

        let cleaned = detector.cleanup_leaked_connections();
        assert_eq!(cleaned, 2);
        assert_eq!(detector.connection_count(), 0);
    }

    #[test]
    fn test_get_tracked_connections() {
        let detector = LeakDetector::with_defaults();

        detector.track_acquisition("test.rs:1".to_string(), Some("SELECT 1".to_string()));
        detector.track_acquisition("test.rs:2".to_string(), Some("SELECT 2".to_string()));

        let connections = detector.get_tracked_connections();
        assert_eq!(connections.len(), 2);
    }

    #[test]
    fn test_clear_all() {
        let detector = LeakDetector::with_defaults();

        detector.track_acquisition("test.rs:1".to_string(), None);
        detector.track_acquisition("test.rs:2".to_string(), None);

        assert_eq!(detector.connection_count(), 2);

        detector.clear_all();
        assert_eq!(detector.connection_count(), 0);
    }

    #[test]
    fn test_max_tracked_connections() {
        let config = LeakDetectorConfig {
            max_tracked_connections: 2,
            ..Default::default()
        };

        let detector = LeakDetector::new(config);

        assert!(detector
            .track_acquisition("test.rs:1".to_string(), None)
            .is_some());
        assert!(detector
            .track_acquisition("test.rs:2".to_string(), None)
            .is_some());
        assert!(detector
            .track_acquisition("test.rs:3".to_string(), None)
            .is_none());

        assert_eq!(detector.connection_count(), 2);
    }

    #[test]
    fn test_leak_report_serialization() {
        let report = LeakReport {
            generated_at: Utc::now(),
            leaked_connections: vec![],
            warning_connections: vec![],
            total_tracked: 5,
            stats: LeakStats {
                leak_count: 0,
                warning_count: 1,
                oldest_connection_age_secs: 120,
                average_connection_age_secs: 45,
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("total_tracked"));
    }

    #[test]
    fn test_leak_stats() {
        let config = LeakDetectorConfig {
            leak_threshold: Duration::from_secs(10),
            warning_threshold: Duration::from_secs(5),
            ..Default::default()
        };

        let detector = LeakDetector::new(config);

        detector.track_acquisition("test.rs:1".to_string(), None);
        thread::sleep(Duration::from_secs(1));
        detector.track_acquisition("test.rs:2".to_string(), None);
        thread::sleep(Duration::from_secs(1));

        let report = detector.check_for_leaks().unwrap();
        assert!(report.stats.oldest_connection_age_secs >= 2);
        assert!(report.stats.average_connection_age_secs >= 1);
    }
}
