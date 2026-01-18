//! API versioning middleware and utilities

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::fmt;

/// API version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ApiVersion {
    #[default]
    V1,
    // V2, // Future versions
}

impl ApiVersion {
    /// Get version from path prefix (e.g., "/api/v1/tokens" -> Some(V1))
    pub fn from_path(path: &str) -> Option<Self> {
        if path.starts_with("/api/v1/") || path == "/api/v1" {
            Some(ApiVersion::V1)
        } else {
            None
        }
    }

    /// Get version from Accept header (e.g., "application/vnd.kaccy.v1+json")
    pub fn from_accept_header(header: &str) -> Option<Self> {
        if header.contains("vnd.kaccy.v1") {
            Some(ApiVersion::V1)
        } else {
            None
        }
    }

    /// Convert to path prefix
    pub fn to_path_prefix(&self) -> &'static str {
        match self {
            ApiVersion::V1 => "/api/v1",
        }
    }

    /// Convert to header value
    pub fn to_header_value(&self) -> &'static str {
        match self {
            ApiVersion::V1 => "v1",
        }
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiVersion::V1 => write!(f, "v1"),
        }
    }
}

/// Extract API version from request
pub fn extract_version(req: &Request) -> ApiVersion {
    // First, try to get version from path
    if let Some(version) = ApiVersion::from_path(req.uri().path()) {
        return version;
    }

    // Then, try to get version from Accept header
    if let Some(accept) = req.headers().get("Accept") {
        if let Ok(accept_str) = accept.to_str() {
            if let Some(version) = ApiVersion::from_accept_header(accept_str) {
                return version;
            }
        }
    }

    // Default to V1
    ApiVersion::default()
}

/// Middleware that adds API version header to response
pub async fn version_middleware(req: Request, next: Next) -> Response {
    let version = extract_version(&req);

    let mut response = next.run(req).await;

    // Add API version header to response
    if let Ok(header_value) = HeaderValue::from_str(version.to_header_value()) {
        response.headers_mut().insert("X-API-Version", header_value);
    }

    response
}

/// Middleware that enforces minimum API version
pub fn require_version(
    min_version: ApiVersion,
) -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, Response> + Clone {
    move |req: Request, next: Next| {
        let min = min_version;
        Box::pin(async move {
            let version = extract_version(&req);

            if version < min {
                return (
                    StatusCode::NOT_ACCEPTABLE,
                    format!(
                        "API version {} is required, but {} was requested",
                        min, version
                    ),
                )
                    .into_response();
            }

            next.run(req).await
        })
    }
}

/// Version negotiation error response
pub struct VersionError {
    pub requested: Option<ApiVersion>,
    pub supported: Vec<ApiVersion>,
}

impl IntoResponse for VersionError {
    fn into_response(self) -> Response {
        let supported_versions: Vec<String> =
            self.supported.iter().map(|v| v.to_string()).collect();

        (
            StatusCode::NOT_ACCEPTABLE,
            format!(
                "Unsupported API version. Supported versions: {}",
                supported_versions.join(", ")
            ),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_from_path() {
        assert_eq!(
            ApiVersion::from_path("/api/v1/tokens"),
            Some(ApiVersion::V1)
        );
        assert_eq!(ApiVersion::from_path("/api/v1"), Some(ApiVersion::V1));
        assert_eq!(ApiVersion::from_path("/api/tokens"), None);
        assert_eq!(ApiVersion::from_path("/health"), None);
    }

    #[test]
    fn test_version_from_accept_header() {
        assert_eq!(
            ApiVersion::from_accept_header("application/vnd.kaccy.v1+json"),
            Some(ApiVersion::V1)
        );
        assert_eq!(ApiVersion::from_accept_header("application/json"), None);
    }

    #[test]
    fn test_version_ordering() {
        assert!(ApiVersion::V1 >= ApiVersion::V1);
        assert!(ApiVersion::V1 <= ApiVersion::V1);
        assert!(ApiVersion::V1 == ApiVersion::V1);
    }

    #[test]
    fn test_version_display() {
        assert_eq!(ApiVersion::V1.to_string(), "v1");
    }

    #[test]
    fn test_version_path_prefix() {
        assert_eq!(ApiVersion::V1.to_path_prefix(), "/api/v1");
    }
}
