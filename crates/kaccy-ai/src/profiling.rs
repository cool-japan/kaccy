//! Performance profiling utilities for AI operations
//!
//! This module provides tools for monitoring, measuring, and optimizing
//! AI service performance including latency, cost, and resource usage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Performance metrics for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetrics {
    /// Operation name/identifier
    pub operation: String,
    /// Duration of the operation
    pub duration: Duration,
    /// Number of tokens used (if applicable)
    pub tokens_used: Option<u32>,
    /// Estimated cost in USD (if applicable)
    pub estimated_cost: Option<f64>,
    /// Whether the operation succeeded
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Timestamp when the operation completed
    pub timestamp: std::time::SystemTime,
}

impl OperationMetrics {
    /// Create new operation metrics
    #[must_use]
    pub fn new(operation: String, duration: Duration, success: bool) -> Self {
        Self {
            operation,
            duration,
            tokens_used: None,
            estimated_cost: None,
            success,
            error: None,
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Set token usage
    #[must_use]
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.tokens_used = Some(tokens);
        self
    }

    /// Set estimated cost
    #[must_use]
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost = Some(cost);
        self
    }

    /// Set error message
    #[must_use]
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
}

/// Aggregated statistics for an operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    /// Total number of operations
    pub total_count: u64,
    /// Number of successful operations
    pub success_count: u64,
    /// Number of failed operations
    pub failure_count: u64,
    /// Average duration
    pub avg_duration: Duration,
    /// Minimum duration
    pub min_duration: Duration,
    /// Maximum duration
    pub max_duration: Duration,
    /// Total tokens used
    pub total_tokens: u64,
    /// Total estimated cost
    pub total_cost: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}

impl Default for OperationStats {
    fn default() -> Self {
        Self {
            total_count: 0,
            success_count: 0,
            failure_count: 0,
            avg_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
            total_tokens: 0,
            total_cost: 0.0,
            success_rate: 0.0,
        }
    }
}

impl OperationStats {
    /// Update stats with a new operation metric
    fn update(&mut self, metric: &OperationMetrics) {
        self.total_count += 1;

        if metric.success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        // Update duration stats
        if metric.duration < self.min_duration {
            self.min_duration = metric.duration;
        }
        if metric.duration > self.max_duration {
            self.max_duration = metric.duration;
        }

        // Update average (incremental mean)
        let total_ms = self.avg_duration.as_millis() as u64 * (self.total_count - 1)
            + metric.duration.as_millis() as u64;
        self.avg_duration = Duration::from_millis(total_ms / self.total_count);

        // Update tokens and cost
        if let Some(tokens) = metric.tokens_used {
            self.total_tokens += u64::from(tokens);
        }
        if let Some(cost) = metric.estimated_cost {
            self.total_cost += cost;
        }

        // Update success rate
        self.success_rate = self.success_count as f64 / self.total_count as f64;
    }

    /// Get average cost per operation
    #[must_use]
    pub fn avg_cost(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            self.total_cost / self.total_count as f64
        }
    }

    /// Get average tokens per operation
    #[must_use]
    pub fn avg_tokens(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            self.total_tokens as f64 / self.total_count as f64
        }
    }
}

