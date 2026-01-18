//! Metrics and monitoring for LLM operations
//!
//! This module provides comprehensive metrics tracking for:
//! - Request/response latencies
//! - Token usage and costs
//! - Error rates and types
//! - Provider-specific statistics

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::AiError;

/// Metrics collector for LLM operations
#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<RwLock<MetricsData>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsData::default())),
        }
    }

    /// Record a successful request
    pub async fn record_success(
        &self,
        provider: &str,
        model: &str,
        latency: Duration,
        tokens: TokenUsage,
    ) {
        let mut data = self.inner.write().await;
        data.record_request(provider, model, latency, tokens, None);
    }

    /// Record a failed request
    pub async fn record_failure(
        &self,
        provider: &str,
        model: &str,
        latency: Duration,
        error: &AiError,
    ) {
        let mut data = self.inner.write().await;
        data.record_request(provider, model, latency, TokenUsage::default(), Some(error));
    }

    /// Get current metrics snapshot
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let data = self.inner.read().await;
        data.snapshot()
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut data = self.inner.write().await;
        *data = MetricsData::default();
    }

    /// Get metrics for a specific provider
    pub async fn provider_metrics(&self, provider: &str) -> Option<ProviderMetrics> {
        let data = self.inner.read().await;
        data.provider_stats
            .get(provider)
            .map(|stats| ProviderMetrics {
                provider: provider.to_string(),
                total_requests: stats.total_requests,
                successful_requests: stats.successful_requests,
                failed_requests: stats.failed_requests,
                total_tokens: stats.total_tokens,
                average_latency_ms: stats.average_latency_ms(),
                error_rate: stats.error_rate(),
            })
    }
}

/// Token usage for a request
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    /// Tokens in the prompt
    pub prompt_tokens: u32,
    /// Tokens in the response
    pub completion_tokens: u32,
}

impl TokenUsage {
    /// Create new token usage
    #[must_use]
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
        }
    }

    /// Total tokens used
    #[must_use]
    pub fn total(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}

/// Internal metrics data
#[derive(Default)]
struct MetricsData {
    provider_stats: HashMap<String, ProviderStats>,
    model_stats: HashMap<String, ModelStats>,
    error_counts: HashMap<String, u32>,
    start_time: Option<Instant>,
}

impl MetricsData {
    fn record_request(
        &mut self,
        provider: &str,
        model: &str,
        latency: Duration,
        tokens: TokenUsage,
        error: Option<&AiError>,
    ) {
        // Initialize start time on first request
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }

        // Update provider stats
        let provider_stats = self
            .provider_stats
            .entry(provider.to_string())
            .or_insert_with(ProviderStats::new);

        provider_stats.total_requests += 1;
        provider_stats.total_latency += latency;
        provider_stats.total_tokens += u64::from(tokens.total());

        if error.is_some() {
            provider_stats.failed_requests += 1;
        } else {
            provider_stats.successful_requests += 1;
        }

        // Update model stats
        let model_key = format!("{provider}:{model}");
        let model_stats = self
            .model_stats
            .entry(model_key)
            .or_insert_with(|| ModelStats::new(model.to_string()));

        model_stats.total_requests += 1;
        model_stats.total_latency += latency;
        model_stats.prompt_tokens += u64::from(tokens.prompt_tokens);
        model_stats.completion_tokens += u64::from(tokens.completion_tokens);

        if error.is_some() {
            model_stats.failed_requests += 1;
        } else {
            model_stats.successful_requests += 1;
        }

        // Track error types
        if let Some(err) = error {
            let error_string = format!("{err:?}");
            let error_type = error_string.split('(').next().unwrap_or("Unknown");
            *self.error_counts.entry(error_type.to_string()).or_insert(0) += 1;
        }
    }

    fn snapshot(&self) -> MetricsSnapshot {
        let uptime = self
            .start_time
            .map(|start| start.elapsed())
            .unwrap_or_default();

        let total_requests: u64 = self.provider_stats.values().map(|s| s.total_requests).sum();

        let total_errors: u64 = self
            .provider_stats
            .values()
            .map(|s| s.failed_requests)
            .sum();

        let total_tokens: u64 = self.provider_stats.values().map(|s| s.total_tokens).sum();

        let avg_latency = if total_requests > 0 {
            let total_latency: Duration =
                self.provider_stats.values().map(|s| s.total_latency).sum();
            total_latency.as_millis() as f64 / total_requests as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            uptime_seconds: uptime.as_secs(),
            total_requests,
            total_errors,
            total_tokens,
            average_latency_ms: avg_latency,
            error_rate: if total_requests > 0 {
                (total_errors as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            },
            providers: self.provider_stats.keys().cloned().collect(),
            error_breakdown: self.error_counts.clone(),
        }
    }
}

