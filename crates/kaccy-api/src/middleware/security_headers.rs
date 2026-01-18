//! Security headers middleware
//!
//! Adds security-related HTTP headers to responses for better protection

use axum::{
    extract::Request,
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// Security headers configuration
#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    /// Enable Strict-Transport-Security (HSTS)
    pub enable_hsts: bool,
    /// HSTS max-age in seconds (default: 1 year)
    pub hsts_max_age: u32,
    /// Include subdomains in HSTS
    pub hsts_include_subdomains: bool,
    /// Enable X-Frame-Options
    pub enable_frame_options: bool,
    /// X-Frame-Options value (DENY, SAMEORIGIN)
    pub frame_options_value: FrameOptions,
    /// Enable X-Content-Type-Options
    pub enable_content_type_options: bool,
    /// Enable X-XSS-Protection
    pub enable_xss_protection: bool,
    /// Enable Referrer-Policy
    pub enable_referrer_policy: bool,
    /// Referrer-Policy value
    pub referrer_policy: ReferrerPolicy,
    /// Enable Content-Security-Policy
    pub enable_csp: bool,
    /// Content-Security-Policy directives
    pub csp_directives: Vec<String>,
    /// Enable Permissions-Policy
    pub enable_permissions_policy: bool,
    /// Permissions-Policy directives
    pub permissions_policy: Vec<String>,
}

/// X-Frame-Options values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOptions {
    /// Deny all framing
    Deny,
    /// Allow same-origin framing
    SameOrigin,
}

impl std::fmt::Display for FrameOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameOptions::Deny => write!(f, "DENY"),
            FrameOptions::SameOrigin => write!(f, "SAMEORIGIN"),
        }
    }
}

/// Referrer-Policy values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferrerPolicy {
    /// No referrer information
    NoReferrer,
    /// No referrer when downgrading
    NoReferrerWhenDowngrade,
    /// Origin only
    Origin,
    /// Origin when cross-origin
    OriginWhenCrossOrigin,
    /// Same origin
    SameOrigin,
    /// Strict origin
    StrictOrigin,
    /// Strict origin when cross-origin
    StrictOriginWhenCrossOrigin,
    /// Unsafe URL
    UnsafeUrl,
}

impl std::fmt::Display for ReferrerPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferrerPolicy::NoReferrer => write!(f, "no-referrer"),
            ReferrerPolicy::NoReferrerWhenDowngrade => write!(f, "no-referrer-when-downgrade"),
            ReferrerPolicy::Origin => write!(f, "origin"),
            ReferrerPolicy::OriginWhenCrossOrigin => write!(f, "origin-when-cross-origin"),
            ReferrerPolicy::SameOrigin => write!(f, "same-origin"),
            ReferrerPolicy::StrictOrigin => write!(f, "strict-origin"),
            ReferrerPolicy::StrictOriginWhenCrossOrigin => {
                write!(f, "strict-origin-when-cross-origin")
            }
            ReferrerPolicy::UnsafeUrl => write!(f, "unsafe-url"),
        }
    }
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            enable_frame_options: true,
            frame_options_value: FrameOptions::Deny,
            enable_content_type_options: true,
            enable_xss_protection: true,
            enable_referrer_policy: true,
            referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin,
            enable_csp: true,
            csp_directives: vec![
                "default-src 'self'".to_string(),
                "script-src 'self' 'unsafe-inline'".to_string(),
                "style-src 'self' 'unsafe-inline'".to_string(),
                "img-src 'self' data: https:".to_string(),
                "font-src 'self'".to_string(),
                "connect-src 'self'".to_string(),
                "frame-ancestors 'none'".to_string(),
                "base-uri 'self'".to_string(),
                "form-action 'self'".to_string(),
            ],
            enable_permissions_policy: true,
            permissions_policy: vec![
                "geolocation=()".to_string(),
                "microphone=()".to_string(),
                "camera=()".to_string(),
                "payment=()".to_string(),
                "usb=()".to_string(),
            ],
        }
    }
}

