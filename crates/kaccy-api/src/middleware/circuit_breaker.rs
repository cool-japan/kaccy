//! Circuit breaker pattern implementation for fault tolerance

use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

/// Circuit breaker error
#[derive(Error, Debug, Clone)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is open")]
    Open,
    #[error("Too many failures: {0}")]
    TooManyFailures(String),
}

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: usize,
    /// Time to wait before attempting to close the circuit
    pub timeout: Duration,
    /// Number of successful requests needed to close the circuit from half-open
    pub success_threshold: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

/// Circuit breaker state tracker
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: usize,
    success_count: usize,
    last_failure_time: Option<Instant>,
}

/// Circuit breaker for protecting against cascading failures
#[derive(Clone)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
            })),
        }
    }

    /// Get current circuit state
    pub async fn state(&self) -> CircuitState {
        self.state.read().await.state
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        // Check if circuit is open
        {
            let state = self.state.read().await;
            match state.state {
                CircuitState::Open => {
                    // Check if timeout has elapsed
                    if let Some(last_failure) = state.last_failure_time {
                        if last_failure.elapsed() < self.config.timeout {
                            return Err(CircuitBreakerError::Open);
                        }
                    }
                }
                CircuitState::Closed | CircuitState::HalfOpen => {}
            }
        }

        // Transition to half-open if timeout elapsed
        {
            let mut state = self.state.write().await;
            if state.state == CircuitState::Open {
                if let Some(last_failure) = state.last_failure_time {
                    if last_failure.elapsed() >= self.config.timeout {
                        state.state = CircuitState::HalfOpen;
                        state.success_count = 0;
                    }
                }
            }
        }

        // Execute the function
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(_) => {
                self.on_failure().await;
                Err(CircuitBreakerError::TooManyFailures(
                    "Request failed".to_string(),
                ))
            }
        }
    }

    /// Record a successful request
    async fn on_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    // Close the circuit
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.last_failure_time = None;
                    tracing::info!("Circuit breaker closed after successful recovery");
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed request
    async fn on_failure(&self) {
        let mut state = self.state.write().await;

        state.failure_count += 1;
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                if state.failure_count >= self.config.failure_threshold {
                    // Open the circuit
                    state.state = CircuitState::Open;
                    state.success_count = 0;
                    tracing::warn!(
                        "Circuit breaker opened after {} failures",
                        state.failure_count
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Go back to open on any failure in half-open state
                state.state = CircuitState::Open;
                state.success_count = 0;
                tracing::warn!("Circuit breaker reopened after failure in half-open state");
            }
            CircuitState::Open => {}
        }
    }

    /// Manually reset the circuit breaker
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_failure_time = None;
        tracing::info!("Circuit breaker manually reset");
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(1),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::with_config(config);

        // Simulate 3 failures
        for _ in 0..3 {
            let result = cb.call(async { Err::<(), &str>("error") }).await;
            assert!(result.is_err());
        }

        // Circuit should be open now
        assert_eq!(cb.state().await, CircuitState::Open);

        // Next call should fail immediately without executing
        let result = cb.call(async { Ok::<(), &str>(()) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::Open)));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::with_config(config);

        // Trigger failures to open circuit
        for _ in 0..2 {
            let _ = cb.call(async { Err::<(), &str>("error") }).await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Next successful call should close the circuit
        let result = cb.call(async { Ok::<(), &str>(()) }).await;
        assert!(result.is_ok());

        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(10),
            success_threshold: 1,
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        for _ in 0..2 {
            let _ = cb.call(async { Err::<(), &str>("error") }).await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Manual reset
        cb.reset().await;

        assert_eq!(cb.state().await, CircuitState::Closed);

        // Should work now
        let result = cb.call(async { Ok::<(), &str>(()) }).await;
        assert!(result.is_ok());
    }
}
