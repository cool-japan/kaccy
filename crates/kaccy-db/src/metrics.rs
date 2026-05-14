//! Prometheus metrics for database operations
//!
//! Provides metrics for:
//! - Connection pool (active connections, wait time, utilization)
//! - Cache operations (hit rate, miss rate, latency)
//! - Query performance (duration, error rate, slow queries)
//! - Repository operations (calls, errors, latency)

use lazy_static::lazy_static;
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
    TextEncoder,
};
use std::time::Instant;

lazy_static! {
    /// Global Prometheus registry
    pub static ref REGISTRY: Registry = Registry::new();

    // ============================================================
    // Connection Pool Metrics
    // ============================================================

    /// Active database connections
    pub static ref POOL_CONNECTIONS_ACTIVE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_pool_connections_active", "Number of active database connections"),
        &["pool_name"]
    ).expect("metric constructor with valid static parameters");

    /// Idle database connections
    pub static ref POOL_CONNECTIONS_IDLE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_pool_connections_idle", "Number of idle database connections"),
        &["pool_name"]
    ).expect("metric constructor with valid static parameters");

    /// Total pool size
    pub static ref POOL_SIZE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_pool_size", "Total connection pool size"),
        &["pool_name"]
    ).expect("metric constructor with valid static parameters");

    /// Connection acquisition attempts
    pub static ref POOL_ACQUIRE_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_pool_acquire_total", "Total connection acquisition attempts"),
        &["pool_name", "status"]
    ).expect("metric constructor with valid static parameters");

    /// Connection acquisition duration
    pub static ref POOL_ACQUIRE_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "kaccy_db_pool_acquire_duration_seconds",
            "Connection acquisition duration in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
        &["pool_name"]
    ).expect("metric constructor with valid static parameters");

    /// Pool connection timeouts
    pub static ref POOL_TIMEOUT_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_pool_timeout_total", "Total connection acquisition timeouts"),
        &["pool_name"]
    ).expect("metric constructor with valid static parameters");

    // ============================================================
    // Cache Metrics
    // ============================================================

    /// Cache hits
    pub static ref CACHE_HITS: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_cache_hits_total", "Total cache hits"),
        &["cache_type"]
    ).expect("metric constructor with valid static parameters");

    /// Cache misses
    pub static ref CACHE_MISSES: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_cache_misses_total", "Total cache misses"),
        &["cache_type"]
    ).expect("metric constructor with valid static parameters");

    /// Cache errors
    pub static ref CACHE_ERRORS: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_cache_errors_total", "Total cache errors"),
        &["cache_type", "operation"]
    ).expect("metric constructor with valid static parameters");

    /// Cache operation duration
    pub static ref CACHE_OPERATION_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "kaccy_db_cache_operation_duration_seconds",
            "Cache operation duration in seconds"
        ).buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
        &["cache_type", "operation"]
    ).expect("metric constructor with valid static parameters");

    /// Cache size (number of keys)
    pub static ref CACHE_SIZE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_cache_size", "Number of keys in cache"),
        &["cache_type"]
    ).expect("metric constructor with valid static parameters");

    /// Cache evictions
    pub static ref CACHE_EVICTIONS: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_cache_evictions_total", "Total cache evictions"),
        &["cache_type"]
    ).expect("metric constructor with valid static parameters");

    // ============================================================
    // Query Metrics
    // ============================================================

    /// Total queries executed
    pub static ref QUERY_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_query_total", "Total database queries executed"),
        &["query_type", "status"]
    ).expect("metric constructor with valid static parameters");

    /// Query duration
    pub static ref QUERY_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "kaccy_db_query_duration_seconds",
            "Query execution duration in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]),
        &["query_type"]
    ).expect("metric constructor with valid static parameters");

    /// Slow queries (>1s)
    pub static ref QUERY_SLOW_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_query_slow_total", "Total slow queries (>1s)"),
        &["query_type"]
    ).expect("metric constructor with valid static parameters");

    /// Query errors
    pub static ref QUERY_ERRORS: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_query_errors_total", "Total query errors"),
        &["query_type", "error_type"]
    ).expect("metric constructor with valid static parameters");

    /// Active queries
    pub static ref QUERY_ACTIVE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_query_active", "Number of currently executing queries"),
        &["query_type"]
    ).expect("metric constructor with valid static parameters");

    // ============================================================
    // Repository Metrics
    // ============================================================

    /// Repository operation calls
    pub static ref REPO_OPERATION_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_repo_operation_total", "Total repository operation calls"),
        &["repository", "operation", "status"]
    ).expect("metric constructor with valid static parameters");

    /// Repository operation duration
    pub static ref REPO_OPERATION_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "kaccy_db_repo_operation_duration_seconds",
            "Repository operation duration in seconds"
        ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
        &["repository", "operation"]
    ).expect("metric constructor with valid static parameters");

    // ============================================================
    // Transaction Metrics
    // ============================================================

    /// Transaction operations
    pub static ref TRANSACTION_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_transaction_total", "Total transaction operations"),
        &["operation", "status"]
    ).expect("metric constructor with valid static parameters");

    /// Transaction duration
    pub static ref TRANSACTION_DURATION: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "kaccy_db_transaction_duration_seconds",
            "Transaction duration in seconds"
        ).buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0]),
        &["operation"]
    ).expect("metric constructor with valid static parameters");

    /// Active transactions
    pub static ref TRANSACTION_ACTIVE: IntGauge = IntGauge::new(
        "kaccy_db_transaction_active",
        "Number of active transactions"
    ).expect("metric constructor with valid static parameters");

    // ============================================================
    // Replica Metrics
    // ============================================================

    /// Replica health status
    pub static ref REPLICA_HEALTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_replica_health", "Replica health status (1=healthy, 0=unhealthy)"),
        &["replica_id"]
    ).expect("metric constructor with valid static parameters");

    /// Replica lag in bytes
    pub static ref REPLICA_LAG_BYTES: IntGaugeVec = IntGaugeVec::new(
        Opts::new("kaccy_db_replica_lag_bytes", "Replica replication lag in bytes"),
        &["replica_id"]
    ).expect("metric constructor with valid static parameters");

    /// Replica queries routed
    pub static ref REPLICA_QUERIES: IntCounterVec = IntCounterVec::new(
        Opts::new("kaccy_db_replica_queries_total", "Total queries routed to replica"),
        &["replica_id"]
    ).expect("metric constructor with valid static parameters");
}

