//! Distributed tracing system
//!
//! This module provides distributed tracing capabilities for tracking requests
//! across services, analyzing transaction flows, identifying performance
//! bottlenecks, and performing latency breakdown analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Span represents a single unit of work in a distributed trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique span ID
    pub span_id: String,

    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,

    /// Trace ID that groups related spans
    pub trace_id: String,

    /// Service name
    pub service_name: String,

    /// Operation name
    pub operation_name: String,

    /// Start timestamp
    pub start_time: DateTime<Utc>,

    /// End timestamp
    pub end_time: Option<DateTime<Utc>>,

    /// Duration in microseconds
    pub duration_us: Option<i64>,

    /// Span tags/attributes
    pub tags: HashMap<String, String>,

    /// Span logs/events
    pub logs: Vec<SpanLog>,

    /// Status
    pub status: SpanStatus,

    /// Error information
    pub error: Option<String>,
}

/// Span log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    /// Timestamp when this log entry was recorded
    pub timestamp: DateTime<Utc>,
    /// Human-readable log message
    pub message: String,
    /// Structured key-value fields associated with this log entry
    pub fields: HashMap<String, String>,
}

/// Completion status of a span
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span completed with an error
    Error,
    /// Span was cancelled before completion
    Cancelled,
}

/// Trace represents a complete distributed trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    /// Trace ID
    pub trace_id: String,

    /// All spans in this trace
    pub spans: Vec<Span>,

    /// Root span ID
    pub root_span_id: String,

    /// Total duration
    pub total_duration_us: i64,

    /// Number of services involved
    pub service_count: usize,

    /// Number of spans
    pub span_count: usize,

    /// Error count
    pub error_count: usize,
}

