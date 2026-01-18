//! Metrics collection for monitoring Bitcoin operations
//!
//! This module provides a trait-based interface for metrics collection
//! that can be integrated with Prometheus or other metrics backends.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::trace;

/// Metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// Metrics backend trait for different implementations
pub trait MetricsBackend: Send + Sync {
    /// Increment a counter
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);

    /// Add to a counter
    fn add_counter(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Set a gauge value
    fn set_gauge(&self, name: &str, value: f64, labels: &[(&str, &str)]);

    /// Observe a histogram value
    fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}

/// Bitcoin metrics collector
pub struct BitcoinMetrics {
    backend: Arc<dyn MetricsBackend>,
    stats: Arc<RwLock<MetricsStats>>,
}

impl BitcoinMetrics {
    /// Create a new metrics collector with a backend
    pub fn new(backend: Arc<dyn MetricsBackend>) -> Self {
        Self {
            backend,
            stats: Arc::new(RwLock::new(MetricsStats::default())),
        }
    }

    /// Record an RPC call
    pub async fn record_rpc_call(&self, method: &str, duration: Duration, success: bool) {
        let duration_ms = duration.as_secs_f64() * 1000.0;

        self.backend.increment_counter(
            "bitcoin_rpc_calls_total",
            &[
                ("method", method),
                ("status", if success { "success" } else { "error" }),
            ],
        );

        self.backend.observe_histogram(
            "bitcoin_rpc_duration_ms",
            duration_ms,
            &[("method", method)],
        );

        let mut stats = self.stats.write().await;
        stats.total_rpc_calls += 1;
        if success {
            stats.successful_rpc_calls += 1;
        } else {
            stats.failed_rpc_calls += 1;
        }
        stats.total_rpc_duration_ms += duration_ms;
    }

    /// Record a transaction volume
    pub fn record_transaction(&self, amount_sats: u64, tx_type: &str) {
        self.backend.add_counter(
            "bitcoin_transaction_volume_sats",
            amount_sats as f64,
            &[("type", tx_type)],
        );

        self.backend
            .increment_counter("bitcoin_transactions_total", &[("type", tx_type)]);
    }

    /// Record fee estimation
    pub fn record_fee_estimation(&self, target_blocks: u16, fee_rate: f64, available: bool) {
        self.backend.observe_histogram(
            "bitcoin_fee_rate_sat_vbyte",
            fee_rate,
            &[("target_blocks", &target_blocks.to_string())],
        );

        if !available {
            self.backend.increment_counter(
                "bitcoin_fee_estimation_unavailable_total",
                &[("target_blocks", &target_blocks.to_string())],
            );
        }
    }

    /// Record confirmation time
    pub async fn record_confirmation_time(&self, blocks: u32, duration: Duration) {
        let duration_secs = duration.as_secs() as f64;

        self.backend.observe_histogram(
            "bitcoin_confirmation_duration_seconds",
            duration_secs,
            &[("blocks", &blocks.to_string())],
        );

        let mut stats = self.stats.write().await;
        stats.total_confirmations += 1;
        stats.avg_confirmation_time_secs = (stats.avg_confirmation_time_secs
            * (stats.total_confirmations - 1) as f64
            + duration_secs)
            / stats.total_confirmations as f64;
    }

    /// Set current block height
    pub fn set_block_height(&self, height: u64) {
        self.backend
            .set_gauge("bitcoin_block_height", height as f64, &[]);
    }

    /// Set mempool size
    pub fn set_mempool_size(&self, size: u64) {
        self.backend
            .set_gauge("bitcoin_mempool_size", size as f64, &[]);
    }

    /// Set connection pool stats
    pub fn set_connection_pool_stats(&self, active: usize, total: usize, max: usize) {
        self.backend
            .set_gauge("bitcoin_connection_pool_active", active as f64, &[]);
        self.backend
            .set_gauge("bitcoin_connection_pool_total", total as f64, &[]);
        self.backend
            .set_gauge("bitcoin_connection_pool_max", max as f64, &[]);
    }

    /// Record cache hit/miss
    pub fn record_cache_access(&self, cache_type: &str, hit: bool) {
        self.backend.increment_counter(
            "bitcoin_cache_accesses_total",
            &[
                ("cache", cache_type),
                ("result", if hit { "hit" } else { "miss" }),
            ],
        );
    }

