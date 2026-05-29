//! Query logging and slow query detection
//!
//! Provides middleware for tracking SQL query performance
//! and detecting slow queries.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::Serialize;
use tracing::{info, warn};

/// Configuration for query logging
#[derive(Debug, Clone)]
pub struct QueryLogConfig {
    /// Threshold for slow query warnings (milliseconds)
    pub slow_query_threshold_ms: u64,
    /// Whether to log all queries (not just slow ones)
    pub log_all_queries: bool,
    /// Whether to capture query statistics
    pub enable_stats: bool,
    /// Maximum number of slow queries to retain in history
    pub max_slow_query_history: usize,
}

impl Default for QueryLogConfig {
    fn default() -> Self {
        Self {
            slow_query_threshold_ms: 100, // 100ms default
            log_all_queries: false,
            enable_stats: true,
            max_slow_query_history: 1000,
        }
    }
}

impl QueryLogConfig {
    /// Development configuration (verbose logging)
    pub fn development() -> Self {
        Self {
            slow_query_threshold_ms: 50,
            log_all_queries: true,
            enable_stats: true,
            max_slow_query_history: 100,
        }
    }

    /// Production configuration (minimal logging)
    pub fn production() -> Self {
        Self {
            slow_query_threshold_ms: 200,
            log_all_queries: false,
            enable_stats: true,
            max_slow_query_history: 500,
        }
    }
}

/// Record of a slow query
#[derive(Debug, Clone, Serialize)]
pub struct SlowQueryRecord {
    /// Hash identifying the normalized query pattern.
    pub query_hash: u64,
    /// Truncated preview of the query SQL.
    pub query_preview: String,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Timestamp when the slow query was executed.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Optional source location (file/line) where the query was issued.
    pub location: Option<String>,
}

/// Statistics for a specific query pattern
#[derive(Debug, Clone, Serialize)]
pub struct QueryStats {
    /// Hash identifying the normalized query pattern.
    pub query_hash: u64,
    /// Truncated preview of the query SQL.
    pub query_preview: String,
    /// Total number of times this query was executed.
    pub call_count: u64,
    /// Sum of all execution durations in milliseconds.
    pub total_duration_ms: u64,
    /// Average execution duration in milliseconds.
    pub avg_duration_ms: f64,
    /// Maximum recorded execution duration in milliseconds.
    pub max_duration_ms: u64,
    /// Minimum recorded execution duration in milliseconds.
    pub min_duration_ms: u64,
    /// Number of executions that exceeded the slow query threshold.
    pub slow_count: u64,
}

/// Aggregate statistics
#[derive(Debug, Clone, Serialize)]
pub struct AggregateStats {
    /// Total number of queries executed since startup.
    pub total_queries: u64,
    /// Total number of queries that exceeded the slow-query threshold.
    pub total_slow_queries: u64,
    /// Cumulative execution time of all queries in milliseconds.
    pub total_duration_ms: u64,
    /// Mean query duration in milliseconds.
    pub avg_duration_ms: f64,
    /// Observed query throughput (queries per second).
    pub queries_per_second: f64,
    /// Number of seconds the logger has been running.
    pub uptime_seconds: u64,
}

/// Query logger service
pub struct QueryLogger {
    config: QueryLogConfig,
    stats: Arc<RwLock<HashMap<u64, QueryStatsInner>>>,
    slow_queries: Arc<RwLock<Vec<SlowQueryRecord>>>,
    total_queries: AtomicU64,
    total_slow: AtomicU64,
    total_duration_ms: AtomicU64,
    start_time: Instant,
}

#[derive(Debug, Clone)]
struct QueryStatsInner {
    query_preview: String,
    call_count: u64,
    total_duration_ms: u64,
    max_duration_ms: u64,
    min_duration_ms: u64,
    slow_count: u64,
}

impl QueryLogger {
    /// Create a new `QueryLogger` with the provided configuration.
    pub fn new(config: QueryLogConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(HashMap::new())),
            slow_queries: Arc::new(RwLock::new(Vec::new())),
            total_queries: AtomicU64::new(0),
            total_slow: AtomicU64::new(0),
            total_duration_ms: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// Create a new `QueryLogger` using default configuration values.
    pub fn with_default_config() -> Self {
        Self::new(QueryLogConfig::default())
    }

