//! Advanced rate limiting algorithms
//!
//! This module provides multiple rate limiting strategies for production systems:
//! - Token Bucket: Allows bursts while maintaining average rate
//! - Leaky Bucket: Smooths traffic by enforcing constant rate
//! - Sliding Window: Prevents request clustering at window boundaries
//! - Distributed: Supports distributed rate limiting across multiple nodes

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Token bucket rate limiter
///
/// Allows bursts up to bucket capacity while maintaining an average fill rate.
/// Tokens are added at a constant rate and consumed per request.
#[derive(Debug)]
pub struct TokenBucket {
    capacity: u64,
    tokens: f64,
    fill_rate: f64, // tokens per second
    last_update: Instant,
}

impl TokenBucket {
    /// Create a new token bucket
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of tokens (burst size)
    /// * `fill_rate` - Tokens added per second (average rate)
    pub fn new(capacity: u64, fill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: capacity as f64,
            fill_rate,
            last_update: Instant::now(),
        }
    }

    /// Attempt to consume tokens
    ///
    /// Returns true if tokens were available and consumed
    pub fn try_consume(&mut self, tokens: u64) -> bool {
        self.refill();

        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        let new_tokens = elapsed * self.fill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.capacity as f64);
        self.last_update = now;
    }

    /// Get current token count
    pub fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    /// Reset the bucket to full capacity
    pub fn reset(&mut self) {
        self.tokens = self.capacity as f64;
        self.last_update = Instant::now();
    }

    /// Get time until N tokens are available
    pub fn time_until_available(&mut self, tokens: u64) -> Option<Duration> {
        self.refill();

        if self.tokens >= tokens as f64 {
            return Some(Duration::ZERO);
        }

        let tokens_needed = tokens as f64 - self.tokens;
        let seconds = tokens_needed / self.fill_rate;
        Some(Duration::from_secs_f64(seconds))
    }
}

/// Leaky bucket rate limiter
///
/// Enforces a constant request rate by "leaking" requests at a fixed rate.
/// Excess requests are queued up to a maximum queue size.
#[derive(Debug)]
pub struct LeakyBucket {
    capacity: usize,
    leak_rate: f64, // requests per second
    queue: VecDeque<Instant>,
    last_leak: Instant,
}

impl LeakyBucket {
    /// Create a new leaky bucket
    ///
    /// # Arguments
    /// * `capacity` - Maximum queue size
    /// * `leak_rate` - Requests processed per second
    pub fn new(capacity: usize, leak_rate: f64) -> Self {
        Self {
            capacity,
            leak_rate,
            queue: VecDeque::new(),
            last_leak: Instant::now(),
        }
    }

    /// Attempt to add a request
    ///
    /// Returns true if request was accepted (either processed or queued)
    pub fn try_acquire(&mut self) -> bool {
        self.leak();

        if self.queue.len() < self.capacity {
            self.queue.push_back(Instant::now());
            true
        } else {
            false
        }
    }

    /// Process (leak) requests based on elapsed time
    fn leak(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_leak).as_secs_f64();
        let requests_to_leak = (elapsed * self.leak_rate) as usize;

        for _ in 0..requests_to_leak.min(self.queue.len()) {
            self.queue.pop_front();
        }

        self.last_leak = now;
    }

    /// Get current queue size
    pub fn queue_size(&mut self) -> usize {
        self.leak();
        self.queue.len()
    }

    /// Reset the bucket
    pub fn reset(&mut self) {
        self.queue.clear();
        self.last_leak = Instant::now();
    }

    /// Get estimated wait time for next available slot
    pub fn estimated_wait_time(&mut self) -> Duration {
        self.leak();

        if self.queue.is_empty() {
            return Duration::ZERO;
        }

        let seconds = self.queue.len() as f64 / self.leak_rate;
        Duration::from_secs_f64(seconds)
    }
}

/// Sliding window rate limiter
///
/// Counts requests in a sliding time window to prevent clustering at boundaries.
/// More accurate than fixed windows but requires more memory.
#[derive(Debug)]
pub struct SlidingWindow {
    max_requests: u64,
    window_size: Duration,
    requests: VecDeque<Instant>,
}

impl SlidingWindow {
    /// Create a new sliding window rate limiter
    ///
    /// # Arguments
    /// * `max_requests` - Maximum requests allowed in window
    /// * `window_size` - Size of the sliding window
    pub fn new(max_requests: u64, window_size: Duration) -> Self {
        Self {
            max_requests,
            window_size,
            requests: VecDeque::new(),
        }
    }

    /// Attempt to record a request
    ///
    /// Returns true if request is within rate limit
    pub fn try_acquire(&mut self) -> bool {
        self.cleanup();

        if self.requests.len() < self.max_requests as usize {
            self.requests.push_back(Instant::now());
            true
        } else {
            false
        }
    }

