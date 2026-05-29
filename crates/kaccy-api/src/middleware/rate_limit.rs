//! Rate limiting middleware

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        }
    }
}

impl RateLimitConfig {
    /// Create a stricter rate limit for authentication endpoints
    pub fn auth_endpoints() -> Self {
        Self {
            max_requests: 10,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        }
    }
}

/// Token bucket entry for rate limiting
#[derive(Debug, Clone)]
struct RateLimitEntry {
    /// Remaining tokens
    tokens: u32,
    /// Last refill time
    last_refill: Instant,
}

/// Rate limiter state
#[derive(Clone)]
pub struct RateLimiter {
    /// IP -> rate limit entry
    entries: Arc<DashMap<IpAddr, RateLimitEntry>>,
    /// Configuration
    config: RateLimitConfig,
    /// Last cleanup time
    last_cleanup: Arc<std::sync::Mutex<Instant>>,
}

impl RateLimiter {
    /// Create a new rate limiter with default configuration
    pub fn new() -> Self {
        Self::with_config(RateLimitConfig::default())
    }

    /// Create a new rate limiter with custom configuration
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            config,
            last_cleanup: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    /// Check if a request should be allowed
    pub fn check(&self, ip: IpAddr) -> RateLimitResult {
        self.maybe_cleanup();

        let now = Instant::now();
        let mut entry = self.entries.entry(ip).or_insert_with(|| RateLimitEntry {
            tokens: self.config.max_requests,
            last_refill: now,
        });

        // Refill tokens based on time passed
        let elapsed = now.duration_since(entry.last_refill);
        if elapsed >= self.config.window {
            entry.tokens = self.config.max_requests;
            entry.last_refill = now;
        }

        if entry.tokens > 0 {
            entry.tokens -= 1;
            RateLimitResult::Allowed {
                remaining: entry.tokens,
                reset_in: self.config.window.saturating_sub(elapsed),
            }
        } else {
            RateLimitResult::Limited {
                retry_after: self.config.window.saturating_sub(elapsed),
            }
        }
    }

    /// Cleanup old entries
    fn maybe_cleanup(&self) {
        let mut last_cleanup = self.last_cleanup.lock().unwrap_or_else(|e| e.into_inner());
        if last_cleanup.elapsed() >= self.config.cleanup_interval {
            let threshold = Instant::now() - self.config.window * 2;
            self.entries
                .retain(|_, entry| entry.last_refill > threshold);
            *last_cleanup = Instant::now();
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a rate limit check
#[derive(Debug)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed {
        /// Number of requests remaining in the current window
        remaining: u32,
        /// Duration until the rate limit window resets
        reset_in: Duration,
    },
    /// Request is rate limited
    Limited {
        /// Duration to wait before retrying the request
        retry_after: Duration,
    },
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let ip = addr.ip();

    match limiter.check(ip) {
        RateLimitResult::Allowed {
            remaining,
            reset_in,
        } => {
            let mut response = next.run(request).await;
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Remaining",
                remaining
                    .to_string()
                    .parse()
                    .expect("integer to_string must be valid HeaderValue"),
            );
            headers.insert(
                "X-RateLimit-Reset",
                reset_in
                    .as_secs()
                    .to_string()
                    .parse()
                    .expect("integer to_string must be valid HeaderValue"),
            );
            response
        }
        RateLimitResult::Limited { retry_after } => {
            let retry_secs = retry_after.as_secs().max(1);
            tracing::warn!(
                ip = %ip,
                retry_after = retry_secs,
                "Rate limit exceeded"
            );
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", retry_secs.to_string())],
                "Rate limit exceeded. Please try again later.",
            )
                .into_response()
        }
    }
}

/// Login attempt tracker for account lockout
#[derive(Clone)]
pub struct LoginAttemptTracker {
    /// IP -> (failed attempts, last attempt time)
    attempts: Arc<DashMap<IpAddr, (u32, Instant)>>,
    /// Maximum failed attempts before lockout
    max_attempts: u32,
    /// Lockout duration
    lockout_duration: Duration,
}

