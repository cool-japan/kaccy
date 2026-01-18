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
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// Counter metric - monotonically increasing value
#[derive(Debug)]
pub struct Counter {
    value: AtomicU64,
    help: String,
    labels: HashMap<String, String>,
}

impl Counter {
    pub fn new(help: impl Into<String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

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
    value: Arc<RwLock<f64>>,
    help: String,
    labels: HashMap<String, String>,
}

impl Gauge {
    pub fn new(help: impl Into<String>) -> Self {
        Self {
            value: Arc::new(RwLock::new(0.0)),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Set gauge to specific value
    pub fn set(&self, val: f64) {
        *self.value.write().unwrap() = val;
    }

    /// Increment gauge
    pub fn inc(&self) {
        *self.value.write().unwrap() += 1.0;
    }

    /// Decrement gauge
    pub fn dec(&self) {
        *self.value.write().unwrap() -= 1.0;
    }

    /// Add to gauge
    pub fn add(&self, val: f64) {
        *self.value.write().unwrap() += val;
    }

    /// Subtract from gauge
    pub fn sub(&self, val: f64) {
        *self.value.write().unwrap() -= val;
    }

    /// Get current value
    pub fn get(&self) -> f64 {
        *self.value.read().unwrap()
    }
}

/// Histogram - tracks distribution of values with configurable buckets
#[derive(Debug)]
pub struct Histogram {
    buckets: Vec<f64>,
    counts: Vec<AtomicU64>,
    sum: Arc<RwLock<f64>>,
    count: AtomicU64,
    help: String,
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

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Observe a value
    pub fn observe(&self, val: f64) {
        // Update sum and count
        *self.sum.write().unwrap() += val;
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
        let sum = *self.sum.read().unwrap();
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

#[derive(Debug, Clone, Serialize)]
pub struct HistogramStats {
    pub sum: f64,
    pub count: u64,
    pub buckets: Vec<BucketCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BucketCount {
    pub le: f64, // Less than or equal to
    pub count: u64,
}

/// Summary - tracks percentiles of observed values
#[derive(Debug)]
pub struct Summary {
    observations: Arc<RwLock<Vec<f64>>>,
    max_age: Duration,
    max_observations: usize,
    help: String,
    labels: HashMap<String, String>,
    last_cleanup: Arc<RwLock<Instant>>,
}

impl Summary {
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

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Observe a value
    pub fn observe(&self, val: f64) {
        let mut obs = self.observations.write().unwrap();

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
        let mut obs = self.observations.read().unwrap().clone();

        if obs.is_empty() {
            return None;
        }

        obs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = ((p / 100.0) * obs.len() as f64) as usize;
        let index = index.min(obs.len() - 1);

        Some(obs[index])
    }

    /// Get summary statistics
    pub fn stats(&self) -> SummaryStats {
        let obs = self.observations.read().unwrap();

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
        let last = *self.last_cleanup.read().unwrap();

        if last.elapsed() > self.max_age {
            self.observations.write().unwrap().clear();
            *self.last_cleanup.write().unwrap() = Instant::now();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryStats {
    pub count: u64,
    pub sum: f64,
    pub p50: f64, // Median
    pub p90: f64,
    pub p99: f64,
}

/// Metrics registry
#[derive(Debug, Default)]
pub struct MetricsRegistry {
    counters: Arc<RwLock<HashMap<String, Arc<Counter>>>>,
    gauges: Arc<RwLock<HashMap<String, Arc<Gauge>>>>,
    histograms: Arc<RwLock<HashMap<String, Arc<Histogram>>>>,
    summaries: Arc<RwLock<HashMap<String, Arc<Summary>>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register or get counter
    pub fn counter(&self, name: &str, help: &str) -> Arc<Counter> {
        let mut counters = self.counters.write().unwrap();
        counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Counter::new(help)))
            .clone()
    }

    /// Register or get gauge
    pub fn gauge(&self, name: &str, help: &str) -> Arc<Gauge> {
        let mut gauges = self.gauges.write().unwrap();
        gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Gauge::new(help)))
            .clone()
    }

    /// Register or get histogram
    pub fn histogram(&self, name: &str, help: &str) -> Arc<Histogram> {
        let mut histograms = self.histograms.write().unwrap();
        histograms
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Histogram::new(help)))
            .clone()
    }

    /// Register or get summary
    pub fn summary(&self, name: &str, help: &str) -> Arc<Summary> {
        let mut summaries = self.summaries.write().unwrap();
        summaries
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(Summary::new(help)))
            .clone()
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();

        // Export counters
        for (name, counter) in self.counters.read().unwrap().iter() {
            output.push_str(&format!("# HELP {} {}\n", name, counter.help));
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, counter.get()));
        }

        // Export gauges
        for (name, gauge) in self.gauges.read().unwrap().iter() {
            output.push_str(&format!("# HELP {} {}\n", name, gauge.help));
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, gauge.get()));
        }

        // Export histograms
        for (name, histogram) in self.histograms.read().unwrap().iter() {
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
        for (name, summary) in self.summaries.read().unwrap().iter() {
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
            "counters": self.counters.read().unwrap().iter()
                .map(|(k, v)| (k.clone(), v.get()))
                .collect::<HashMap<_, _>>(),
            "gauges": self.gauges.read().unwrap().iter()
                .map(|(k, v)| (k.clone(), v.get()))
                .collect::<HashMap<_, _>>(),
            "histograms": self.histograms.read().unwrap().iter()
                .map(|(k, v)| (k.clone(), v.stats()))
                .collect::<HashMap<_, _>>(),
            "summaries": self.summaries.read().unwrap().iter()
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
