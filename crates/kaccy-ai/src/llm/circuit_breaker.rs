//! Circuit breaker pattern for external API resilience
//!
//! Prevents cascading failures by temporarily blocking requests to failing services.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{AiError, Result};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// How long to keep circuit open before trying again
    pub timeout: Duration,
    /// How many successful requests needed to close circuit from half-open
    pub success_threshold: u32,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
            failure_window: Duration::from_secs(60),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new circuit breaker config
    #[must_use]
    pub fn new(failure_threshold: u32, timeout: Duration) -> Self {
        Self {
            failure_threshold,
            timeout,
            ..Default::default()
        }
    }

    /// Set success threshold
    #[must_use]
    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Set failure window
    #[must_use]
    pub fn with_failure_window(mut self, window: Duration) -> Self {
        self.failure_window = window;
        self
    }
}

/// Internal state tracking for circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    /// Current state
    state: CircuitState,
    /// Number of consecutive failures
    failure_count: u32,
    /// Number of consecutive successes (in half-open state)
    success_count: u32,
    /// When the circuit was opened
    opened_at: Option<Instant>,
    /// Recent failure timestamps
    recent_failures: Vec<Instant>,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            opened_at: None,
            recent_failures: Vec::new(),
        }
    }

    /// Clean up old failures outside the window
    fn clean_old_failures(&mut self, window: Duration) {
        let cutoff = Instant::now().checked_sub(window).expect("window duration is valid and Instant::now() minus it does not overflow");
        self.recent_failures.retain(|&t| t > cutoff);
    }
}

/// Circuit breaker for protecting against cascading failures
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
    name: String,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::new())),
            name: name.into(),
        }
    }

    /// Get current circuit state
    pub async fn state(&self) -> CircuitState {
        let state = self.state.read().await;
        state.state
    }

    /// Execute an operation through the circuit breaker
    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Check if circuit should transition to half-open
        self.check_timeout().await;

        // Check current state
        let current_state = {
            let state = self.state.read().await;
            state.state
        };

        match current_state {
            CircuitState::Open => {
                tracing::warn!(
                    circuit = %self.name,
                    "Circuit breaker is open, rejecting request"
                );
                Err(AiError::ServiceUnavailable)
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // Try the operation
                match operation().await {
                    Ok(result) => {
                        self.on_success().await;
                        Ok(result)
                    }
                    Err(err) => {
                        self.on_failure().await;
                        Err(err)
                    }
                }
            }
        }
    }

    /// Check if circuit should transition from open to half-open
    async fn check_timeout(&self) {
        let mut state = self.state.write().await;

        if state.state == CircuitState::Open {
            if let Some(opened_at) = state.opened_at {
                if opened_at.elapsed() >= self.config.timeout {
                    tracing::info!(
                        circuit = %self.name,
                        "Circuit breaker timeout elapsed, transitioning to half-open"
                    );
                    state.state = CircuitState::HalfOpen;
                    state.success_count = 0;
                }
            }
        }
    }

    /// Handle successful operation
    async fn on_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    tracing::info!(
                        circuit = %self.name,
                        "Circuit breaker closing after {} successful requests",
                        state.success_count
                    );
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.recent_failures.clear();
                    state.opened_at = None;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
            }
            CircuitState::Open => {
                // Shouldn't happen, but reset if it does
                state.failure_count = 0;
            }
        }
    }

    /// Handle failed operation
    async fn on_failure(&self) {
        let mut state = self.state.write().await;

        state.recent_failures.push(Instant::now());
        state.clean_old_failures(self.config.failure_window);

        match state.state {
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                tracing::warn!(
                    circuit = %self.name,
                    "Circuit breaker reopening after failure in half-open state"
                );
                state.state = CircuitState::Open;
                state.opened_at = Some(Instant::now());
                state.success_count = 0;
            }
            CircuitState::Closed => {
                state.failure_count += 1;

                // Check if we should open the circuit
                if state.recent_failures.len() >= self.config.failure_threshold as usize {
                    tracing::warn!(
                        circuit = %self.name,
                        failures = state.recent_failures.len(),
                        threshold = self.config.failure_threshold,
                        "Circuit breaker opening due to failure threshold"
                    );
                    state.state = CircuitState::Open;
                    state.opened_at = Some(Instant::now());
                }
            }
            CircuitState::Open => {
                // Already open, just log
                tracing::debug!(
                    circuit = %self.name,
                    "Additional failure while circuit is open"
                );
            }
        }
    }

    /// Manually reset the circuit breaker
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        tracing::info!(circuit = %self.name, "Manually resetting circuit breaker");
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.recent_failures.clear();
        state.opened_at = None;
    }

    /// Get circuit breaker metrics
    pub async fn metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.read().await;
        CircuitBreakerMetrics {
            state: state.state,
            failure_count: state.failure_count,
            success_count: state.success_count,
            recent_failure_count: state.recent_failures.len() as u32,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    /// Current state
    pub state: CircuitState,
    /// Consecutive failures
    pub failure_count: u32,
    /// Consecutive successes (in half-open)
    pub success_count: u32,
    /// Recent failures in window
    pub recent_failure_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let config = CircuitBreakerConfig::new(3, Duration::from_secs(1));
        let cb = CircuitBreaker::new("test", config);

        // Initially closed
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Successful calls should keep it closed
        let result = cb.call(|| async { Ok::<_, AiError>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig::new(3, Duration::from_secs(1))
            .with_failure_window(Duration::from_secs(10));
        let cb = CircuitBreaker::new("test", config);

        // Trigger failures to open circuit
        for _ in 0..3 {
            let _ = cb
                .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
                .await;
        }

        // Circuit should be open
        assert_eq!(cb.state().await, CircuitState::Open);

        // Requests should be rejected
        let result = cb.call(|| async { Ok::<_, AiError>(42) }).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let config =
            CircuitBreakerConfig::new(2, Duration::from_millis(100)).with_success_threshold(2);
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
                .await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Next call should transition to half-open and succeed
        let result = cb.call(|| async { Ok::<_, AiError>(42) }).await;
        assert!(result.is_ok());

        // Still half-open, need one more success
        let metrics = cb.metrics().await;
        assert_eq!(metrics.success_count, 1);

        // One more success should close the circuit
        let result = cb.call(|| async { Ok::<_, AiError>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_reopens_on_failure() {
        let config = CircuitBreakerConfig::new(2, Duration::from_millis(100));
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
                .await;
        }

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Failure in half-open should reopen
        let _ = cb
            .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
            .await;

        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig::new(2, Duration::from_secs(10));
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
                .await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);

        // Manual reset should close it
        cb.reset().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_failure_window() {
        let config = CircuitBreakerConfig::new(3, Duration::from_secs(1))
            .with_failure_window(Duration::from_millis(200));
        let cb = CircuitBreaker::new("test", config);

        // Two failures
        for _ in 0..2 {
            let _ = cb
                .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
                .await;
        }

        // Wait for failures to expire
        sleep(Duration::from_millis(250)).await;

        // Circuit should still be closed since failures expired
        assert_eq!(cb.state().await, CircuitState::Closed);

        // One more failure shouldn't open it (old failures expired)
        let _ = cb
            .call(|| async { Err::<i32, _>(AiError::ServiceUnavailable) })
            .await;

        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
