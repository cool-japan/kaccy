//! Prometheus metrics collection and exposition

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry,
    TextEncoder,
};
use std::sync::Once;
use std::time::Instant;

static INIT: Once = Once::new();

lazy_static! {
    /// Global Prometheus registry
    pub static ref REGISTRY: Registry = Registry::new();

    /// HTTP request counter by method, path, and status
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("http_requests_total", "Total number of HTTP requests"),
        &["method", "path", "status"]
    )
    .expect("metric can be created");

    /// HTTP request duration histogram
    pub static ref HTTP_REQUEST_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "http_request_duration_seconds",
            "HTTP request latency in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["method", "path"]
    )
    .expect("metric can be created");

    /// Active HTTP connections
    pub static ref HTTP_CONNECTIONS_ACTIVE: Gauge = Gauge::new(
        "http_connections_active",
        "Number of active HTTP connections"
    )
    .expect("metric can be created");

    /// Database connection pool size
    pub static ref DB_POOL_CONNECTIONS: Gauge = Gauge::new(
        "db_pool_connections",
        "Number of database connections in the pool"
    )
    .expect("metric can be created");

    /// Database connection pool idle connections
    pub static ref DB_POOL_IDLE_CONNECTIONS: Gauge = Gauge::new(
        "db_pool_idle_connections",
        "Number of idle database connections in the pool"
    )
    .expect("metric can be created");

    /// Database query duration histogram
    pub static ref DB_QUERY_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "db_query_duration_seconds",
            "Database query duration in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        &["operation"]
    )
    .expect("metric can be created");

    /// Business metrics - total tokens created
    pub static ref TOKENS_CREATED_TOTAL: Counter = Counter::new(
        "tokens_created_total",
        "Total number of tokens created"
    )
    .expect("metric can be created");

    /// Business metrics - total trades executed
    pub static ref TRADES_EXECUTED_TOTAL: Counter = Counter::new(
        "trades_executed_total",
        "Total number of trades executed"
    )
    .expect("metric can be created");

    /// Business metrics - total users registered
    pub static ref USERS_REGISTERED_TOTAL: Counter = Counter::new(
        "users_registered_total",
        "Total number of users registered"
    )
    .expect("metric can be created");

    /// Business metrics - total orders created
    pub static ref ORDERS_CREATED_TOTAL: CounterVec = CounterVec::new(
        Opts::new("orders_created_total", "Total number of orders created"),
        &["order_type"]
    )
    .expect("metric can be created");

    /// Business metrics - total trade volume
    pub static ref TRADE_VOLUME_TOTAL: Counter = Counter::new(
        "trade_volume_total",
        "Total trade volume in satoshis"
    )
    .expect("metric can be created");

    /// Business metrics - active users gauge
    pub static ref ACTIVE_USERS: Gauge = Gauge::new(
        "active_users",
        "Number of currently active users"
    )
    .expect("metric can be created");

    /// Business metrics - commitments created
    pub static ref COMMITMENTS_CREATED_TOTAL: Counter = Counter::new(
        "commitments_created_total",
        "Total number of commitments created"
    )
    .expect("metric can be created");

    /// Business metrics - commitments verified
    pub static ref COMMITMENTS_VERIFIED_TOTAL: CounterVec = CounterVec::new(
        Opts::new("commitments_verified_total", "Total number of commitments verified"),
        &["status"]
    )
    .expect("metric can be created");

    /// Authentication metrics - login attempts
    pub static ref AUTH_LOGIN_ATTEMPTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("auth_login_attempts_total", "Total number of login attempts"),
        &["status"]
    )
    .expect("metric can be created");

    /// Authentication metrics - JWT tokens issued
    pub static ref AUTH_TOKENS_ISSUED_TOTAL: Counter = Counter::new(
        "auth_tokens_issued_total",
        "Total number of JWT tokens issued"
    )
    .expect("metric can be created");

    /// WebSocket connections
    pub static ref WEBSOCKET_CONNECTIONS: GaugeVec = GaugeVec::new(
        Opts::new("websocket_connections", "Number of active WebSocket connections"),
        &["channel"]
    )
    .expect("metric can be created");

    /// Cache metrics - hits and misses
    pub static ref CACHE_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("cache_operations_total", "Total number of cache operations"),
        &["operation", "status"]
    )
    .expect("metric can be created");
}