impl LoginAttemptTracker {
    /// Create a new login attempt tracker
    pub fn new(max_attempts: u32, lockout_duration: Duration) -> Self {
        Self {
            attempts: Arc::new(DashMap::new()),
            max_attempts,
            lockout_duration,
        }
    }

    /// Check if an IP is locked out
    pub fn is_locked_out(&self, ip: IpAddr) -> Option<Duration> {
        if let Some(entry) = self.attempts.get(&ip) {
            let (attempts, last_attempt) = *entry;
            if attempts >= self.max_attempts {
                let elapsed = last_attempt.elapsed();
                if elapsed < self.lockout_duration {
                    return Some(self.lockout_duration - elapsed);
                }
            }
        }
        None
    }

    /// Record a failed login attempt
    pub fn record_failure(&self, ip: IpAddr) {
        let now = Instant::now();
        let mut entry = self.attempts.entry(ip).or_insert((0, now));

        // Reset if lockout period has passed
        if entry.1.elapsed() >= self.lockout_duration {
            entry.0 = 0;
        }

        entry.0 += 1;
        entry.1 = now;
    }

    /// Clear failed attempts for an IP (on successful login)
    pub fn clear(&self, ip: IpAddr) {
        self.attempts.remove(&ip);
    }
}

impl Default for LoginAttemptTracker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(900)) // 5 attempts, 15 minute lockout
    }
}

/// Per-user rate limiter for authenticated requests
#[derive(Clone)]
pub struct UserRateLimiter {
    /// user_id -> rate limit entry
    entries: Arc<DashMap<String, RateLimitEntry>>,
    /// Configuration
    config: RateLimitConfig,
    /// Last cleanup time
    last_cleanup: Arc<std::sync::Mutex<Instant>>,
}

impl UserRateLimiter {
    /// Create a new user rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            config,
            last_cleanup: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    /// Check rate limit for a user
    pub fn check(&self, user_id: &str) -> RateLimitResult {
        self.maybe_cleanup();

        let now = Instant::now();
        let mut entry = self
            .entries
            .entry(user_id.to_string())
            .or_insert_with(|| RateLimitEntry {
                tokens: self.config.max_requests,
                last_refill: now,
            });

        // Refill tokens based on time passed
        let elapsed = now.duration_since(entry.last_refill);
        if elapsed >= self.config.window {
            entry.tokens = self.config.max_requests;
            entry.last_refill = now;
        }

        if entry.tokens > 0 {
            entry.tokens -= 1;
            RateLimitResult::Allowed {
                remaining: entry.tokens,
                reset_in: self.config.window.saturating_sub(elapsed),
            }
        } else {
            RateLimitResult::Limited {
                retry_after: self.config.window.saturating_sub(elapsed),
            }
        }
    }

    /// Cleanup old entries
    fn maybe_cleanup(&self) {
        let mut last_cleanup = self.last_cleanup.lock().unwrap_or_else(|e| e.into_inner());
        if last_cleanup.elapsed() >= self.config.cleanup_interval {
            let threshold = Instant::now() - self.config.window * 2;
            self.entries
                .retain(|_, entry| entry.last_refill > threshold);
            *last_cleanup = Instant::now();
        }
    }
}

/// Per-endpoint rate limiter
#[derive(Clone)]
pub struct EndpointRateLimiter {
    /// (ip, endpoint) -> rate limit entry
    entries: Arc<DashMap<(IpAddr, String), RateLimitEntry>>,
    /// endpoint -> config
    configs: Arc<DashMap<String, RateLimitConfig>>,
    /// Default configuration
    default_config: RateLimitConfig,
    /// Last cleanup time
    last_cleanup: Arc<std::sync::Mutex<Instant>>,
}