/// Trace context for propagation across service boundaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Globally unique identifier for the entire trace
    pub trace_id: String,
    /// Identifier for the current span within the trace
    pub span_id: String,
    /// Whether this trace is being sampled for collection
    pub sampled: bool,
    /// Arbitrary key-value metadata propagated with the trace
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    /// Create new trace context
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            sampled: true,
            baggage: HashMap::new(),
        }
    }

    /// Create child context
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            sampled: self.sampled,
            baggage: self.baggage.clone(),
        }
    }

    /// Inject context into headers
    pub fn inject(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("x-trace-id".to_string(), self.trace_id.clone());
        headers.insert("x-span-id".to_string(), self.span_id.clone());
        headers.insert("x-sampled".to_string(), self.sampled.to_string());
        headers
    }

    /// Extract context from headers
    pub fn extract(headers: &HashMap<String, String>) -> Option<Self> {
        let trace_id = headers.get("x-trace-id")?.clone();
        let span_id = headers.get("x-span-id")?.clone();
        let sampled = headers
            .get("x-sampled")
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        Some(Self {
            trace_id,
            span_id,
            sampled,
            baggage: HashMap::new(),
        })
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Span {
    /// Create a new span
    pub fn new(
        trace_id: String,
        service_name: String,
        operation_name: String,
        parent_span_id: Option<String>,
    ) -> Self {
        Self {
            span_id: Uuid::new_v4().to_string(),
            parent_span_id,
            trace_id,
            service_name,
            operation_name,
            start_time: Utc::now(),
            end_time: None,
            duration_us: None,
            tags: HashMap::new(),
            logs: Vec::new(),
            status: SpanStatus::Ok,
            error: None,
        }
    }

    /// Add a tag to the span
    pub fn set_tag(&mut self, key: String, value: String) {
        self.tags.insert(key, value);
    }

    /// Add a log entry
    pub fn log(&mut self, message: String, fields: HashMap<String, String>) {
        self.logs.push(SpanLog {
            timestamp: Utc::now(),
            message,
            fields,
        });
    }

    /// Finish the span
    pub fn finish(&mut self) {
        let end_time = Utc::now();
        self.end_time = Some(end_time);
        self.duration_us = Some((end_time - self.start_time).num_microseconds().unwrap_or(0));
    }

    /// Mark span as error
    pub fn set_error(&mut self, error: String) {
        self.status = SpanStatus::Error;
        self.error = Some(error);
    }
}

/// Distributed tracer
pub struct DistributedTracer {
    /// Service name
    service_name: String,

    /// Active spans
    active_spans: Arc<RwLock<HashMap<String, Span>>>,

    /// Completed traces
    completed_traces: Arc<RwLock<HashMap<String, Trace>>>,

    /// Sampling rate (0.0 to 1.0)
    #[allow(dead_code)]
    sampling_rate: f64,
}

impl DistributedTracer {
    /// Create a new distributed tracer
    pub fn new(service_name: String, sampling_rate: f64) -> Self {
        Self {
            service_name,
            active_spans: Arc::new(RwLock::new(HashMap::new())),
            completed_traces: Arc::new(RwLock::new(HashMap::new())),
            sampling_rate: sampling_rate.clamp(0.0, 1.0),
        }
    }

    /// Start a new span
    pub async fn start_span(&self, operation_name: String, context: Option<TraceContext>) -> Span {
        let (trace_id, parent_span_id) = if let Some(ctx) = context {
            (ctx.trace_id, Some(ctx.span_id))
        } else {
            (Uuid::new_v4().to_string(), None)
        };

        let span = Span::new(
            trace_id,
            self.service_name.clone(),
            operation_name,
            parent_span_id,
        );

        let mut active = self.active_spans.write().await;
        active.insert(span.span_id.clone(), span.clone());

        span
    }

    /// Finish a span
    pub async fn finish_span(&self, mut span: Span) {
        span.finish();

        let mut active = self.active_spans.write().await;
        active.remove(&span.span_id);

        // Try to build complete trace
        self.try_build_trace(&span).await;
    }

    /// Try to build a complete trace from finished spans
    async fn try_build_trace(&self, finished_span: &Span) {
        // For now, store individual spans
        // In a real implementation, we would aggregate spans by trace_id
        let mut traces = self.completed_traces.write().await;

        let trace = traces
            .entry(finished_span.trace_id.clone())
            .or_insert_with(|| Trace {
                trace_id: finished_span.trace_id.clone(),
                spans: Vec::new(),
                root_span_id: finished_span.span_id.clone(),
                total_duration_us: 0,
                service_count: 0,
                span_count: 0,
                error_count: 0,
            });

        trace.spans.push(finished_span.clone());
        trace.span_count = trace.spans.len();

        // Update statistics
        let mut services = std::collections::HashSet::new();
        let mut total_duration = 0i64;
        let mut error_count = 0;

        for span in &trace.spans {
            services.insert(span.service_name.clone());
            if let Some(duration) = span.duration_us {
                total_duration = total_duration.max(duration);
            }
            if span.status == SpanStatus::Error {
                error_count += 1;
            }
        }

        trace.service_count = services.len();
        trace.total_duration_us = total_duration;
        trace.error_count = error_count;
    }

    /// Get a trace by ID
    pub async fn get_trace(&self, trace_id: &str) -> Option<Trace> {
        let traces = self.completed_traces.read().await;
        traces.get(trace_id).cloned()
    }

    /// Get all traces
    pub async fn get_all_traces(&self) -> Vec<Trace> {
        let traces = self.completed_traces.read().await;
        traces.values().cloned().collect()
    }
}

/// Performance bottleneck identifier
pub struct BottleneckAnalyzer;

impl BottleneckAnalyzer {
    /// Identify bottlenecks in a trace
    pub fn identify_bottlenecks(trace: &Trace, threshold_ms: i64) -> Vec<BottleneckReport> {
        let threshold_us = threshold_ms * 1000;
        let mut bottlenecks = Vec::new();

        for span in &trace.spans {
            if let Some(duration) = span.duration_us {
                if duration > threshold_us {
                    bottlenecks.push(BottleneckReport {
                        span_id: span.span_id.clone(),
                        operation_name: span.operation_name.clone(),
                        service_name: span.service_name.clone(),
                        duration_ms: duration / 1000,
                        percentage: ((duration as f64 / trace.total_duration_us as f64) * 100.0)
                            as i32,
                    });
                }
            }
        }

        bottlenecks.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
        bottlenecks
    }
}

/// Bottleneck report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckReport {
    /// Span identifier of the bottleneck
    pub span_id: String,
    /// Name of the slow operation
    pub operation_name: String,
    /// Service that owns the slow operation
    pub service_name: String,
    /// Duration of the span in milliseconds
    pub duration_ms: i64,
    /// Duration as a percentage of the total trace duration
    pub percentage: i32,
}

