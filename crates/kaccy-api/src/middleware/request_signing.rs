//! Request signing middleware for HMAC-based authentication

use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

/// Request signature header name
pub const SIGNATURE_HEADER: &str = "X-Signature";

/// Timestamp header name
pub const TIMESTAMP_HEADER: &str = "X-Timestamp";

/// Nonce header name (for replay attack prevention)
pub const NONCE_HEADER: &str = "X-Nonce";

/// Request signing configuration
#[derive(Debug, Clone)]
pub struct SigningConfig {
    /// Maximum age of a request (prevents replay attacks)
    pub max_age: Duration,
    /// Enable nonce tracking for replay prevention
    pub enable_nonce_tracking: bool,
    /// Nonce cache TTL
    pub nonce_ttl: Duration,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(300), // 5 minutes
            enable_nonce_tracking: true,
            nonce_ttl: Duration::from_secs(600), // 10 minutes
        }
    }
}

/// Nonce tracker for replay attack prevention
#[derive(Clone)]
pub struct NonceTracker {
    /// nonce -> expiration timestamp
    nonces: Arc<DashMap<String, DateTime<Utc>>>,
    /// TTL for nonces
    ttl: Duration,
}

impl NonceTracker {
    /// Create a new nonce tracker
    pub fn new(ttl: Duration) -> Self {
        Self {
            nonces: Arc::new(DashMap::new()),
            ttl,
        }
    }

    /// Check if a nonce has been used
    pub fn check_and_store(&self, nonce: &str) -> bool {
        // Cleanup expired nonces
        self.cleanup();

        let expires_at = Utc::now() + chrono::Duration::from_std(self.ttl).unwrap();

        // Try to insert the nonce
        if self.nonces.contains_key(nonce) {
            // Nonce already used
            false
        } else {
            self.nonces.insert(nonce.to_string(), expires_at);
            true
        }
    }

    /// Cleanup expired nonces
    fn cleanup(&self) {
        let now = Utc::now();
        self.nonces.retain(|_, expires_at| *expires_at > now);
    }
}

/// Request signer for generating HMAC signatures
pub struct RequestSigner {
    secret: Vec<u8>,
}

impl RequestSigner {
    /// Create a new request signer with a secret key
    pub fn new(secret: impl AsRef<[u8]>) -> Self {
        Self {
            secret: secret.as_ref().to_vec(),
        }
    }

    /// Generate signature for a request
    pub fn sign(
        &self,
        method: &str,
        path: &str,
        timestamp: &str,
        nonce: &str,
        body: &[u8],
    ) -> String {
        use hmac::{Hmac, KeyInit, Mac};
        type HmacSha256 = Hmac<Sha256>;

        // Create the message to sign: METHOD\nPATH\nTIMESTAMP\nNONCE\nBODY_HASH
        let body_hash = hex::encode(Sha256::digest(body));
        let message = format!(
            "{}\n{}\n{}\n{}\n{}",
            method, path, timestamp, nonce, body_hash
        );

        // Generate HMAC
        let mut mac =
            HmacSha256::new_from_slice(&self.secret).expect("HMAC can take key of any size");
        mac.update(message.as_bytes());

        hex::encode(mac.finalize().into_bytes().as_slice())
    }

    /// Verify a request signature
    pub fn verify(
        &self,
        signature: &str,
        method: &str,
        path: &str,
        timestamp: &str,
        nonce: &str,
        body: &[u8],
    ) -> bool {
        let expected = self.sign(method, path, timestamp, nonce, body);
        constant_time_eq(signature.as_bytes(), expected.as_bytes())
    }
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }

    result == 0
}

/// Request signing verifier
#[derive(Clone)]
pub struct SigningVerifier {
    signer: Arc<RequestSigner>,
    config: SigningConfig,
    nonce_tracker: Arc<NonceTracker>,
}

impl SigningVerifier {
    /// Create a new signing verifier
    pub fn new(secret: impl AsRef<[u8]>, config: SigningConfig) -> Self {
        Self {
            signer: Arc::new(RequestSigner::new(secret)),
            nonce_tracker: Arc::new(NonceTracker::new(config.nonce_ttl)),
            config,
        }
    }