    /// Set cache size
    pub fn set_cache_size(&self, cache_type: &str, size: usize, max_size: usize) {
        self.backend
            .set_gauge("bitcoin_cache_size", size as f64, &[("cache", cache_type)]);
        self.backend.set_gauge(
            "bitcoin_cache_max_size",
            max_size as f64,
            &[("cache", cache_type)],
        );
    }

    /// Record UTXO selection
    pub async fn record_utxo_selection(
        &self,
        strategy: &str,
        utxos_selected: usize,
        amount_sats: u64,
        duration: Duration,
    ) {
        self.backend
            .increment_counter("bitcoin_utxo_selections_total", &[("strategy", strategy)]);

        self.backend.observe_histogram(
            "bitcoin_utxo_selection_count",
            utxos_selected as f64,
            &[("strategy", strategy)],
        );

        self.backend.observe_histogram(
            "bitcoin_utxo_selection_amount_sats",
            amount_sats as f64,
            &[("strategy", strategy)],
        );

        self.backend.observe_histogram(
            "bitcoin_utxo_selection_duration_ms",
            duration.as_secs_f64() * 1000.0,
            &[("strategy", strategy)],
        );

        let mut stats = self.stats.write().await;
        stats.total_utxo_selections += 1;
    }

    /// Record payment monitoring
    pub fn record_payment_status(&self, status: &str) {
        self.backend
            .increment_counter("bitcoin_payments_total", &[("status", status)]);
    }

    /// Record address generation
    pub async fn record_address_generation(&self, address_type: &str, duration: Duration) {
        self.backend.increment_counter(
            "bitcoin_addresses_generated_total",
            &[("type", address_type)],
        );

        self.backend.observe_histogram(
            "bitcoin_address_generation_duration_ms",
            duration.as_secs_f64() * 1000.0,
            &[("type", address_type)],
        );

        let mut stats = self.stats.write().await;
        stats.total_addresses_generated += 1;
    }

    /// Get aggregated statistics
    pub async fn get_stats(&self) -> MetricsStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = MetricsStats::default();
    }
}

/// Aggregated metrics statistics
#[derive(Debug, Clone, Default)]
pub struct MetricsStats {
    pub total_rpc_calls: u64,
    pub successful_rpc_calls: u64,
    pub failed_rpc_calls: u64,
    pub total_rpc_duration_ms: f64,
    pub total_confirmations: u64,
    pub avg_confirmation_time_secs: f64,
    pub total_utxo_selections: u64,
    pub total_addresses_generated: u64,
}

impl MetricsStats {
    /// Get average RPC duration in milliseconds
    pub fn avg_rpc_duration_ms(&self) -> f64 {
        if self.total_rpc_calls == 0 {
            0.0
        } else {
            self.total_rpc_duration_ms / self.total_rpc_calls as f64
        }
    }

    /// Get RPC success rate (0.0 to 1.0)
    pub fn rpc_success_rate(&self) -> f64 {
        if self.total_rpc_calls == 0 {
            0.0
        } else {
            self.successful_rpc_calls as f64 / self.total_rpc_calls as f64
        }
    }
}

/// No-op metrics backend for testing
pub struct NoOpMetricsBackend;

impl MetricsBackend for NoOpMetricsBackend {
    fn increment_counter(&self, _name: &str, _labels: &[(&str, &str)]) {
        trace!("NoOp: increment_counter");
    }

    fn add_counter(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        trace!("NoOp: add_counter");
    }

    fn set_gauge(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        trace!("NoOp: set_gauge");
    }

    fn observe_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {
        trace!("NoOp: observe_histogram");
    }
}

/// Timer utility for measuring durations
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed duration
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Stop timer and return duration
    pub fn stop(self) -> Duration {
        self.elapsed()
    }
}

/// In-memory metrics backend for testing and development
pub struct InMemoryMetricsBackend {
    counters: Arc<RwLock<std::collections::HashMap<String, f64>>>,
    gauges: Arc<RwLock<std::collections::HashMap<String, f64>>>,
    histograms: Arc<RwLock<std::collections::HashMap<String, Vec<f64>>>>,
}

impl InMemoryMetricsBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(std::collections::HashMap::new())),
            gauges: Arc::new(RwLock::new(std::collections::HashMap::new())),
            histograms: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get counter value
    pub async fn get_counter(&self, name: &str) -> Option<f64> {
        self.counters.read().await.get(name).copied()
    }

    /// Get gauge value
    pub async fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.read().await.get(name).copied()
    }

    /// Get histogram values
    pub async fn get_histogram(&self, name: &str) -> Option<Vec<f64>> {
        self.histograms.read().await.get(name).cloned()
    }

    /// Clear all metrics
    pub async fn clear(&self) {
        self.counters.write().await.clear();
        self.gauges.write().await.clear();
        self.histograms.write().await.clear();
    }
}