    /// Log a query execution
    pub fn log_query(&self, query: &str, duration: Duration, location: Option<&str>) {
        let duration_ms = duration.as_millis() as u64;
        let query_hash = hash_query(query);
        let query_preview = truncate_query(query, 200);

        // Update totals
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        self.total_duration_ms
            .fetch_add(duration_ms, Ordering::Relaxed);

        let is_slow = duration_ms >= self.config.slow_query_threshold_ms;

        if is_slow {
            self.total_slow.fetch_add(1, Ordering::Relaxed);

            warn!(
                query = %query_preview,
                duration_ms = duration_ms,
                location = ?location,
                "Slow query detected"
            );

            // Record slow query
            let mut slow_queries = self.slow_queries.write();
            slow_queries.push(SlowQueryRecord {
                query_hash,
                query_preview: query_preview.clone(),
                duration_ms,
                timestamp: chrono::Utc::now(),
                location: location.map(|s| s.to_string()),
            });

            // Trim history if needed
            if slow_queries.len() > self.config.max_slow_query_history {
                slow_queries.remove(0);
            }
        } else if self.config.log_all_queries {
            info!(
                query = %query_preview,
                duration_ms = duration_ms,
                "Query executed"
            );
        }

        // Update stats if enabled
        if self.config.enable_stats {
            let mut stats = self.stats.write();
            let entry = stats.entry(query_hash).or_insert_with(|| QueryStatsInner {
                query_preview,
                call_count: 0,
                total_duration_ms: 0,
                max_duration_ms: 0,
                min_duration_ms: u64::MAX,
                slow_count: 0,
            });

            entry.call_count += 1;
            entry.total_duration_ms += duration_ms;
            entry.max_duration_ms = entry.max_duration_ms.max(duration_ms);
            entry.min_duration_ms = entry.min_duration_ms.min(duration_ms);
            if is_slow {
                entry.slow_count += 1;
            }
        }
    }

    /// Get statistics for all queries
    pub fn get_stats(&self) -> Vec<QueryStats> {
        let stats = self.stats.read();
        stats
            .iter()
            .map(|(hash, inner)| QueryStats {
                query_hash: *hash,
                query_preview: inner.query_preview.clone(),
                call_count: inner.call_count,
                total_duration_ms: inner.total_duration_ms,
                avg_duration_ms: inner.total_duration_ms as f64 / inner.call_count.max(1) as f64,
                max_duration_ms: inner.max_duration_ms,
                min_duration_ms: if inner.min_duration_ms == u64::MAX {
                    0
                } else {
                    inner.min_duration_ms
                },
                slow_count: inner.slow_count,
            })
            .collect()
    }

    /// Get top N slowest query patterns
    pub fn get_slowest_queries(&self, n: usize) -> Vec<QueryStats> {
        let mut stats = self.get_stats();
        stats.sort_by(|a, b| {
            b.avg_duration_ms
                .partial_cmp(&a.avg_duration_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        stats.truncate(n);
        stats
    }

    /// Get top N most frequent queries
    pub fn get_most_frequent_queries(&self, n: usize) -> Vec<QueryStats> {
        let mut stats = self.get_stats();
        stats.sort_by_key(|b| std::cmp::Reverse(b.call_count));
        stats.truncate(n);
        stats
    }

    /// Get recent slow query records
    pub fn get_slow_query_history(&self, limit: usize) -> Vec<SlowQueryRecord> {
        let slow_queries = self.slow_queries.read();
        slow_queries.iter().rev().take(limit).cloned().collect()
    }

    /// Get aggregate statistics
    pub fn get_aggregate_stats(&self) -> AggregateStats {
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let total_slow = self.total_slow.load(Ordering::Relaxed);
        let total_duration = self.total_duration_ms.load(Ordering::Relaxed);
        let uptime = self.start_time.elapsed().as_secs();

        AggregateStats {
            total_queries,
            total_slow_queries: total_slow,
            total_duration_ms: total_duration,
            avg_duration_ms: total_duration as f64 / total_queries.max(1) as f64,
            queries_per_second: total_queries as f64 / uptime.max(1) as f64,
            uptime_seconds: uptime,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.write().clear();
        self.slow_queries.write().clear();
        self.total_queries.store(0, Ordering::Relaxed);
        self.total_slow.store(0, Ordering::Relaxed);
        self.total_duration_ms.store(0, Ordering::Relaxed);
    }

    /// Check if a duration is considered slow
    pub fn is_slow(&self, duration: Duration) -> bool {
        duration.as_millis() as u64 >= self.config.slow_query_threshold_ms
    }
}

/// Simple hash function for query strings
fn hash_query(query: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    // Normalize whitespace for consistent hashing
    let normalized: String = query.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.hash(&mut hasher);
    hasher.finish()
}

/// Truncate query string for display
fn truncate_query(query: &str, max_len: usize) -> String {
    let normalized: String = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= max_len {
        normalized
    } else {
        format!("{}...", &normalized[..max_len])
    }
}

/// Query timing guard - logs query duration when dropped
pub struct QueryTimer<'a> {
    logger: &'a QueryLogger,
    query: String,
    location: Option<String>,
    start: Instant,
}

impl<'a> QueryTimer<'a> {
    /// Create a new `QueryTimer` that will record timing to the given logger.
    pub fn new(logger: &'a QueryLogger, query: impl Into<String>) -> Self {
        Self {
            logger,
            query: query.into(),
            location: None,
            start: Instant::now(),
        }
    }

    /// Attach a source location label to be included in the log entry.
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Manually finish timing (called automatically on drop)
    pub fn finish(self) {
        // Drop will handle logging
    }
}

impl Drop for QueryTimer<'_> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        self.logger
            .log_query(&self.query, duration, self.location.as_deref());
    }
}

/// Start timing a query
#[inline]
pub fn time_query(logger: &QueryLogger, query: impl Into<String>) -> QueryTimer<'_> {
    QueryTimer::new(logger, query)
}