/// Latency breakdown analyzer
pub struct LatencyAnalyzer;

impl LatencyAnalyzer {
    /// Analyze latency breakdown by service
    pub fn by_service(trace: &Trace) -> HashMap<String, ServiceLatency> {
        let mut latency_by_service = HashMap::new();

        for span in &trace.spans {
            if let Some(duration) = span.duration_us {
                let entry = latency_by_service
                    .entry(span.service_name.clone())
                    .or_insert_with(|| ServiceLatency {
                        service_name: span.service_name.clone(),
                        total_duration_us: 0,
                        span_count: 0,
                        avg_duration_us: 0,
                        min_duration_us: i64::MAX,
                        max_duration_us: 0,
                    });

                entry.total_duration_us += duration;
                entry.span_count += 1;
                entry.min_duration_us = entry.min_duration_us.min(duration);
                entry.max_duration_us = entry.max_duration_us.max(duration);
            }
        }

        // Calculate averages
        for latency in latency_by_service.values_mut() {
            if latency.span_count > 0 {
                latency.avg_duration_us = latency.total_duration_us / latency.span_count as i64;
            }
        }

        latency_by_service
    }

    /// Analyze latency breakdown by operation
    pub fn by_operation(trace: &Trace) -> HashMap<String, OperationLatency> {
        let mut latency_by_operation = HashMap::new();

        for span in &trace.spans {
            if let Some(duration) = span.duration_us {
                let entry = latency_by_operation
                    .entry(span.operation_name.clone())
                    .or_insert_with(|| OperationLatency {
                        operation_name: span.operation_name.clone(),
                        total_duration_us: 0,
                        span_count: 0,
                        avg_duration_us: 0,
                        p50_duration_us: 0,
                        p95_duration_us: 0,
                        p99_duration_us: 0,
                        durations: Vec::new(),
                    });

                entry.total_duration_us += duration;
                entry.span_count += 1;
                entry.durations.push(duration);
            }
        }

        // Calculate statistics
        for latency in latency_by_operation.values_mut() {
            if latency.span_count > 0 {
                latency.avg_duration_us = latency.total_duration_us / latency.span_count as i64;

                // Calculate percentiles
                let mut sorted = latency.durations.clone();
                sorted.sort_unstable();

                let p50_idx = (sorted.len() as f64 * 0.50) as usize;
                let p95_idx = (sorted.len() as f64 * 0.95) as usize;
                let p99_idx = (sorted.len() as f64 * 0.99) as usize;

                latency.p50_duration_us = sorted.get(p50_idx).copied().unwrap_or(0);
                latency.p95_duration_us = sorted.get(p95_idx).copied().unwrap_or(0);
                latency.p99_duration_us = sorted.get(p99_idx).copied().unwrap_or(0);
            }
        }

        latency_by_operation
    }
}

/// Service latency statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLatency {
    /// Name of the service
    pub service_name: String,
    /// Sum of all span durations for this service in microseconds
    pub total_duration_us: i64,
    /// Number of spans counted
    pub span_count: usize,
    /// Average span duration in microseconds
    pub avg_duration_us: i64,
    /// Minimum span duration in microseconds
    pub min_duration_us: i64,
    /// Maximum span duration in microseconds
    pub max_duration_us: i64,
}

/// Operation latency statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLatency {
    /// Name of the operation
    pub operation_name: String,
    /// Sum of all span durations for this operation in microseconds
    pub total_duration_us: i64,
    /// Number of spans counted
    pub span_count: usize,
    /// Average duration in microseconds
    pub avg_duration_us: i64,
    /// 50th-percentile (median) duration in microseconds
    pub p50_duration_us: i64,
    /// 95th-percentile duration in microseconds
    pub p95_duration_us: i64,
    /// 99th-percentile duration in microseconds
    pub p99_duration_us: i64,
    #[serde(skip)]
    durations: Vec<i64>,
}