    /// Remove expired requests from the window
    fn cleanup(&mut self) {
        let now = Instant::now();
        let cutoff = now - self.window_size;

        while let Some(&request_time) = self.requests.front() {
            if request_time < cutoff {
                self.requests.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get current request count in window
    pub fn current_count(&mut self) -> usize {
        self.cleanup();
        self.requests.len()
    }

    /// Get remaining capacity
    pub fn remaining(&mut self) -> u64 {
        self.cleanup();
        self.max_requests.saturating_sub(self.requests.len() as u64)
    }

    /// Reset the window
    pub fn reset(&mut self) {
        self.requests.clear();
    }

    /// Get time until next request is allowed
    pub fn time_until_available(&mut self) -> Duration {
        self.cleanup();

        if (self.requests.len() as u64) < self.max_requests {
            return Duration::ZERO;
        }

        if let Some(&oldest) = self.requests.front() {
            let elapsed = Instant::now().duration_since(oldest);
            self.window_size.saturating_sub(elapsed)
        } else {
            Duration::ZERO
        }
    }
}

/// Distributed rate limiter using shared state
///
/// Supports rate limiting across multiple nodes using a shared counter.
/// In production, this would use Redis or similar distributed cache.
#[derive(Debug, Clone)]
pub struct DistributedRateLimiter {
    inner: Arc<Mutex<DistributedLimiterState>>,
}

#[derive(Debug)]
struct DistributedLimiterState {
    counters: HashMap<String, SlidingWindow>,
    max_requests: u64,
    window_size: Duration,
}

impl DistributedRateLimiter {
    /// Create a new distributed rate limiter
    pub fn new(max_requests: u64, window_size: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DistributedLimiterState {
                counters: HashMap::new(),
                max_requests,
                window_size,
            })),
        }
    }

    /// Try to acquire for a specific key (e.g., user ID, IP address)
    pub fn try_acquire(&self, key: &str) -> bool {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let max_requests = state.max_requests;
        let window_size = state.window_size;

        let limiter = state
            .counters
            .entry(key.to_string())
            .or_insert_with(|| SlidingWindow::new(max_requests, window_size));

        limiter.try_acquire()
    }

    /// Get current count for a key
    pub fn current_count(&self, key: &str) -> usize {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state
            .counters
            .get_mut(key)
            .map(|l| l.current_count())
            .unwrap_or(0)
    }

    /// Get remaining capacity for a key
    pub fn remaining(&self, key: &str) -> u64 {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state
            .counters
            .get_mut(key)
            .map(|l| l.remaining())
            .unwrap_or(state.max_requests)
    }

    /// Reset a specific key
    pub fn reset_key(&self, key: &str) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.counters.remove(key);
    }

    /// Clean up inactive keys (removes keys with no recent requests)
    pub fn cleanup(&self) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.counters.retain(|_, limiter| {
            limiter.cleanup();
            limiter.current_count() > 0
        });
    }
}

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Algorithm used to enforce the rate limit.
    pub algorithm: RateLimitAlgorithm,
    /// Maximum allowed requests per window.
    pub max_requests: u64,
    /// Length of the rate-limit window in seconds.
    pub window_seconds: u64,
    /// Optional burst allowance above the base limit.
    pub burst_size: Option<u64>,
}

/// Rate limiting algorithm selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitAlgorithm {
    /// Token bucket algorithm allowing configurable bursts.
    TokenBucket,
    /// Leaky bucket algorithm enforcing a constant drain rate.
    LeakyBucket,
    /// Sliding window counter algorithm.
    SlidingWindow,
    /// Distributed rate limiting across multiple nodes.
    Distributed,
}

impl RateLimitConfig {
    /// Create a token bucket config
    pub fn token_bucket(capacity: u64, _fill_rate: f64) -> Self {
        Self {
            algorithm: RateLimitAlgorithm::TokenBucket,
            max_requests: capacity,
            window_seconds: 1,
            burst_size: Some(capacity),
        }
    }

    /// Create a leaky bucket config
    pub fn leaky_bucket(capacity: usize, _leak_rate: f64) -> Self {
        Self {
            algorithm: RateLimitAlgorithm::LeakyBucket,
            max_requests: capacity as u64,
            window_seconds: 1,
            burst_size: None,
        }
    }

    /// Create a sliding window config
    pub fn sliding_window(max_requests: u64, window_seconds: u64) -> Self {
        Self {
            algorithm: RateLimitAlgorithm::SlidingWindow,
            max_requests,
            window_seconds,
            burst_size: None,
        }
    }

    /// Create a distributed config
    pub fn distributed(max_requests: u64, window_seconds: u64) -> Self {
        Self {
            algorithm: RateLimitAlgorithm::Distributed,
            max_requests,
            window_seconds,
            burst_size: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(10, 1.0);

        // Should allow burst up to capacity
        assert!(bucket.try_consume(10));
        assert!(!bucket.try_consume(1)); // Bucket empty

        // Wait for refill (1 second = 1 token at 1.0/sec)
        thread::sleep(Duration::from_secs(1));
        assert!(bucket.try_consume(1)); // Some tokens refilled
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10, 10.0); // 10 tokens/sec

        assert!(bucket.try_consume(10));

        // Wait 1 second for refill
        thread::sleep(Duration::from_secs(1));

        // Should have refilled to capacity
        assert!(bucket.available_tokens() >= 9.0);
    }

