//! Observability and structured logging for LLM operations
//!
//! This module provides helpers for structured logging and tracing
//! to improve debugging and monitoring of AI operations.

use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::error::AiError;

/// Log levels for different operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
}

/// Structured log entry for LLM operations
#[derive(Debug, Clone)]
pub struct LlmOperation {
    /// Operation name
    pub operation: String,
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// Request latency
    pub latency: Option<Duration>,
    /// Token usage
    pub tokens: Option<u32>,
    /// Success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl LlmOperation {
    /// Create a new LLM operation log
    pub fn new(
        operation: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            operation: operation.into(),
            provider: provider.into(),
            model: model.into(),
            latency: None,
            tokens: None,
            success: false,
            error: None,
        }
    }

    /// Set latency
    #[must_use]
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = Some(latency);
        self
    }

    /// Set token usage
    #[must_use]
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.tokens = Some(tokens);
        self
    }

    /// Mark as successful
    #[must_use]
    pub fn success(mut self) -> Self {
        self.success = true;
        self
    }

    /// Mark as failed with error
    #[must_use]
    pub fn failed(mut self, error: &AiError) -> Self {
        self.success = false;
        self.error = Some(format!("{error}"));
        self
    }

    /// Log this operation
    pub fn log(&self, level: LogLevel) {
        let latency_ms = self.latency.map_or(0, |d| d.as_millis());
        let tokens = self.tokens.unwrap_or(0);

        match level {
            LogLevel::Debug => {
                debug!(
                    operation = %self.operation,
                    provider = %self.provider,
                    model = %self.model,
                    latency_ms = latency_ms,
                    tokens = tokens,
                    success = self.success,
                    error = ?self.error,
                    "LLM operation"
                );
            }
            LogLevel::Info => {
                info!(
                    operation = %self.operation,
                    provider = %self.provider,
                    model = %self.model,
                    latency_ms = latency_ms,
                    tokens = tokens,
                    success = self.success,
                    error = ?self.error,
                    "LLM operation"
                );
            }
            LogLevel::Warn => {
                warn!(
                    operation = %self.operation,
                    provider = %self.provider,
                    model = %self.model,
                    latency_ms = latency_ms,
                    tokens = tokens,
                    success = self.success,
                    error = ?self.error,
                    "LLM operation"
                );
            }
            LogLevel::Error => {
                error!(
                    operation = %self.operation,
                    provider = %self.provider,
                    model = %self.model,
                    latency_ms = latency_ms,
                    tokens = tokens,
                    success = self.success,
                    error = ?self.error,
                    "LLM operation"
                );
            }
        }
    }
}

/// Helper for tracing AI evaluation operations
pub fn log_evaluation(
    operation: &str,
    subject: &str,
    score: f64,
    confidence: f64,
    latency: Duration,
) {
    info!(
        operation = %operation,
        subject = %subject,
        score = score,
        confidence = confidence,
        latency_ms = latency.as_millis(),
        "AI evaluation completed"
    );
}

/// Helper for tracing verification operations
pub fn log_verification(
    operation: &str,
    evidence_type: &str,
    approved: bool,
    confidence: f64,
    latency: Duration,
) {
    if approved {
        info!(
            operation = %operation,
            evidence_type = %evidence_type,
            approved = approved,
            confidence = confidence,
            latency_ms = latency.as_millis(),
            "Verification completed"
        );
    } else {
        warn!(
            operation = %operation,
            evidence_type = %evidence_type,
            approved = approved,
            confidence = confidence,
            latency_ms = latency.as_millis(),
            "Verification rejected"
        );
    }
}

/// Helper for tracing fraud detection operations
pub fn log_fraud_check(
    user_id: &str,
    risk_level: &str,
    risk_score: f64,
    indicators: usize,
    latency: Duration,
) {
    match risk_level {
        "low" | "very_low" => {
            debug!(
                user_id = %user_id,
                risk_level = %risk_level,
                risk_score = risk_score,
                indicators = indicators,
                latency_ms = latency.as_millis(),
                "Fraud check completed"
            );
        }
        "medium" => {
            info!(
                user_id = %user_id,
                risk_level = %risk_level,
                risk_score = risk_score,
                indicators = indicators,
                latency_ms = latency.as_millis(),
                "Fraud check - medium risk detected"
            );
        }
        _ => {
            warn!(
                user_id = %user_id,
                risk_level = %risk_level,
                risk_score = risk_score,
                indicators = indicators,
                latency_ms = latency.as_millis(),
                "Fraud check - high risk detected"
            );
        }
    }
}

/// Helper for tracing cache operations
pub fn log_cache_hit(key: &str, ttl_remaining: Duration) {
    debug!(
        key = %key,
        ttl_remaining_secs = ttl_remaining.as_secs(),
        "Cache hit"
    );
}

/// Helper for tracing cache misses
pub fn log_cache_miss(key: &str) {
    debug!(
        key = %key,
        "Cache miss"
    );
}

/// Helper for tracing circuit breaker state changes
pub fn log_circuit_breaker_state_change(
    circuit: &str,
    old_state: &str,
    new_state: &str,
    reason: &str,
) {
    warn!(
        circuit = %circuit,
        old_state = %old_state,
        new_state = %new_state,
        reason = %reason,
        "Circuit breaker state changed"
    );
}

/// Helper for tracing rate limiting
pub fn log_rate_limit_exceeded(tier: &str, limit: f64) {
    warn!(
        tier = %tier,
        limit = limit,
        "Rate limit exceeded"
    );
}

/// Helper for tracing retry attempts
pub fn log_retry_attempt(operation: &str, attempt: u32, max_attempts: u32, delay: Duration) {
    debug!(
        operation = %operation,
        attempt = attempt,
        max_attempts = max_attempts,
        delay_ms = delay.as_millis(),
        "Retrying operation"
    );
}

/// Performance span for measuring operation duration
pub struct PerformanceSpan {
    name: String,
    start: std::time::Instant,
}

impl PerformanceSpan {
    /// Start a new performance span
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: std::time::Instant::now(),
        }
    }

    /// End the span and log the duration
    pub fn end(self) {
        let duration = self.start.elapsed();
        debug!(
            span = %self.name,
            duration_ms = duration.as_millis(),
            "Performance span completed"
        );
    }

    /// End the span with a custom message
    pub fn end_with_message(self, message: &str) {
        let duration = self.start.elapsed();
        info!(
            span = %self.name,
            duration_ms = duration.as_millis(),
            message = %message,
            "Performance span completed"
        );
    }

    /// Get elapsed time without ending the span
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_operation_builder() {
        let op = LlmOperation::new("chat", "openai", "gpt-4")
            .with_latency(Duration::from_millis(500))
            .with_tokens(150)
            .success();

        assert_eq!(op.operation, "chat");
        assert_eq!(op.provider, "openai");
        assert_eq!(op.model, "gpt-4");
        assert_eq!(op.latency, Some(Duration::from_millis(500)));
        assert_eq!(op.tokens, Some(150));
        assert!(op.success);
        assert!(op.error.is_none());
    }

    #[test]
    fn test_llm_operation_failed() {
        let op = LlmOperation::new("chat", "openai", "gpt-4").failed(&AiError::RateLimitExceeded);

        assert!(!op.success);
        assert!(op.error.is_some());
    }

    #[test]
    fn test_performance_span() {
        let span = PerformanceSpan::start("test_operation");
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = span.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