impl EndpointRateLimiter {
    /// Create a new endpoint rate limiter
    pub fn new(default_config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            configs: Arc::new(DashMap::new()),
            default_config,
            last_cleanup: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    /// Set rate limit for a specific endpoint
    pub fn set_endpoint_limit(&self, endpoint: String, config: RateLimitConfig) {
        self.configs.insert(endpoint, config);
    }

    /// Check rate limit for an endpoint
    pub fn check(&self, ip: IpAddr, endpoint: &str) -> RateLimitResult {
        self.maybe_cleanup();

        let config = self
            .configs
            .get(endpoint)
            .map(|c| c.clone())
            .unwrap_or_else(|| self.default_config.clone());

        let now = Instant::now();
        let key = (ip, endpoint.to_string());
        let mut entry = self.entries.entry(key).or_insert_with(|| RateLimitEntry {
            tokens: config.max_requests,
            last_refill: now,
        });

        // Refill tokens based on time passed
        let elapsed = now.duration_since(entry.last_refill);
        if elapsed >= config.window {
            entry.tokens = config.max_requests;
            entry.last_refill = now;
        }

        if entry.tokens > 0 {
            entry.tokens -= 1;
            RateLimitResult::Allowed {
                remaining: entry.tokens,
                reset_in: config.window.saturating_sub(elapsed),
            }
        } else {
            RateLimitResult::Limited {
                retry_after: config.window.saturating_sub(elapsed),
            }
        }
    }

    /// Cleanup old entries
    fn maybe_cleanup(&self) {
        let mut last_cleanup = self.last_cleanup.lock().unwrap_or_else(|e| e.into_inner());
        if last_cleanup.elapsed() >= self.default_config.cleanup_interval {
            let threshold = Instant::now() - self.default_config.window * 2;
            self.entries
                .retain(|_, entry| entry.last_refill > threshold);
            *last_cleanup = Instant::now();
        }
    }
}

/// Rate limit analytics tracker
#[derive(Clone)]
pub struct RateLimitAnalytics {
    /// Total requests
    total_requests: Arc<std::sync::atomic::AtomicU64>,
    /// Rate limited requests
    rate_limited: Arc<std::sync::atomic::AtomicU64>,
    /// Per-endpoint statistics (endpoint -> (total, limited))
    endpoint_stats: Arc<DashMap<String, (u64, u64)>>,
}

impl RateLimitAnalytics {
    /// Create a new rate limit analytics tracker
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            rate_limited: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            endpoint_stats: Arc::new(DashMap::new()),
        }
    }

    /// Record a request
    pub fn record_request(&self, endpoint: &str, was_limited: bool) {
        use std::sync::atomic::Ordering;

        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if was_limited {
            self.rate_limited.fetch_add(1, Ordering::Relaxed);
        }

        let mut stats = self
            .endpoint_stats
            .entry(endpoint.to_string())
            .or_insert((0, 0));
        stats.0 += 1;
        if was_limited {
            stats.1 += 1;
        }
    }

    /// Get total requests
    pub fn total_requests(&self) -> u64 {
        self.total_requests
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get rate limited requests
    pub fn rate_limited(&self) -> u64 {
        self.rate_limited.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get rate limit percentage
    pub fn rate_limit_percentage(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.rate_limited() as f64 / total as f64) * 100.0
        }
    }

    /// Get endpoint statistics
    pub fn endpoint_stats(&self) -> Vec<(String, u64, u64, f64)> {
        self.endpoint_stats
            .iter()
            .map(|entry| {
                let endpoint = entry.key().clone();
                let (total, limited) = *entry.value();
                let percentage = if total == 0 {
                    0.0
                } else {
                    (limited as f64 / total as f64) * 100.0
                };
                (endpoint, total, limited, percentage)
            })
            .collect()
    }
}