    #[test]
    fn test_token_bucket_partial_consume() {
        let mut bucket = TokenBucket::new(100, 10.0);

        assert!(bucket.try_consume(30));
        assert!(bucket.try_consume(30));
        assert!(bucket.try_consume(30));
        assert!(!bucket.try_consume(20)); // Only ~10 tokens left
    }

    #[test]
    fn test_leaky_bucket_basic() {
        let mut bucket = LeakyBucket::new(10, 1.0);

        // Should accept up to capacity
        for _ in 0..10 {
            assert!(bucket.try_acquire());
        }

        // Capacity exceeded
        assert!(!bucket.try_acquire());
    }

    #[test]
    fn test_leaky_bucket_leak() {
        let mut bucket = LeakyBucket::new(10, 10.0); // 10 req/sec

        // Fill bucket
        for _ in 0..10 {
            assert!(bucket.try_acquire());
        }

        // Wait for leak
        thread::sleep(Duration::from_millis(500));

        // Some capacity should be available (leaked ~5 requests)
        assert!(bucket.queue_size() < 10);
    }

    #[test]
    fn test_sliding_window_basic() {
        let mut window = SlidingWindow::new(5, Duration::from_secs(1));

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(window.try_acquire());
        }

        // 6th request should fail
        assert!(!window.try_acquire());
        assert_eq!(window.current_count(), 5);
    }

    #[test]
    fn test_sliding_window_expiration() {
        let mut window = SlidingWindow::new(5, Duration::from_millis(100));

        // Fill window
        for _ in 0..5 {
            assert!(window.try_acquire());
        }

        // Wait for window to expire
        thread::sleep(Duration::from_millis(150));

        // Should allow new requests
        assert!(window.try_acquire());
        assert_eq!(window.current_count(), 1);
    }

    #[test]
    fn test_sliding_window_remaining() {
        let mut window = SlidingWindow::new(10, Duration::from_secs(1));

        assert_eq!(window.remaining(), 10);

        window.try_acquire();
        window.try_acquire();

        assert_eq!(window.remaining(), 8);
    }

    #[test]
    fn test_distributed_limiter() {
        let limiter = DistributedRateLimiter::new(5, Duration::from_secs(1));

        // Different keys should be independent
        assert!(limiter.try_acquire("user1"));
        assert!(limiter.try_acquire("user2"));

        assert_eq!(limiter.current_count("user1"), 1);
        assert_eq!(limiter.current_count("user2"), 1);
    }

    #[test]
    fn test_distributed_limiter_per_key_limit() {
        let limiter = DistributedRateLimiter::new(3, Duration::from_secs(1));

        // Fill limit for user1
        for _ in 0..3 {
            assert!(limiter.try_acquire("user1"));
        }

        // Should be blocked
        assert!(!limiter.try_acquire("user1"));

        // user2 should still work
        assert!(limiter.try_acquire("user2"));
    }

    #[test]
    fn test_distributed_limiter_reset() {
        let limiter = DistributedRateLimiter::new(2, Duration::from_secs(1));

        limiter.try_acquire("user1");
        limiter.try_acquire("user1");

        assert!(!limiter.try_acquire("user1"));

        limiter.reset_key("user1");

        assert!(limiter.try_acquire("user1"));
    }

    #[test]
    fn test_rate_limit_config() {
        let token_config = RateLimitConfig::token_bucket(100, 10.0);
        assert!(matches!(
            token_config.algorithm,
            RateLimitAlgorithm::TokenBucket
        ));

        let sliding_config = RateLimitConfig::sliding_window(100, 60);
        assert!(matches!(
            sliding_config.algorithm,
            RateLimitAlgorithm::SlidingWindow
        ));
    }

    #[test]
    fn test_token_bucket_time_until_available() {
        let mut bucket = TokenBucket::new(10, 10.0);
        bucket.try_consume(10);

        let wait_time = bucket.time_until_available(5).unwrap();
        // Should need ~0.5 seconds for 5 tokens at 10/sec
        assert!(wait_time >= Duration::from_millis(400));
        assert!(wait_time <= Duration::from_millis(600));
    }

    #[test]
    fn test_leaky_bucket_estimated_wait() {
        let mut bucket = LeakyBucket::new(10, 10.0);

        // Fill bucket
        for _ in 0..10 {
            bucket.try_acquire();
        }

        let wait = bucket.estimated_wait_time();
        // Should need ~1 second to process 10 requests at 10/sec
        assert!(wait >= Duration::from_millis(900));
    }
}
