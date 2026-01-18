//! Multi-tier caching with L1 (in-memory) and L2 (Redis) layers.
//!
//! This module provides:
//! - L1 in-memory cache (local)
//! - L2 Redis cache (distributed)
//! - Automatic cache tier promotion/demotion

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::cache::RedisCache;
use crate::error::Result;

/// Configuration for multi-tier cache
#[derive(Debug, Clone)]
pub struct MultiTierCacheConfig {
    /// Maximum size of L1 cache
    pub l1_max_size: usize,
    /// TTL for L1 cache entries (seconds)
    pub l1_ttl_secs: u64,
    /// TTL for L2 cache entries (seconds)
    pub l2_ttl_secs: u64,
    /// Promote to L1 after N L2 hits
    pub promotion_threshold: u64,
    /// Demote from L1 if not accessed for N seconds
    pub demotion_threshold_secs: u64,
}

impl Default for MultiTierCacheConfig {
    fn default() -> Self {
        Self {
            l1_max_size: 1000,
            l1_ttl_secs: 300,  // 5 minutes
            l2_ttl_secs: 3600, // 1 hour
            promotion_threshold: 3,
            demotion_threshold_secs: 600, // 10 minutes
        }
    }
}

/// L1 cache entry
#[derive(Debug, Clone)]
struct L1Entry<T> {
    /// Cached value
    value: T,
    /// When the entry was created
    created_at: Instant,
    /// Last access time
    last_accessed: Instant,
    /// Number of accesses
    access_count: u64,
}

impl<T> L1Entry<T> {
    fn new(value: T) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    fn should_demote(&self, threshold: Duration) -> bool {
        self.last_accessed.elapsed() > threshold
    }
}

/// Statistics for multi-tier cache
#[derive(Debug, Clone, Default)]
pub struct MultiTierStats {
    /// L1 cache hits
    pub l1_hits: u64,
    /// L2 cache hits
    pub l2_hits: u64,
    /// Cache misses (both L1 and L2)
    pub misses: u64,
    /// Promotions from L2 to L1
    pub promotions: u64,
    /// Demotions from L1 to L2
    pub demotions: u64,
    /// L1 evictions
    pub l1_evictions: u64,
}

impl MultiTierStats {
    /// Calculate total hits
    pub fn total_hits(&self) -> u64 {
        self.l1_hits + self.l2_hits
    }

    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits() + self.misses;
        if total == 0 {
            0.0
        } else {
            self.total_hits() as f64 / total as f64
        }
    }

    /// Calculate L1 hit rate
    pub fn l1_hit_rate(&self) -> f64 {
        let total = self.l1_hits + self.l2_hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.l1_hits as f64 / total as f64
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.l1_hits = 0;
        self.l2_hits = 0;
        self.misses = 0;
        self.promotions = 0;
        self.demotions = 0;
        self.l1_evictions = 0;
    }
}

/// Multi-tier cache with L1 (in-memory) and L2 (Redis) layers
pub struct MultiTierCache {
    /// L1 cache (in-memory, fast)
    l1_cache: Arc<DashMap<String, L1Entry<Vec<u8>>>>,
    /// L2 cache (Redis, distributed)
    l2_cache: Arc<RedisCache>,
    /// LRU queue for L1 eviction
    lru_queue: Arc<RwLock<VecDeque<String>>>,
    /// Access counters for L2 (for promotion)
    l2_access_counts: Arc<DashMap<String, u64>>,
    /// Configuration
    config: MultiTierCacheConfig,
    /// Statistics
    stats: Arc<RwLock<MultiTierStats>>,
}

impl MultiTierCache {
    /// Create a new multi-tier cache
    pub fn new(l2_cache: Arc<RedisCache>, config: MultiTierCacheConfig) -> Self {
        Self {
            l1_cache: Arc::new(DashMap::new()),
            l2_cache,
            lru_queue: Arc::new(RwLock::new(VecDeque::new())),
            l2_access_counts: Arc::new(DashMap::new()),
            config,
            stats: Arc::new(RwLock::new(MultiTierStats::default())),
        }
    }

    /// Get a value from the cache
    pub async fn get<T: DeserializeOwned + Serialize>(&self, key: &str) -> Result<Option<T>> {
        // Try L1 cache first
        if let Some(mut entry) = self.l1_cache.get_mut(key) {
            // Check if expired
            if entry.is_expired(Duration::from_secs(self.config.l1_ttl_secs)) {
                drop(entry);
                self.l1_cache.remove(key);
                self.remove_from_lru(key);
                debug!(key = %key, "L1 entry expired");
            } else {
                entry.touch();
                self.stats.write().l1_hits += 1;
                debug!(key = %key, "L1 cache hit");

                let value: T = serde_json::from_slice(&entry.value).map_err(|e| {
                    crate::error::DbError::Cache(format!("Deserialization error: {}", e))
                })?;

                return Ok(Some(value));
            }
        }

        // Try L2 cache
        let l2_result: Option<T> = self.l2_cache.get(key).await?;

        if let Some(value) = l2_result.as_ref() {
            self.stats.write().l2_hits += 1;
            debug!(key = %key, "L2 cache hit");

            // Track L2 access for potential promotion
            let mut count = self.l2_access_counts.entry(key.to_string()).or_insert(0);
            *count += 1;

            // Promote to L1 if threshold reached
            if *count >= self.config.promotion_threshold {
                debug!(key = %key, count = *count, "Promoting to L1");
                self.promote_to_l1(key, value).await?;
                self.l2_access_counts.remove(key);
                self.stats.write().promotions += 1;
            }

            Ok(Some(l2_result.unwrap()))
        } else {
            self.stats.write().misses += 1;
            debug!(key = %key, "Cache miss");
            Ok(None)
        }
    }

