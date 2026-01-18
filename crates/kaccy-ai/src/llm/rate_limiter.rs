//! Rate limiting for API requests
//!
//! This module provides token bucket rate limiting to prevent
//! exceeding API rate limits and ensure fair resource usage.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{AiError, Result};

/// Rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<RateLimiterInner>,
}

struct RateLimiterInner {
    /// Maximum tokens in the bucket
    capacity: usize,
    /// Current token count
    tokens: RwLock<f64>,
    /// Token refill rate per second
    refill_rate: f64,
    /// Last refill time
    last_refill: RwLock<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `requests_per_second` - Maximum requests allowed per second
    /// * `burst_size` - Maximum burst size (defaults to `requests_per_second` if None)
    #[must_use]
    pub fn new(requests_per_second: f64, burst_size: Option<usize>) -> Self {
        let capacity = burst_size.unwrap_or(requests_per_second.ceil() as usize);

        Self {
            inner: Arc::new(RateLimiterInner {
                capacity,
                tokens: RwLock::new(capacity as f64),
                refill_rate: requests_per_second,
                last_refill: RwLock::new(Instant::now()),
            }),
        }
    }

    /// Acquire permission to make a request
    ///
    /// This will wait until a token is available if the rate limit is exceeded.
    pub async fn acquire(&self) -> Result<RateLimitGuard> {
        // Refill tokens based on elapsed time
        self.refill_tokens().await;

        // Try to acquire a token
        loop {
            {
                let mut tokens = self.inner.tokens.write().await;
                if *tokens >= 1.0 {
                    *tokens -= 1.0;
                    return Ok(RateLimitGuard {
                        limiter: self.clone(),
                    });
                }
            }

            // Wait a bit before trying again
            tokio::time::sleep(Duration::from_millis(10)).await;
            self.refill_tokens().await;
        }
    }

    /// Try to acquire permission without waiting
    ///
    /// Returns None if rate limit is exceeded.
    pub async fn try_acquire(&self) -> Option<RateLimitGuard> {
        self.refill_tokens().await;

        let mut tokens = self.inner.tokens.write().await;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            Some(RateLimitGuard {
                limiter: self.clone(),
            })
        } else {
            None
        }
    }

    /// Refill tokens based on elapsed time
    async fn refill_tokens(&self) {
        let mut last_refill = self.inner.last_refill.write().await;
        let elapsed = last_refill.elapsed();

        if elapsed >= Duration::from_millis(10) {
            let mut tokens = self.inner.tokens.write().await;

            // Calculate tokens to add based on elapsed time
            let tokens_to_add = self.inner.refill_rate * elapsed.as_secs_f64();
            *tokens = (*tokens + tokens_to_add).min(self.inner.capacity as f64);

            *last_refill = Instant::now();
        }
    }

    /// Get current token count (for monitoring)
    pub async fn available_tokens(&self) -> f64 {
        self.refill_tokens().await;
        *self.inner.tokens.read().await
    }

    /// Get capacity
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    /// Get refill rate
    #[must_use]
    pub fn refill_rate(&self) -> f64 {
        self.inner.refill_rate
    }
}

/// Guard returned when acquiring rate limit permission
///
/// The guard represents permission to make one request.
pub struct RateLimitGuard {
    #[allow(dead_code)]
    limiter: RateLimiter,
}

/// Multi-tier rate limiter with different limits for different tiers
pub struct TieredRateLimiter {
    limiters: Arc<RwLock<std::collections::HashMap<String, RateLimiter>>>,
}

