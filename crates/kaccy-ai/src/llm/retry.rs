//! Retry logic with exponential backoff for LLM API calls
//!
//! This module provides resilient retry mechanisms for handling transient failures
//! when communicating with LLM providers.

use std::time::Duration;
use tokio::time::sleep;

use crate::error::{AiError, Result};

/// Retry configuration for LLM calls
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial retry delay
    pub initial_delay: Duration,
    /// Maximum retry delay (to prevent excessive waits)
    pub max_delay: Duration,
    /// Backoff multiplier (e.g., 2.0 for doubling)
    pub backoff_multiplier: f64,
    /// Whether to add jitter to avoid thundering herd
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    /// Set initial delay
    #[must_use]
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set max delay
    #[must_use]
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    #[must_use]
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    #[must_use]
    pub fn with_jitter(mut self, use_jitter: bool) -> Self {
        self.use_jitter = use_jitter;
        self
    }

    /// Calculate delay for a specific attempt
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let mut delay =
            self.initial_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32);

        // Cap at max delay
        delay = delay.min(self.max_delay.as_millis() as f64);

        // Add jitter to prevent thundering herd
        if self.use_jitter {
            use rand::RngExt;
            let jitter = rand::rng().random_range(0.0..=0.3);
            delay *= 1.0 + jitter;
        }

        Duration::from_millis(delay as u64)
    }
}

/// Retry a fallible async operation with exponential backoff
pub async fn retry_with_backoff<F, Fut, T>(config: &RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                // Check if error is retryable
                if !is_retryable_error(&err) {
                    return Err(err);
                }

                last_error = Some(err);

                // Don't sleep after the last attempt
                if attempt < config.max_attempts - 1 {
                    let delay = config.calculate_delay(attempt);
                    tracing::debug!(
                        "Retry attempt {}/{}, waiting {:?}",
                        attempt + 1,
                        config.max_attempts,
                        delay
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AiError::Internal("All retry attempts exhausted with no error".to_string())
    }))
}

/// Determine if an error is retryable
fn is_retryable_error(error: &AiError) -> bool {
    matches!(
        error,
        AiError::RateLimitExceeded
            | AiError::ServiceUnavailable
            | AiError::Unavailable(_)
            | AiError::ProviderError(_)
    )
}

/// Retry policy for different error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    /// Never retry
    Never,
    /// Retry on transient errors only
    OnTransientErrors,
    /// Always retry
    Always,
}

/// Advanced retry executor with policy-based retries
pub struct RetryExecutor {
    config: RetryConfig,
    policy: RetryPolicy,
}

impl Default for RetryExecutor {
    fn default() -> Self {
        Self::new(RetryConfig::default(), RetryPolicy::OnTransientErrors)
    }
}

impl RetryExecutor {
    /// Create a new retry executor
    #[must_use]
    pub fn new(config: RetryConfig, policy: RetryPolicy) -> Self {
        Self { config, policy }
    }

    /// Execute an operation with retry logic
    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match self.policy {
            RetryPolicy::Never => operation().await,
            RetryPolicy::OnTransientErrors => retry_with_backoff(&self.config, operation).await,
            RetryPolicy::Always => {
                // Custom retry that retries on all errors
                self.retry_always(operation).await
            }
        }
    }

    /// Retry on all errors (use with caution)
    #[allow(dead_code)]
    async fn retry_always<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..self.config.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_error = Some(err);

                    if attempt < self.config.max_attempts - 1 {
                        let delay = self.config.calculate_delay(attempt);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| AiError::Internal("All retry attempts exhausted".to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_delay_calculation() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            use_jitter: false,
        };

        // Without jitter for predictable testing
        let delay0 = config.calculate_delay(0);
        let delay1 = config.calculate_delay(1);
        let delay2 = config.calculate_delay(2);

        assert_eq!(delay0.as_millis(), 100);
        assert_eq!(delay1.as_millis(), 200);
        assert_eq!(delay2.as_millis(), 400);
    }

    #[test]
    fn test_retry_config_max_delay() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            use_jitter: false,
        };

        // High attempt should be capped at max_delay
        let delay = config.calculate_delay(20);
        assert!(delay <= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::default();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Ok::<_, AiError>(42)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::new(3);
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 3 {
                    Err(AiError::ServiceUnavailable)
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::new(3);
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(AiError::InvalidInput("Bad input".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        // Should not retry on non-retryable errors
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_exhaustion() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::new(3);
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(&config, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(AiError::ServiceUnavailable)
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error(&AiError::RateLimitExceeded));
        assert!(is_retryable_error(&AiError::ServiceUnavailable));
        assert!(is_retryable_error(&AiError::Unavailable(
            "test".to_string()
        )));

        assert!(!is_retryable_error(&AiError::InvalidInput(
            "test".to_string()
        )));
        assert!(!is_retryable_error(&AiError::Configuration(
            "test".to_string()
        )));
    }

    #[test]
    fn test_retry_config_builder() {
        let config = RetryConfig::new(5)
            .with_initial_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(60))
            .with_backoff_multiplier(3.0)
            .with_jitter(false);

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 3.0);
        assert!(!config.use_jitter);
    }
}