/// Performance profiler for tracking AI operations
pub struct PerformanceProfiler {
    /// All recorded metrics
    metrics: Arc<Mutex<Vec<OperationMetrics>>>,
    /// Aggregated statistics per operation type
    stats: Arc<Mutex<HashMap<String, OperationStats>>>,
    /// Whether profiling is enabled
    enabled: bool,
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceProfiler {
    /// Create a new performance profiler
    #[must_use]
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
            enabled: true,
        }
    }

    /// Enable profiling
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable profiling
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if profiling is enabled
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Record an operation metric
    pub fn record(&self, metric: OperationMetrics) {
        if !self.enabled {
            return;
        }

        // Update aggregated stats
        if let Ok(mut stats) = self.stats.lock() {
            let operation_stats = stats
                .entry(metric.operation.clone())
                .or_insert_with(OperationStats::default);
            operation_stats.update(&metric);
        }

        // Store the metric
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.push(metric);
        }
    }

    /// Get statistics for a specific operation
    #[must_use]
    pub fn get_stats(&self, operation: &str) -> Option<OperationStats> {
        self.stats
            .lock()
            .ok()
            .and_then(|stats| stats.get(operation).cloned())
    }

    /// Get statistics for all operations
    #[must_use]
    pub fn get_all_stats(&self) -> HashMap<String, OperationStats> {
        self.stats
            .lock()
            .ok()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Get all recorded metrics
    #[must_use]
    pub fn get_all_metrics(&self) -> Vec<OperationMetrics> {
        self.metrics
            .lock()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default()
    }

    /// Clear all recorded data
    pub fn clear(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.clear();
        }
        if let Ok(mut stats) = self.stats.lock() {
            stats.clear();
        }
    }

    /// Get total operations count
    #[must_use]
    pub fn total_operations(&self) -> u64 {
        self.stats
            .lock()
            .ok()
            .map_or(0, |s| s.values().map(|stat| stat.total_count).sum())
    }

    /// Get total cost across all operations
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.stats
            .lock()
            .ok()
            .map_or(0.0, |s| s.values().map(|stat| stat.total_cost).sum())
    }

    /// Get total tokens used across all operations
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.stats
            .lock()
            .ok()
            .map_or(0, |s| s.values().map(|stat| stat.total_tokens).sum())
    }

    /// Generate a performance report
    #[must_use]
    pub fn generate_report(&self) -> PerformanceReport {
        let stats = self.get_all_stats();
        let total_ops = self.total_operations();
        let total_cost = self.total_cost();
        let total_tokens = self.total_tokens();

        let overall_success_rate = if total_ops > 0 {
            stats.values().map(|s| s.success_count).sum::<u64>() as f64 / total_ops as f64
        } else {
            0.0
        };

        PerformanceReport {
            total_operations: total_ops,
            total_cost,
            total_tokens,
            overall_success_rate,
            operation_stats: stats,
        }
    }
}

/// Complete performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Total number of operations across all types
    pub total_operations: u64,
    /// Total cost across all operations
    pub total_cost: f64,
    /// Total tokens used across all operations
    pub total_tokens: u64,
    /// Overall success rate
    pub overall_success_rate: f64,
    /// Statistics per operation type
    pub operation_stats: HashMap<String, OperationStats>,
}

impl PerformanceReport {
    /// Print a human-readable report
    pub fn print(&self) {
        println!("=== Performance Report ===");
        println!("Total Operations: {}", self.total_operations);
        println!("Total Cost: ${:.4}", self.total_cost);
        println!("Total Tokens: {}", self.total_tokens);
        println!(
            "Overall Success Rate: {:.2}%",
            self.overall_success_rate * 100.0
        );
        println!("\nPer-Operation Statistics:");

        for (operation, stats) in &self.operation_stats {
            println!("\n  {operation}:");
            println!("    Count: {}", stats.total_count);
            println!("    Success Rate: {:.2}%", stats.success_rate * 100.0);
            println!("    Avg Duration: {:?}", stats.avg_duration);
            println!("    Min Duration: {:?}", stats.min_duration);
            println!("    Max Duration: {:?}", stats.max_duration);
            println!("    Avg Cost: ${:.6}", stats.avg_cost());
            println!("    Avg Tokens: {:.1}", stats.avg_tokens());
        }
    }
}

/// Scoped profiler for automatic timing
pub struct ScopedProfiler {
    operation: String,
    start: Instant,
    profiler: Arc<PerformanceProfiler>,
    tokens: Option<u32>,
    cost: Option<f64>,
}

impl ScopedProfiler {
    /// Create a new scoped profiler
    #[must_use]
    pub fn new(operation: String, profiler: Arc<PerformanceProfiler>) -> Self {
        Self {
            operation,
            start: Instant::now(),
            profiler,
            tokens: None,
            cost: None,
        }
    }

    /// Set token usage
    pub fn set_tokens(&mut self, tokens: u32) {
        self.tokens = Some(tokens);
    }

    /// Set cost
    pub fn set_cost(&mut self, cost: f64) {
        self.cost = Some(cost);
    }

    /// Complete with success
    pub fn complete_success(self) {
        self.complete(true, None);
    }

    /// Complete with error
    pub fn complete_error(self, error: String) {
        self.complete(false, Some(error));
    }