impl Default for TieredRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl TieredRateLimiter {
    /// Create a new tiered rate limiter
    #[must_use]
    pub fn new() -> Self {
        Self {
            limiters: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Add a rate limiter for a specific tier
    pub async fn add_tier(&self, tier: impl Into<String>, limiter: RateLimiter) {
        let mut limiters = self.limiters.write().await;
        limiters.insert(tier.into(), limiter);
    }

    /// Acquire permission for a specific tier
    pub async fn acquire(&self, tier: &str) -> Result<RateLimitGuard> {
        let limiters = self.limiters.read().await;
        let limiter = limiters
            .get(tier)
            .ok_or_else(|| AiError::Configuration(format!("No rate limiter for tier: {tier}")))?;
        limiter.acquire().await
    }

    /// Try to acquire permission for a specific tier
    pub async fn try_acquire(&self, tier: &str) -> Option<RateLimitGuard> {
        let limiters = self.limiters.read().await;
        let limiter = limiters.get(tier)?;
        limiter.try_acquire().await
    }
}

/// Rate limiter configuration builder
pub struct RateLimiterConfig {
    requests_per_second: f64,
    burst_size: Option<usize>,
}

impl RateLimiterConfig {
    /// Create a new rate limiter config
    #[must_use]
    pub fn new(requests_per_second: f64) -> Self {
        Self {
            requests_per_second,
            burst_size: None,
        }
    }

    /// Set burst size
    #[must_use]
    pub fn with_burst_size(mut self, burst_size: usize) -> Self {
        self.burst_size = Some(burst_size);
        self
    }

    /// Build the rate limiter
    #[must_use]
    pub fn build(self) -> RateLimiter {
        RateLimiter::new(self.requests_per_second, self.burst_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rate_limiter_basic() {
        let limiter = RateLimiter::new(10.0, Some(10));

        // Should be able to acquire immediately
        let _guard = limiter.acquire().await;
        assert!(_guard.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_try_acquire() {
        let limiter = RateLimiter::new(10.0, Some(1));

        // First acquire should succeed
        let _guard1 = limiter.try_acquire().await;
        assert!(_guard1.is_some());

        // Second should fail (burst size is 1)
        let guard2 = limiter.try_acquire().await;
        assert!(guard2.is_none());
    }

    #[tokio::test]
    async fn test_rate_limiter_refill() {
        let limiter = RateLimiter::new(100.0, Some(1));

        // Exhaust the bucket
        let _guard = limiter.try_acquire().await.unwrap();
        drop(_guard);

        // Should be empty now
        assert!(limiter.try_acquire().await.is_none());

        // Wait for refill (100 requests/sec = 10ms per request)
        sleep(Duration::from_millis(20)).await;

        // Should have tokens again
        assert!(limiter.try_acquire().await.is_some());
    }

    #[tokio::test]
    async fn test_rate_limiter_burst() {
        let limiter = RateLimiter::new(10.0, Some(5));

        // Should be able to acquire burst_size requests immediately
        let mut guards = Vec::new();
        for _ in 0..5 {
            if let Some(guard) = limiter.try_acquire().await {
                guards.push(guard);
            }
        }
        assert_eq!(guards.len(), 5);

        // Next one should fail
        assert!(limiter.try_acquire().await.is_none());
    }

    #[tokio::test]
    async fn test_rate_limiter_available_tokens() {
        let limiter = RateLimiter::new(10.0, Some(10));

        let tokens = limiter.available_tokens().await;
        assert_eq!(tokens, 10.0);

        let _guard = limiter.try_acquire().await.unwrap();
        let tokens = limiter.available_tokens().await;
        assert_eq!(tokens, 9.0);
    }

    #[tokio::test]
    async fn test_tiered_rate_limiter() {
        let tiered = TieredRateLimiter::new();

        tiered
            .add_tier("free", RateLimiter::new(1.0, Some(1)))
            .await;
        tiered
            .add_tier("premium", RateLimiter::new(10.0, Some(10)))
            .await;

        // Free tier should work
        let _guard = tiered.acquire("free").await;
        assert!(_guard.is_ok());

        // Premium tier should work
        let _guard = tiered.acquire("premium").await;
        assert!(_guard.is_ok());

        // Non-existent tier should fail
        let result = tiered.acquire("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_config_builder() {
        let limiter = RateLimiterConfig::new(5.0).with_burst_size(10).build();

        assert_eq!(limiter.capacity(), 10);
        assert_eq!(limiter.refill_rate(), 5.0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let limiter = RateLimiter::new(100.0, Some(10));
        let limiter_clone = limiter.clone();

        // Spawn multiple concurrent requests
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let limiter = limiter_clone.clone();
                tokio::spawn(async move {
                    let _guard = limiter.acquire().await.unwrap();
                    tokio::time::sleep(Duration::from_millis(10)).await;
                })
            })
            .collect();

        // All should complete successfully
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