/// Transaction flow visualizer
pub struct FlowVisualizer;

impl FlowVisualizer {
    /// Generate a tree representation of the trace
    pub fn generate_tree(trace: &Trace) -> String {
        let mut output = String::new();

        // Find root span
        let root_span = trace
            .spans
            .iter()
            .find(|s| s.parent_span_id.is_none())
            .or_else(|| trace.spans.first());

        if let Some(root) = root_span {
            Self::append_span(&mut output, root, &trace.spans, 0);
        }

        output
    }

    fn append_span(output: &mut String, span: &Span, all_spans: &[Span], indent: usize) {
        let indent_str = "  ".repeat(indent);
        let duration_ms = span.duration_us.unwrap_or(0) / 1000;
        let status_char = match span.status {
            SpanStatus::Ok => "✓",
            SpanStatus::Error => "✗",
            SpanStatus::Cancelled => "⊘",
        };

        output.push_str(&format!(
            "{}{} {} [{}ms] - {}\n",
            indent_str, status_char, span.operation_name, duration_ms, span.service_name
        ));

        // Find and append children
        for child_span in all_spans
            .iter()
            .filter(|s| s.parent_span_id.as_ref() == Some(&span.span_id))
        {
            Self::append_span(output, child_span, all_spans, indent + 1);
        }
    }

    /// Generate critical path through the trace
    pub fn critical_path(trace: &Trace) -> Vec<String> {
        let mut path = Vec::new();

        // Find the longest path through the trace
        let mut current = trace
            .spans
            .iter()
            .max_by_key(|s| s.duration_us.unwrap_or(0));

        while let Some(span) = current {
            path.push(format!(
                "{} ({}ms)",
                span.operation_name,
                span.duration_us.unwrap_or(0) / 1000
            ));

            // Find parent
            current = span
                .parent_span_id
                .as_ref()
                .and_then(|pid| trace.spans.iter().find(|s| &s.span_id == pid));
        }

        path.reverse();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.sampled);
    }

    #[test]
    fn test_trace_context_child() {
        let parent = TraceContext::new();
        let child = parent.child();

        assert_eq!(parent.trace_id, child.trace_id);
        assert_ne!(parent.span_id, child.span_id);
    }

    #[test]
    fn test_trace_context_inject_extract() {
        let ctx = TraceContext::new();
        let headers = ctx.inject();
        let extracted = TraceContext::extract(&headers).unwrap();

        assert_eq!(ctx.trace_id, extracted.trace_id);
        assert_eq!(ctx.span_id, extracted.span_id);
    }

    #[test]
    fn test_span_creation() {
        let span = Span::new(
            "trace-1".to_string(),
            "test-service".to_string(),
            "test-operation".to_string(),
            None,
        );

        assert!(!span.span_id.is_empty());
        assert_eq!(span.trace_id, "trace-1");
        assert_eq!(span.service_name, "test-service");
        assert_eq!(span.operation_name, "test-operation");
        assert_eq!(span.status, SpanStatus::Ok);
    }

    #[test]
    fn test_span_finish() {
        let mut span = Span::new(
            "trace-1".to_string(),
            "test-service".to_string(),
            "test-operation".to_string(),
            None,
        );

        std::thread::sleep(std::time::Duration::from_millis(10));
        span.finish();

        assert!(span.end_time.is_some());
        assert!(span.duration_us.is_some());
        assert!(span.duration_us.unwrap() > 0);
    }

    #[tokio::test]
    async fn test_distributed_tracer() {
        let tracer = DistributedTracer::new("test-service".to_string(), 1.0);

        let span = tracer.start_span("test-operation".to_string(), None).await;
        let trace_id = span.trace_id.clone();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        tracer.finish_span(span).await;

        let trace = tracer.get_trace(&trace_id).await;
        assert!(trace.is_some());

        let trace = trace.unwrap();
        assert_eq!(trace.span_count, 1);
        assert_eq!(trace.service_count, 1);
    }
}
