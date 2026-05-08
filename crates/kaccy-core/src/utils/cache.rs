//! Caching layer interface and implementations

use chrono::{DateTime, Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::{CoreError, Result};

/// Cache entry with expiration
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Option<DateTime<Utc>>,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Option<Duration>) -> Self {
        let expires_at = ttl.map(|d| Utc::now() + d);
        Self { value, expires_at }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| Utc::now() > exp).unwrap_or(false)
    }
}

/// Cache interface trait
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// Get a value from cache
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>>;

    /// Set a value in cache with optional TTL
    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()>;

    /// Delete a value from cache
    async fn delete(&self, key: &str) -> Result<()>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Clear all cache entries
    async fn clear(&self) -> Result<()>;

    /// Get multiple values at once
    async fn get_many<T: DeserializeOwned + Send>(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, T>>;

    /// Set multiple values at once
    async fn set_many<T: Serialize + Send + Sync>(
        &self,
        values: HashMap<String, T>,
        ttl: Option<Duration>,
    ) -> Result<()>;

    /// Delete multiple keys at once
    async fn delete_many(&self, keys: &[String]) -> Result<()>;

    /// Increment a counter
    async fn increment(&self, key: &str, delta: i64) -> Result<i64>;

    /// Decrement a counter
    async fn decrement(&self, key: &str, delta: i64) -> Result<i64>;
}

/// In-memory cache implementation
#[derive(Clone)]
pub struct MemoryCache {
    data: Arc<RwLock<HashMap<String, CacheEntry<Vec<u8>>>>>,
}

impl MemoryCache {
    /// Create a new in-memory cache
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clean up expired entries
    pub fn cleanup_expired(&self) {
        let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
        data.retain(|_, entry| !entry.is_expired());
    }

