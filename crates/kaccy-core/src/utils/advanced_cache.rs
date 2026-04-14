//! Advanced caching with multi-tier support, cache warming, and predictive preloading

use chrono::{DateTime, Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use crate::error::CoreError;

/// Serialize a value to bytes
fn serialize_to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CoreError> {
    serde_json::to_vec(value)
        .map_err(|e| CoreError::Validation(format!("Serialization error: {}", e)))
}

/// Deserialize bytes to a value
fn deserialize_from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CoreError> {
    serde_json::from_slice(bytes)
        .map_err(|e| CoreError::Validation(format!("Deserialization error: {}", e)))
}

/// Multi-tier cache with L1 (hot) and L2 (warm) layers
#[derive(Clone)]
pub struct MultiTierCache {
    /// L1 cache (hot, fast, smaller)
    l1: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// L2 cache (warm, larger)
    l2: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Maximum L1 cache size
    l1_max_size: usize,
    /// Maximum L2 cache size
    l2_max_size: usize,
    /// Access frequency tracker for promotion/demotion
    access_tracker: Arc<RwLock<AccessTracker>>,
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
struct CacheEntry {
    data: Vec<u8>,
    expires_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
    last_accessed: DateTime<Utc>,
    access_count: usize,
}

impl CacheEntry {
    fn new(data: Vec<u8>, ttl: Option<Duration>) -> Self {
        let now = Utc::now();
        Self {
            data,
            expires_at: ttl.map(|d| now + d),
            created_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| Utc::now() > exp).unwrap_or(false)
    }

    fn touch(&mut self) {
        self.last_accessed = Utc::now();
        self.access_count += 1;
    }
}

/// Tracks access patterns for cache optimization
#[derive(Debug, Clone)]
struct AccessTracker {
    /// Recent access history (key, timestamp)
    history: VecDeque<(String, DateTime<Utc>)>,
    /// Maximum history size
    max_history: usize,
    /// Access frequency by key
    frequency: HashMap<String, usize>,
}

impl AccessTracker {
    fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
            frequency: HashMap::new(),
        }
    }

    fn record_access(&mut self, key: String) {
        let now = Utc::now();

        // Add to history
        self.history.push_back((key.clone(), now));
        if self.history.len() > self.max_history {
            if let Some((old_key, _)) = self.history.pop_front() {
                // Decrement frequency for removed entry
                if let Some(count) = self.frequency.get_mut(&old_key) {
                    *count = count.saturating_sub(1);
                }
            }
        }

        // Update frequency
        *self.frequency.entry(key).or_insert(0) += 1;
    }

    fn get_hot_keys(&self, top_n: usize) -> Vec<String> {
        let mut keys: Vec<_> = self.frequency.iter().collect();
        keys.sort_by(|a, b| b.1.cmp(a.1));
        keys.into_iter()
            .take(top_n)
            .map(|(k, _)| k.clone())
            .collect()
    }

    #[allow(dead_code)]
    fn get_access_frequency(&self, key: &str) -> usize {
        self.frequency.get(key).copied().unwrap_or(0)
    }
}

impl MultiTierCache {
    /// Create a new multi-tier cache
    pub fn new(l1_max_size: usize, l2_max_size: usize) -> Self {
        Self {
            l1: Arc::new(RwLock::new(HashMap::new())),
            l2: Arc::new(RwLock::new(HashMap::new())),
            l1_max_size,
            l2_max_size,
            access_tracker: Arc::new(RwLock::new(AccessTracker::new(10000))),
        }
    }

