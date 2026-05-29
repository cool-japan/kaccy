//! Prometheus-compatible metrics collection
//!
//! Provides metrics collection and aggregation for monitoring and observability.
//! Supports counters, gauges, histograms, and summaries compatible with Prometheus.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Metric types supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricType {
    /// Monotonically increasing value
    Counter,
    /// Value that can increase or decrease
    Gauge,
    /// Distribution of values in configurable buckets
    Histogram,
    /// Sliding-window percentile summary
    Summary,
}

/// Counter metric - monotonically increasing value
#[derive(Debug)]
pub struct Counter {
    /// Atomic counter value
    value: AtomicU64,
    /// Help text describing what this counter measures
    help: String,
    /// Label key-value pairs for this metric instance
    labels: HashMap<String, String>,
}

impl Counter {
    /// Create a new counter with the given help text
    pub fn new(help: impl Into<String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Attach label key-value pairs to this counter
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Increment counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment counter by n
    pub fn add(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset counter to zero
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

/// Gauge metric - value that can go up or down
#[derive(Debug)]
pub struct Gauge {
    /// Current gauge value
    value: Arc<RwLock<f64>>,
    /// Help text describing what this gauge measures
    help: String,
    /// Label key-value pairs for this metric instance
    labels: HashMap<String, String>,
}

impl Gauge {
    /// Create a new gauge with the given help text
    pub fn new(help: impl Into<String>) -> Self {
        Self {
            value: Arc::new(RwLock::new(0.0)),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Attach label key-value pairs to this gauge
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set gauge to specific value
    pub fn set(&self, val: f64) {
        *self.value.write().unwrap_or_else(|e| e.into_inner()) = val;
    }

    /// Increment gauge
    pub fn inc(&self) {
        *self.value.write().unwrap_or_else(|e| e.into_inner()) += 1.0;
    }

    /// Decrement gauge
    pub fn dec(&self) {
        *self.value.write().unwrap_or_else(|e| e.into_inner()) -= 1.0;
    }

    /// Add to gauge
    pub fn add(&self, val: f64) {
        *self.value.write().unwrap_or_else(|e| e.into_inner()) += val;
    }

    /// Subtract from gauge
    pub fn sub(&self, val: f64) {
        *self.value.write().unwrap_or_else(|e| e.into_inner()) -= val;
    }

    /// Get current value
    pub fn get(&self) -> f64 {
        *self.value.read().unwrap_or_else(|e| e.into_inner())
    }
}

/// Histogram - tracks distribution of values with configurable buckets
#[derive(Debug)]
pub struct Histogram {
    /// Upper bounds of each histogram bucket
    buckets: Vec<f64>,
    /// Number of observations in each bucket
    counts: Vec<AtomicU64>,
    /// Sum of all observed values
    sum: Arc<RwLock<f64>>,
    /// Total number of observations
    count: AtomicU64,
    /// Help text describing what this histogram measures
    help: String,
    /// Label key-value pairs for this metric instance
    labels: HashMap<String, String>,
}

impl Histogram {
    /// Create histogram with default buckets
    pub fn new(help: impl Into<String>) -> Self {
        Self::with_buckets(
            help,
            vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
        )
    }

    /// Create histogram with custom buckets
    pub fn with_buckets(help: impl Into<String>, buckets: Vec<f64>) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();

        Self {
            buckets,
            counts,
            sum: Arc::new(RwLock::new(0.0)),
            count: AtomicU64::new(0),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Attach label key-value pairs to this histogram
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Observe a value
    pub fn observe(&self, val: f64) {
        // Update sum and count
        *self.sum.write().unwrap_or_else(|e| e.into_inner()) += val;
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update bucket counts
        for (i, &bucket) in self.buckets.iter().enumerate() {
            if val <= bucket {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get histogram statistics
    pub fn stats(&self) -> HistogramStats {
        let sum = *self.sum.read().unwrap_or_else(|e| e.into_inner());
        let count = self.count.load(Ordering::Relaxed);

        let buckets: Vec<_> = self
            .buckets
            .iter()
            .zip(self.counts.iter())
            .map(|(&le, count)| BucketCount {
                le,
                count: count.load(Ordering::Relaxed),
            })
            .collect();

        HistogramStats {
            sum,
            count,
            buckets,
        }
    }
}

/// Snapshot of histogram statistics
#[derive(Debug, Clone, Serialize)]
pub struct HistogramStats {
    /// Sum of all observed values
    pub sum: f64,
    /// Total number of observations
    pub count: u64,
    /// Per-bucket observation counts
    pub buckets: Vec<BucketCount>,
}

/// Count of observations up to an inclusive bucket boundary
#[derive(Debug, Clone, Serialize)]
pub struct BucketCount {
    /// Bucket upper bound (less than or equal to)
    pub le: f64,
    /// Cumulative count of observations in this bucket
    pub count: u64,
}

/// Summary - tracks percentiles of observed values
#[derive(Debug)]
pub struct Summary {
    /// Sliding window of recent observed values
    observations: Arc<RwLock<Vec<f64>>>,
    /// Maximum age of observations before cleanup
    max_age: Duration,
    /// Maximum number of observations to retain
    max_observations: usize,
    /// Help text describing what this summary measures
    help: String,
    /// Label key-value pairs for this metric instance
    labels: HashMap<String, String>,
    /// Timestamp of the last cleanup pass
    last_cleanup: Arc<RwLock<Instant>>,
}

impl Summary {
    /// Create a new summary with the given help text
    pub fn new(help: impl Into<String>) -> Self {
        Self {
            observations: Arc::new(RwLock::new(Vec::new())),
            max_age: Duration::from_secs(600), // 10 minutes
            max_observations: 1000,
            help: help.into(),
            labels: HashMap::new(),
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Attach label key-value pairs to this summary
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Observe a value
    pub fn observe(&self, val: f64) {
        let mut obs = self.observations.write().unwrap_or_else(|e| e.into_inner());

        // Add observation
        obs.push(val);

        // Limit number of observations
        if obs.len() > self.max_observations {
            obs.remove(0);
        }

        drop(obs);
        self.cleanup_if_needed();
    }

    /// Get percentile value
    pub fn percentile(&self, p: f64) -> Option<f64> {
        let mut obs = self
            .observations
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if obs.is_empty() {
            return None;
        }

        obs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let index = ((p / 100.0) * obs.len() as f64) as usize;
        let index = index.min(obs.len() - 1);

        Some(obs[index])
    }

    /// Get summary statistics
    pub fn stats(&self) -> SummaryStats {
        let obs = self.observations.read().unwrap_or_else(|e| e.into_inner());

        let count = obs.len() as u64;
        let sum: f64 = obs.iter().sum();

        drop(obs);

        SummaryStats {
            count,
            sum,
            p50: self.percentile(50.0).unwrap_or(0.0),
            p90: self.percentile(90.0).unwrap_or(0.0),
            p99: self.percentile(99.0).unwrap_or(0.0),
        }
    }

    fn cleanup_if_needed(&self) {
        let last = *self.last_cleanup.read().unwrap_or_else(|e| e.into_inner());

        if last.elapsed() > self.max_age {
            self.observations
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            *self.last_cleanup.write().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        }
    }
}

/// Snapshot of summary statistics
#[derive(Debug, Clone, Serialize)]
pub struct SummaryStats {
    /// Total number of observations in the window
    pub count: u64,
    /// Sum of all observations in the window
    pub sum: f64,
    /// 50th percentile (median)
    pub p50: f64,
    /// 90th percentile
    pub p90: f64,
    /// 99th percentile
    pub p99: f64,
}

/// Metrics registry
#[derive(Debug, Default)]
pub struct MetricsRegistry {
    /// Registered counters by name
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    /// Registered gauges by name
    gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    /// Registered histograms by name
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
    /// Registered summaries by name
    summaries: Arc<RwLock<HashMap<String, Arc<Summary>>>>,
}

impl MetricsRegistry {
    /// Create a new empty metrics registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or get counter
    pub fn counter(&self, name: &str, help: &str) -> Arc<Counter> {
        let mut counters = self.counters.write().unwrap_or_else(|e| e.into_inner());
        counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new(help)))
            .clone()
    }

    /// Register or get gauge
    pub fn gauge(&self, name: &str, help: &str) -> Arc<Gauge> {
        let mut gauges = self.gauges.write().unwrap_or_else(|e| e.into_inner());
        gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::new(help)))
            .clone()
    }

    /// Register or get histogram
    pub fn histogram(&self, name: &str, help: &str) -> Arc<Histogram> {
        let mut histograms = self.histograms.write().unwrap_or_else(|e| e.into_inner());
        histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new(help)))
            .clone()
    }

    /// Register or get summary
    pub fn summary(&self, name: &str, help: &str) -> Arc<Summary> {
        let mut summaries = self.summaries.write().unwrap_or_else(|e| e.into_inner());
        summaries
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Summary::new(help)))
            .clone()
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        for (name, counter) in self
            .counters
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            output.push_str(&format!("# HELP {} {}\n", name, counter.help));
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, counter.get()));
        }

        // Export gauges
        for (name, gauge) in self.gauges.read().unwrap_or_else(|e| e.into_inner()).iter() {
            output.push_str(&format!("# HELP {} {}\n", name, gauge.help));
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, gauge.get()));
        }

        // Export histograms
        for (name, histogram) in self
            .histograms
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            output.push_str(&format!("# HELP {} {}\n", name, histogram.help));
            output.push_str(&format!("# TYPE {} histogram\n", name));

            let stats = histogram.stats();
            for bucket in stats.buckets {
                output.push_str(&format!(
                    "{}_bucket{{le=\"{}\"}} {}\n",
                    name, bucket.le, bucket.count
                ));
            }
            output.push_str(&format!("{}_sum {}\n", name, stats.sum));
            output.push_str(&format!("{}_count {}\n", name, stats.count));
        }

        // Export summaries
        for (name, summary) in self
            .summaries
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            output.push_str(&format!("# HELP {} {}\n", name, summary.help));
            output.push_str(&format!("# TYPE {} summary\n", name));

            let stats = summary.stats();
            output.push_str(&format!("{}{{quantile=\"0.5\"}} {}\n", name, stats.p50));
            output.push_str(&format!("{}{{quantile=\"0.9\"}} {}\n", name, stats.p90));
            output.push_str(&format!("{}{{quantile=\"0.99\"}} {}\n", name, stats.p99));
            output.push_str(&format!("{}_sum {}\n", name, stats.sum));
            output.push_str(&format!("{}_count {}\n", name, stats.count));
        }

        output
    }

    /// Get all metrics as JSON
    pub fn export_json(&self) -> serde_json::Value {
        serde_json::json!({
            "counters": self.counters.read().unwrap_or_else(|e| e.into_inner()).iter()
                .map(|(k, v)| (k.clone(), v.get()))
                .collect::<HashMap<_, _>>(),
            "gauges": self.gauges.read().unwrap_or_else(|e| e.into_inner()).iter()
                .map(|(k, v)| (k.clone(), v.get()))
                .collect::<HashMap<_, _>>(),
            "histograms": self.histograms.read().unwrap_or_else(|e| e.into_inner()).iter()
                .map(|(k, v)| (k.clone(), v.stats()))
                .collect::<HashMap<_, _>>(),
            "summaries": self.summaries.read().unwrap_or_else(|e| e.into_inner()).iter()
                .map(|(k, v)| (k.clone(), v.stats()))
                .collect::<HashMap<_, _>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_basic() {
        let counter = Counter::new("test counter");

        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.add(10);
        assert_eq!(counter.get(), 11);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge_basic() {
        let gauge = Gauge::new("test gauge");

        gauge.set(10.0);
        assert_eq!(gauge.get(), 10.0);

        gauge.inc();
        assert_eq!(gauge.get(), 11.0);

        gauge.dec();
        assert_eq!(gauge.get(), 10.0);

        gauge.add(5.5);
        assert_eq!(gauge.get(), 15.5);

        gauge.sub(2.5);
        assert_eq!(gauge.get(), 13.0);
    }

    #[test]
    fn test_histogram_basic() {
        let histogram = Histogram::new("test histogram");

        histogram.observe(0.1);
        histogram.observe(0.5);
        histogram.observe(1.5);
        histogram.observe(5.5);

        let stats = histogram.stats();
        assert_eq!(stats.count, 4);
        assert!((stats.sum - 7.6).abs() < 0.01);
    }

    #[test]
    fn test_histogram_buckets() {
        let histogram = Histogram::with_buckets("test", vec![1.0, 5.0, 10.0]);

        histogram.observe(0.5);
        histogram.observe(3.0);
        histogram.observe(7.0);
        histogram.observe(15.0);

        let stats = histogram.stats();

        // 0.5 <= 1.0
        assert_eq!(stats.buckets[0].count, 1);
        // 0.5, 3.0 <= 5.0
        assert_eq!(stats.buckets[1].count, 2);
        // 0.5, 3.0, 7.0 <= 10.0
        assert_eq!(stats.buckets[2].count, 3);
    }

    #[test]
    fn test_summary_basic() {
        let summary = Summary::new("test summary");

        for i in 1..=100 {
            summary.observe(i as f64);
        }

        let stats = summary.stats();
        assert_eq!(stats.count, 100);

        // Median should be around 50
        assert!((stats.p50 - 50.0).abs() < 5.0);
        // P90 should be around 90
        assert!((stats.p90 - 90.0).abs() < 5.0);
        // P99 should be around 99
        assert!((stats.p99 - 99.0).abs() < 5.0);
    }

    #[test]
    fn test_registry() {
        let registry = MetricsRegistry::new();

        let counter = registry.counter("requests_total", "Total requests");
        counter.inc();
        counter.inc();

        let gauge = registry.gauge("active_connections", "Active connections");
        gauge.set(42.0);

        // Get same counter again
        let counter2 = registry.counter("requests_total", "Total requests");
        assert_eq!(counter.get(), counter2.get());
        assert_eq!(counter2.get(), 2);
    }

    #[test]
    fn test_prometheus_export() {
        let registry = MetricsRegistry::new();

        let counter = registry.counter("http_requests_total", "Total HTTP requests");
        counter.add(100);

        let gauge = registry.gauge("memory_usage_bytes", "Memory usage");
        gauge.set(1024.0);

        let output = registry.export_prometheus();

        assert!(output.contains("http_requests_total 100"));
        assert!(output.contains("memory_usage_bytes 1024"));
        assert!(output.contains("# TYPE http_requests_total counter"));
        assert!(output.contains("# TYPE memory_usage_bytes gauge"));
    }

    #[test]
    fn test_json_export() {
        let registry = MetricsRegistry::new();

        let counter = registry.counter("test_counter", "Test");
        counter.add(42);

        let json = registry.export_json();

        assert_eq!(json["counters"]["test_counter"], 42);
    }
}
