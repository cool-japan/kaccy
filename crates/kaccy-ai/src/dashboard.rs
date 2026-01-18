//! Dashboard integration and metrics export
//!
//! This module provides utilities for exporting AI operation metrics
//! to popular monitoring and dashboard platforms like Prometheus, Grafana, and Datadog.

use crate::error::AiError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

/// Metric type for dashboard exports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Counter metric (monotonically increasing)
    Counter,
    /// Gauge metric (can increase or decrease)
    Gauge,
    /// Histogram metric (distribution of values)
    Histogram,
    /// Summary metric (statistical summary)
    Summary,
}

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp (Unix epoch in seconds)
    pub timestamp: u64,
    /// Metric value
    pub value: f64,
    /// Optional labels/tags
    pub labels: HashMap<String, String>,
}

impl TimeSeriesPoint {
    /// Create a new time series point with current timestamp
    #[must_use]
    pub fn now(value: f64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            timestamp,
            value,
            labels: HashMap::new(),
        }
    }

    /// Create a time series point with custom timestamp
    #[must_use]
    pub fn with_timestamp(timestamp: u64, value: f64) -> Self {
        Self {
            timestamp,
            value,
            labels: HashMap::new(),
        }
    }

    /// Add a label/tag to the data point
    #[must_use]
    pub fn with_label(mut self, key: String, value: String) -> Self {
        self.labels.insert(key, value);
        self
    }
}

/// Prometheus-format metric export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusMetric {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Help text describing the metric
    pub help: String,
    /// Data points
    pub points: Vec<TimeSeriesPoint>,
}

impl PrometheusMetric {
    /// Create a new Prometheus metric
    #[must_use]
    pub fn new(name: String, metric_type: MetricType, help: String) -> Self {
        Self {
            name,
            metric_type,
            help,
            points: Vec::new(),
        }
    }

    /// Add a data point
    pub fn add_point(&mut self, point: TimeSeriesPoint) {
        self.points.push(point);
    }