    /// Set a value in the cache
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        // Serialize once for both tiers
        let bytes = serde_json::to_vec(value)
            .map_err(|e| crate::error::DbError::Cache(format!("Serialization error: {}", e)))?;

        // Store in L1
        self.evict_if_needed();

        let entry = L1Entry::new(bytes.clone());
        self.l1_cache.insert(key.to_string(), entry);
        self.lru_queue.write().push_front(key.to_string());

        debug!(key = %key, "Stored in L1");

        // Store in L2
        self.l2_cache
            .set(key, value, self.config.l2_ttl_secs)
            .await?;

        debug!(key = %key, "Stored in L2");

        Ok(())
    }

    /// Delete a value from both cache tiers
    pub async fn delete(&self, key: &str) -> Result<bool> {
        self.l1_cache.remove(key);
        self.remove_from_lru(key);
        self.l2_access_counts.remove(key);

        let l2_deleted = self.l2_cache.delete(key).await?;

        debug!(key = %key, "Deleted from both tiers");

        Ok(l2_deleted)
    }

    /// Promote a value to L1 cache
    async fn promote_to_l1<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| crate::error::DbError::Cache(format!("Serialization error: {}", e)))?;

        self.evict_if_needed();

        let entry = L1Entry::new(bytes);
        self.l1_cache.insert(key.to_string(), entry);
        self.lru_queue.write().push_front(key.to_string());

        Ok(())
    }

    /// Evict entries from L1 if needed
    fn evict_if_needed(&self) {
        while self.l1_cache.len() >= self.config.l1_max_size {
            if let Some(oldest_key) = self.lru_queue.write().pop_back() {
                self.l1_cache.remove(&oldest_key);
                self.stats.write().l1_evictions += 1;
                debug!(key = %oldest_key, "Evicted from L1");
            } else {
                break;
            }
        }
    }

    /// Remove a key from the LRU queue
    fn remove_from_lru(&self, key: &str) {
        let mut queue = self.lru_queue.write();
        if let Some(pos) = queue.iter().position(|k| k == key) {
            queue.remove(pos);
        }
    }

    /// Cleanup expired and inactive L1 entries
    pub fn cleanup_l1(&self) {
        let ttl = Duration::from_secs(self.config.l1_ttl_secs);
        let demotion_threshold = Duration::from_secs(self.config.demotion_threshold_secs);

        let keys_to_remove: Vec<String> = self
            .l1_cache
            .iter()
            .filter(|entry| {
                entry.value().is_expired(ttl) || entry.value().should_demote(demotion_threshold)
            })
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.l1_cache.remove(&key);
            self.remove_from_lru(&key);
            self.stats.write().demotions += 1;
            debug!(key = %key, "Demoted/expired from L1");
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> MultiTierStats {
        self.stats.read().clone()
    }

    /// Get L1 cache size
    pub fn l1_size(&self) -> usize {
        self.l1_cache.len()
    }

    /// Clear both cache tiers
    pub async fn clear(&self) -> Result<()> {
        self.l1_cache.clear();
        self.lru_queue.write().clear();
        self.l2_access_counts.clear();

        // Note: Clearing L2 requires pattern deletion which may be expensive
        // For now, we only clear L1
        warn!("Cleared L1 cache, L2 entries will expire naturally");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_tier_config_default() {
        let config = MultiTierCacheConfig::default();
        assert_eq!(config.l1_max_size, 1000);
        assert_eq!(config.l1_ttl_secs, 300);
        assert_eq!(config.l2_ttl_secs, 3600);
        assert_eq!(config.promotion_threshold, 3);
        assert_eq!(config.demotion_threshold_secs, 600);
    }

    #[test]
    fn test_l1_entry_expiration() {
        let entry = L1Entry::new("test".to_string());
        assert!(!entry.is_expired(Duration::from_secs(3600)));
    }

    #[test]
    fn test_l1_entry_touch() {
        let mut entry = L1Entry::new("test".to_string());
        let initial_count = entry.access_count;

        entry.touch();

        assert_eq!(entry.access_count, initial_count + 1);
    }

    #[test]
    fn test_multi_tier_stats() {
        let stats = MultiTierStats {
            l1_hits: 80,
            l2_hits: 15,
            misses: 5,
            promotions: 3,
            demotions: 2,
            l1_evictions: 1,
        };

        assert_eq!(stats.total_hits(), 95);
        assert!((stats.hit_rate() - 0.95).abs() < 0.01);
        assert!((stats.l1_hit_rate() - 0.80).abs() < 0.01);
    }

    #[test]
    fn test_stats_reset() {
        let mut stats = MultiTierStats {
            l1_hits: 100,
            l2_hits: 50,
            misses: 10,
            promotions: 5,
            demotions: 3,
            l1_evictions: 2,
        };

        stats.reset();

        assert_eq!(stats.l1_hits, 0);
        assert_eq!(stats.l2_hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.promotions, 0);
        assert_eq!(stats.demotions, 0);
        assert_eq!(stats.l1_evictions, 0);
    }
}