    /// Verify a signed request
    pub fn verify_request(
        &self,
        signature: &str,
        method: &str,
        path: &str,
        timestamp: &str,
        nonce: &str,
        body: &[u8],
    ) -> Result<(), SigningError> {
        // Verify timestamp
        let request_time = timestamp
            .parse::<DateTime<Utc>>()
            .map_err(|_| SigningError::InvalidTimestamp)?;

        let now = Utc::now();
        let age = (now - request_time)
            .to_std()
            .map_err(|_| SigningError::InvalidTimestamp)?;

        if age > self.config.max_age {
            return Err(SigningError::RequestTooOld);
        }

        // Verify nonce (replay attack prevention)
        if self.config.enable_nonce_tracking && !self.nonce_tracker.check_and_store(nonce) {
            return Err(SigningError::NonceReused);
        }

        // Verify signature
        if !self
            .signer
            .verify(signature, method, path, timestamp, nonce, body)
        {
            return Err(SigningError::InvalidSignature);
        }

        Ok(())
    }
}

/// Signing error types
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    /// The provided signature does not match the expected value
    #[error("Invalid signature")]
    InvalidSignature,
    /// The timestamp header could not be parsed as a valid timestamp
    #[error("Invalid timestamp format")]
    InvalidTimestamp,
    /// The request timestamp is outside the acceptable time window
    #[error("Request is too old")]
    RequestTooOld,
    /// The nonce in the request has already been seen (replay attack prevention)
    #[error("Nonce has already been used (possible replay attack)")]
    NonceReused,
    /// A required signing header is absent from the request
    #[error("Missing required header: {0}")]
    MissingHeader(String),
}

/// Middleware to verify request signatures
pub async fn request_signing_middleware(
    verifier: SigningVerifier,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract headers
    let signature = req
        .headers()
        .get(SIGNATURE_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let timestamp = req
        .headers()
        .get(TIMESTAMP_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let nonce = req
        .headers()
        .get(NONCE_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let method = req.method().as_str();
    let path = req.uri().path();

    // For now, we'll assume empty body for simplicity
    // In a real implementation, you'd need to read and buffer the body
    let body = &[];

    // Verify the request
    match verifier.verify_request(signature, method, path, timestamp, nonce, body) {
        Ok(()) => Ok(next.run(req).await),
        Err(e) => {
            tracing::warn!("Request signature verification failed: {}", e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_signer() {
        let signer = RequestSigner::new(b"secret_key");

        let signature = signer.sign(
            "POST",
            "/api/tokens",
            "2024-01-01T00:00:00Z",
            "nonce123",
            b"request_body",
        );

        assert!(signer.verify(
            &signature,
            "POST",
            "/api/tokens",
            "2024-01-01T00:00:00Z",
            "nonce123",
            b"request_body"
        ));

        // Different body should fail
        assert!(!signer.verify(
            &signature,
            "POST",
            "/api/tokens",
            "2024-01-01T00:00:00Z",
            "nonce123",
            b"different_body"
        ));
    }

    #[test]
    fn test_nonce_tracker() {
        let tracker = NonceTracker::new(Duration::from_secs(60));

        // First use should succeed
        assert!(tracker.check_and_store("nonce1"));

        // Second use should fail (replay attack)
        assert!(!tracker.check_and_store("nonce1"));

        // Different nonce should succeed
        assert!(tracker.check_and_store("nonce2"));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hello!"));
    }

    #[test]
    fn test_signing_verifier() {
        let config = SigningConfig {
            max_age: Duration::from_secs(300),
            enable_nonce_tracking: true,
            nonce_ttl: Duration::from_secs(600),
        };
        let verifier = SigningVerifier::new(b"secret_key", config);
        let signer = RequestSigner::new(b"secret_key");

        let now = Utc::now();
        let timestamp = now.to_rfc3339();
        let nonce = "test_nonce";

        let signature = signer.sign("GET", "/api/test", &timestamp, nonce, b"");

        // Should verify successfully
        assert!(
            verifier
                .verify_request(&signature, "GET", "/api/test", &timestamp, nonce, b"")
                .is_ok()
        );

        // Same nonce should fail (replay attack)
        assert!(matches!(
            verifier.verify_request(&signature, "GET", "/api/test", &timestamp, nonce, b""),
            Err(SigningError::NonceReused)
        ));
    }

    #[test]
    fn test_timestamp_expiration() {
        let config = SigningConfig {
            max_age: Duration::from_secs(60),
            enable_nonce_tracking: false,
            nonce_ttl: Duration::from_secs(600),
        };
        let verifier = SigningVerifier::new(b"secret_key", config);
        let signer = RequestSigner::new(b"secret_key");

        // Old timestamp (2 hours ago)
        let old_time = Utc::now() - chrono::Duration::hours(2);
        let timestamp = old_time.to_rfc3339();
        let nonce = "test_nonce";

        let signature = signer.sign("GET", "/api/test", &timestamp, nonce, b"");

        // Should fail due to old timestamp
        assert!(matches!(
            verifier.verify_request(&signature, "GET", "/api/test", &timestamp, nonce, b""),
            Err(SigningError::RequestTooOld)
        ));
    }
}
