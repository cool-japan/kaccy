//! Circuit breaker pattern for resilient cache operations
//!
//! Implements the circuit breaker pattern to prevent cascading failures
//! when the cache (Redis) is unavailable or experiencing high error rates.
//!
//! States:
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Too many failures, requests fail fast
//! - **HalfOpen**: Testing if service has recovered

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Service is failing - fail fast without calling service
    Open,
    /// Testing if service has recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Time window for counting failures (seconds)
    pub window_duration: Duration,
    /// Time to wait before attempting recovery (seconds)
    pub timeout_duration: Duration,
    /// Number of successful calls needed to close circuit from half-open
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            window_duration: Duration::from_secs(60),
            timeout_duration: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

/// Internal state of the circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            last_state_change: Instant::now(),
        }
    }
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
            state: Arc::new(RwLock::new(CircuitBreakerState::default())),
        }
    }

    /// Check if a call is allowed
    pub fn allow_request(&self) -> bool {
        let state = self.state.read();

        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if enough time has passed to try again
                let elapsed = state.last_state_change.elapsed();
                if elapsed >= self.config.timeout_duration {
                    drop(state);
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call
    pub fn record_success(&self) {
        let mut state = self.state.write();

        match state.state {
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
                state.last_failure_time = None;
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.last_failure_time = None;
                    state.last_state_change = Instant::now();
                    tracing::info!("Circuit breaker closed - service recovered");
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
                tracing::warn!("Received success while circuit is open");
            }
        }
    }

    /// Record a failed call
    pub fn record_failure(&self) {
        let mut state = self.state.write();
        let now = Instant::now();

        match state.state {
            CircuitState::Closed => {
                // Check if we're within the time window
                if let Some(last_failure) = state.last_failure_time {
                    if now.duration_since(last_failure) > self.config.window_duration {
                        // Window expired, reset counter
                        state.failure_count = 1;
                        state.last_failure_time = Some(now);
                        return;
                    }
                } else {
                    state.last_failure_time = Some(now);
                }

                state.failure_count += 1;

                if state.failure_count >= self.config.failure_threshold {
                    state.state = CircuitState::Open;
                    state.last_state_change = now;
                    tracing::warn!(
                        failure_count = state.failure_count,
                        "Circuit breaker opened - too many failures"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Failed during recovery attempt, reopen circuit
                state.state = CircuitState::Open;
                state.failure_count = self.config.failure_threshold;
                state.success_count = 0;
                state.last_state_change = now;
                tracing::warn!("Circuit breaker reopened - recovery attempt failed");
            }
            CircuitState::Open => {
                // Already open, just update timestamp
                state.last_failure_time = Some(now);
            }
        }
    }

    /// Get current circuit state
    pub fn state(&self) -> CircuitState {
        self.state.read().state
    }

    /// Get circuit breaker statistics
    pub fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read();
        CircuitBreakerStats {
            state: state.state,
            failure_count: state.failure_count,
            success_count: state.success_count,
            time_in_current_state: state.last_state_change.elapsed(),
        }
    }

    /// Manually reset the circuit breaker
    pub fn reset(&self) {
        let mut state = self.state.write();
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_failure_time = None;
        state.last_state_change = Instant::now();
        tracing::info!("Circuit breaker manually reset");
    }

    /// Transition to half-open state (for internal use)
    fn transition_to_half_open(&self) {
        let mut state = self.state.write();
        if state.state == CircuitState::Open {
            state.state = CircuitState::HalfOpen;
            state.success_count = 0;
            state.last_state_change = Instant::now();
            tracing::info!("Circuit breaker transitioning to half-open - testing recovery");
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Circuit breaker statistics
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub time_in_current_state: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Record failures
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_resets_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_success();
        let stats = cb.stats();
        assert_eq!(stats.failure_count, 0);
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for timeout
        thread::sleep(Duration::from_millis(150));

        // Next request should transition to half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_closes_from_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            success_threshold: 2,
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait and transition to half-open
        thread::sleep(Duration::from_millis(150));
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record successful recoveries
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_reopens_on_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait and transition to half-open
        thread::sleep(Duration::from_millis(150));
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Fail during recovery
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_manual_reset() {
        let cb = CircuitBreaker::new();

        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_failure_window_expiration() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            window_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let cb = CircuitBreaker::with_config(config);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Wait for window to expire
        thread::sleep(Duration::from_millis(150));

        // Next failure should reset the counter
        cb.record_failure();
        let stats = cb.stats();
        assert_eq!(stats.failure_count, 1);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_stats() {
        let cb = CircuitBreaker::new();

        let stats = cb.stats();
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_count, 0);

        cb.record_failure();
        let stats = cb.stats();
        assert_eq!(stats.failure_count, 1);
    }
}
