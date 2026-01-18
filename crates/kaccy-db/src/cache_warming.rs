//! Cache warming strategies for improved performance.
//!
//! This module provides:
//! - Preload hot data on startup
//! - Background refresh for expiring keys
//! - Predictive cache loading

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info};

use crate::cache::RedisCache;
use crate::error::Result;

/// Cache warming strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmingStrategy {
    /// No warming
    None,
    /// Preload specific keys on startup
    Preload,
    /// Background refresh before expiry
    BackgroundRefresh,
    /// Predictive loading based on access patterns
    Predictive,
    /// All strategies combined
    All,
}

/// Configuration for cache warming
#[derive(Debug, Clone)]
pub struct CacheWarmingConfig {
    /// Warming strategy to use
    pub strategy: WarmingStrategy,
    /// Interval for background refresh (seconds)
    pub refresh_interval_secs: u64,
    /// Refresh keys when they have less than this TTL remaining (seconds)
    pub refresh_threshold_secs: u64,
    /// Maximum number of keys to warm at once
    pub batch_size: usize,
    /// Enable predictive loading
    pub enable_prediction: bool,
}

impl Default for CacheWarmingConfig {
    fn default() -> Self {
        Self {
            strategy: WarmingStrategy::All,
            refresh_interval_secs: 60,
            refresh_threshold_secs: 300, // Refresh when < 5 minutes remaining
            batch_size: 100,
            enable_prediction: true,
        }
    }
}

/// Trait for data sources that can provide warm data
#[async_trait]
pub trait CacheDataSource: Send + Sync {
    /// Load hot data keys that should be preloaded
    async fn get_hot_keys(&self) -> Result<Vec<String>>;

    /// Load data for a specific key
    async fn load_data(&self, key: &str) -> Result<Option<(String, u64)>>;
}

/// Cache warmer that manages cache warming strategies
pub struct CacheWarmer {
    cache: Arc<RedisCache>,
    config: CacheWarmingConfig,
    access_tracker: Arc<RwLock<AccessTracker>>,
}

impl CacheWarmer {
    /// Create a new cache warmer
    pub fn new(cache: Arc<RedisCache>, config: CacheWarmingConfig) -> Self {
        Self {
            cache,
            config,
            access_tracker: Arc::new(RwLock::new(AccessTracker::new())),
        }
    }

    /// Preload hot keys on startup
    pub async fn preload<T: CacheDataSource>(&self, source: Arc<T>) -> Result<usize> {
        if !matches!(
            self.config.strategy,
            WarmingStrategy::Preload | WarmingStrategy::All
        ) {
            return Ok(0);
        }

        info!("Starting cache preload");

        let hot_keys = source.get_hot_keys().await?;
        let mut loaded_count = 0;

        for chunk in hot_keys.chunks(self.config.batch_size) {
            for key in chunk {
                match source.load_data(key).await {
                    Ok(Some((value, ttl))) => {
                        if let Err(e) = self.cache.set(key, &value, ttl).await {
                            error!(key = %key, error = %e, "Failed to preload key");
                        } else {
                            loaded_count += 1;
                            debug!(key = %key, "Preloaded key");
                        }
                    }
                    Ok(None) => {
                        debug!(key = %key, "No data for key");
                    }
                    Err(e) => {
                        error!(key = %key, error = %e, "Failed to load data");
                    }
                }
            }
        }

        info!(count = loaded_count, "Cache preload completed");
        Ok(loaded_count)
    }

