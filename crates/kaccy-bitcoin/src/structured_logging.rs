//! Structured logging enhancements for Bitcoin operations
//!
//! This module provides enhanced logging capabilities including:
//! - Request/response logging with correlation IDs
//! - Performance tracing spans
//! - Structured context for Bitcoin RPC operations

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{Span, info, warn};
use uuid::Uuid;

/// Correlation ID for tracing requests across the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the correlation ID as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Context for logging Bitcoin RPC operations
#[derive(Debug, Clone)]
pub struct RpcLogContext {
    /// Correlation ID for this request
    pub correlation_id: CorrelationId,
    /// RPC method name
    pub method: String,
    /// Optional transaction ID being operated on
    pub txid: Option<String>,
    /// Optional address being queried
    pub address: Option<String>,
    /// Start time of the operation
    pub start_time: Instant,
}

impl RpcLogContext {
    /// Create a new RPC log context
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            correlation_id: CorrelationId::new(),
            method: method.into(),
            txid: None,
            address: None,
            start_time: Instant::now(),
        }
    }

    /// Set the transaction ID
    pub fn with_txid(mut self, txid: impl Into<String>) -> Self {
        self.txid = Some(txid.into());
        self
    }

    /// Set the address
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// Get elapsed time since context creation
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Log the start of an RPC request
    pub fn log_request(&self, params: &str) {
        info!(
            correlation_id = %self.correlation_id,
            method = %self.method,
            txid = ?self.txid,
            address = ?self.address,
            params = %params,
            "RPC request started"
        );
    }

    /// Log a successful RPC response
    pub fn log_response(&self, response_size: usize) {
        let duration_ms = self.elapsed().as_millis();
        info!(
            correlation_id = %self.correlation_id,
            method = %self.method,
            duration_ms = duration_ms,
            response_size = response_size,
            "RPC request completed successfully"
        );
    }

    /// Log an RPC error
    pub fn log_error(&self, error: &str) {
        let duration_ms = self.elapsed().as_millis();
        warn!(
            correlation_id = %self.correlation_id,
            method = %self.method,
            duration_ms = duration_ms,
            error = %error,
            "RPC request failed"
        );
    }
}

/// Performance tracing span for Bitcoin operations
pub struct PerformanceSpan {
    /// Name of the operation
    name: String,
    /// Correlation ID
    correlation_id: CorrelationId,
    /// Start time
    start_time: Instant,
    /// Tracing span
    _span: Span,
}

impl PerformanceSpan {
    /// Create a new performance span
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let correlation_id = CorrelationId::new();
        let _span = tracing::info_span!(
            "performance",
            operation = %name,
            correlation_id = %correlation_id
        );

        Self {
            name,
            correlation_id,
            start_time: Instant::now(),
            _span,
        }
    }

    /// Create a span with an existing correlation ID
    pub fn with_correlation_id(name: impl Into<String>, correlation_id: CorrelationId) -> Self {
        let name = name.into();
        let _span = tracing::info_span!(
            "performance",
            operation = %name,
            correlation_id = %correlation_id
        );

        Self {
            name,
            correlation_id,
            start_time: Instant::now(),
            _span,
        }
    }

    /// Get the correlation ID
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    /// End the span and log the duration
    pub fn end(self) {
        let duration_ms = self.start_time.elapsed().as_millis();
        info!(
            operation = %self.name,
            correlation_id = %self.correlation_id,
            duration_ms = duration_ms,
            "Operation completed"
        );
    }

    /// End with error logging
    pub fn end_with_error(self, error: &str) {
        let duration_ms = self.start_time.elapsed().as_millis();
        warn!(
            operation = %self.name,
            correlation_id = %self.correlation_id,
            duration_ms = duration_ms,
            error = %error,
            "Operation failed"
        );
    }
}

/// Request/response logger for Bitcoin operations
#[derive(Debug)]
pub struct RequestResponseLogger {
    /// Whether to log request details
    pub log_requests: bool,
    /// Whether to log response details
    pub log_responses: bool,
    /// Maximum size of logged payloads (in bytes)
    pub max_payload_size: usize,
}

impl Default for RequestResponseLogger {
    fn default() -> Self {
        Self {
            log_requests: true,
            log_responses: true,
            max_payload_size: 1024, // 1 KB
        }
    }
}

impl RequestResponseLogger {
    /// Create a new request/response logger
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure request logging
    #[allow(dead_code)]
    pub fn with_requests(mut self, enabled: bool) -> Self {
        self.log_requests = enabled;
        self
    }

    /// Configure response logging
    #[allow(dead_code)]
    pub fn with_responses(mut self, enabled: bool) -> Self {
        self.log_responses = enabled;
        self
    }

    /// Configure maximum payload size to log
    #[allow(dead_code)]
    pub fn with_max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = size;
        self
    }

    /// Log a request
    pub fn log_request(&self, context: &RpcLogContext, method: &str, params: &str) {
        if !self.log_requests {
            return;
        }

        let truncated_params = if params.len() > self.max_payload_size {
            format!("{}... (truncated)", &params[..self.max_payload_size])
        } else {
            params.to_string()
        };

        info!(
            correlation_id = %context.correlation_id,
            method = %method,
            params = %truncated_params,
            "Bitcoin RPC request"
        );
    }

    /// Log a response
    pub fn log_response(&self, context: &RpcLogContext, success: bool, response_data: &str) {
        if !self.log_responses {
            return;
        }

        let truncated_response = if response_data.len() > self.max_payload_size {
            format!("{}... (truncated)", &response_data[..self.max_payload_size])
        } else {
            response_data.to_string()
        };

        let duration_ms = context.elapsed().as_millis();

        if success {
            info!(
                correlation_id = %context.correlation_id,
                method = %context.method,
                duration_ms = duration_ms,
                response = %truncated_response,
                "Bitcoin RPC response (success)"
            );
        } else {
            warn!(
                correlation_id = %context.correlation_id,
                method = %context.method,
                duration_ms = duration_ms,
                error = %truncated_response,
                "Bitcoin RPC response (error)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_id_generation() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn test_correlation_id_from_string() {
        let id = CorrelationId::from_string("test-123");
        assert_eq!(id.as_str(), "test-123");
    }

    #[test]
    fn test_rpc_log_context_creation() {
        let context = RpcLogContext::new("getblock");
        assert_eq!(context.method, "getblock");
        assert!(context.txid.is_none());
        assert!(context.address.is_none());
    }

    #[test]
    fn test_rpc_log_context_with_txid() {
        let context = RpcLogContext::new("getrawtransaction").with_txid("abc123");
        assert_eq!(context.txid, Some("abc123".to_string()));
    }

    #[test]
    fn test_rpc_log_context_with_address() {
        let context = RpcLogContext::new("getreceivedbyaddress")
            .with_address("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
        assert!(context.address.is_some());
    }

    #[test]
    fn test_request_response_logger_default() {
        let logger = RequestResponseLogger::default();
        assert!(logger.log_requests);
        assert!(logger.log_responses);
        assert_eq!(logger.max_payload_size, 1024);
    }

    #[test]
    fn test_performance_span_correlation_id() {
        let span = PerformanceSpan::new("test_operation");
        assert!(!span.correlation_id().as_str().is_empty());
    }

    #[test]
    fn test_performance_span_with_correlation_id() {
        let corr_id = CorrelationId::from_string("test-correlation-id");
        let span = PerformanceSpan::with_correlation_id("test_op", corr_id.clone());
        assert_eq!(span.correlation_id().as_str(), corr_id.as_str());
    }
}