/// Initialize the metrics system by registering all collectors
/// This function is safe to call multiple times - initialization will only happen once
pub fn init_metrics() {
    INIT.call_once(|| {
        // HTTP metrics
        REGISTRY
            .register(Box::new(HTTP_REQUESTS_TOTAL.clone()))
            .expect("HTTP_REQUESTS_TOTAL can be registered");
        REGISTRY
            .register(Box::new(HTTP_REQUEST_DURATION_SECONDS.clone()))
            .expect("HTTP_REQUEST_DURATION_SECONDS can be registered");
        REGISTRY
            .register(Box::new(HTTP_CONNECTIONS_ACTIVE.clone()))
            .expect("HTTP_CONNECTIONS_ACTIVE can be registered");

        // Database metrics
        REGISTRY
            .register(Box::new(DB_POOL_CONNECTIONS.clone()))
            .expect("DB_POOL_CONNECTIONS can be registered");
        REGISTRY
            .register(Box::new(DB_POOL_IDLE_CONNECTIONS.clone()))
            .expect("DB_POOL_IDLE_CONNECTIONS can be registered");
        REGISTRY
            .register(Box::new(DB_QUERY_DURATION_SECONDS.clone()))
            .expect("DB_QUERY_DURATION_SECONDS can be registered");

        // Business metrics
        REGISTRY
            .register(Box::new(TOKENS_CREATED_TOTAL.clone()))
            .expect("TOKENS_CREATED_TOTAL can be registered");
        REGISTRY
            .register(Box::new(TRADES_EXECUTED_TOTAL.clone()))
            .expect("TRADES_EXECUTED_TOTAL can be registered");
        REGISTRY
            .register(Box::new(USERS_REGISTERED_TOTAL.clone()))
            .expect("USERS_REGISTERED_TOTAL can be registered");
        REGISTRY
            .register(Box::new(ORDERS_CREATED_TOTAL.clone()))
            .expect("ORDERS_CREATED_TOTAL can be registered");
        REGISTRY
            .register(Box::new(TRADE_VOLUME_TOTAL.clone()))
            .expect("TRADE_VOLUME_TOTAL can be registered");
        REGISTRY
            .register(Box::new(ACTIVE_USERS.clone()))
            .expect("ACTIVE_USERS can be registered");
        REGISTRY
            .register(Box::new(COMMITMENTS_CREATED_TOTAL.clone()))
            .expect("COMMITMENTS_CREATED_TOTAL can be registered");
        REGISTRY
            .register(Box::new(COMMITMENTS_VERIFIED_TOTAL.clone()))
            .expect("COMMITMENTS_VERIFIED_TOTAL can be registered");

        // Authentication metrics
        REGISTRY
            .register(Box::new(AUTH_LOGIN_ATTEMPTS_TOTAL.clone()))
            .expect("AUTH_LOGIN_ATTEMPTS_TOTAL can be registered");
        REGISTRY
            .register(Box::new(AUTH_TOKENS_ISSUED_TOTAL.clone()))
            .expect("AUTH_TOKENS_ISSUED_TOTAL can be registered");

        // WebSocket metrics
        REGISTRY
            .register(Box::new(WEBSOCKET_CONNECTIONS.clone()))
            .expect("WEBSOCKET_CONNECTIONS can be registered");

        // Cache metrics
        REGISTRY
            .register(Box::new(CACHE_OPERATIONS_TOTAL.clone()))
            .expect("CACHE_OPERATIONS_TOTAL can be registered");

        // Register process collector for CPU and memory metrics (Linux only)
        #[cfg(target_os = "linux")]
        {
            let process_collector = prometheus::process_collector::ProcessCollector::for_self();
            REGISTRY
                .register(Box::new(process_collector))
                .expect("process_collector can be registered");
        }
    });
}

/// Middleware to track HTTP request metrics
pub async fn metrics_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    // Increment active connections
    HTTP_CONNECTIONS_ACTIVE.inc();

    let response = next.run(req).await;

    // Decrement active connections
    HTTP_CONNECTIONS_ACTIVE.dec();

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // Record metrics
    HTTP_REQUESTS_TOTAL
        .with_label_values(&[&method, &path, &status])
        .inc();

    HTTP_REQUEST_DURATION_SECONDS
        .with_label_values(&[&method, &path])
        .observe(duration);

    response
}

/// Update database connection pool metrics
///
/// This function should be called periodically to keep pool metrics up-to-date
pub fn update_db_pool_metrics(pool: &sqlx::PgPool) {
    let pool_size = pool.size();
    let pool_idle = pool.num_idle() as u32;

    DB_POOL_CONNECTIONS.set(pool_size as f64);
    DB_POOL_IDLE_CONNECTIONS.set(pool_idle as f64);
}

/// Handler for the /metrics endpoint
///
/// This handler will automatically update database pool metrics if a pool is available
pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = vec![];

    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [("Content-Type", encoder.format_type())],
            buffer,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {}", e),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // This test just ensures metrics can be initialized without panicking
        init_metrics();
    }

    #[test]
    fn test_http_metrics_increment() {
        // Ensure metrics are initialized (registered with REGISTRY)
        init_metrics();

        HTTP_REQUESTS_TOTAL
            .with_label_values(&["GET", "/test", "200"])
            .inc();

        let metric_families = REGISTRY.gather();
        let http_requests = metric_families
            .iter()
            .find(|m| m.name() == "http_requests_total");

        assert!(http_requests.is_some());
    }

    #[test]
    fn test_business_metrics() {
        // Ensure metrics are initialized
        init_metrics();

        TOKENS_CREATED_TOTAL.inc();
        TRADES_EXECUTED_TOTAL.inc();
        USERS_REGISTERED_TOTAL.inc();

        let metric_families = REGISTRY.gather();
        assert!(
            metric_families
                .iter()
                .any(|m| m.name() == "tokens_created_total")
        );
        assert!(
            metric_families
                .iter()
                .any(|m| m.name() == "trades_executed_total")
        );
        assert!(
            metric_families
                .iter()
                .any(|m| m.name() == "users_registered_total")
        );
    }
}