/// Statistics for a provider
#[derive(Debug, Clone)]
struct ProviderStats {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency: Duration,
    total_tokens: u64,
}

impl ProviderStats {
    fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_latency: Duration::ZERO,
            total_tokens: 0,
        }
    }

    fn average_latency_ms(&self) -> f64 {
        if self.total_requests > 0 {
            self.total_latency.as_millis() as f64 / self.total_requests as f64
        } else {
            0.0
        }
    }

    fn error_rate(&self) -> f64 {
        if self.total_requests > 0 {
            (self.failed_requests as f64 / self.total_requests as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Statistics for a specific model
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ModelStats {
    model_name: String,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency: Duration,
    prompt_tokens: u64,
    completion_tokens: u64,
}

impl ModelStats {
    fn new(model_name: String) -> Self {
        Self {
            model_name,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_latency: Duration::ZERO,
            prompt_tokens: 0,
            completion_tokens: 0,
        }
    }
}

/// Snapshot of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Total requests made
    pub total_requests: u64,
    /// Total errors encountered
    pub total_errors: u64,
    /// Total tokens consumed
    pub total_tokens: u64,
    /// Average latency in milliseconds
    pub average_latency_ms: f64,
    /// Error rate as percentage
    pub error_rate: f64,
    /// List of providers used
    pub providers: Vec<String>,
    /// Breakdown of errors by type
    pub error_breakdown: HashMap<String, u32>,
}

/// Metrics for a specific provider
#[derive(Debug, Clone)]
pub struct ProviderMetrics {
    /// Provider name
    pub provider: String,
    /// Total requests to this provider
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Total tokens used
    pub total_tokens: u64,
    /// Average latency in ms
    pub average_latency_ms: f64,
    /// Error rate as percentage
    pub error_rate: f64,
}

/// Helper for timing operations
pub struct OperationTimer {
    start: Instant,
}

impl OperationTimer {
    /// Start a new timer
    #[must_use]
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_success() {
        let collector = MetricsCollector::new();

        let tokens = TokenUsage::new(100, 50);
        collector
            .record_success("openai", "gpt-4", Duration::from_millis(500), tokens)
            .await;

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.total_errors, 0);
        assert_eq!(snapshot.total_tokens, 150);
    }

    #[tokio::test]
    async fn test_metrics_collector_failure() {
        let collector = MetricsCollector::new();

        collector
            .record_failure(
                "openai",
                "gpt-4",
                Duration::from_millis(100),
                &AiError::ServiceUnavailable,
            )
            .await;

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.total_errors, 1);
        assert_eq!(snapshot.error_rate, 100.0);
    }

    #[tokio::test]
    async fn test_provider_metrics() {
        let collector = MetricsCollector::new();

        let tokens = TokenUsage::new(100, 50);
        collector
            .record_success("openai", "gpt-4", Duration::from_millis(500), tokens)
            .await;

        collector
            .record_success("openai", "gpt-3.5", Duration::from_millis(300), tokens)
            .await;

        let metrics = collector.provider_metrics("openai").await.unwrap();
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.successful_requests, 2);
        assert_eq!(metrics.total_tokens, 300);
    }

    #[tokio::test]
    async fn test_error_rate_calculation() {
        let collector = MetricsCollector::new();

        // 7 successes, 3 failures = 30% error rate
        for _ in 0..7 {
            collector
                .record_success(
                    "openai",
                    "gpt-4",
                    Duration::from_millis(500),
                    TokenUsage::default(),
                )
                .await;
        }

        for _ in 0..3 {
            collector
                .record_failure(
                    "openai",
                    "gpt-4",
                    Duration::from_millis(100),
                    &AiError::RateLimitExceeded,
                )
                .await;
        }

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.total_requests, 10);
        assert_eq!(snapshot.total_errors, 3);
        assert_eq!(snapshot.error_rate, 30.0);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let collector = MetricsCollector::new();

        collector
            .record_success(
                "openai",
                "gpt-4",
                Duration::from_millis(500),
                TokenUsage::new(100, 50),
            )
            .await;

        collector.reset().await;

        let snapshot = collector.snapshot().await;
        assert_eq!(snapshot.total_requests, 0);
        assert_eq!(snapshot.total_tokens, 0);
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_operation_timer() {
        let timer = OperationTimer::start();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