    /// Export to Prometheus text format
    #[must_use]
    pub fn to_prometheus_format(&self) -> String {
        let mut output = String::new();

        // Write HELP and TYPE directives
        let _ = writeln!(output, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(output, "# TYPE {} {}", self.name, self.metric_type_str());

        // Write data points
        for point in &self.points {
            if point.labels.is_empty() {
                let _ = writeln!(
                    output,
                    "{} {} {}",
                    self.name,
                    point.value,
                    point.timestamp * 1000
                );
            } else {
                let labels = Self::format_labels(&point.labels);
                let _ = writeln!(
                    output,
                    "{}{{{}}} {} {}",
                    self.name,
                    labels,
                    point.value,
                    point.timestamp * 1000
                );
            }
        }

        output
    }

    fn metric_type_str(&self) -> &str {
        match self.metric_type {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
            MetricType::Summary => "summary",
        }
    }

    fn format_labels(labels: &HashMap<String, String>) -> String {
        labels
            .iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Dashboard metrics collector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    /// AI operation request count
    pub request_count: u64,
    /// Total cost in USD
    pub total_cost: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Error count
    pub error_count: u64,
    /// Success rate (0-100)
    pub success_rate: f64,
    /// Active LLM providers
    pub active_providers: Vec<String>,
    /// Cache hit rate (0-100)
    pub cache_hit_rate: f64,
    /// Circuit breaker open count
    pub circuit_breaker_open: u64,
    /// Budget utilization percentage (0-100)
    pub budget_utilization: f64,
    /// Custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

impl Default for DashboardMetrics {
    fn default() -> Self {
        Self {
            request_count: 0,
            total_cost: 0.0,
            avg_latency_ms: 0.0,
            error_count: 0,
            success_rate: 100.0,
            active_providers: Vec::new(),
            cache_hit_rate: 0.0,
            circuit_breaker_open: 0,
            budget_utilization: 0.0,
            custom_metrics: HashMap::new(),
        }
    }
}

impl DashboardMetrics {
    /// Create new dashboard metrics
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a custom metric
    pub fn add_custom_metric(&mut self, name: String, value: f64) {
        self.custom_metrics.insert(name, value);
    }

    /// Export to Prometheus format
    #[must_use]
    pub fn to_prometheus(&self) -> Vec<PrometheusMetric> {
        let mut metrics = Vec::new();

        // Request count
        let mut request_metric = PrometheusMetric::new(
            "kaccy_ai_requests_total".to_string(),
            MetricType::Counter,
            "Total number of AI operation requests".to_string(),
        );
        request_metric.add_point(TimeSeriesPoint::now(self.request_count as f64));
        metrics.push(request_metric);

        // Total cost
        let mut cost_metric = PrometheusMetric::new(
            "kaccy_ai_cost_total_usd".to_string(),
            MetricType::Counter,
            "Total AI operation cost in USD".to_string(),
        );
        cost_metric.add_point(TimeSeriesPoint::now(self.total_cost));
        metrics.push(cost_metric);

        // Average latency
        let mut latency_metric = PrometheusMetric::new(
            "kaccy_ai_latency_avg_ms".to_string(),
            MetricType::Gauge,
            "Average AI operation latency in milliseconds".to_string(),
        );
        latency_metric.add_point(TimeSeriesPoint::now(self.avg_latency_ms));
        metrics.push(latency_metric);

        // Error count
        let mut error_metric = PrometheusMetric::new(
            "kaccy_ai_errors_total".to_string(),
            MetricType::Counter,
            "Total number of AI operation errors".to_string(),
        );
        error_metric.add_point(TimeSeriesPoint::now(self.error_count as f64));
        metrics.push(error_metric);

        // Success rate
        let mut success_metric = PrometheusMetric::new(
            "kaccy_ai_success_rate_percent".to_string(),
            MetricType::Gauge,
            "AI operation success rate percentage".to_string(),
        );
        success_metric.add_point(TimeSeriesPoint::now(self.success_rate));
        metrics.push(success_metric);

        // Cache hit rate
        let mut cache_metric = PrometheusMetric::new(
            "kaccy_ai_cache_hit_rate_percent".to_string(),
            MetricType::Gauge,
            "Cache hit rate percentage".to_string(),
        );
        cache_metric.add_point(TimeSeriesPoint::now(self.cache_hit_rate));
        metrics.push(cache_metric);

        // Circuit breaker status
        let mut cb_metric = PrometheusMetric::new(
            "kaccy_ai_circuit_breaker_open_total".to_string(),
            MetricType::Counter,
            "Total number of circuit breaker opens".to_string(),
        );
        cb_metric.add_point(TimeSeriesPoint::now(self.circuit_breaker_open as f64));
        metrics.push(cb_metric);

        // Budget utilization
        let mut budget_metric = PrometheusMetric::new(
            "kaccy_ai_budget_utilization_percent".to_string(),
            MetricType::Gauge,
            "Budget utilization percentage".to_string(),
        );
        budget_metric.add_point(TimeSeriesPoint::now(self.budget_utilization));
        metrics.push(budget_metric);

        // Custom metrics
        for (name, value) in &self.custom_metrics {
            let mut custom_metric = PrometheusMetric::new(
                format!("kaccy_ai_custom_{name}"),
                MetricType::Gauge,
                format!("Custom metric: {name}"),
            );
            custom_metric.add_point(TimeSeriesPoint::now(*value));
            metrics.push(custom_metric);
        }

        metrics
    }

    /// Export to JSON format (for Grafana, Datadog, etc.)
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize metrics: {e}")))
    }
}

/// Grafana-compatible JSON data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrafanaDataPoint {
    /// Metric name
    pub target: String,
    /// Time series data points [[value, timestamp], ...]
    pub datapoints: Vec<[f64; 2]>,
}

impl GrafanaDataPoint {
    /// Create a new Grafana data point
    #[must_use]
    pub fn new(target: String) -> Self {
        Self {
            target,
            datapoints: Vec::new(),
        }
    }

    /// Add a data point (value, timestamp in milliseconds)
    pub fn add_point(&mut self, value: f64, timestamp_ms: f64) {
        self.datapoints.push([value, timestamp_ms]);
    }

    /// Export to JSON
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize data: {e}")))
    }
}

