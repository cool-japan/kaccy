//! API middleware

pub mod api_key;
pub mod audit;
pub mod auth;
pub mod cache;
pub mod circuit_breaker;
pub mod ip_filter;
pub mod logging;
pub mod metrics;
pub mod profiling;
pub mod rate_limit;
pub mod request_id;
pub mod request_signing;
pub mod request_validation;
pub mod security_headers;
pub mod telemetry;
pub mod versioning;

pub use api_key::{
    API_KEY_HEADER, API_KEY_PREFIX, ApiKeyAuth, ApiKeyScope, api_key_middleware,
    require_api_key_scope,
};
pub use audit::{AuditEvent, AuditEventType, AuditResult};
pub use auth::*;
pub use cache::{CacheConfig, CacheInvalidator, cache_middleware};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState,
};
pub use ip_filter::{IpFilter, IpFilterConfig, IpFilterStats, extract_ip, ip_filter_middleware};
pub use logging::logging_middleware;
pub use metrics::{init_metrics, metrics_handler, metrics_middleware, update_db_pool_metrics};
pub use profiling::{ProfiledOperation, Profiler, profiling_middleware};
pub use rate_limit::{
    DynamicRateLimiter, EndpointRateLimiter, LoginAttemptTracker, RateLimitAnalytics,
    RateLimitConfig, RateLimitResult, RateLimiter, UserRateLimiter, rate_limit_middleware,
};
pub use request_id::{REQUEST_ID_HEADER, RequestId, request_id_middleware};
pub use request_signing::{
    NONCE_HEADER, NonceTracker, RequestSigner, SIGNATURE_HEADER, SigningConfig, SigningError,
    SigningVerifier, TIMESTAMP_HEADER, request_signing_middleware,
};
pub use request_validation::{
    RequestValidationConfig, ValidationError, request_validation_middleware,
};
pub use security_headers::{
    FrameOptions, ReferrerPolicy, SecurityHeadersConfig, security_headers_middleware,
};
pub use telemetry::{
    TracingConfig, init_telemetry, inject_trace_context, record_error, shutdown_telemetry,
    tracing_middleware,
};
pub use versioning::{
    ApiVersion, VersionError, extract_version, require_version, version_middleware,
};
