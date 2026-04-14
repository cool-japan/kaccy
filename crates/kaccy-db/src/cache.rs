//! Redis caching layer for high-performance data access
//!
//! Provides caching for:
//! - User sessions and profiles
//! - Token prices and metadata
//! - User balances (hot data)
//! - Rate limiting counters

use std::sync::Arc;

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::error::{DbError, Result};

/// Cache key prefixes for namespacing
pub mod keys {
    /// Key prefix for user session data.
    pub const USER_SESSION: &str = "session:";
    /// Key prefix for user profile data.
    pub const USER_PROFILE: &str = "user:";
    /// Key prefix for token price data.
    pub const TOKEN_PRICE: &str = "price:";
    /// Key prefix for token metadata.
    pub const TOKEN_META: &str = "token:";
    /// Key prefix for user balance data.
    pub const USER_BALANCE: &str = "balance:";
    /// Key prefix for rate limiting counters.
    pub const RATE_LIMIT: &str = "ratelimit:";
    /// Key prefix for distributed locks.
    pub const LOCK: &str = "lock:";
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Redis connection URL
    pub redis_url: String,
    /// Default TTL for cached items (seconds)
    pub default_ttl_secs: u64,
    /// TTL for user sessions (seconds)
    pub session_ttl_secs: u64,
    /// TTL for price cache (seconds) - should be short
    pub price_ttl_secs: u64,
    /// TTL for user profiles (seconds)
    pub profile_ttl_secs: u64,
    /// TTL for balance cache (seconds)
    pub balance_ttl_secs: u64,
    /// Key prefix for this application instance
    pub key_prefix: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            default_ttl_secs: 3600,  // 1 hour
            session_ttl_secs: 86400, // 24 hours
            price_ttl_secs: 5,       // 5 seconds (volatile)
            profile_ttl_secs: 300,   // 5 minutes
            balance_ttl_secs: 30,    // 30 seconds
            key_prefix: "kaccy:".to_string(),
        }
    }
}

impl CacheConfig {
    /// Create config from environment variable
    pub fn from_env() -> Self {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self {
            redis_url,
            ..Default::default()
        }
    }

    /// Set custom TTLs
    pub fn with_ttls(
        mut self,
        default: u64,
        session: u64,
        price: u64,
        profile: u64,
        balance: u64,
    ) -> Self {
        self.default_ttl_secs = default;
        self.session_ttl_secs = session;
        self.price_ttl_secs = price;
        self.profile_ttl_secs = profile;
        self.balance_ttl_secs = balance;
        self
    }
}

/// Redis cache client with connection management
#[derive(Clone)]
pub struct RedisCache {
    conn: ConnectionManager,
    config: CacheConfig,
}

