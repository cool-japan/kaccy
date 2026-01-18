//! Request ID middleware for request tracking

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use uuid::Uuid;

/// Header name for request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Request ID marker for extension
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware that adds request ID to each request
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    // Check if request already has an ID from client
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| RequestId(s.to_string()))
        .unwrap_or_else(RequestId::new);

    // Store in extensions for handlers to access
    req.extensions_mut().insert(request_id.clone());

    // Add to tracing span
    let span = tracing::info_span!(
        "request",
        request_id = %request_id.0,
        method = %req.method(),
        uri = %req.uri(),
    );

    let _guard = span.enter();

    let mut response = next.run(req).await;

    // Add request ID to response headers
    if let Ok(value) = HeaderValue::from_str(&request_id.0) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

/// Extract request ID from request extensions
pub fn get_request_id(req: &Request) -> Option<String> {
    req.extensions().get::<RequestId>().map(|r| r.0.clone())
}
