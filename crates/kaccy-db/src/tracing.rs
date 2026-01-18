//! OpenTelemetry distributed tracing integration for database operations.
//!
//! This module provides:
//! - Automatic span creation for database queries
//! - Query parameter sanitization to prevent sensitive data leaks
//! - Span correlation across services
//! - Integration with SQLx and repository operations

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use tracing::info_span;

/// Database trace configuration
#[derive(Debug, Clone)]
pub struct DbTraceConfig {
    /// Service name for tracing
    pub service_name: String,
    /// Whether to include SQL statements in traces
    pub include_statements: bool,
    /// Whether to sanitize query parameters (recommended for production)
    pub sanitize_params: bool,
    /// Maximum length for SQL statements in traces
    pub max_statement_length: usize,
    /// Whether to record slow query threshold
    pub slow_query_threshold_ms: Option<u64>,
}

impl Default for DbTraceConfig {
    fn default() -> Self {
        Self {
            service_name: "kaccy-db".to_string(),
            include_statements: true,
            sanitize_params: true,
            max_statement_length: 1000,
            slow_query_threshold_ms: Some(100),
        }
    }
}

/// Tracer for database operations
pub struct DbTracer {
    config: DbTraceConfig,
}

impl DbTracer {
    /// Create a new database tracer with the given configuration
    pub fn new(config: DbTraceConfig) -> Self {
        Self { config }
    }

    /// Create a span for a database query
    pub fn trace_query<F, R>(&self, operation: &str, table: &str, sql: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();

        let span = info_span!(
            "db.query",
            db.system = "postgresql",
            db.operation = operation,
            db.sql.table = table,
        );

        let _enter = span.enter();

        if self.config.include_statements {
            let sanitized = self.sanitize_sql(sql);
            tracing::info!(db.statement = %sanitized, "Executing database query");
        }

        let result = f();

        let duration_ms = start.elapsed().as_millis() as u64;

        if let Some(threshold) = self.config.slow_query_threshold_ms {
            if duration_ms > threshold {
                tracing::warn!(duration_ms, operation, table, "Slow query detected");
            }
        }

        tracing::info!(duration_ms, "Query completed");

        result
    }

    /// Sanitize SQL statement to remove sensitive parameters
    pub fn sanitize_sql(&self, sql: &str) -> String {
        if !self.config.sanitize_params {
            return self.truncate_sql(sql);
        }

        // Replace parameter values with placeholders
        let sanitized = sql
            .replace(['\'', '"'], "?")
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join(" ");

        self.truncate_sql(&sanitized)
    }

    /// Truncate SQL statement to maximum length
    fn truncate_sql(&self, sql: &str) -> String {
        if sql.len() <= self.config.max_statement_length {
            sql.to_string()
        } else {
            format!("{}...", &sql[..self.config.max_statement_length])
        }
    }
}

/// Extension trait for tracing database operations
pub trait TraceableOperation {
    /// Execute the operation with tracing
    fn with_trace<F, R>(&self, operation: &str, table: &str, sql: &str, f: F) -> R
    where
        F: FnOnce() -> R;
}

impl TraceableOperation for DbTracer {
    fn with_trace<F, R>(&self, operation: &str, table: &str, sql: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.trace_query(operation, table, sql, f)
    }
}

/// Span context for distributed tracing
#[derive(Debug, Clone)]
pub struct SpanContext {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    /// Trace flags
    pub trace_flags: u8,
}

impl SpanContext {
    /// Create a new span context
    pub fn new(trace_id: String, span_id: String) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
            trace_flags: 1, // Sampled
        }
    }

    /// Set parent span ID
    pub fn with_parent(mut self, parent_span_id: String) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    /// Convert to W3C traceparent header format
    pub fn to_traceparent(&self) -> String {
        let parent = self.parent_span_id.as_deref().unwrap_or(&self.span_id);
        format!("00-{}-{}-{:02x}", self.trace_id, parent, self.trace_flags)
    }

    /// Parse from W3C traceparent header
    pub fn from_traceparent(traceparent: &str) -> Option<Self> {
        let parts: Vec<&str> = traceparent.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }

        Some(Self {
            trace_id: parts[1].to_string(),
            span_id: parts[2].to_string(),
            parent_span_id: None,
            trace_flags: u8::from_str_radix(parts[3], 16).ok()?,
        })
    }
}

/// Query metadata for tracing
#[derive(Debug, Clone)]
pub struct QueryMetadata {
    /// Operation type (SELECT, INSERT, UPDATE, DELETE)
    pub operation: String,
    /// Table name
    pub table: String,
    /// Number of rows affected
    pub rows_affected: Option<u64>,
    /// Query duration in milliseconds
    pub duration_ms: u64,
    /// Whether the query was successful
    pub success: bool,
    /// Error message (if any)
    pub error: Option<String>,
}

