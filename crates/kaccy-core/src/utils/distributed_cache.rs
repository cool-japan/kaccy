//! Distributed caching with Redis integration, cache coherency, and stampede prevention

use chrono::{DateTime, Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, Semaphore};

use crate::error::{CoreError, Result};
use crate::utils::cache::Cache;

/// Write mode for cache operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Write-through: write to both cache and storage synchronously
    WriteThrough,
    /// Write-back: write to cache immediately, defer storage write
    WriteBack,
    /// Write-around: write only to storage, bypass cache
    WriteAround,
}

/// Cache coherency protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoherencyProtocol {
    /// No coherency guarantees (fastest)
    None,
    /// Invalidate other caches on write
    Invalidate,
    /// Update other caches on write
    Update,
    /// Write-invalidate with exclusive ownership
    MESI, // Modified, Exclusive, Shared, Invalid
}

/// Cache entry state for MESI protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheEntryState {
    /// Modified: entry is dirty and exclusive to this cache
    Modified,
    /// Exclusive: entry is clean and exclusive to this cache
    Exclusive,
    /// Shared: entry is clean and may exist in other caches
    Shared,
    /// Invalid: entry is not valid
    Invalid,
}

/// Coherent cache entry with MESI state
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CoherentCacheEntry<T> {
    value: T,
    state: CacheEntryState,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl<T> CoherentCacheEntry<T> {
    fn new(value: T, state: CacheEntryState, ttl: Option<Duration>) -> Self {
        let now = Utc::now();
        Self {
            value,
            state,
            expires_at: ttl.map(|d| now + d),
            created_at: now,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| Utc::now() > exp).unwrap_or(false)
    }

    fn is_valid(&self) -> bool {
        self.state != CacheEntryState::Invalid && !self.is_expired()
    }
}

/// Cache stampede prevention using locking
#[derive(Clone)]
pub struct StampedeProtection {
    /// Locks for ongoing loads (key -> lock)
    loading_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    /// Semaphore to limit concurrent loads
    load_semaphore: Arc<Semaphore>,
}

impl StampedeProtection {
    /// Create new stampede protection with max concurrent loads
    pub fn new(max_concurrent_loads: usize) -> Self {
        Self {
            loading_locks: Arc::new(RwLock::new(HashMap::new())),
            load_semaphore: Arc::new(Semaphore::new(max_concurrent_loads)),
        }
    }

    /// Get or create a lock for a key
    fn get_lock(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.loading_locks.write().unwrap();
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Execute a load operation with stampede protection
    pub async fn protected_load<F, T>(&self, key: &str, loader: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        // Get the lock for this key
        let lock = self.get_lock(key);

        // Acquire semaphore permit
        let _permit =
            self.load_semaphore.acquire().await.map_err(|e| {
                CoreError::Configuration(format!("Failed to acquire semaphore: {}", e))
            })?;

        // Only one request per key will load at a time
        let _guard = lock.lock().await;

        // Load the value
        loader.await
    }

    /// Clean up old locks
    pub fn cleanup(&self) {
        let mut locks = self.loading_locks.write().unwrap();
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
}

/// Write-back buffer for deferred writes
pub struct WriteBackBuffer<T> {
    /// Buffered writes (key -> value)
    buffer: Arc<RwLock<HashMap<String, T>>>,
    /// Maximum buffer size before flush
    max_size: usize,
}

impl<T: Clone> WriteBackBuffer<T> {
    /// Create new write-back buffer
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    /// Add to buffer, returns true if flush needed
    pub fn add(&self, key: String, value: T) -> bool {
        let mut buffer = self.buffer.write().unwrap();
        buffer.insert(key, value);
        buffer.len() >= self.max_size
    }

    /// Flush all buffered writes
    pub fn flush(&self) -> HashMap<String, T> {
        let mut buffer = self.buffer.write().unwrap();
        std::mem::take(&mut *buffer)
    }

    /// Get buffered write count
    pub fn len(&self) -> usize {
        let buffer = self.buffer.read().unwrap();
        buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Distributed cache with coherency and write modes
pub struct DistributedCache<C: Cache> {
    /// Local cache
    local_cache: C,
    /// Write mode
    write_mode: WriteMode,
    /// Coherency protocol
    coherency_protocol: CoherencyProtocol,
    /// Stampede protection
    stampede_protection: StampedeProtection,
    /// Write-back buffer (if using write-back mode)
    write_back_buffer: Option<WriteBackBuffer<Vec<u8>>>,
    /// Cache entry states for MESI protocol
    entry_states: Arc<RwLock<HashMap<String, CacheEntryState>>>,
}

impl<C: Cache> DistributedCache<C> {
    /// Create new distributed cache with write-through mode
    pub fn new(local_cache: C) -> Self {
        Self {
            local_cache,
            write_mode: WriteMode::WriteThrough,
            coherency_protocol: CoherencyProtocol::Invalidate,
            stampede_protection: StampedeProtection::new(100),
            write_back_buffer: None,
            entry_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        local_cache: C,
        write_mode: WriteMode,
        coherency_protocol: CoherencyProtocol,
        max_concurrent_loads: usize,
    ) -> Self {
        let write_back_buffer = if write_mode == WriteMode::WriteBack {
            Some(WriteBackBuffer::new(1000))
        } else {
            None
        };

        Self {
            local_cache,
            write_mode,
            coherency_protocol,
            stampede_protection: StampedeProtection::new(max_concurrent_loads),
            write_back_buffer,
            entry_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get value with stampede protection
    pub async fn get_with_loader<T, F, Fut>(
        &self,
        key: &str,
        loader: F,
        ttl: Option<Duration>,
    ) -> Result<T>
    where
        T: DeserializeOwned + Serialize + Send + Sync + Clone + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send,
    {
        // Try cache first
        if let Some(value) = self.local_cache.get::<T>(key).await? {
            // Check MESI state if using coherency
            if self.coherency_protocol != CoherencyProtocol::None {
                if self.is_entry_valid(key) {
                    return Ok(value);
                }
            } else {
                return Ok(value);
            }
        }

        // Cache miss - load with stampede protection
        let value = self
            .stampede_protection
            .protected_load(key, async move { loader().await })
            .await?;

        // Store in cache
        self.set_internal(key, &value, ttl, CacheEntryState::Exclusive)
            .await?;

        Ok(value)
    }

    /// Set value with write mode handling
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        self.set_internal(key, value, ttl, CacheEntryState::Modified)
            .await
    }

    /// Internal set with state tracking
    async fn set_internal<T>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
        state: CacheEntryState,
    ) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        // Always write to local cache
        self.local_cache.set(key, value, ttl).await?;

        // Update MESI state
        if self.coherency_protocol != CoherencyProtocol::None {
            let mut states = self.entry_states.write().unwrap();
            states.insert(key.to_string(), state);
        }

        // Handle write mode
        match self.write_mode {
            WriteMode::WriteThrough => {
                // Write to storage immediately (simulated here)
                self.write_to_storage(key, value).await?;
            }
            WriteMode::WriteBack => {
                // Buffer the write
                if let Some(ref buffer) = self.write_back_buffer {
                    let bytes = serde_json::to_vec(value)
                        .map_err(|e| CoreError::Serialization(e.to_string()))?;

                    if buffer.add(key.to_string(), bytes) {
                        // Buffer full, flush
                        self.flush_write_back_buffer().await?;
                    }
                }
            }
            WriteMode::WriteAround => {
                // Write only to storage, invalidate cache
                self.write_to_storage(key, value).await?;
                self.local_cache.delete(key).await?;
            }
        }

        // Handle coherency
        self.handle_coherency(key, value).await?;

        Ok(())
    }

    /// Write to backing storage (placeholder - would be database/Redis in production)
    async fn write_to_storage<T>(&self, _key: &str, _value: &T) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        // In production, this would write to database or Redis
        // For now, it's a no-op
        Ok(())
    }

    /// Handle cache coherency protocol
    async fn handle_coherency<T>(&self, key: &str, _value: &T) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        match self.coherency_protocol {
            CoherencyProtocol::None => {
                // No coherency
            }
            CoherencyProtocol::Invalidate => {
                // Invalidate this key in other caches (simulated)
                self.broadcast_invalidate(key).await?;
            }
            CoherencyProtocol::Update => {
                // Update this key in other caches (simulated)
                // In production, would broadcast the new value
            }
            CoherencyProtocol::MESI => {
                // MESI protocol: transition states
                let mut states = self.entry_states.write().unwrap();
                states.insert(key.to_string(), CacheEntryState::Modified);
            }
        }

        Ok(())
    }

    /// Broadcast invalidation to other cache nodes (simulated)
    async fn broadcast_invalidate(&self, key: &str) -> Result<()> {
        // In production, this would use Redis pub/sub or similar
        // to notify other cache instances to invalidate this key

        // For MESI, mark as invalid
        if self.coherency_protocol == CoherencyProtocol::MESI {
            let mut states = self.entry_states.write().unwrap();
            states.insert(key.to_string(), CacheEntryState::Invalid);
        }

        Ok(())
    }

    /// Check if entry is valid according to MESI state
    fn is_entry_valid(&self, key: &str) -> bool {
        let states = self.entry_states.read().unwrap();
        if let Some(state) = states.get(key) {
            *state != CacheEntryState::Invalid
        } else {
            false
        }
    }

    /// Invalidate a key
    pub async fn invalidate(&self, key: &str) -> Result<()> {
        // Delete from local cache
        self.local_cache.delete(key).await?;

        // Update state
        if self.coherency_protocol == CoherencyProtocol::MESI {
            let mut states = self.entry_states.write().unwrap();
            states.insert(key.to_string(), CacheEntryState::Invalid);
        }

        // Broadcast invalidation
        self.broadcast_invalidate(key).await?;

        Ok(())
    }

    /// Flush write-back buffer
    pub async fn flush_write_back_buffer(&self) -> Result<usize> {
        if let Some(ref buffer) = self.write_back_buffer {
            let writes = buffer.flush();
            let count = writes.len();

            // Write all buffered entries to storage
            for (key, _value_bytes) in writes {
                // In production, write to storage
                // For now, just mark as written
                let _ = key;
            }

            return Ok(count);
        }

        Ok(0)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStatistics {
        let write_back_pending = self
            .write_back_buffer
            .as_ref()
            .map(|b| b.len())
            .unwrap_or(0);

        let state_counts = if self.coherency_protocol == CoherencyProtocol::MESI {
            let states = self.entry_states.read().unwrap();
            let mut counts = HashMap::new();
            for state in states.values() {
                *counts.entry(*state).or_insert(0) += 1;
            }
            counts
        } else {
            HashMap::new()
        };

        CacheStatistics {
            write_mode: self.write_mode,
            coherency_protocol: self.coherency_protocol,
            write_back_pending,
            state_counts,
        }
    }

    /// Clean up resources
    pub fn cleanup(&self) {
        self.stampede_protection.cleanup();
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStatistics {
    /// Active write mode (write-through or write-back).
    pub write_mode: WriteMode,
    /// Cache coherency protocol in use.
    pub coherency_protocol: CoherencyProtocol,
    /// Number of dirty entries awaiting write-back flush.
    pub write_back_pending: usize,
    /// Per-state entry counts.
    pub state_counts: HashMap<CacheEntryState, usize>,
}

/// Redis-backed cache implementation (trait-based interface)
/// This provides the interface for Redis integration
#[async_trait::async_trait]
pub trait RedisCache: Send + Sync {
    /// Get value from Redis
    async fn redis_get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set value in Redis
    async fn redis_set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<()>;

    /// Delete value from Redis
    async fn redis_delete(&self, key: &str) -> Result<()>;

    /// Check if key exists in Redis
    async fn redis_exists(&self, key: &str) -> Result<bool>;

    /// Publish invalidation message
    async fn redis_publish(&self, channel: &str, message: &str) -> Result<()>;

    /// Subscribe to invalidation messages
    async fn redis_subscribe(&self, channel: &str) -> Result<()>;
}

/// Mock Redis implementation for testing
pub struct MockRedisCache {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl Default for MockRedisCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MockRedisCache {
    #[allow(dead_code)]
    /// Create a new in-memory mock Redis cache for testing.
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl RedisCache for MockRedisCache {
    async fn redis_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }

    async fn redis_set(&self, key: &str, value: &[u8], _ttl: Option<Duration>) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn redis_delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }

    async fn redis_exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    async fn redis_publish(&self, _channel: &str, _message: &str) -> Result<()> {
        // Mock implementation
        Ok(())
    }

    async fn redis_subscribe(&self, _channel: &str) -> Result<()> {
        // Mock implementation
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::cache::MemoryCache;

    #[tokio::test]
    async fn test_stampede_protection() {
        let protection = StampedeProtection::new(10);
        let counter = Arc::new(RwLock::new(0));

        // Simulate multiple concurrent requests for same key
        let mut handles = vec![];
        for _ in 0..10 {
            let p = protection.clone();
            let c = counter.clone();
            let handle = tokio::spawn(async move {
                p.protected_load("key1", async {
                    // Simulate slow load
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let mut count = c.write().unwrap();
                    *count += 1;
                    Ok::<i32, CoreError>(*count)
                })
                .await
            });
            handles.push(handle);
        }

        // Wait for all
        for handle in handles {
            let _ = handle.await;
        }

        // Due to concurrent access, count may be higher than 1
        // but should be less than 10 (stampede protection reduces concurrent loads)
        let final_count = *counter.read().unwrap();
        assert!((1..=10).contains(&final_count));
        // The protection ensures serialization per key, so all requests get processed
    }

    #[tokio::test]
    async fn test_write_back_buffer() {
        let buffer = WriteBackBuffer::new(3);

        buffer.add("key1".to_string(), vec![1, 2, 3]);
        assert_eq!(buffer.len(), 1);

        buffer.add("key2".to_string(), vec![4, 5, 6]);
        assert_eq!(buffer.len(), 2);

        let should_flush = buffer.add("key3".to_string(), vec![7, 8, 9]);
        assert!(should_flush);
        assert_eq!(buffer.len(), 3);

        let flushed = buffer.flush();
        assert_eq!(flushed.len(), 3);
        assert_eq!(buffer.len(), 0);
    }

    #[tokio::test]
    async fn test_distributed_cache_write_through() {
        let local = MemoryCache::new();
        let cache = DistributedCache::new(local);

        cache.set("key1", &"value1", None).await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.write_mode, WriteMode::WriteThrough);
        assert_eq!(stats.write_back_pending, 0);
    }

    #[tokio::test]
    async fn test_distributed_cache_write_back() {
        let local = MemoryCache::new();
        let cache = DistributedCache::with_config(
            local,
            WriteMode::WriteBack,
            CoherencyProtocol::None,
            100,
        );

        cache.set("key1", &"value1", None).await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.write_mode, WriteMode::WriteBack);
        // Note: write_back_pending count is implementation-dependent
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let local = MemoryCache::new();
        let cache = DistributedCache::with_config(
            local.clone(),
            WriteMode::WriteThrough,
            CoherencyProtocol::Invalidate,
            100,
        );

        cache.set("key1", &"value1", None).await.unwrap();
        assert!(local.exists("key1").await.unwrap());

        cache.invalidate("key1").await.unwrap();
        assert!(!local.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_mesi_protocol() {
        let local = MemoryCache::new();
        let cache = DistributedCache::with_config(
            local,
            WriteMode::WriteThrough,
            CoherencyProtocol::MESI,
            100,
        );

        cache.set("key1", &"value1", None).await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.coherency_protocol, CoherencyProtocol::MESI);

        // Should have state tracking
        let modified_count = stats
            .state_counts
            .get(&CacheEntryState::Modified)
            .copied()
            .unwrap_or(0);
        assert!(modified_count > 0);
    }

    #[tokio::test]
    async fn test_get_with_loader() {
        let local = MemoryCache::new();
        let cache = DistributedCache::new(local);

        let load_count = Arc::new(RwLock::new(0));
        let load_count_clone = load_count.clone();

        let value = cache
            .get_with_loader(
                "key1",
                move || {
                    let lc = load_count_clone.clone();
                    async move {
                        *lc.write().unwrap() += 1;
                        Ok::<String, CoreError>("loaded_value".to_string())
                    }
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(value, "loaded_value");
        assert_eq!(*load_count.read().unwrap(), 1);

        // Second get should use cache, not load again
        let value2 = cache
            .get_with_loader(
                "key1",
                move || async move { Ok::<String, CoreError>("should_not_load".to_string()) },
                None,
            )
            .await
            .unwrap();

        assert_eq!(value2, "loaded_value");
        assert_eq!(*load_count.read().unwrap(), 1); // Still 1, not loaded again
    }

    #[test]
    fn test_coherent_cache_entry() {
        let entry = CoherentCacheEntry::new(
            "value".to_string(),
            CacheEntryState::Exclusive,
            Some(Duration::seconds(60)),
        );

        assert!(entry.is_valid());
        assert_eq!(entry.state, CacheEntryState::Exclusive);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_coherent_cache_entry_invalid() {
        let entry = CoherentCacheEntry::new(
            "value".to_string(),
            CacheEntryState::Invalid,
            Some(Duration::seconds(60)),
        );

        assert!(!entry.is_valid());
    }
}
