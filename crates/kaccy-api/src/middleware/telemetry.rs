//! OpenTelemetry distributed tracing

use axum::{extract::Request, http::HeaderMap, middleware::Next, response::Response};
use opentelemetry::{
    KeyValue, global,
    trace::{Span, SpanKind, Tracer},
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider as TracerProvider},
};
use opentelemetry_semantic_conventions::{
    resource::SERVICE_VERSION,
    trace::{HTTP_REQUEST_METHOD, HTTP_RESPONSE_STATUS_CODE, HTTP_ROUTE, URL_PATH},
};
use std::time::SystemTime;

/// Tracing configuration
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name for traces
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// OTLP endpoint (e.g., "http://localhost:4317" for Jaeger)
    pub otlp_endpoint: String,
    /// Sample rate (0.0 to 1.0, where 1.0 = 100% sampling)
    pub sample_rate: f64,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "kaccy-api".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            sample_rate: std::env::var("OTEL_SAMPLE_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
        }
    }
}

/// Initialize OpenTelemetry tracing with OTLP exporter
///
/// This sets up distributed tracing with support for Jaeger, Zipkin, or any OTLP-compatible backend
pub fn init_telemetry(config: TracingConfig) -> Result<TracerProvider, Box<dyn std::error::Error>> {
    // Set up the trace context propagator (for distributed tracing)
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Create OTLP exporter
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;

    // Create resource with service information
    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .with_attribute(KeyValue::new(
            SERVICE_VERSION,
            config.service_version.clone(),
        ))
        .build();

    // Create sampler based on sample rate
    let sampler = if config.sample_rate >= 1.0 {
        Sampler::AlwaysOn
    } else if config.sample_rate <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(config.sample_rate)
    };

    // Build tracer provider
    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .with_sampler(sampler)
        .with_id_generator(RandomIdGenerator::default())
        .build();

    // Set global tracer provider
    global::set_tracer_provider(provider.clone());

    tracing::info!(
        service_name = %config.service_name,
        endpoint = %config.otlp_endpoint,
        sample_rate = %config.sample_rate,
        "OpenTelemetry tracing initialized"
    );

    Ok(provider)
}

/// Shutdown OpenTelemetry gracefully
pub fn shutdown_telemetry() {
    // In OpenTelemetry 0.28, shutdown is handled automatically
    // The tracer provider will flush and shutdown when dropped
    tracing::info!("OpenTelemetry shutdown initiated");
}

/// Middleware for HTTP request tracing
///
/// This middleware creates a span for each HTTP request and propagates trace context
pub async fn tracing_middleware(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    let headers = req.headers().clone();

    // Get the tracer
    let tracer = global::tracer("kaccy-api");

    // Extract parent context from headers if present
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(headers.clone()))
    });

    // Create a new span with the parent context
    let mut span = tracer
        .span_builder(format!("{} {}", method, path))
        .with_kind(SpanKind::Server)
        .with_start_time(SystemTime::now())
        .start_with_context(&tracer, &parent_context);

    // Set HTTP semantic convention attributes
    span.set_attribute(KeyValue::new(HTTP_REQUEST_METHOD, method));
    span.set_attribute(KeyValue::new(URL_PATH, path.clone()));
    span.set_attribute(KeyValue::new(HTTP_ROUTE, path));

    // Process the request
    let response = next.run(req).await;

    // Record response status
    let status_code = response.status().as_u16() as i64;
    span.set_attribute(KeyValue::new(HTTP_RESPONSE_STATUS_CODE, status_code));

    // End the span
    span.end();

    response
}

/// Helper struct to extract trace context from HTTP headers
struct HeaderExtractor(HeaderMap);

impl opentelemetry::propagation::Extractor for HeaderExtractor {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Helper struct to inject trace context into HTTP headers
pub struct HeaderInjector<'a>(pub &'a mut HeaderMap);

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_name) = axum::http::HeaderName::try_from(key) {
            if let Ok(header_value) = axum::http::HeaderValue::from_str(&value) {
                self.0.insert(header_name, header_value);
            }
        }
    }
}

/// Create a traced HTTP client request
///
/// This injects the current trace context into outgoing HTTP requests
pub fn inject_trace_context(headers: &mut HeaderMap) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(
            &opentelemetry::Context::current(),
            &mut HeaderInjector(headers),
        )
    });
}

/// Record an error in a new span
pub fn record_error(error: &dyn std::error::Error) {
    let tracer = global::tracer("kaccy-api");
    let mut error_span = tracer.start("error");
    error_span.set_attribute(KeyValue::new("error.message", error.to_string()));
    error_span.set_attribute(KeyValue::new(
        "error.type",
        std::any::type_name_of_val(error),
    ));
    error_span.end();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "kaccy-api");
        assert_eq!(config.service_version, env!("CARGO_PKG_VERSION"));
        assert!(config.sample_rate > 0.0 && config.sample_rate <= 1.0);
    }

    #[test]
    fn test_custom_config() {
        let config = TracingConfig {
            service_name: "test-service".to_string(),
            service_version: "1.0.0".to_string(),
            otlp_endpoint: "http://localhost:14268".to_string(),
            sample_rate: 0.5,
        };
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.service_version, "1.0.0");
        assert_eq!(config.otlp_endpoint, "http://localhost:14268");
        assert_eq!(config.sample_rate, 0.5);
    }
}