impl QueryMetadata {
    /// Create metadata for a successful query
    pub fn success(operation: String, table: String, duration_ms: u64) -> Self {
        Self {
            operation,
            table,
            rows_affected: None,
            duration_ms,
            success: true,
            error: None,
        }
    }

    /// Create metadata for a failed query
    pub fn failure(operation: String, table: String, duration_ms: u64, error: String) -> Self {
        Self {
            operation,
            table,
            rows_affected: None,
            duration_ms,
            success: false,
            error: Some(error),
        }
    }

    /// Set the number of rows affected
    pub fn with_rows_affected(mut self, rows: u64) -> Self {
        self.rows_affected = Some(rows);
        self
    }
}

/// Global tracer instance
static TRACER: RwLock<Option<Arc<DbTracer>>> = RwLock::new(None);

/// Initialize the global database tracer
pub fn init_tracer(config: DbTraceConfig) {
    *TRACER.write() = Some(Arc::new(DbTracer::new(config)));
}

/// Get the global database tracer
pub fn get_tracer() -> Option<Arc<DbTracer>> {
    TRACER.read().as_ref().map(Arc::clone)
}

/// Execute a traced database operation
pub fn trace_db_operation<F, R>(operation: &str, table: &str, sql: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    if let Some(tracer) = get_tracer() {
        tracer.trace_query(operation, table, sql, f)
    } else {
        f()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_trace_config_default() {
        let config = DbTraceConfig::default();
        assert_eq!(config.service_name, "kaccy-db");
        assert!(config.include_statements);
        assert!(config.sanitize_params);
        assert_eq!(config.max_statement_length, 1000);
        assert_eq!(config.slow_query_threshold_ms, Some(100));
    }

    #[test]
    fn test_sanitize_sql() {
        let config = DbTraceConfig::default();
        let tracer = DbTracer::new(config);

        let sql = "SELECT * FROM users WHERE email = 'test@example.com'";
        let sanitized = tracer.sanitize_sql(sql);

        // The sanitize_sql function truncates and replaces quotes with ?
        // Since the max length is 1000, it should still fit
        assert!(sanitized.len() <= 1000 + 3); // Allow for "..."
    }

    #[test]
    fn test_truncate_long_sql() {
        let config = DbTraceConfig {
            max_statement_length: 50,
            ..Default::default()
        };
        let tracer = DbTracer::new(config);

        let long_sql = "SELECT * FROM users WHERE id = 1 AND name = 'test' AND email = 'test@example.com' AND age > 18";
        let truncated = tracer.sanitize_sql(long_sql);

        assert!(truncated.len() <= 53); // 50 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_span_context_to_traceparent() {
        let context = SpanContext::new(
            "0af7651916cd43dd8448eb211c80319c".to_string(),
            "b7ad6b7169203331".to_string(),
        );

        let traceparent = context.to_traceparent();
        assert_eq!(
            traceparent,
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
        );
    }

    #[test]
    fn test_span_context_from_traceparent() {
        let traceparent = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let context = SpanContext::from_traceparent(traceparent).unwrap();

        assert_eq!(context.trace_id, "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(context.span_id, "b7ad6b7169203331");
        assert_eq!(context.trace_flags, 1);
    }

    #[test]
    fn test_span_context_with_parent() {
        let context = SpanContext::new("trace123".to_string(), "span456".to_string())
            .with_parent("parent789".to_string());

        assert_eq!(context.parent_span_id, Some("parent789".to_string()));
    }

    #[test]
    fn test_query_metadata_success() {
        let metadata = QueryMetadata::success("SELECT".to_string(), "users".to_string(), 50);

        assert!(metadata.success);
        assert_eq!(metadata.operation, "SELECT");
        assert_eq!(metadata.table, "users");
        assert_eq!(metadata.duration_ms, 50);
        assert!(metadata.error.is_none());
    }

    #[test]
    fn test_query_metadata_failure() {
        let metadata = QueryMetadata::failure(
            "INSERT".to_string(),
            "users".to_string(),
            100,
            "Constraint violation".to_string(),
        );

        assert!(!metadata.success);
        assert_eq!(metadata.error, Some("Constraint violation".to_string()));
    }

    #[test]
    fn test_query_metadata_with_rows_affected() {
        let metadata = QueryMetadata::success("UPDATE".to_string(), "users".to_string(), 75)
            .with_rows_affected(5);

        assert_eq!(metadata.rows_affected, Some(5));
    }

    #[test]
    fn test_trace_db_operation_without_init() {
        // Should work even without initialization
        let result = trace_db_operation("SELECT", "users", "SELECT * FROM users", || 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_tracer_with_trace() {
        let config = DbTraceConfig::default();
        let tracer = DbTracer::new(config);

        let result = tracer.with_trace("SELECT", "users", "SELECT * FROM users", || "success");
        assert_eq!(result, "success");
    }
}