/// Security headers middleware
pub async fn security_headers_middleware(
    config: axum::Extension<SecurityHeadersConfig>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // HSTS (HTTP Strict Transport Security)
    if config.enable_hsts {
        let mut hsts_value = format!("max-age={}", config.hsts_max_age);
        if config.hsts_include_subdomains {
            hsts_value.push_str("; includeSubDomains");
        }
        if let Ok(header_value) = HeaderValue::from_str(&hsts_value) {
            headers.insert(header::STRICT_TRANSPORT_SECURITY, header_value);
        }
    }

    // X-Frame-Options
    if config.enable_frame_options {
        if let Ok(header_value) = HeaderValue::from_str(&config.frame_options_value.to_string()) {
            headers.insert(
                header::HeaderName::from_static("x-frame-options"),
                header_value,
            );
        }
    }

    // X-Content-Type-Options
    if config.enable_content_type_options {
        headers.insert(
            header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        );
    }

    // X-XSS-Protection
    if config.enable_xss_protection {
        headers.insert(
            header::HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        );
    }

    // Referrer-Policy
    if config.enable_referrer_policy {
        if let Ok(header_value) = HeaderValue::from_str(&config.referrer_policy.to_string()) {
            headers.insert(
                header::HeaderName::from_static("referrer-policy"),
                header_value,
            );
        }
    }

    // Content-Security-Policy
    if config.enable_csp && !config.csp_directives.is_empty() {
        let csp_value = config.csp_directives.join("; ");
        if let Ok(header_value) = HeaderValue::from_str(&csp_value) {
            headers.insert(
                header::HeaderName::from_static("content-security-policy"),
                header_value,
            );
        }
    }

    // Permissions-Policy
    if config.enable_permissions_policy && !config.permissions_policy.is_empty() {
        let permissions_value = config.permissions_policy.join(", ");
        if let Ok(header_value) = HeaderValue::from_str(&permissions_value) {
            headers.insert(
                header::HeaderName::from_static("permissions-policy"),
                header_value,
            );
        }
    }

    response
}

/// Create a relaxed security headers config for development
impl SecurityHeadersConfig {
    /// Create a development-friendly configuration
    pub fn development() -> Self {
        Self {
            enable_hsts: false,
            enable_csp: true,
            csp_directives: vec![
                "default-src 'self' 'unsafe-inline' 'unsafe-eval'".to_string(),
                "img-src 'self' data: https: http:".to_string(),
            ],
            ..Default::default()
        }
    }

    /// Create a production configuration with strict security
    pub fn production() -> Self {
        Self::default()
    }

    /// Create a configuration optimized for APIs
    pub fn api() -> Self {
        Self {
            enable_hsts: true,
            hsts_max_age: 31536000,
            hsts_include_subdomains: true,
            enable_frame_options: true,
            frame_options_value: FrameOptions::Deny,
            enable_content_type_options: true,
            enable_xss_protection: true,
            enable_referrer_policy: true,
            referrer_policy: ReferrerPolicy::NoReferrer,
            enable_csp: true,
            csp_directives: vec![
                "default-src 'none'".to_string(),
                "frame-ancestors 'none'".to_string(),
            ],
            enable_permissions_policy: true,
            permissions_policy: vec![
                "geolocation=()".to_string(),
                "microphone=()".to_string(),
                "camera=()".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_options_display() {
        assert_eq!(FrameOptions::Deny.to_string(), "DENY");
        assert_eq!(FrameOptions::SameOrigin.to_string(), "SAMEORIGIN");
    }

    #[test]
    fn test_referrer_policy_display() {
        assert_eq!(ReferrerPolicy::NoReferrer.to_string(), "no-referrer");
        assert_eq!(
            ReferrerPolicy::StrictOriginWhenCrossOrigin.to_string(),
            "strict-origin-when-cross-origin"
        );
    }

    #[test]
    fn test_default_config() {
        let config = SecurityHeadersConfig::default();
        assert!(config.enable_hsts);
        assert!(config.enable_frame_options);
        assert!(config.enable_content_type_options);
        assert!(config.enable_xss_protection);
        assert!(config.enable_referrer_policy);
        assert!(config.enable_csp);
        assert!(config.enable_permissions_policy);
    }

    #[test]
    fn test_development_config() {
        let config = SecurityHeadersConfig::development();
        assert!(!config.enable_hsts);
        assert!(config.enable_csp);
    }

    #[test]
    fn test_api_config() {
        let config = SecurityHeadersConfig::api();
        assert!(config.enable_hsts);
        assert_eq!(config.referrer_policy, ReferrerPolicy::NoReferrer);
        assert_eq!(config.frame_options_value, FrameOptions::Deny);
    }
}