impl RedisCache {
    /// Create a new Redis cache connection
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let client = Client::open(config.redis_url.as_str())
            .map_err(|e| DbError::Connection(format!("Redis client error: {}", e)))?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| DbError::Connection(format!("Redis connection error: {}", e)))?;

        tracing::info!("Redis cache connected to {}", config.redis_url);

        Ok(Self { conn, config })
    }

    /// Get the full key with prefix
    fn full_key(&self, key: &str) -> String {
        format!("{}{}", self.config.key_prefix, key)
    }

    /// Get a value from cache
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let value: Option<String> = conn
            .get(&full_key)
            .await
            .map_err(|e| DbError::Cache(format!("Redis GET error: {}", e)))?;

        match value {
            Some(json) => {
                let parsed: T = serde_json::from_str(&json)
                    .map_err(|e| DbError::Cache(format!("Deserialization error: {}", e)))?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }

    /// Set a value in cache with TTL
    pub async fn set<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> Result<()> {
        let full_key = self.full_key(key);
        let json = serde_json::to_string(value)
            .map_err(|e| DbError::Cache(format!("Serialization error: {}", e)))?;

        let mut conn = self.conn.clone();
        let _: () = conn
            .set_ex(&full_key, json, ttl_secs)
            .await
            .map_err(|e| DbError::Cache(format!("Redis SET error: {}", e)))?;

        Ok(())
    }

    /// Set a value with default TTL
    pub async fn set_default<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set(key, value, self.config.default_ttl_secs).await
    }

    /// Delete a key from cache
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let deleted: i64 = conn
            .del(&full_key)
            .await
            .map_err(|e| DbError::Cache(format!("Redis DEL error: {}", e)))?;

        Ok(deleted > 0)
    }

    /// Delete multiple keys matching a pattern
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u64> {
        let full_pattern = self.full_key(pattern);
        let mut conn = self.conn.clone();

        let keys: Vec<String> = conn
            .keys(&full_pattern)
            .await
            .map_err(|e| DbError::Cache(format!("Redis KEYS error: {}", e)))?;

        if keys.is_empty() {
            return Ok(0);
        }

        let deleted: i64 = conn
            .del(&keys)
            .await
            .map_err(|e| DbError::Cache(format!("Redis DEL error: {}", e)))?;

        Ok(deleted as u64)
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let exists: bool = conn
            .exists(&full_key)
            .await
            .map_err(|e| DbError::Cache(format!("Redis EXISTS error: {}", e)))?;

        Ok(exists)
    }

    /// Increment a counter (for rate limiting)
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let value: i64 = conn
            .incr(&full_key, 1)
            .await
            .map_err(|e| DbError::Cache(format!("Redis INCR error: {}", e)))?;

        Ok(value)
    }

    /// Increment with expiry (for rate limiting windows)
    pub async fn incr_with_expiry(&self, key: &str, ttl_secs: u64) -> Result<i64> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        // Use MULTI/EXEC for atomicity
        let (value,): (i64,) = redis::pipe()
            .atomic()
            .incr(&full_key, 1)
            .expire(&full_key, ttl_secs as i64)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::Cache(format!("Redis INCR/EXPIRE error: {}", e)))?;

        Ok(value)
    }

    /// Set TTL on existing key
    pub async fn expire(&self, key: &str, ttl_secs: u64) -> Result<bool> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let set: bool = conn
            .expire(&full_key, ttl_secs as i64)
            .await
            .map_err(|e| DbError::Cache(format!("Redis EXPIRE error: {}", e)))?;

        Ok(set)
    }

    /// Get remaining TTL for a key
    pub async fn ttl(&self, key: &str) -> Result<i64> {
        let full_key = self.full_key(key);
        let mut conn = self.conn.clone();

        let ttl: i64 = conn
            .ttl(&full_key)
            .await
            .map_err(|e| DbError::Cache(format!("Redis TTL error: {}", e)))?;

        Ok(ttl)
    }

    /// Try to acquire a distributed lock
    pub async fn try_lock(&self, resource: &str, ttl_secs: u64) -> Result<Option<String>> {
        let key = format!("{}{}:{}", keys::LOCK, resource, Uuid::new_v4());
        let full_key = self.full_key(&key);
        let lock_id = Uuid::new_v4().to_string();
        let mut conn = self.conn.clone();

        let set: bool = conn
            .set_nx(&full_key, &lock_id)
            .await
            .map_err(|e| DbError::Cache(format!("Redis SETNX error: {}", e)))?;

        if set {
            let _: () = conn
                .expire(&full_key, ttl_secs as i64)
                .await
                .map_err(|e| DbError::Cache(format!("Redis EXPIRE error: {}", e)))?;
            Ok(Some(lock_id))
        } else {
            Ok(None)
        }
    }

    /// Release a distributed lock
    pub async fn release_lock(&self, resource: &str, lock_id: &str) -> Result<bool> {
        let key = format!("{}{}:{}", keys::LOCK, resource, lock_id);
        self.delete(&key).await
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.conn.clone();
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| DbError::Cache(format!("Redis PING error: {}", e)))?;

        Ok(pong == "PONG")
    }
}

// ============== Specialized Cache Operations ==============

/// User session cache operations
impl RedisCache {
    /// Cache a user session
    pub async fn set_session(&self, session_id: &str, user_id: Uuid) -> Result<()> {
        let key = format!("{}{}", keys::USER_SESSION, session_id);
        self.set(&key, &user_id, self.config.session_ttl_secs).await
    }

    /// Get user ID from session
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Uuid>> {
        let key = format!("{}{}", keys::USER_SESSION, session_id);
        self.get(&key).await
    }

    /// Invalidate a session
    pub async fn invalidate_session(&self, session_id: &str) -> Result<bool> {
        let key = format!("{}{}", keys::USER_SESSION, session_id);
        self.delete(&key).await
    }

    /// Invalidate all sessions for a user
    #[allow(dead_code)]
    pub async fn invalidate_user_sessions(&self, user_id: Uuid) -> Result<u64> {
        // This requires scanning - consider using a SET per user for production
        // Note: This is a simplified implementation
        // In production, maintain a set of session IDs per user
        tracing::warn!(
            "invalidate_user_sessions: scanning all sessions for user {}",
            user_id
        );
        Ok(0) // Placeholder - implement with user session tracking
    }
}

/// User profile cache operations
impl RedisCache {
    /// Cache user profile
    pub async fn set_user_profile<T: Serialize>(&self, user_id: Uuid, profile: &T) -> Result<()> {
        let key = format!("{}{}", keys::USER_PROFILE, user_id);
        self.set(&key, profile, self.config.profile_ttl_secs).await
    }

    /// Get cached user profile
    pub async fn get_user_profile<T: DeserializeOwned>(&self, user_id: Uuid) -> Result<Option<T>> {
        let key = format!("{}{}", keys::USER_PROFILE, user_id);
        self.get(&key).await
    }