    /// Get the number of entries in cache
    pub fn len(&self) -> usize {
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        data.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        let data = self.data.read().unwrap_or_else(|e| e.into_inner());
        data.is_empty()
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Cache for MemoryCache {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let (bytes, is_expired) = {
            let data = self.data.read().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = data.get(key) {
                (Some(entry.value.clone()), entry.is_expired())
            } else {
                (None, false)
            }
        };

        if is_expired {
            self.delete(key).await?;
            return Ok(None);
        }

        if let Some(bytes) = bytes {
            let value: T = serde_json::from_slice(&bytes)
                .map_err(|e| CoreError::Serialization(e.to_string()))?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()> {
        let bytes =
            serde_json::to_vec(value).map_err(|e| CoreError::Serialization(e.to_string()))?;

        let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
        data.insert(key.to_string(), CacheEntry::new(bytes, ttl));

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
        data.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let (exists, is_expired) = {
            let data = self.data.read().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = data.get(key) {
                (true, entry.is_expired())
            } else {
                (false, false)
            }
        };

        if exists && is_expired {
            self.delete(key).await?;
            Ok(false)
        } else {
            Ok(exists)
        }
    }

    async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
        data.clear();
        Ok(())
    }

    async fn get_many<T: DeserializeOwned + Send>(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, T>> {
        let mut result = HashMap::new();

        for key in keys {
            if let Some(value) = self.get::<T>(key).await? {
                result.insert(key.clone(), value);
            }
        }

        Ok(result)
    }

    async fn set_many<T: Serialize + Send + Sync>(
        &self,
        values: HashMap<String, T>,
        ttl: Option<Duration>,
    ) -> Result<()> {
        for (key, value) in values {
            self.set(&key, &value, ttl).await?;
        }

        Ok(())
    }

    async fn delete_many(&self, keys: &[String]) -> Result<()> {
        let mut data = self.data.write().unwrap_or_else(|e| e.into_inner());
        for key in keys {
            data.remove(key);
        }

        Ok(())
    }

    async fn increment(&self, key: &str, delta: i64) -> Result<i64> {
        let current: i64 = self.get(key).await?.unwrap_or(0);
        let new_value = current + delta;
        self.set(key, &new_value, None).await?;
        Ok(new_value)
    }

    async fn decrement(&self, key: &str, delta: i64) -> Result<i64> {
        self.increment(key, -delta).await
    }
}

/// Cache key builder helper
pub struct CacheKey;

impl CacheKey {
    /// Build a key for a user
    pub fn user(user_id: &uuid::Uuid) -> String {
        format!("user:{}", user_id)
    }

    /// Build a key for a token
    pub fn token(token_id: &uuid::Uuid) -> String {
        format!("token:{}", token_id)
    }

    /// Build a key for an order
    pub fn order(order_id: &uuid::Uuid) -> String {
        format!("order:{}", order_id)
    }

    /// Build a key for a trade
    pub fn trade(trade_id: &uuid::Uuid) -> String {
        format!("trade:{}", trade_id)
    }

    /// Build a key for user balances
    pub fn user_balances(user_id: &uuid::Uuid) -> String {
        format!("user:{}:balances", user_id)
    }

    /// Build a key for token orders
    pub fn token_orders(token_id: &uuid::Uuid) -> String {
        format!("token:{}:orders", token_id)
    }

    /// Build a custom key with prefix
    pub fn custom(prefix: &str, suffix: &str) -> String {
        format!("{}:{}", prefix, suffix)
    }
}

/// Cache warming strategy
pub struct CacheWarmer<C: Cache> {
    cache: C,
}

impl<C: Cache> CacheWarmer<C> {
    /// Create a new cache warmer
    pub fn new(cache: C) -> Self {
        Self { cache }
    }

    /// Warm cache with a value
    pub async fn warm<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<()> {
        self.cache.set(key, value, ttl).await
    }

    /// Warm cache with multiple values
    pub async fn warm_many<T: Serialize + Send + Sync>(
        &self,
        values: HashMap<String, T>,
        ttl: Option<Duration>,
    ) -> Result<()> {
        self.cache.set_many(values, ttl).await
    }
}

/// Cache invalidation helper
pub struct CacheInvalidator<C: Cache> {
    cache: C,
}

impl<C: Cache> CacheInvalidator<C> {
    /// Create a new cache invalidator
    pub fn new(cache: C) -> Self {
        Self { cache }
    }

    /// Invalidate a single key
    pub async fn invalidate(&self, key: &str) -> Result<()> {
        self.cache.delete(key).await
    }

    /// Invalidate multiple keys
    pub async fn invalidate_many(&self, keys: &[String]) -> Result<()> {
        self.cache.delete_many(keys).await
    }

    /// Invalidate all user-related cache
    pub async fn invalidate_user(&self, user_id: &uuid::Uuid) -> Result<()> {
        let keys = vec![CacheKey::user(user_id), CacheKey::user_balances(user_id)];
        self.invalidate_many(&keys).await
    }

    /// Invalidate all token-related cache
    pub async fn invalidate_token(&self, token_id: &uuid::Uuid) -> Result<()> {
        let keys = vec![CacheKey::token(token_id), CacheKey::token_orders(token_id)];
        self.invalidate_many(&keys).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_cache_get_set() {
        let cache = MemoryCache::new();

        cache.set("key1", &"value1", None).await.unwrap();

        let value: Option<String> = cache.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_memory_cache_ttl() {
        let cache = MemoryCache::new();

        let ttl = Duration::milliseconds(100);
        cache.set("key1", &"value1", Some(ttl)).await.unwrap();

        // Value should exist immediately
        let value: Option<String> = cache.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Value should be expired
        let value: Option<String> = cache.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_memory_cache_delete() {
        let cache = MemoryCache::new();

        cache.set("key1", &"value1", None).await.unwrap();
        assert!(cache.exists("key1").await.unwrap());

        cache.delete("key1").await.unwrap();
        assert!(!cache.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_cache_clear() {
        let cache = MemoryCache::new();

        cache.set("key1", &"value1", None).await.unwrap();
        cache.set("key2", &"value2", None).await.unwrap();

        cache.clear().await.unwrap();

        assert!(!cache.exists("key1").await.unwrap());
        assert!(!cache.exists("key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_cache_increment() {
        let cache = MemoryCache::new();

        let value = cache.increment("counter", 1).await.unwrap();
        assert_eq!(value, 1);

        let value = cache.increment("counter", 5).await.unwrap();
        assert_eq!(value, 6);
    }

    #[tokio::test]
    async fn test_memory_cache_decrement() {
        let cache = MemoryCache::new();

        cache.set("counter", &10i64, None).await.unwrap();

        let value = cache.decrement("counter", 3).await.unwrap();
        assert_eq!(value, 7);
    }

    #[tokio::test]
    async fn test_cache_key_builder() {
        let user_id = uuid::Uuid::new_v4();
        let key = CacheKey::user(&user_id);
        assert!(key.starts_with("user:"));

        let token_id = uuid::Uuid::new_v4();
        let key = CacheKey::token(&token_id);
        assert!(key.starts_with("token:"));
    }

    #[tokio::test]
    async fn test_cache_warmer() {
        let cache = MemoryCache::new();
        let warmer = CacheWarmer::new(cache.clone());

        warmer.warm("key1", &"value1", None).await.unwrap();

        let value: Option<String> = cache.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_cache_invalidator() {
        let cache = MemoryCache::new();
        let invalidator = CacheInvalidator::new(cache.clone());

        cache.set("key1", &"value1", None).await.unwrap();
        assert!(cache.exists("key1").await.unwrap());

        invalidator.invalidate("key1").await.unwrap();
        assert!(!cache.exists("key1").await.unwrap());
    }
}
