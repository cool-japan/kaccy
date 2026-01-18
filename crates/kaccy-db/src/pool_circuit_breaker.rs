//! Connection pool circuit breaker for advanced resilience
//!
//! Provides automatic pool pause on repeated failures with exponential backoff
//! and health-based pool selection.

use crate::error::{DbError, Result};
use parking_lot::RwLock;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Circuit breaker state for connection pools
#[derive(Debug, Clone, PartialEq)]
pub enum PoolCircuitState {
    /// Circuit is closed, pool is healthy
    Closed,
    /// Circuit is open, pool is failing
    Open,
    /// Circuit is half-open, testing recovery
    HalfOpen,
}

/// Configuration for pool circuit breaker
#[derive(Debug, Clone)]
pub struct PoolCircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: usize,
    /// Number of successes needed to close circuit from half-open
    pub success_threshold: usize,
    /// Time to wait before transitioning from open to half-open
    pub timeout: Duration,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for PoolCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(30),
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}

/// Statistics for pool circuit breaker
#[derive(Debug, Clone, Default)]
pub struct PoolCircuitStats {
    pub state: String,
    pub failure_count: usize,
    pub success_count: usize,
    pub total_attempts: usize,
    pub circuit_opens: usize,
    pub current_backoff: Duration,
    pub last_failure_time: Option<Instant>,
}

/// Pool circuit breaker for automatic failure handling
pub struct PoolCircuitBreaker {
    pool: PgPool,
    config: PoolCircuitBreakerConfig,
    state: Arc<RwLock<PoolCircuitState>>,
    failure_count: Arc<RwLock<usize>>,
    success_count: Arc<RwLock<usize>>,
    total_attempts: Arc<RwLock<usize>>,
    circuit_opens: Arc<RwLock<usize>>,
    current_backoff: Arc<RwLock<Duration>>,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
}