impl Default for RateLimitAnalytics {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic rate limiter that adjusts limits based on system load
#[derive(Clone)]
pub struct DynamicRateLimiter {
    /// Base rate limiter
    limiter: Arc<std::sync::RwLock<RateLimiter>>,
    /// Current multiplier for rate limits (1.0 = normal, 0.5 = half, 2.0 = double)
    multiplier: Arc<std::sync::RwLock<f64>>,
    /// Base configuration
    base_config: RateLimitConfig,
    /// Analytics for monitoring
    analytics: Arc<RateLimitAnalytics>,
    /// Last adjustment time
    last_adjustment: Arc<std::sync::Mutex<Instant>>,
    /// Adjustment interval
    adjustment_interval: Duration,
}

impl DynamicRateLimiter {
    /// Create a new dynamic rate limiter
    pub fn new(base_config: RateLimitConfig) -> Self {
        Self {
            limiter: Arc::new(std::sync::RwLock::new(RateLimiter::with_config(
                base_config.clone(),
            ))),
            multiplier: Arc::new(std::sync::RwLock::new(1.0)),
            base_config,
            analytics: Arc::new(RateLimitAnalytics::new()),
            last_adjustment: Arc::new(std::sync::Mutex::new(Instant::now())),
            adjustment_interval: Duration::from_secs(60),
        }
    }

    /// Check rate limit with dynamic adjustment
    pub fn check(&self, ip: IpAddr) -> RateLimitResult {
        self.maybe_adjust();

        let limiter = self.limiter.read().unwrap_or_else(|e| e.into_inner());
        let result = limiter.check(ip);

        // Record analytics
        let was_limited = matches!(result, RateLimitResult::Limited { .. });
        self.analytics.record_request("global", was_limited);

        result
    }

    /// Manually adjust the rate limit multiplier
    pub fn set_multiplier(&self, multiplier: f64) {
        let multiplier = multiplier.clamp(0.1, 10.0); // Limit between 10% and 1000%
        *self.multiplier.write().unwrap_or_else(|e| e.into_inner()) = multiplier;
        self.apply_multiplier();
        tracing::info!(
            multiplier = multiplier,
            "Rate limit multiplier manually adjusted"
        );
    }

    /// Get current multiplier
    pub fn get_multiplier(&self) -> f64 {
        *self.multiplier.read().unwrap_or_else(|e| e.into_inner())
    }

    /// Automatically adjust based on system metrics
    fn maybe_adjust(&self) {
        let last_adjustment = self
            .last_adjustment
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if last_adjustment.elapsed() >= self.adjustment_interval {
            drop(last_adjustment); // Release lock before adjustment

            // Check if we're getting too many rate limit hits
            let rate_limit_percentage = self.analytics.rate_limit_percentage();

            let new_multiplier = if rate_limit_percentage > 50.0 {
                // Too many requests being rate limited - decrease limits
                let current = *self.multiplier.read().unwrap_or_else(|e| e.into_inner());
                (current * 0.8).max(0.1)
            } else if rate_limit_percentage < 5.0 {
                // Very few rate limits - can increase limits
                let current = *self.multiplier.read().unwrap_or_else(|e| e.into_inner());
                (current * 1.1).min(2.0)
            } else {
                // Stable - no adjustment needed
                *self.multiplier.read().unwrap_or_else(|e| e.into_inner())
            };

            if (*self.multiplier.read().unwrap_or_else(|e| e.into_inner()) - new_multiplier).abs()
                > 0.01
            {
                *self.multiplier.write().unwrap_or_else(|e| e.into_inner()) = new_multiplier;
                self.apply_multiplier();
                tracing::info!(
                    multiplier = new_multiplier,
                    rate_limit_pct = rate_limit_percentage,
                    "Rate limit automatically adjusted"
                );
            }

            *self
                .last_adjustment
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Instant::now();
        }
    }

