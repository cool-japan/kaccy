//! AI service error types

use thiserror::Error;

/// Top-level error type for the kaccy-ai crate.
#[derive(Error, Debug)]
pub enum AiError {
    /// An evaluation step failed.
    #[error("Evaluation failed: {0}")]
    EvaluationFailed(String),

    /// A verification step failed.
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Input or data failed validation.
    #[error("Validation error: {0}")]
    Validation(String),

    /// The upstream LLM provider returned an error.
    #[error("LLM provider error: {0}")]
    ProviderError(String),

    /// The provider's rate limit was exceeded.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// No LLM provider has been configured.
    #[error("No LLM provider configured")]
    NoProviderConfigured,

    /// The service is temporarily unavailable.
    #[error("Service unavailable")]
    ServiceUnavailable,

    /// A GitHub API call failed.
    #[error("GitHub API error: {0}")]
    GitHub(String),

    /// An unexpected internal error occurred.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Failed to parse a response or data format.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// The caller supplied invalid input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A configuration value is missing or incorrect.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// The service or resource is unavailable with a reason.
    #[error("Service unavailable: {0}")]
    Unavailable(String),

    /// A usage quota was exceeded.
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    /// The requested feature is not available.
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),

    /// An operational limit was exceeded.
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),

    /// The requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Convenience alias for `Result<T, AiError>`.
pub type Result<T> = std::result::Result<T, AiError>;