/// Register all metrics with the Prometheus registry
pub fn register_metrics() -> Result<(), prometheus::Error> {
    // Pool metrics
    REGISTRY.register(Box::new(POOL_CONNECTIONS_ACTIVE.clone()))?;
    REGISTRY.register(Box::new(POOL_CONNECTIONS_IDLE.clone()))?;
    REGISTRY.register(Box::new(POOL_SIZE.clone()))?;
    REGISTRY.register(Box::new(POOL_ACQUIRE_TOTAL.clone()))?;
    REGISTRY.register(Box::new(POOL_ACQUIRE_DURATION.clone()))?;
    REGISTRY.register(Box::new(POOL_TIMEOUT_TOTAL.clone()))?;

    // Cache metrics
    REGISTRY.register(Box::new(CACHE_HITS.clone()))?;
    REGISTRY.register(Box::new(CACHE_MISSES.clone()))?;
    REGISTRY.register(Box::new(CACHE_ERRORS.clone()))?;
    REGISTRY.register(Box::new(CACHE_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(CACHE_SIZE.clone()))?;
    REGISTRY.register(Box::new(CACHE_EVICTIONS.clone()))?;

    // Query metrics
    REGISTRY.register(Box::new(QUERY_TOTAL.clone()))?;
    REGISTRY.register(Box::new(QUERY_DURATION.clone()))?;
    REGISTRY.register(Box::new(QUERY_SLOW_TOTAL.clone()))?;
    REGISTRY.register(Box::new(QUERY_ERRORS.clone()))?;
    REGISTRY.register(Box::new(QUERY_ACTIVE.clone()))?;

    // Repository metrics
    REGISTRY.register(Box::new(REPO_OPERATION_TOTAL.clone()))?;
    REGISTRY.register(Box::new(REPO_OPERATION_DURATION.clone()))?;

    // Transaction metrics
    REGISTRY.register(Box::new(TRANSACTION_TOTAL.clone()))?;
    REGISTRY.register(Box::new(TRANSACTION_DURATION.clone()))?;
    REGISTRY.register(Box::new(TRANSACTION_ACTIVE.clone()))?;

    // Replica metrics
    REGISTRY.register(Box::new(REPLICA_HEALTH.clone()))?;
    REGISTRY.register(Box::new(REPLICA_LAG_BYTES.clone()))?;
    REGISTRY.register(Box::new(REPLICA_QUERIES.clone()))?;

    Ok(())
}

/// Encode metrics in Prometheus text format
pub fn gather_metrics() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder.encode_to_string(&metric_families)
}

/// Timer for measuring operation duration
pub struct MetricsTimer {
    start: Instant,
    histogram: Option<Histogram>,
}

impl MetricsTimer {
    /// Create a new timer
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: Instant::now(),
            histogram: Some(histogram),
        }
    }

    /// Create a timer without recording
    pub fn noop() -> Self {
        Self {
            start: Instant::now(),
            histogram: None,
        }
    }

    /// Stop the timer and record the duration
    pub fn observe(mut self) {
        if let Some(histogram) = self.histogram.take() {
            let duration = self.start.elapsed().as_secs_f64();
            histogram.observe(duration);
        }
    }

    /// Stop the timer and return the duration without recording
    pub fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

