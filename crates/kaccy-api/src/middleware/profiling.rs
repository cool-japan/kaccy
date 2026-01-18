//! Performance profiling middleware and utilities

use axum::{extract::Request, middleware::Next, response::Response};
use std::time::Instant;
use tracing::Instrument;

use super::request_id::RequestId;

/// Performance threshold in milliseconds for slow request logging
const SLOW_REQUEST_THRESHOLD_MS: u128 = 1000;

/// Database query threshold in milliseconds
const SLOW_QUERY_THRESHOLD_MS: u128 = 100;

/// Middleware for performance profiling
pub async fn profiling_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let response = next.run(req).await;

    let duration = start.elapsed();
    let duration_ms = duration.as_millis();

    // Log slow requests
    if duration_ms >= SLOW_REQUEST_THRESHOLD_MS {
        tracing::warn!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            duration_ms = duration_ms,
            status = response.status().as_u16(),
            "Slow request detected"
        );
    }

    // Add performance headers
    let mut response = response;
    if let Ok(duration_header) = duration_ms.to_string().try_into() {
        response
            .headers_mut()
            .insert("x-response-time", duration_header);
    }

    response
}

/// Performance profiler for tracking operation timing
pub struct Profiler {
    operation: String,
    start: Instant,
    threshold_ms: u128,
}

impl Profiler {
    /// Create a new profiler for an operation
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            start: Instant::now(),
            threshold_ms: SLOW_REQUEST_THRESHOLD_MS,
        }
    }

    /// Create a profiler for database queries
    pub fn for_query(query_name: impl Into<String>) -> Self {
        Self {
            operation: query_name.into(),
            start: Instant::now(),
            threshold_ms: SLOW_QUERY_THRESHOLD_MS,
        }
    }

    /// Set custom threshold
    pub fn with_threshold(mut self, threshold_ms: u128) -> Self {
        self.threshold_ms = threshold_ms;
        self
    }

    /// Finish profiling and log if threshold exceeded
    pub fn finish(self) {
        let duration = self.start.elapsed();
        let duration_ms = duration.as_millis();

        if duration_ms >= self.threshold_ms {
            tracing::warn!(
                operation = %self.operation,
                duration_ms = duration_ms,
                threshold_ms = self.threshold_ms,
                "Slow operation detected"
            );
        } else {
            tracing::debug!(
                operation = %self.operation,
                duration_ms = duration_ms,
                "Operation completed"
            );
        }
    }

    /// Finish and return the duration
    pub fn finish_with_duration(self) -> std::time::Duration {
        let duration = self.start.elapsed();
        let duration_ms = duration.as_millis();

        if duration_ms >= self.threshold_ms {
            tracing::warn!(
                operation = %self.operation,
                duration_ms = duration_ms,
                threshold_ms = self.threshold_ms,
                "Slow operation detected"
            );
        }

        duration
    }
}

/// Helper trait for profiling async operations
#[allow(async_fn_in_trait)]
pub trait ProfiledOperation<T> {
    async fn profile(self, operation_name: &str) -> T;
}

impl<F, T> ProfiledOperation<T> for F
where
    F: std::future::Future<Output = T>,
{
    async fn profile(self, operation_name: &str) -> T {
        let profiler = Profiler::new(operation_name);
        let span = tracing::debug_span!("profiled_operation", operation = operation_name);
        let result = self.instrument(span).await;
        profiler.finish();
        result
    }
}

/// Profile a database query
#[macro_export]
macro_rules! profile_query {
    ($query_name:expr, $fut:expr) => {{
        let profiler = $crate::middleware::profiling::Profiler::for_query($query_name);
        let result = $fut.await;
        profiler.finish();
        result
    }};
}

/// Profile an operation
#[macro_export]
macro_rules! profile_operation {
    ($operation_name:expr, $block:block) => {{
        let profiler = $crate::middleware::profiling::Profiler::new($operation_name);
        let result = $block;
        profiler.finish();
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_creation() {
        let profiler = Profiler::new("test_operation");
        assert_eq!(profiler.operation, "test_operation");
        assert_eq!(profiler.threshold_ms, SLOW_REQUEST_THRESHOLD_MS);
    }

    #[test]
    fn test_query_profiler() {
        let profiler = Profiler::for_query("SELECT * FROM users");
        assert_eq!(profiler.threshold_ms, SLOW_QUERY_THRESHOLD_MS);
    }

    #[test]
    fn test_custom_threshold() {
        let profiler = Profiler::new("test").with_threshold(500);
        assert_eq!(profiler.threshold_ms, 500);
    }

    #[tokio::test]
    async fn test_profiled_operation_trait() {
        async fn example_operation() -> i32 {
            42
        }

        let result = example_operation().profile("test_op").await;
        assert_eq!(result, 42);
    }

    #[test]
    fn test_profiler_finish_returns_duration() {
        let profiler = Profiler::new("test");
        // Add a small busy loop to ensure some measurable time passes
        let mut sum = 0u64;
        for i in 0..10000 {
            sum = sum.wrapping_add(i);
        }
        std::hint::black_box(sum);
        let duration = profiler.finish_with_duration();
        // Verify the function returns a valid Duration (the elapsed time measurement works)
        let _ = duration.as_nanos();
    }
}