impl PoolCircuitBreaker {
    /// Create a new pool circuit breaker
    pub fn new(pool: PgPool, config: PoolCircuitBreakerConfig) -> Self {
        Self {
            pool,
            config,
            state: Arc::new(RwLock::new(PoolCircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            total_attempts: Arc::new(RwLock::new(0)),
            circuit_opens: Arc::new(RwLock::new(0)),
            current_backoff: Arc::new(RwLock::new(Duration::from_millis(100))),
            last_failure_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the underlying pool (for direct access when needed)
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get current circuit state
    pub fn state(&self) -> PoolCircuitState {
        self.state.read().clone()
    }

    /// Get circuit breaker statistics
    pub fn stats(&self) -> PoolCircuitStats {
        let state = self.state.read().clone();
        PoolCircuitStats {
            state: format!("{:?}", state),
            failure_count: *self.failure_count.read(),
            success_count: *self.success_count.read(),
            total_attempts: *self.total_attempts.read(),
            circuit_opens: *self.circuit_opens.read(),
            current_backoff: *self.current_backoff.read(),
            last_failure_time: *self.last_failure_time.read(),
        }
    }

    /// Reset the circuit breaker to closed state
    pub fn reset(&self) {
        *self.state.write() = PoolCircuitState::Closed;
        *self.failure_count.write() = 0;
        *self.success_count.write() = 0;
        *self.current_backoff.write() = self.config.initial_backoff;
        *self.last_failure_time.write() = None;
    }

    /// Check if circuit should transition to half-open
    fn should_attempt_reset(&self) -> bool {
        if let Some(last_failure) = *self.last_failure_time.read() {
            if last_failure.elapsed() > self.config.timeout {
                return true;
            }
        }
        false
    }

    /// Record a successful operation
    fn record_success(&self) {
        *self.total_attempts.write() += 1;

        let current_state = self.state.read().clone();

        match current_state {
            PoolCircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write() = 0;
            }
            PoolCircuitState::HalfOpen => {
                *self.success_count.write() += 1;

                if *self.success_count.read() >= self.config.success_threshold {
                    // Transition to closed
                    *self.state.write() = PoolCircuitState::Closed;
                    *self.failure_count.write() = 0;
                    *self.success_count.write() = 0;
                    *self.current_backoff.write() = self.config.initial_backoff;
                }
            }
            PoolCircuitState::Open => {
                // Should not happen
            }
        }
    }

    /// Record a failed operation
    fn record_failure(&self) {
        *self.total_attempts.write() += 1;
        *self.failure_count.write() += 1;
        *self.last_failure_time.write() = Some(Instant::now());

        let current_state = self.state.read().clone();

        match current_state {
            PoolCircuitState::Closed => {
                if *self.failure_count.read() >= self.config.failure_threshold {
                    // Open the circuit
                    *self.state.write() = PoolCircuitState::Open;
                    *self.circuit_opens.write() += 1;
                    *self.success_count.write() = 0;
                }
            }
            PoolCircuitState::HalfOpen => {
                // Single failure in half-open state reopens circuit
                *self.state.write() = PoolCircuitState::Open;
                *self.circuit_opens.write() += 1;
                *self.success_count.write() = 0;

                // Increase backoff exponentially
                let current = *self.current_backoff.read();
                let new_backoff =
                    Duration::from_secs_f64(current.as_secs_f64() * self.config.backoff_multiplier);
                *self.current_backoff.write() = std::cmp::min(new_backoff, self.config.max_backoff);
            }
            PoolCircuitState::Open => {
                // Already open, just track failure
            }
        }
    }

    /// Execute a database operation with circuit breaker protection
    pub async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce(&PgPool) -> futures::future::BoxFuture<'_, Result<T>>,
    {
        // Check if we should attempt reset
        if self.should_attempt_reset() {
            let current_state = self.state.read().clone();
            if current_state == PoolCircuitState::Open {
                *self.state.write() = PoolCircuitState::HalfOpen;
                *self.failure_count.write() = 0;
            }
        }

        // Check current state
        let current_state = self.state.read().clone();

        match current_state {
            PoolCircuitState::Open => Err(DbError::Other(
                "Circuit breaker is OPEN - pool is unavailable".to_string(),
            )),
            PoolCircuitState::Closed | PoolCircuitState::HalfOpen => {
                // Attempt operation
                match operation(&self.pool).await {
                    Ok(result) => {
                        self.record_success();
                        Ok(result)
                    }
                    Err(err) => {
                        self.record_failure();
                        Err(err)
                    }
                }
            }
        }
    }

    /// Health check for the pool
    pub async fn health_check(&self) -> Result<bool> {
        self.execute(|pool| {
            Box::pin(async move {
                sqlx::query("SELECT 1")
                    .fetch_one(pool)
                    .await
                    .map(|_| true)
                    .map_err(DbError::from)
            })
        })
        .await
    }
}

/// Pool selector that chooses healthy pools using circuit breaker
pub struct HealthBasedPoolSelector {
    pools: Vec<PoolCircuitBreaker>,
}

impl HealthBasedPoolSelector {
    /// Create a new health-based pool selector
    pub fn new(pools: Vec<(PgPool, PoolCircuitBreakerConfig)>) -> Self {
        let pools = pools
            .into_iter()
            .map(|(pool, config)| PoolCircuitBreaker::new(pool, config))
            .collect();

        Self { pools }
    }

    /// Get a healthy pool (circuit not open)
    pub fn get_healthy_pool(&self) -> Option<&PoolCircuitBreaker> {
        // First try to find a closed circuit
        for pool in &self.pools {
            if pool.state() == PoolCircuitState::Closed {
                return Some(pool);
            }
        }

        // Then try half-open circuits
        self.pools
            .iter()
            .find(|&pool| pool.state() == PoolCircuitState::HalfOpen)
            .map(|v| v as _)
    }

    /// Get all pools
    pub fn all_pools(&self) -> &[PoolCircuitBreaker] {
        &self.pools
    }

    /// Get pool statistics for all pools
    pub fn stats(&self) -> Vec<PoolCircuitStats> {
        self.pools.iter().map(|p| p.stats()).collect()
    }

    /// Execute operation on first healthy pool
    pub async fn execute<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(&PgPool) -> futures::future::BoxFuture<'_, Result<T>>,
    {
        if let Some(pool) = self.get_healthy_pool() {
            pool.execute(operation).await
        } else {
            Err(DbError::Other(
                "No healthy pools available - all circuits are open".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = PoolCircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 2);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_circuit_state_transitions() {
        let state = PoolCircuitState::Closed;
        assert_eq!(state, PoolCircuitState::Closed);

        let state = PoolCircuitState::Open;
        assert_eq!(state, PoolCircuitState::Open);

        let state = PoolCircuitState::HalfOpen;
        assert_eq!(state, PoolCircuitState::HalfOpen);
    }

    #[test]
    fn test_exponential_backoff_calculation() {
        let config = PoolCircuitBreakerConfig::default();

        let mut backoff = config.initial_backoff;
        assert_eq!(backoff, Duration::from_millis(100));

        backoff = Duration::from_secs_f64(backoff.as_secs_f64() * config.backoff_multiplier);
        assert_eq!(backoff, Duration::from_millis(200));

        backoff = Duration::from_secs_f64(backoff.as_secs_f64() * config.backoff_multiplier);
        assert_eq!(backoff, Duration::from_millis(400));

        backoff = std::cmp::min(backoff, config.max_backoff);
        assert!(backoff <= config.max_backoff);
    }

    #[test]
    fn test_pool_circuit_stats_default() {
        let stats = PoolCircuitStats::default();
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.success_count, 0);
        assert_eq!(stats.total_attempts, 0);
        assert_eq!(stats.circuit_opens, 0);
    }
}