    /// Apply the current multiplier to the base configuration
    fn apply_multiplier(&self) {
        let multiplier = *self.multiplier.read().unwrap_or_else(|e| e.into_inner());
        let adjusted_max_requests =
            (self.base_config.max_requests as f64 * multiplier).max(1.0) as u32;

        let new_config = RateLimitConfig {
            max_requests: adjusted_max_requests,
            window: self.base_config.window,
            cleanup_interval: self.base_config.cleanup_interval,
        };

        *self.limiter.write().unwrap_or_else(|e| e.into_inner()) =
            RateLimiter::with_config(new_config);
    }

    /// Get analytics
    pub fn analytics(&self) -> &RateLimitAnalytics {
        &self.analytics
    }

    /// Reset to base configuration
    pub fn reset(&self) {
        *self.multiplier.write().unwrap_or_else(|e| e.into_inner()) = 1.0;
        self.apply_multiplier();
        tracing::info!("Rate limit reset to base configuration");
    }
}

impl Default for DynamicRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_rate_limiter() {
        let config = RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = UserRateLimiter::new(config);

        // Should allow first 5 requests
        for _ in 0..5 {
            match limiter.check("user123") {
                RateLimitResult::Allowed { .. } => {}
                _ => panic!("Should be allowed"),
            }
        }

        // 6th request should be limited
        match limiter.check("user123") {
            RateLimitResult::Limited { .. } => {}
            _ => panic!("Should be limited"),
        }
    }

    #[test]
    fn test_endpoint_rate_limiter() {
        let default_config = RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = EndpointRateLimiter::new(default_config);

        // Set custom limit for /api/auth/login
        let auth_config = RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        limiter.set_endpoint_limit("/api/auth/login".to_string(), auth_config);

        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Should allow 3 requests to /api/auth/login
        for _ in 0..3 {
            match limiter.check(ip, "/api/auth/login") {
                RateLimitResult::Allowed { .. } => {}
                _ => panic!("Should be allowed"),
            }
        }

        // 4th request should be limited
        match limiter.check(ip, "/api/auth/login") {
            RateLimitResult::Limited { .. } => {}
            _ => panic!("Should be limited"),
        }

        // But other endpoints should still work
        match limiter.check(ip, "/api/users/me") {
            RateLimitResult::Allowed { .. } => {}
            _ => panic!("Should be allowed for other endpoint"),
        }
    }

    #[test]
    fn test_rate_limit_analytics() {
        let analytics = RateLimitAnalytics::new();

        analytics.record_request("/api/tokens", false);
        analytics.record_request("/api/tokens", false);
        analytics.record_request("/api/tokens", true);
        analytics.record_request("/api/orders", false);

        assert_eq!(analytics.total_requests(), 4);
        assert_eq!(analytics.rate_limited(), 1);
        assert_eq!(analytics.rate_limit_percentage(), 25.0);
    }

    #[test]
    fn test_dynamic_rate_limiter_manual_adjustment() {
        let config = RateLimitConfig {
            max_requests: 10,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        };
        let limiter = DynamicRateLimiter::new(config);

        // Initially multiplier should be 1.0
        assert_eq!(limiter.get_multiplier(), 1.0);

        // Set multiplier to 0.5 (half the rate)
        limiter.set_multiplier(0.5);
        assert_eq!(limiter.get_multiplier(), 0.5);

        // Set multiplier to 2.0 (double the rate)
        limiter.set_multiplier(2.0);
        assert_eq!(limiter.get_multiplier(), 2.0);

        // Reset should bring it back to 1.0
        limiter.reset();
        assert_eq!(limiter.get_multiplier(), 1.0);
    }

    #[test]
    fn test_dynamic_rate_limiter_bounds() {
        let limiter = DynamicRateLimiter::default();

        // Should clamp to minimum 0.1
        limiter.set_multiplier(0.01);
        assert_eq!(limiter.get_multiplier(), 0.1);

        // Should clamp to maximum 10.0
        limiter.set_multiplier(100.0);
        assert_eq!(limiter.get_multiplier(), 10.0);
    }
}