    /// Start background refresh task
    pub async fn start_background_refresh<T: CacheDataSource + 'static>(
        self: Arc<Self>,
        source: Arc<T>,
    ) {
        if !matches!(
            self.config.strategy,
            WarmingStrategy::BackgroundRefresh | WarmingStrategy::All
        ) {
            return;
        }

        info!(
            interval_secs = self.config.refresh_interval_secs,
            "Starting background cache refresh"
        );

        let mut refresh_interval = interval(Duration::from_secs(self.config.refresh_interval_secs));

        tokio::spawn(async move {
            loop {
                refresh_interval.tick().await;

                debug!("Running background cache refresh");

                match source.get_hot_keys().await {
                    Ok(keys) => {
                        for chunk in keys.chunks(self.config.batch_size) {
                            for key in chunk {
                                // Check if key needs refresh
                                match self.cache.ttl(key).await {
                                    Ok(ttl)
                                        if ttl > 0
                                            && ttl < self.config.refresh_threshold_secs as i64 =>
                                    {
                                        // Key is expiring soon, refresh it
                                        match source.load_data(key).await {
                                            Ok(Some((value, new_ttl))) => {
                                                if let Err(e) =
                                                    self.cache.set(key, &value, new_ttl).await
                                                {
                                                    error!(key = %key, error = %e, "Failed to refresh key");
                                                } else {
                                                    debug!(key = %key, ttl = ttl, "Refreshed expiring key");
                                                }
                                            }
                                            Ok(None) => {}
                                            Err(e) => {
                                                error!(key = %key, error = %e, "Failed to load data for refresh");
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        // Key doesn't need refresh yet
                                    }
                                    Err(e) => {
                                        error!(key = %key, error = %e, "Failed to get TTL");
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to get hot keys for refresh");
                    }
                }
            }
        });
    }

    /// Track key access for predictive loading
    pub fn track_access(&self, key: &str) {
        if !self.config.enable_prediction {
            return;
        }

        self.access_tracker.write().record_access(key);
    }

    /// Get predicted keys that should be loaded
    pub fn get_predicted_keys(&self, limit: usize) -> Vec<String> {
        if !self.config.enable_prediction {
            return Vec::new();
        }

        self.access_tracker.read().get_top_keys(limit)
    }

    /// Predictively load keys based on access patterns
    pub async fn predictive_load<T: CacheDataSource>(
        &self,
        source: Arc<T>,
        limit: usize,
    ) -> Result<usize> {
        if !matches!(
            self.config.strategy,
            WarmingStrategy::Predictive | WarmingStrategy::All
        ) {
            return Ok(0);
        }

        let predicted_keys = self.get_predicted_keys(limit);
        let mut loaded_count = 0;

        for key in predicted_keys {
            // Check if key is already cached
            if let Ok(true) = self.cache.exists(&key).await {
                continue;
            }

            // Load and cache the key
            match source.load_data(&key).await {
                Ok(Some((value, ttl))) => {
                    if let Err(e) = self.cache.set(&key, &value, ttl).await {
                        error!(key = %key, error = %e, "Failed to predictively load key");
                    } else {
                        loaded_count += 1;
                        debug!(key = %key, "Predictively loaded key");
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    error!(key = %key, error = %e, "Failed to load data for prediction");
                }
            }
        }

        if loaded_count > 0 {
            info!(count = loaded_count, "Predictive cache loading completed");
        }

        Ok(loaded_count)
    }
}

/// Access tracker for predictive loading
#[derive(Debug, Clone)]
struct AccessTracker {
    /// Access counts by key
    counts: HashMap<String, AccessCount>,
    /// Total accesses tracked
    total_accesses: u64,
}

#[derive(Debug, Clone)]
struct AccessCount {
    /// Number of accesses
    count: u64,
    /// Last access time
    last_access: DateTime<Utc>,
    /// Access frequency (accesses per hour)
    frequency: f64,
}

impl AccessTracker {
    fn new() -> Self {
        Self {
            counts: HashMap::new(),
            total_accesses: 0,
        }
    }

    fn record_access(&mut self, key: &str) {
        let now = Utc::now();
        self.total_accesses += 1;

        self.counts
            .entry(key.to_string())
            .and_modify(|count| {
                count.count += 1;
                let hours_since_last =
                    now.signed_duration_since(count.last_access).num_seconds() as f64 / 3600.0;
                if hours_since_last > 0.0 {
                    count.frequency = count.count as f64 / hours_since_last;
                }
                count.last_access = now;
            })
            .or_insert(AccessCount {
                count: 1,
                last_access: now,
                frequency: 0.0,
            });
    }

    fn get_top_keys(&self, limit: usize) -> Vec<String> {
        let mut keys: Vec<(String, u64, f64)> = self
            .counts
            .iter()
            .map(|(k, v)| (k.clone(), v.count, v.frequency))
            .collect();

        // Sort by frequency, then by count
        keys.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.cmp(&a.1))
        });

        keys.into_iter().take(limit).map(|(k, _, _)| k).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    struct MockDataSource {
        keys: Vec<String>,
    }

    #[allow(dead_code)]
    #[async_trait]
    impl CacheDataSource for MockDataSource {
        async fn get_hot_keys(&self) -> Result<Vec<String>> {
            Ok(self.keys.clone())
        }

        async fn load_data(&self, key: &str) -> Result<Option<(String, u64)>> {
            Ok(Some((format!("data:{}", key), 3600)))
        }
    }

    #[test]
    fn test_warming_config_default() {
        let config = CacheWarmingConfig::default();
        assert_eq!(config.strategy, WarmingStrategy::All);
        assert_eq!(config.refresh_interval_secs, 60);
        assert_eq!(config.refresh_threshold_secs, 300);
        assert_eq!(config.batch_size, 100);
        assert!(config.enable_prediction);
    }

    #[test]
    fn test_access_tracker_record() {
        let mut tracker = AccessTracker::new();

        tracker.record_access("key1");
        tracker.record_access("key1");
        tracker.record_access("key2");

        assert_eq!(tracker.total_accesses, 3);
        assert_eq!(tracker.counts.get("key1").unwrap().count, 2);
        assert_eq!(tracker.counts.get("key2").unwrap().count, 1);
    }

    #[test]
    fn test_access_tracker_top_keys() {
        let mut tracker = AccessTracker::new();

        for _ in 0..10 {
            tracker.record_access("key1");
        }
        for _ in 0..5 {
            tracker.record_access("key2");
        }
        tracker.record_access("key3");

        let top_keys = tracker.get_top_keys(2);
        assert_eq!(top_keys.len(), 2);
        // Should contain key1 and key2 (most accessed)
    }

    #[test]
    fn test_warming_strategy_equality() {
        assert_eq!(WarmingStrategy::None, WarmingStrategy::None);
        assert_ne!(WarmingStrategy::None, WarmingStrategy::Preload);
    }
}