    /// Internal completion
    fn complete(self, success: bool, error: Option<String>) {
        let duration = self.start.elapsed();
        let mut metric = OperationMetrics::new(self.operation.clone(), duration, success);

        if let Some(tokens) = self.tokens {
            metric = metric.with_tokens(tokens);
        }
        if let Some(cost) = self.cost {
            metric = metric.with_cost(cost);
        }
        if let Some(err) = error {
            metric = metric.with_error(err);
        }

        self.profiler.record(metric);
    }
}

impl Drop for ScopedProfiler {
    fn drop(&mut self) {
        // Auto-complete as success if not explicitly completed
        let duration = self.start.elapsed();
        let mut metric = OperationMetrics::new(self.operation.clone(), duration, true);

        if let Some(tokens) = self.tokens {
            metric = metric.with_tokens(tokens);
        }
        if let Some(cost) = self.cost {
            metric = metric.with_cost(cost);
        }

        self.profiler.record(metric);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_metrics_creation() {
        let metric = OperationMetrics::new("test_op".to_string(), Duration::from_millis(100), true)
            .with_tokens(500)
            .with_cost(0.01);

        assert_eq!(metric.operation, "test_op");
        assert_eq!(metric.duration, Duration::from_millis(100));
        assert!(metric.success);
        assert_eq!(metric.tokens_used, Some(500));
        assert_eq!(metric.estimated_cost, Some(0.01));
    }

    #[test]
    fn test_operation_stats_update() {
        let mut stats = OperationStats::default();

        let metric1 = OperationMetrics::new("test".to_string(), Duration::from_millis(100), true)
            .with_tokens(500)
            .with_cost(0.01);

        let metric2 = OperationMetrics::new("test".to_string(), Duration::from_millis(200), true)
            .with_tokens(600)
            .with_cost(0.02);

        stats.update(&metric1);
        stats.update(&metric2);

        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.success_count, 2);
        assert_eq!(stats.total_tokens, 1100);
        assert_eq!(stats.total_cost, 0.03);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_profiler_record() {
        let profiler = PerformanceProfiler::new();

        let metric = OperationMetrics::new("eval".to_string(), Duration::from_millis(100), true)
            .with_tokens(500)
            .with_cost(0.01);

        profiler.record(metric);

        assert_eq!(profiler.total_operations(), 1);
        assert_eq!(profiler.total_tokens(), 500);
        assert_eq!(profiler.total_cost(), 0.01);
    }

    #[test]
    fn test_profiler_stats() {
        let profiler = PerformanceProfiler::new();

        profiler.record(OperationMetrics::new(
            "op1".to_string(),
            Duration::from_millis(100),
            true,
        ));
        profiler.record(OperationMetrics::new(
            "op1".to_string(),
            Duration::from_millis(200),
            true,
        ));
        profiler.record(OperationMetrics::new(
            "op2".to_string(),
            Duration::from_millis(150),
            true,
        ));

        let stats = profiler.get_stats("op1").unwrap();
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.success_count, 2);

        assert_eq!(profiler.total_operations(), 3);
    }

    #[test]
    fn test_profiler_clear() {
        let profiler = PerformanceProfiler::new();

        profiler.record(OperationMetrics::new(
            "test".to_string(),
            Duration::from_millis(100),
            true,
        ));

        assert_eq!(profiler.total_operations(), 1);

        profiler.clear();

        assert_eq!(profiler.total_operations(), 0);
    }

    #[test]
    fn test_performance_report() {
        let profiler = PerformanceProfiler::new();

        profiler.record(
            OperationMetrics::new("eval".to_string(), Duration::from_millis(100), true)
                .with_tokens(500)
                .with_cost(0.01),
        );

        let report = profiler.generate_report();

        assert_eq!(report.total_operations, 1);
        assert_eq!(report.total_tokens, 500);
        assert_eq!(report.total_cost, 0.01);
        assert_eq!(report.overall_success_rate, 1.0);
    }

    #[test]
    fn test_profiler_enable_disable() {
        let mut profiler = PerformanceProfiler::new();

        profiler.disable();
        assert!(!profiler.is_enabled());

        profiler.record(OperationMetrics::new(
            "test".to_string(),
            Duration::from_millis(100),
            true,
        ));

        assert_eq!(profiler.total_operations(), 0);

        profiler.enable();
        assert!(profiler.is_enabled());

        profiler.record(OperationMetrics::new(
            "test".to_string(),
            Duration::from_millis(100),
            true,
        ));

        assert_eq!(profiler.total_operations(), 1);
    }
}