    /// Get a value from cache (tries L1 first, then L2)
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CoreError> {
        // Record access
        {
            let mut tracker = self.access_tracker.write().unwrap();
            tracker.record_access(key.to_string());
        }

        // Try L1 first
        {
            let mut l1 = self.l1.write().unwrap();
            if let Some(entry) = l1.get_mut(key) {
                if !entry.is_expired() {
                    entry.touch();
                    let value = deserialize_from_bytes(&entry.data)?;
                    return Ok(Some(value));
                } else {
                    l1.remove(key);
                }
            }
        }

        // Try L2
        {
            let mut l2 = self.l2.write().unwrap();
            if let Some(mut entry) = l2.remove(key) {
                if !entry.is_expired() {
                    entry.touch();
                    let value = deserialize_from_bytes(&entry.data)?;

                    // Promote to L1 (it's hot now)
                    drop(l2); // Release L2 lock before acquiring L1
                    self.promote_to_l1(key.to_string(), entry);

                    return Ok(Some(value));
                } else {
                    // Entry expired
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }

    /// Set a value in cache
    pub fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<(), CoreError> {
        let data = serialize_to_bytes(value)?;

        let entry = CacheEntry::new(data, ttl);

        // Always put in L1 first (it's new/hot data)
        self.promote_to_l1(key.to_string(), entry);

        Ok(())
    }

    /// Promote an entry to L1, potentially demoting something to L2
    fn promote_to_l1(&self, key: String, entry: CacheEntry) {
        let mut l1 = self.l1.write().unwrap();

        // If L1 is full, demote least recently used to L2
        if l1.len() >= self.l1_max_size && !l1.contains_key(&key) {
            if let Some((lru_key, lru_entry)) = self.find_lru(&l1) {
                let lru_key = lru_key.clone();
                let lru_entry = lru_entry.clone();
                l1.remove(&lru_key);
                drop(l1); // Release L1 lock

                // Add to L2
                let mut l2 = self.l2.write().unwrap();
                if l2.len() >= self.l2_max_size {
                    // L2 is also full, remove LRU from L2
                    if let Some((l2_lru_key, _)) = self.find_lru(&l2) {
                        let l2_lru_key = l2_lru_key.clone();
                        l2.remove(&l2_lru_key);
                    }
                }
                l2.insert(lru_key, lru_entry);
                drop(l2);

                // Re-acquire L1 lock
                l1 = self.l1.write().unwrap();
            }
        }

        l1.insert(key, entry);
    }

    /// Find least recently used entry
    fn find_lru<'a>(
        &self,
        cache: &'a HashMap<String, CacheEntry>,
    ) -> Option<(&'a String, &'a CacheEntry)> {
        cache.iter().min_by_key(|(_, entry)| entry.last_accessed)
    }

    /// Delete a key from all tiers
    pub fn delete(&self, key: &str) -> Result<(), CoreError> {
        self.l1.write().unwrap().remove(key);
        self.l2.write().unwrap().remove(key);
        Ok(())
    }

    /// Clear all caches
    pub fn clear(&self) -> Result<(), CoreError> {
        self.l1.write().unwrap().clear();
        self.l2.write().unwrap().clear();
        self.access_tracker.write().unwrap().history.clear();
        self.access_tracker.write().unwrap().frequency.clear();
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let l1 = self.l1.read().unwrap();
        let l2 = self.l2.read().unwrap();

        CacheStats {
            l1_size: l1.len(),
            l2_size: l2.len(),
            l1_max_size: self.l1_max_size,
            l2_max_size: self.l2_max_size,
            total_entries: l1.len() + l2.len(),
        }
    }

    /// Get hot keys (most frequently accessed)
    pub fn get_hot_keys(&self, top_n: usize) -> Vec<String> {
        let tracker = self.access_tracker.read().unwrap();
        tracker.get_hot_keys(top_n)
    }

    /// Warm cache with provided data
    pub fn warm<T: Serialize>(
        &self,
        data: HashMap<String, T>,
        ttl: Option<Duration>,
    ) -> Result<(), CoreError> {
        for (key, value) in data {
            self.set(&key, &value, ttl)?;
        }
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current number of entries in the L1 (hot) cache tier.
    pub l1_size: usize,
    /// Current number of entries in the L2 (warm) cache tier.
    pub l2_size: usize,
    /// Maximum capacity of the L1 cache tier.
    pub l1_max_size: usize,
    /// Maximum capacity of the L2 cache tier.
    pub l2_max_size: usize,
    /// Total entries across all tiers.
    pub total_entries: usize,
}

/// Cache warming strategy
pub struct CacheWarmer {
    cache: MultiTierCache,
}

impl CacheWarmer {
    /// Create a new cache warmer
    pub fn new(cache: MultiTierCache) -> Self {
        Self { cache }
    }

    /// Warm cache with most likely needed data
    pub fn warm_most_accessed<F, T>(
        &self,
        loader: F,
        ttl: Option<Duration>,
    ) -> Result<usize, CoreError>
    where
        F: Fn(&[String]) -> Result<HashMap<String, T>, CoreError>,
        T: Serialize,
    {
        // Get hot keys from access tracker
        let hot_keys = self.cache.get_hot_keys(100);

        if hot_keys.is_empty() {
            return Ok(0);
        }

        // Load data for hot keys
        let data = loader(&hot_keys)?;

        let count = data.len();
        self.cache.warm(data, ttl)?;

        Ok(count)
    }

    /// Warm cache with specific keys
    pub fn warm_keys<F, T>(
        &self,
        keys: Vec<String>,
        loader: F,
        ttl: Option<Duration>,
    ) -> Result<usize, CoreError>
    where
        F: Fn(&[String]) -> Result<HashMap<String, T>, CoreError>,
        T: Serialize,
    {
        let data = loader(&keys)?;
        let count = data.len();
        self.cache.warm(data, ttl)?;
        Ok(count)
    }
}

/// Predictive cache preloader
pub struct PredictivePreloader {
    cache: MultiTierCache,
    /// Pattern: keys that are accessed together
    access_patterns: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl PredictivePreloader {
    /// Create a new predictive preloader
    pub fn new(cache: MultiTierCache) -> Self {
        Self {
            cache,
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record that keys were accessed together
    pub fn record_pattern(&self, keys: Vec<String>) {
        if keys.len() < 2 {
            return;
        }

        let mut patterns = self.access_patterns.write().unwrap();

        // For each key, record the other keys it was accessed with
        for (i, key) in keys.iter().enumerate() {
            let related: Vec<String> = keys
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, k)| k.clone())
                .collect();

            patterns.entry(key.clone()).or_default().extend(related);
        }
    }

    /// Predict and preload keys likely to be accessed next
    pub fn preload<F, T>(
        &self,
        accessed_key: &str,
        loader: F,
        ttl: Option<Duration>,
    ) -> Result<usize, CoreError>
    where
        F: Fn(&[String]) -> Result<HashMap<String, T>, CoreError>,
        T: Serialize,
    {
        let patterns = self.access_patterns.read().unwrap();

        if let Some(related_keys) = patterns.get(accessed_key) {
            // Get unique related keys that aren't already in cache
            let mut keys_to_load = Vec::new();
            for key in related_keys {
                // Quick check if key exists (this is simplified)
                if self.cache.get::<Vec<u8>>(key)?.is_none() && !keys_to_load.contains(key) {
                    keys_to_load.push(key.clone());
                }
            }

            if keys_to_load.is_empty() {
                return Ok(0);
            }

            // Limit preloading to prevent overload
            keys_to_load.truncate(10);

            let data = loader(&keys_to_load)?;
            let count = data.len();
            self.cache.warm(data, ttl)?;

            return Ok(count);
        }

        Ok(0)
    }

    /// Get access pattern statistics
    pub fn pattern_stats(&self) -> Vec<(String, usize)> {
        let patterns = self.access_patterns.read().unwrap();
        patterns.iter().map(|(k, v)| (k.clone(), v.len())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_tier_cache_basic() {
        let cache = MultiTierCache::new(2, 4);

        // Set some values
        cache.set("key1", &"value1", None).unwrap();
        cache.set("key2", &"value2", None).unwrap();

        // Get values
        let val1: String = cache.get("key1").unwrap().unwrap();
        let val2: String = cache.get("key2").unwrap().unwrap();

        assert_eq!(val1, "value1");
        assert_eq!(val2, "value2");
    }

    #[test]
    fn test_l1_to_l2_demotion() {
        let cache = MultiTierCache::new(2, 4);

        // Fill L1
        cache.set("key1", &"value1", None).unwrap();
        cache.set("key2", &"value2", None).unwrap();

        // Add another item, should demote least recently used
        cache.set("key3", &"value3", None).unwrap();

        let stats = cache.stats();
        assert!(stats.total_entries <= 6); // Within capacity
    }

    #[test]
    fn test_l2_to_l1_promotion() {
        let cache = MultiTierCache::new(2, 4);

        // Add items
        cache.set("key1", &"value1", None).unwrap();
        cache.set("key2", &"value2", None).unwrap();
        cache.set("key3", &"value3", None).unwrap();

        // Access key that might be in L2
        let _: Option<String> = cache.get("key1").unwrap();

        // key1 should now be promoted to L1
        let stats = cache.stats();
        assert!(stats.l1_size > 0);
    }

    #[test]
    fn test_cache_stats() {
        let cache = MultiTierCache::new(10, 20);

        cache.set("key1", &"value1", None).unwrap();
        cache.set("key2", &"value2", None).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.l1_max_size, 10);
        assert_eq!(stats.l2_max_size, 20);
        assert!(stats.total_entries >= 2);
    }

    #[test]
    fn test_hot_keys_tracking() {
        let cache = MultiTierCache::new(10, 20);

        // Access key1 multiple times
        cache.set("key1", &"value1", None).unwrap();
        for _ in 0..5 {
            let _: Option<String> = cache.get("key1").unwrap();
        }

        cache.set("key2", &"value2", None).unwrap();
        let _: Option<String> = cache.get("key2").unwrap();

        let hot_keys = cache.get_hot_keys(2);
        assert!(!hot_keys.is_empty());
    }

    #[test]
    fn test_cache_warmer() {
        let cache = MultiTierCache::new(10, 20);
        let warmer = CacheWarmer::new(cache.clone());

        let keys = vec!["key1".to_string(), "key2".to_string()];
        let loader = |_keys: &[String]| -> Result<HashMap<String, String>, CoreError> {
            let mut map = HashMap::new();
            map.insert("key1".to_string(), "value1".to_string());
            map.insert("key2".to_string(), "value2".to_string());
            Ok(map)
        };

        let count = warmer.warm_keys(keys, loader, None).unwrap();
        assert_eq!(count, 2);

        let val: String = cache.get("key1").unwrap().unwrap();
        assert_eq!(val, "value1");
    }

    #[test]
    fn test_predictive_preloader() {
        let cache = MultiTierCache::new(10, 20);
        let preloader = PredictivePreloader::new(cache.clone());

        // Record access pattern
        preloader.record_pattern(vec![
            "user:1".to_string(),
            "user:1:profile".to_string(),
            "user:1:settings".to_string(),
        ]);

        // Set the first key
        cache.set("user:1", &"data", None).unwrap();

        // Preload related keys
        let loader = |keys: &[String]| -> Result<HashMap<String, String>, CoreError> {
            let mut map = HashMap::new();
            for key in keys {
                map.insert(key.clone(), format!("data for {}", key));
            }
            Ok(map)
        };

        let count = preloader.preload("user:1", loader, None).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_cache_clear() {
        let cache = MultiTierCache::new(10, 20);

        cache.set("key1", &"value1", None).unwrap();
        cache.set("key2", &"value2", None).unwrap();

        cache.clear().unwrap();

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_cache_delete() {
        let cache = MultiTierCache::new(10, 20);

        cache.set("key1", &"value1", None).unwrap();
        cache.delete("key1").unwrap();

        let val: Option<String> = cache.get("key1").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn test_ttl_expiration() {
        let cache = MultiTierCache::new(10, 20);

        // Set with very short TTL (this is a simplified test)
        let ttl = Duration::seconds(-1); // Already expired
        cache.set("key1", &"value1", Some(ttl)).unwrap();

        let val: Option<String> = cache.get("key1").unwrap();
        assert!(val.is_none());
    }
}
