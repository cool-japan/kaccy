use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Metric value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
}

/// Business metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    /// Metric name
    pub name: String,
    /// Metric value
    pub value: MetricValue,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Tags for grouping/filtering
    pub tags: std::collections::HashMap<String, String>,
}

impl Metric {
    /// Create a new metric
    pub fn new(name: String, value: MetricValue) -> Self {
        Self {
            name,
            value,
            timestamp: Utc::now(),
            tags: std::collections::HashMap::new(),
        }
    }

    /// Add a tag
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }
}

/// Business metrics tracker
pub struct BusinessMetrics {
    /// Metrics by name
    metrics: Arc<DashMap<String, Vec<Metric>>>,
    /// Counters (cumulative values)
    counters: Arc<DashMap<String, i64>>,
    /// Gauges (current values)
    gauges: Arc<DashMap<String, f64>>,
}

impl BusinessMetrics {
    /// Create a new business metrics tracker
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(DashMap::new()),
            counters: Arc::new(DashMap::new()),
            gauges: Arc::new(DashMap::new()),
        }
    }

    /// Track a metric
    pub async fn track(&self, metric: Metric) {
        let name = metric.name.clone();

        self.metrics.entry(name.clone()).or_default().push(metric);

        tracing::debug!("Tracked business metric: {}", name);
    }

    /// Increment a counter
    pub async fn increment_counter(&self, name: &str, value: i64) {
        self.counters
            .entry(name.to_string())
            .and_modify(|v| *v += value)
            .or_insert(value);
    }

    /// Get counter value
    pub async fn get_counter(&self, name: &str) -> i64 {
        self.counters.get(name).map(|v| *v.value()).unwrap_or(0)
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64) {
        self.gauges.insert(name.to_string(), value);
    }

    /// Get gauge value
    pub async fn get_gauge(&self, name: &str) -> Option<f64> {
        self.gauges.get(name).map(|v| *v.value())
    }

    /// Get metrics by name
    pub async fn get_metrics(&self, name: &str) -> Vec<Metric> {
        self.metrics
            .get(name)
            .map(|m| m.value().clone())
            .unwrap_or_default()
    }

    /// Get all counter names
    pub async fn get_counter_names(&self) -> Vec<String> {
        self.counters.iter().map(|e| e.key().clone()).collect()
    }

    /// Get all gauge names
    pub async fn get_gauge_names(&self) -> Vec<String> {
        self.gauges.iter().map(|e| e.key().clone()).collect()
    }

    /// Get summary of all metrics
    pub async fn get_summary(&self) -> MetricsSummary {
        let mut counters = std::collections::HashMap::new();
        for entry in self.counters.iter() {
            counters.insert(entry.key().clone(), *entry.value());
        }

        let mut gauges = std::collections::HashMap::new();
        for entry in self.gauges.iter() {
            gauges.insert(entry.key().clone(), *entry.value());
        }

        MetricsSummary { counters, gauges }
    }

    /// Clear all metrics
    pub async fn clear(&self) {
        self.metrics.clear();
        self.counters.clear();
        self.gauges.clear();
    }
}

impl Default for BusinessMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub counters: std::collections::HashMap<String, i64>,
    pub gauges: std::collections::HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_business_metrics_creation() {
        let metrics = BusinessMetrics::new();
        let summary = metrics.get_summary().await;
        assert_eq!(summary.counters.len(), 0);
        assert_eq!(summary.gauges.len(), 0);
    }

    #[tokio::test]
    async fn test_track_metric() {
        let metrics = BusinessMetrics::new();

        let metric = Metric::new("revenue".to_string(), MetricValue::Float(1000.0));

        metrics.track(metric).await;

        let tracked = metrics.get_metrics("revenue").await;
        assert_eq!(tracked.len(), 1);
    }

    #[tokio::test]
    async fn test_counter() {
        let metrics = BusinessMetrics::new();

        metrics.increment_counter("user_registrations", 1).await;
        metrics.increment_counter("user_registrations", 5).await;

        let count = metrics.get_counter("user_registrations").await;
        assert_eq!(count, 6);
    }

    #[tokio::test]
    async fn test_gauge() {
        let metrics = BusinessMetrics::new();

        metrics.set_gauge("active_users", 42.0).await;

        let value = metrics.get_gauge("active_users").await;
        assert_eq!(value, Some(42.0));
    }

    #[tokio::test]
    async fn test_metrics_summary() {
        let metrics = BusinessMetrics::new();

        metrics.increment_counter("total_orders", 10).await;
        metrics.increment_counter("total_trades", 5).await;
        metrics.set_gauge("current_price", 100.5).await;

        let summary = metrics.get_summary().await;
        assert_eq!(summary.counters.len(), 2);
        assert_eq!(summary.gauges.len(), 1);
        assert_eq!(summary.counters.get("total_orders"), Some(&10));
        assert_eq!(summary.gauges.get("current_price"), Some(&100.5));
    }
}