impl Default for InMemoryMetricsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsBackend for InMemoryMetricsBackend {
    fn increment_counter(&self, name: &str, _labels: &[(&str, &str)]) {
        let counters = self.counters.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            let mut counters = counters.write().await;
            *counters.entry(name).or_insert(0.0) += 1.0;
        });
    }

    fn add_counter(&self, name: &str, value: f64, _labels: &[(&str, &str)]) {
        let counters = self.counters.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            let mut counters = counters.write().await;
            *counters.entry(name).or_insert(0.0) += value;
        });
    }

    fn set_gauge(&self, name: &str, value: f64, _labels: &[(&str, &str)]) {
        let gauges = self.gauges.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            let mut gauges = gauges.write().await;
            gauges.insert(name, value);
        });
    }

    fn observe_histogram(&self, name: &str, value: f64, _labels: &[(&str, &str)]) {
        let histograms = self.histograms.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            let mut histograms = histograms.write().await;
            histograms.entry(name).or_default().push(value);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_timer() {
        let timer = MetricsTimer::start();
        std::thread::sleep(Duration::from_millis(10));
        let duration = timer.stop();
        assert!(duration.as_millis() >= 10);
    }

    #[tokio::test]
    async fn test_metrics_stats() {
        let backend = Arc::new(NoOpMetricsBackend);
        let metrics = BitcoinMetrics::new(backend);

        metrics
            .record_rpc_call("getblockcount", Duration::from_millis(100), true)
            .await;
        metrics
            .record_rpc_call("getblockcount", Duration::from_millis(200), true)
            .await;
        metrics
            .record_rpc_call("getblockcount", Duration::from_millis(300), false)
            .await;

        let stats = metrics.get_stats().await;
        assert_eq!(stats.total_rpc_calls, 3);
        assert_eq!(stats.successful_rpc_calls, 2);
        assert_eq!(stats.failed_rpc_calls, 1);
        assert_eq!(stats.avg_rpc_duration_ms(), 200.0);
    }

    #[tokio::test]
    async fn test_in_memory_backend() {
        let backend = InMemoryMetricsBackend::new();

        backend.increment_counter("test_counter", &[]);
        backend.increment_counter("test_counter", &[]);

        // Give async tasks time to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        let value = backend.get_counter("test_counter").await;
        assert_eq!(value, Some(2.0));
    }

    #[tokio::test]
    async fn test_in_memory_gauge() {
        let backend = InMemoryMetricsBackend::new();

        backend.set_gauge("test_gauge", 42.0, &[]);

        tokio::time::sleep(Duration::from_millis(10)).await;

        let value = backend.get_gauge("test_gauge").await;
        assert_eq!(value, Some(42.0));
    }

    #[tokio::test]
    async fn test_metrics_success_rate() {
        let stats = MetricsStats {
            total_rpc_calls: 10,
            successful_rpc_calls: 8,
            failed_rpc_calls: 2,
            ..Default::default()
        };

        assert_eq!(stats.rpc_success_rate(), 0.8);
    }

    #[tokio::test]
    async fn test_record_transaction() {
        let backend = Arc::new(InMemoryMetricsBackend::new());
        let metrics = BitcoinMetrics::new(backend.clone());

        metrics.record_transaction(100_000, "received");
        metrics.record_transaction(50_000, "sent");

        tokio::time::sleep(Duration::from_millis(10)).await;

        let volume = backend.get_counter("bitcoin_transaction_volume_sats").await;
        assert_eq!(volume, Some(150_000.0));
    }

    #[tokio::test]
    async fn test_record_fee_estimation() {
        let backend = Arc::new(InMemoryMetricsBackend::new());
        let metrics = BitcoinMetrics::new(backend);

        metrics.record_fee_estimation(6, 10.5, true);
        metrics.record_fee_estimation(3, 20.0, false);

        // Metrics should be recorded without errors
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let backend = Arc::new(NoOpMetricsBackend);
        let metrics = BitcoinMetrics::new(backend);

        metrics
            .record_rpc_call("test", Duration::from_millis(100), true)
            .await;
        let stats = metrics.get_stats().await;
        assert_eq!(stats.total_rpc_calls, 1);

        metrics.reset_stats().await;
        let stats = metrics.get_stats().await;
        assert_eq!(stats.total_rpc_calls, 0);
    }
}
