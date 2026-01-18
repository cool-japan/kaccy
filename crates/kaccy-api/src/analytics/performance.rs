use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Performance metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    /// Metric name
    pub name: String,
    /// Metric value (duration in milliseconds)
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl PerformanceMetric {
    /// Create a new performance metric
    pub fn new(name: String, duration: Duration) -> Self {
        Self {
            name,
            duration_ms: duration.as_millis() as u64,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Metric name
    pub name: String,
    /// Number of samples
    pub count: usize,
    /// Average duration in milliseconds
    pub avg_ms: f64,
    /// Minimum duration in milliseconds
    pub min_ms: u64,
    /// Maximum duration in milliseconds
    pub max_ms: u64,
    /// 95th percentile in milliseconds
    pub p95_ms: u64,
    /// 99th percentile in milliseconds
    pub p99_ms: u64,
}

/// Performance tracker
pub struct PerformanceTracker {
    /// Metrics by name
    metrics: Arc<DashMap<String, Vec<PerformanceMetric>>>,
    /// Maximum metrics per name
    max_metrics_per_name: usize,
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(DashMap::new()),
            max_metrics_per_name: 10_000,
        }
    }

    /// Track a performance metric
    pub async fn track(&self, metric: PerformanceMetric) {
        let name = metric.name.clone();

        self.metrics.entry(name.clone()).or_default().push(metric);

        // Enforce max metrics limit
        if let Some(mut entry) = self.metrics.get_mut(&name) {
            if entry.len() > self.max_metrics_per_name {
                let to_remove = entry.len() - self.max_metrics_per_name;
                entry.drain(0..to_remove);
            }
        }

        tracing::debug!("Tracked performance metric: {}", name);
    }

    /// Get statistics for a metric
    pub async fn get_stats(&self, name: &str) -> Option<PerformanceStats> {
        let entry = self.metrics.get(name)?;
        let metrics = entry.value();

        if metrics.is_empty() {
            return None;
        }

        let mut durations: Vec<u64> = metrics.iter().map(|m| m.duration_ms).collect();
        durations.sort_unstable();

        let count = durations.len();
        let sum: u64 = durations.iter().sum();
        let avg_ms = sum as f64 / count as f64;
        let min_ms = durations[0];
        let max_ms = durations[count - 1];

        let p95_index = (count as f64 * 0.95) as usize;
        let p95_ms = durations[p95_index.min(count - 1)];

        let p99_index = (count as f64 * 0.99) as usize;
        let p99_ms = durations[p99_index.min(count - 1)];

        Some(PerformanceStats {
            name: name.to_string(),
            count,
            avg_ms,
            min_ms,
            max_ms,
            p95_ms,
            p99_ms,
        })
    }

    /// Get all metric names
    pub async fn get_metric_names(&self) -> Vec<String> {
        self.metrics.iter().map(|e| e.key().clone()).collect()
    }

    /// Clear metrics for a name
    pub async fn clear(&self, name: &str) {
        self.metrics.remove(name);
    }

    /// Clear all metrics
    pub async fn clear_all(&self) {
        self.metrics.clear();
    }
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_tracker_creation() {
        let tracker = PerformanceTracker::new();
        let names = tracker.get_metric_names().await;
        assert_eq!(names.len(), 0);
    }

    #[tokio::test]
    async fn test_track_metric() {
        let tracker = PerformanceTracker::new();

        let metric = PerformanceMetric::new("api_request".to_string(), Duration::from_millis(150));

        tracker.track(metric).await;

        let names = tracker.get_metric_names().await;
        assert_eq!(names.len(), 1);
    }

    #[tokio::test]
    async fn test_performance_stats() {
        let tracker = PerformanceTracker::new();

        // Track multiple metrics
        for i in 1..=100 {
            let metric =
                PerformanceMetric::new("api_request".to_string(), Duration::from_millis(i * 10));
            tracker.track(metric).await;
        }

        let stats = tracker.get_stats("api_request").await.unwrap();
        assert_eq!(stats.count, 100);
        assert_eq!(stats.min_ms, 10);
        assert_eq!(stats.max_ms, 1000);
        assert!(stats.avg_ms > 0.0);
        assert!(stats.p95_ms > stats.avg_ms as u64);
    }

    #[tokio::test]
    async fn test_clear_metrics() {
        let tracker = PerformanceTracker::new();

        tracker
            .track(PerformanceMetric::new(
                "test_metric".to_string(),
                Duration::from_millis(100),
            ))
            .await;

        assert_eq!(tracker.get_metric_names().await.len(), 1);

        tracker.clear("test_metric").await;

        assert_eq!(tracker.get_metric_names().await.len(), 0);
    }
}