/// Convert dashboard metrics to Grafana format
#[must_use]
pub fn to_grafana_format(metrics: &DashboardMetrics) -> Vec<GrafanaDataPoint> {
    let timestamp_ms = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        * 1000) as f64;

    let mut datapoints = Vec::new();

    // Request count
    let mut requests = GrafanaDataPoint::new("requests_total".to_string());
    requests.add_point(metrics.request_count as f64, timestamp_ms);
    datapoints.push(requests);

    // Cost
    let mut cost = GrafanaDataPoint::new("cost_total_usd".to_string());
    cost.add_point(metrics.total_cost, timestamp_ms);
    datapoints.push(cost);

    // Latency
    let mut latency = GrafanaDataPoint::new("latency_avg_ms".to_string());
    latency.add_point(metrics.avg_latency_ms, timestamp_ms);
    datapoints.push(latency);

    // Success rate
    let mut success = GrafanaDataPoint::new("success_rate_percent".to_string());
    success.add_point(metrics.success_rate, timestamp_ms);
    datapoints.push(success);

    // Cache hit rate
    let mut cache = GrafanaDataPoint::new("cache_hit_rate_percent".to_string());
    cache.add_point(metrics.cache_hit_rate, timestamp_ms);
    datapoints.push(cache);

    // Budget utilization
    let mut budget = GrafanaDataPoint::new("budget_utilization_percent".to_string());
    budget.add_point(metrics.budget_utilization, timestamp_ms);
    datapoints.push(budget);

    datapoints
}

/// Health check status for dashboards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckStatus {
    /// Overall health status
    pub healthy: bool,
    /// Timestamp of check
    pub timestamp: u64,
    /// Component health statuses
    pub components: HashMap<String, ComponentHealth>,
    /// Additional details
    pub details: Option<String>,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component is healthy
    pub healthy: bool,
    /// Last check timestamp
    pub last_check: u64,
    /// Response time in milliseconds
    pub response_time_ms: Option<f64>,
    /// Error message if unhealthy
    pub error: Option<String>,
}

impl HealthCheckStatus {
    /// Create a new health check status
    #[must_use]
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            healthy: true,
            timestamp,
            components: HashMap::new(),
            details: None,
        }
    }

    /// Add component health status
    pub fn add_component(&mut self, name: String, health: ComponentHealth) {
        if !health.healthy {
            self.healthy = false;
        }
        self.components.insert(name, health);
    }

    /// Export to JSON
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize health status: {e}")))
    }
}