impl Drop for MetricsTimer {
    fn drop(&mut self) {
        if let Some(histogram) = self.histogram.take() {
            let duration = self.start.elapsed().as_secs_f64();
            histogram.observe(duration);
        }
    }
}

/// Helper to record pool statistics
pub fn record_pool_stats(pool_name: &str, size: u32, idle: u32) {
    POOL_SIZE.with_label_values(&[pool_name]).set(size as i64);
    POOL_CONNECTIONS_IDLE
        .with_label_values(&[pool_name])
        .set(idle as i64);
    POOL_CONNECTIONS_ACTIVE
        .with_label_values(&[pool_name])
        .set((size - idle) as i64);
}

/// Helper to record cache hit
pub fn record_cache_hit(cache_type: &str) {
    CACHE_HITS.with_label_values(&[cache_type]).inc();
}

/// Helper to record cache miss
pub fn record_cache_miss(cache_type: &str) {
    CACHE_MISSES.with_label_values(&[cache_type]).inc();
}

/// Helper to record cache error
pub fn record_cache_error(cache_type: &str, operation: &str) {
    CACHE_ERRORS
        .with_label_values(&[cache_type, operation])
        .inc();
}

/// Helper to calculate cache hit rate
pub fn cache_hit_rate(cache_type: &str) -> f64 {
    let hits = CACHE_HITS.with_label_values(&[cache_type]).get() as f64;
    let misses = CACHE_MISSES.with_label_values(&[cache_type]).get() as f64;
    let total = hits + misses;
    if total == 0.0 {
        0.0
    } else {
        hits / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        use prometheus::IntCounter;

        // Reset registry for testing
        let test_registry = Registry::new();

        // Test registering individual metrics
        let counter = IntCounter::new("test_counter", "Test counter").unwrap();
        assert!(test_registry.register(Box::new(counter)).is_ok());

        let gauge = IntGauge::new("test_gauge", "Test gauge").unwrap();
        assert!(test_registry.register(Box::new(gauge)).is_ok());
    }

    #[test]
    fn test_pool_stats_recording() {
        record_pool_stats("test_pool", 10, 5);

        assert_eq!(POOL_SIZE.with_label_values(&["test_pool"]).get(), 10);
        assert_eq!(
            POOL_CONNECTIONS_IDLE
                .with_label_values(&["test_pool"])
                .get(),
            5
        );
        assert_eq!(
            POOL_CONNECTIONS_ACTIVE
                .with_label_values(&["test_pool"])
                .get(),
            5
        );
    }

    #[test]
    fn test_cache_hit_rate() {
        let cache_type = "test_cache_hit_rate";

        // Initial hit rate should be 0
        assert_eq!(cache_hit_rate(cache_type), 0.0);

        // Record some hits and misses
        record_cache_hit(cache_type);
        record_cache_hit(cache_type);
        record_cache_hit(cache_type);
        record_cache_miss(cache_type);

        // Hit rate should be 3/4 = 0.75
        assert_eq!(cache_hit_rate(cache_type), 0.75);
    }

    #[test]
    fn test_cache_operations() {
        let cache_type = "test_cache_ops";

        // Record operations
        record_cache_hit(cache_type);
        record_cache_miss(cache_type);
        record_cache_error(cache_type, "get");

        // Verify counters
        assert_eq!(CACHE_HITS.with_label_values(&[cache_type]).get(), 1);
        assert_eq!(CACHE_MISSES.with_label_values(&[cache_type]).get(), 1);
        assert_eq!(
            CACHE_ERRORS.with_label_values(&[cache_type, "get"]).get(),
            1
        );
    }

    #[test]
    fn test_metrics_timer() {
        let histogram =
            Histogram::with_opts(HistogramOpts::new("test_timer", "Test timer")).unwrap();

        let timer = MetricsTimer::new(histogram.clone());
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.observe();

        // Histogram should have recorded a value
        assert!(histogram.get_sample_count() > 0);
    }

    #[test]
    fn test_metrics_timer_drop() {
        let histogram =
            Histogram::with_opts(HistogramOpts::new("test_timer_drop", "Test timer drop")).unwrap();

        {
            let _timer = MetricsTimer::new(histogram.clone());
            std::thread::sleep(std::time::Duration::from_millis(10));
            // Timer dropped here
        }

        // Histogram should have recorded a value on drop
        assert!(histogram.get_sample_count() > 0);
    }

    #[test]
    fn test_gather_metrics() {
        // This should not panic
        let result = gather_metrics();
        assert!(result.is_ok());
    }
}