    /// Invalidate user profile cache
    pub async fn invalidate_user_profile(&self, user_id: Uuid) -> Result<bool> {
        let key = format!("{}{}", keys::USER_PROFILE, user_id);
        self.delete(&key).await
    }
}

/// Token price cache operations
impl RedisCache {
    /// Cache token price
    pub async fn set_token_price(&self, token_id: Uuid, price_btc: f64) -> Result<()> {
        let key = format!("{}{}", keys::TOKEN_PRICE, token_id);
        self.set(&key, &price_btc, self.config.price_ttl_secs).await
    }

    /// Get cached token price
    pub async fn get_token_price(&self, token_id: Uuid) -> Result<Option<f64>> {
        let key = format!("{}{}", keys::TOKEN_PRICE, token_id);
        self.get(&key).await
    }

    /// Cache multiple token prices
    pub async fn set_token_prices(&self, prices: &[(Uuid, f64)]) -> Result<()> {
        for (token_id, price) in prices {
            self.set_token_price(*token_id, *price).await?;
        }
        Ok(())
    }
}

/// Token metadata cache operations
impl RedisCache {
    /// Cache token metadata
    pub async fn set_token_meta<T: Serialize>(&self, token_id: Uuid, meta: &T) -> Result<()> {
        let key = format!("{}{}", keys::TOKEN_META, token_id);
        self.set(&key, meta, self.config.default_ttl_secs).await
    }

    /// Get cached token metadata
    pub async fn get_token_meta<T: DeserializeOwned>(&self, token_id: Uuid) -> Result<Option<T>> {
        let key = format!("{}{}", keys::TOKEN_META, token_id);
        self.get(&key).await
    }

    /// Invalidate token metadata cache
    pub async fn invalidate_token_meta(&self, token_id: Uuid) -> Result<bool> {
        let key = format!("{}{}", keys::TOKEN_META, token_id);
        self.delete(&key).await
    }
}

/// User balance cache operations
impl RedisCache {
    /// Cache user balance for a token
    pub async fn set_balance(&self, user_id: Uuid, token_id: Uuid, balance: f64) -> Result<()> {
        let key = format!("{}{}:{}", keys::USER_BALANCE, user_id, token_id);
        self.set(&key, &balance, self.config.balance_ttl_secs).await
    }

    /// Get cached balance
    pub async fn get_balance(&self, user_id: Uuid, token_id: Uuid) -> Result<Option<f64>> {
        let key = format!("{}{}:{}", keys::USER_BALANCE, user_id, token_id);
        self.get(&key).await
    }

    /// Invalidate all balances for a user
    pub async fn invalidate_user_balances(&self, user_id: Uuid) -> Result<u64> {
        let pattern = format!("{}{}:*", keys::USER_BALANCE, user_id);
        self.delete_pattern(&pattern).await
    }

    /// Invalidate all balances for a token
    pub async fn invalidate_token_balances(&self, token_id: Uuid) -> Result<u64> {
        let pattern = format!("{}*:{}", keys::USER_BALANCE, token_id);
        self.delete_pattern(&pattern).await
    }
}

/// Rate limiting operations
impl RedisCache {
    /// Check and increment rate limit
    /// Returns (current_count, is_allowed)
    pub async fn check_rate_limit(
        &self,
        identifier: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<(u64, bool)> {
        let key = format!("{}{}", keys::RATE_LIMIT, identifier);
        let count = self.incr_with_expiry(&key, window_secs).await? as u64;
        Ok((count, count <= limit))
    }

    /// Get current rate limit count
    pub async fn get_rate_limit_count(&self, identifier: &str) -> Result<u64> {
        let key = format!("{}{}", keys::RATE_LIMIT, identifier);
        let count: Option<u64> = self.get(&key).await?;
        Ok(count.unwrap_or(0))
    }

    /// Reset rate limit for identifier
    pub async fn reset_rate_limit(&self, identifier: &str) -> Result<bool> {
        let key = format!("{}{}", keys::RATE_LIMIT, identifier);
        self.delete(&key).await
    }
}

/// Cached repository wrapper for read-through caching
pub struct CachedRepository<R> {
    cache: Arc<RedisCache>,
    repo: R,
}

impl<R> CachedRepository<R> {
    /// Create a new cached repository wrapping the given repo and cache.
    pub fn new(cache: Arc<RedisCache>, repo: R) -> Self {
        Self { cache, repo }
    }

    /// Get the underlying repository
    pub fn repo(&self) -> &R {
        &self.repo
    }

    /// Get the cache
    pub fn cache(&self) -> &RedisCache {
        &self.cache
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CacheStats {
    /// Number of successful cache lookups.
    pub hits: u64,
    /// Number of cache lookups that did not find an entry.
    pub misses: u64,
    /// Number of values written to the cache.
    pub sets: u64,
    /// Number of values removed from the cache.
    pub deletes: u64,
}

impl CacheStats {
    /// Calculate the cache hit rate as a fraction between 0.0 and 1.0.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