impl Default for HealthCheckStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_series_point_creation() {
        let point = TimeSeriesPoint::now(42.5);
        assert_eq!(point.value, 42.5);
        assert!(point.timestamp > 0);
        assert!(point.labels.is_empty());
    }

    #[test]
    fn test_time_series_point_with_labels() {
        let point = TimeSeriesPoint::now(100.0)
            .with_label("provider".to_string(), "openai".to_string())
            .with_label("operation".to_string(), "evaluation".to_string());

        assert_eq!(point.value, 100.0);
        assert_eq!(point.labels.len(), 2);
        assert_eq!(point.labels.get("provider"), Some(&"openai".to_string()));
    }

    #[test]
    fn test_prometheus_metric_creation() {
        let mut metric = PrometheusMetric::new(
            "test_metric".to_string(),
            MetricType::Counter,
            "Test metric".to_string(),
        );

        metric.add_point(TimeSeriesPoint::now(42.0));
        metric.add_point(TimeSeriesPoint::now(43.0));

        assert_eq!(metric.points.len(), 2);
    }

    #[test]
    fn test_prometheus_format_export() {
        let mut metric = PrometheusMetric::new(
            "test_counter".to_string(),
            MetricType::Counter,
            "Test counter metric".to_string(),
        );

        metric.add_point(TimeSeriesPoint::with_timestamp(1_000_000, 42.0));

        let output = metric.to_prometheus_format();

        assert!(output.contains("# HELP test_counter Test counter metric"));
        assert!(output.contains("# TYPE test_counter counter"));
        assert!(output.contains("test_counter 42"));
    }

    #[test]
    fn test_prometheus_format_with_labels() {
        let mut metric = PrometheusMetric::new(
            "labeled_metric".to_string(),
            MetricType::Gauge,
            "Labeled metric".to_string(),
        );

        let point = TimeSeriesPoint::with_timestamp(1_000_000, 100.0)
            .with_label("env".to_string(), "prod".to_string());

        metric.add_point(point);

        let output = metric.to_prometheus_format();

        assert!(output.contains("labeled_metric{"));
        assert!(output.contains("env=\"prod\""));
    }

    #[test]
    fn test_dashboard_metrics_default() {
        let metrics = DashboardMetrics::default();

        assert_eq!(metrics.request_count, 0);
        assert_eq!(metrics.total_cost, 0.0);
        assert_eq!(metrics.error_count, 0);
        assert_eq!(metrics.success_rate, 100.0);
    }

    #[test]
    fn test_dashboard_metrics_custom() {
        let mut metrics = DashboardMetrics::new();
        metrics.add_custom_metric("my_metric".to_string(), 123.45);

        assert_eq!(metrics.custom_metrics.len(), 1);
        assert_eq!(metrics.custom_metrics.get("my_metric"), Some(&123.45));
    }

    #[test]
    fn test_dashboard_metrics_to_prometheus() {
        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 1000;
        metrics.total_cost = 25.50;
        metrics.avg_latency_ms = 150.0;
        metrics.success_rate = 99.5;

        let prometheus_metrics = metrics.to_prometheus();

        assert!(!prometheus_metrics.is_empty());
        assert!(
            prometheus_metrics
                .iter()
                .any(|m| m.name == "kaccy_ai_requests_total")
        );
        assert!(
            prometheus_metrics
                .iter()
                .any(|m| m.name == "kaccy_ai_cost_total_usd")
        );
    }

    #[test]
    fn test_dashboard_metrics_to_json() {
        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 500;
        metrics.total_cost = 12.75;

        let json = metrics.to_json().unwrap();

        assert!(json.contains("\"request_count\""));
        assert!(json.contains("\"total_cost\""));
    }

    #[test]
    fn test_grafana_data_point() {
        let mut datapoint = GrafanaDataPoint::new("test_metric".to_string());
        datapoint.add_point(42.0, 1_000_000.0);
        datapoint.add_point(43.0, 1_001_000.0);

        assert_eq!(datapoint.datapoints.len(), 2);
        assert_eq!(datapoint.datapoints[0][0], 42.0);
    }

    #[test]
    fn test_to_grafana_format() {
        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 1234;
        metrics.total_cost = 45.67;
        metrics.success_rate = 98.5;

        let datapoints = to_grafana_format(&metrics);

        assert!(!datapoints.is_empty());
        assert!(datapoints.iter().any(|d| d.target == "requests_total"));
        assert!(datapoints.iter().any(|d| d.target == "cost_total_usd"));
    }

    #[test]
    fn test_health_check_status_creation() {
        let status = HealthCheckStatus::new();

        assert!(status.healthy);
        assert!(status.timestamp > 0);
        assert!(status.components.is_empty());
    }

    #[test]
    fn test_health_check_add_component() {
        let mut status = HealthCheckStatus::new();

        let component = ComponentHealth {
            healthy: true,
            last_check: 1_000_000,
            response_time_ms: Some(50.0),
            error: None,
        };

        status.add_component("llm_client".to_string(), component);

        assert_eq!(status.components.len(), 1);
        assert!(status.healthy);
    }

    #[test]
    fn test_health_check_unhealthy_component() {
        let mut status = HealthCheckStatus::new();

        let unhealthy_component = ComponentHealth {
            healthy: false,
            last_check: 1_000_000,
            response_time_ms: None,
            error: Some("Connection timeout".to_string()),
        };

        status.add_component("database".to_string(), unhealthy_component);

        assert!(!status.healthy);
    }

    #[test]
    fn test_health_check_to_json() {
        let mut status = HealthCheckStatus::new();

        let component = ComponentHealth {
            healthy: true,
            last_check: 1_000_000,
            response_time_ms: Some(25.0),
            error: None,
        };

        status.add_component("api".to_string(), component);

        let json = status.to_json().unwrap();

        assert!(json.contains("\"healthy\""));
        assert!(json.contains("\"components\""));
    }
}
