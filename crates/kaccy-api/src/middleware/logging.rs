//! Request logging middleware

use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::time::Instant;

use super::request_id::RequestId;

/// Middleware that logs request details and timing
pub async fn logging_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Log request start
    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "Request started"
    );

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    // Log based on status code
    if status.is_server_error() {
        tracing::error!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            duration_ms = duration.as_millis(),
            "Request completed with server error"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            duration_ms = duration.as_millis(),
            "Request completed with client error"
        );
    } else {
        tracing::info!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = status.as_u16(),
            duration_ms = duration.as_millis(),
            "Request completed"
        );
    }

    response
}

/// Extension trait for Request to check content type
pub trait RequestExt {
    /// Returns true if the request has a JSON content type
    fn is_json(&self) -> bool;
}

impl RequestExt for Request<Body> {
    fn is_json(&self) -> bool {
        self.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.contains("application/json"))
            .unwrap_or(false)
    }
}

/// Extract status code category for logging
pub fn status_category(status: StatusCode) -> &'static str {
    match status.as_u16() {
        100..=199 => "informational",
        200..=299 => "success",
        300..=399 => "redirect",
        400..=499 => "client_error",
        500..=599 => "server_error",
        _ => "unknown",
    }
}
