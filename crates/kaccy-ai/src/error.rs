//! AI service error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("Evaluation failed: {0}")]
    EvaluationFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("LLM provider error: {0}")]
    ProviderError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("No LLM provider configured")]
    NoProviderConfigured,

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),

    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, AiError>;
