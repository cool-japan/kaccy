//! Request validation middleware
//!
//! Validates incoming requests for size, content type, and other properties

use axum::{
    Json,
    extract::Request,
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::collections::HashSet;

/// Request validation configuration
#[derive(Debug, Clone)]
pub struct RequestValidationConfig {
    /// Maximum request body size in bytes (default: 10MB)
    pub max_body_size: usize,
    /// Allowed content types for POST/PUT/PATCH requests
    pub allowed_content_types: HashSet<String>,
    /// Maximum URL length
    pub max_url_length: usize,
    /// Maximum header size in bytes
    pub max_header_size: usize,
    /// Enforce HTTPS (reject HTTP requests)
    pub enforce_https: bool,
    /// Allowed HTTP methods
    pub allowed_methods: HashSet<Method>,
}

impl Default for RequestValidationConfig {
    fn default() -> Self {
        let mut allowed_content_types = HashSet::new();
        allowed_content_types.insert("application/json".to_string());
        allowed_content_types.insert("application/x-www-form-urlencoded".to_string());
        allowed_content_types.insert("multipart/form-data".to_string());

        let mut allowed_methods = HashSet::new();
        allowed_methods.insert(Method::GET);
        allowed_methods.insert(Method::POST);
        allowed_methods.insert(Method::PUT);
        allowed_methods.insert(Method::PATCH);
        allowed_methods.insert(Method::DELETE);
        allowed_methods.insert(Method::HEAD);
        allowed_methods.insert(Method::OPTIONS);

        Self {
            max_body_size: 10 * 1024 * 1024, // 10MB
            allowed_content_types,
            max_url_length: 2048,
            max_header_size: 8 * 1024, // 8KB
            enforce_https: false,
            allowed_methods,
        }
    }
}

impl RequestValidationConfig {
    /// Create a strict configuration for production
    pub fn strict() -> Self {
        Self {
            max_body_size: 5 * 1024 * 1024, // 5MB
            max_url_length: 1024,
            max_header_size: 4 * 1024, // 4KB
            enforce_https: true,
            ..Default::default()
        }
    }

    /// Create a relaxed configuration for development
    pub fn relaxed() -> Self {
        Self {
            max_body_size: 50 * 1024 * 1024, // 50MB
            max_url_length: 4096,
            enforce_https: false,
            ..Default::default()
        }
    }
}

/// Validation error response
#[derive(Debug)]
pub enum ValidationError {
    /// Request body too large
    BodyTooLarge {
        /// Actual body size in bytes
        size: usize,
        /// Maximum allowed body size in bytes
        max: usize,
    },
    /// Invalid content type
    InvalidContentType {
        /// The content type that was received
        actual: String,
        /// The list of permitted content types
        allowed: Vec<String>,
    },
    /// URL too long
    UrlTooLong {
        /// Actual URL length in bytes
        length: usize,
        /// Maximum allowed URL length in bytes
        max: usize,
    },
    /// Header too large
    HeaderTooLarge {
        /// Actual header size in bytes
        size: usize,
        /// Maximum allowed header size in bytes
        max: usize,
    },
    /// HTTPS required
    HttpsRequired,
    /// Method not allowed
    MethodNotAllowed {
        /// The HTTP method that was used
        method: String,
    },
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ValidationError::BodyTooLarge { size, max } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "Request body too large: {} bytes (max: {} bytes)",
                    size, max
                ),
            ),
            ValidationError::InvalidContentType { actual, allowed } => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "Invalid content type: {}. Allowed: {}",
                    actual,
                    allowed.join(", ")
                ),
            ),
            ValidationError::UrlTooLong { length, max } => (
                StatusCode::URI_TOO_LONG,
                format!("URL too long: {} characters (max: {})", length, max),
            ),
            ValidationError::HeaderTooLarge { size, max } => (
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                format!("Header too large: {} bytes (max: {} bytes)", size, max),
            ),
            ValidationError::HttpsRequired => {
                (StatusCode::UPGRADE_REQUIRED, "HTTPS required".to_string())
            }
            ValidationError::MethodNotAllowed { method } => (
                StatusCode::METHOD_NOT_ALLOWED,
                format!("Method not allowed: {}", method),
            ),
        };

        (
            status,
            Json(json!({
                "error": "validation_error",
                "message": message,
            })),
        )
            .into_response()
    }
}

/// Request validation middleware
pub async fn request_validation_middleware(
    config: axum::Extension<RequestValidationConfig>,
    request: Request,
    next: Next,
) -> Result<Response, ValidationError> {
    let (parts, body) = request.into_parts();

    // Validate HTTP method
    if !config.allowed_methods.contains(&parts.method) {
        return Err(ValidationError::MethodNotAllowed {
            method: parts.method.to_string(),
        });
    }

    // Validate HTTPS if enforced
    if config.enforce_https {
        if let Some(scheme) = parts.uri.scheme() {
            if scheme != &axum::http::uri::Scheme::HTTPS {
                return Err(ValidationError::HttpsRequired);
            }
        }
    }

    // Validate URL length
    let url = parts.uri.to_string();
    if url.len() > config.max_url_length {
        return Err(ValidationError::UrlTooLong {
            length: url.len(),
            max: config.max_url_length,
        });
    }

    // Validate header size
    let header_size: usize = parts
        .headers
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum();
    if header_size > config.max_header_size {
        return Err(ValidationError::HeaderTooLarge {
            size: header_size,
            max: config.max_header_size,
        });
    }

    // Validate content type for POST/PUT/PATCH requests
    if matches!(parts.method, Method::POST | Method::PUT | Method::PATCH) {
        if let Some(content_type) = parts.headers.get(header::CONTENT_TYPE) {
            let content_type_str = content_type.to_str().unwrap_or("");
            let base_type = content_type_str.split(';').next().unwrap_or("").trim();

            if !config.allowed_content_types.contains(base_type) && !base_type.is_empty() {
                return Err(ValidationError::InvalidContentType {
                    actual: base_type.to_string(),
                    allowed: config.allowed_content_types.iter().cloned().collect(),
                });
            }
        }
    }

    // Validate body size (if Content-Length header is present)
    if let Some(content_length) = parts.headers.get(header::CONTENT_LENGTH) {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                if length > config.max_body_size {
                    return Err(ValidationError::BodyTooLarge {
                        size: length,
                        max: config.max_body_size,
                    });
                }
            }
        }
    }

    // Reconstruct request and continue
    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RequestValidationConfig::default();
        assert_eq!(config.max_body_size, 10 * 1024 * 1024);
        assert_eq!(config.max_url_length, 2048);
        assert!(!config.enforce_https);
    }

    #[test]
    fn test_strict_config() {
        let config = RequestValidationConfig::strict();
        assert_eq!(config.max_body_size, 5 * 1024 * 1024);
        assert_eq!(config.max_url_length, 1024);
        assert!(config.enforce_https);
    }

    #[test]
    fn test_relaxed_config() {
        let config = RequestValidationConfig::relaxed();
        assert_eq!(config.max_body_size, 50 * 1024 * 1024);
        assert_eq!(config.max_url_length, 4096);
        assert!(!config.enforce_https);
    }
}
