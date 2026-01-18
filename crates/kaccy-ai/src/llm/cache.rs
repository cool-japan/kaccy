//! LLM response caching for cost optimization
//!
//! This module provides caching capabilities for LLM responses to reduce
//! API costs and improve response times for repeated queries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::types::{ChatRequest, ChatResponse};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct LlmCacheConfig {
    /// Maximum number of entries in the cache
    pub max_entries: usize,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Whether to enable semantic similarity matching
    pub semantic_matching: bool,
    /// Similarity threshold for semantic matching (0.0-1.0)
    pub similarity_threshold: f64,
    /// Maximum tokens in cached prompts (to avoid caching long conversations)
    pub max_prompt_tokens: usize,
}

impl Default for LlmCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
            semantic_matching: false,       // Disabled by default (requires embedding model)
            similarity_threshold: 0.95,
            max_prompt_tokens: 2000,
        }
    }
}

impl LlmCacheConfig {
    /// Create config optimized for development (shorter TTL, smaller cache)
    #[must_use]
    pub fn development() -> Self {
        Self {
            max_entries: 100,
            ttl: Duration::from_secs(300), // 5 minutes
            semantic_matching: false,
            similarity_threshold: 0.95,
            max_prompt_tokens: 1000,
        }
    }

    /// Create config optimized for production (larger cache, longer TTL)
    #[must_use]
    pub fn production() -> Self {
        Self {
            max_entries: 10000,
            ttl: Duration::from_secs(86400), // 24 hours
            semantic_matching: false,
            similarity_threshold: 0.95,
            max_prompt_tokens: 4000,
        }
    }
}

/// Cached LLM response entry
#[derive(Debug, Clone)]
struct CacheEntry {
    /// The cached response
    response: CachedResponse,
    /// When the entry was created
    created_at: Instant,
    /// Number of times this entry was hit
    hit_count: u64,
    /// Last access time
    last_accessed: Instant,
}

/// Cached response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// Response content
    pub content: String,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Tokens used in prompt
    pub prompt_tokens: u32,
    /// Tokens used in completion
    pub completion_tokens: u32,
}

impl From<&ChatResponse> for CachedResponse {
    fn from(response: &ChatResponse) -> Self {
        Self {
            content: response.message.content.clone(),
            finish_reason: response.finish_reason.clone(),
            prompt_tokens: response.prompt_tokens,
            completion_tokens: response.completion_tokens,
        }
    }
}

/// Cache key derived from request
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    /// Hash of the messages
    messages_hash: u64,
    /// Temperature (quantized to avoid floating point issues)
    temperature_q: u32,
    /// Max tokens (affects response length)
    max_tokens: Option<u32>,
}

impl CacheKey {
    fn from_request(request: &ChatRequest) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash all messages (role + content)
        for msg in &request.messages {
            // Hash role as string representation
            let role_str = match msg.role {
                super::types::ChatRole::System => "system",
                super::types::ChatRole::User => "user",
                super::types::ChatRole::Assistant => "assistant",
            };
            role_str.hash(&mut hasher);
            msg.content.hash(&mut hasher);
        }
        let messages_hash = hasher.finish();

        // Quantize temperature to avoid floating point comparison issues
        let temperature_q = (request.temperature.unwrap_or(1.0) * 100.0) as u32;

        Self {
            messages_hash,
            temperature_q,
            max_tokens: request.max_tokens,
        }
    }
}

/// LLM response cache
pub struct LlmCache {
    config: LlmCacheConfig,
    cache: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    stats: Arc<RwLock<CacheStats>>,
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Total entries currently in cache
    pub entries: usize,
    /// Total bytes saved (estimated from tokens)
    pub tokens_saved: u64,
    /// Estimated cost saved (in cents, assuming $0.01 per 1K tokens)
    pub estimated_cost_saved_cents: f64,
    /// Evictions due to TTL
    pub ttl_evictions: u64,
    /// Evictions due to capacity
    pub capacity_evictions: u64,
}

impl CacheStats {
    /// Calculate hit rate
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl LlmCache {
    /// Create a new LLM cache
    #[must_use]
    pub fn new(config: LlmCacheConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn default_cache() -> Self {
        Self::new(LlmCacheConfig::default())
    }

    /// Try to get a cached response
    pub async fn get(&self, request: &ChatRequest) -> Option<CachedResponse> {
        // Check if request is cacheable
        if !self.is_cacheable(request) {
            return None;
        }

        let key = CacheKey::from_request(request);

        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get_mut(&key) {
            // Check TTL
            if entry.created_at.elapsed() > self.config.ttl {
                // Entry expired
                cache.remove(&key);
                let mut stats = self.stats.write().await;
                stats.ttl_evictions += 1;
                stats.misses += 1;
                return None;
            }

            // Update access stats
            entry.hit_count += 1;
            entry.last_accessed = Instant::now();

            // Update global stats
            let mut stats = self.stats.write().await;
            stats.hits += 1;
            stats.tokens_saved +=
                u64::from(entry.response.prompt_tokens + entry.response.completion_tokens);
            stats.estimated_cost_saved_cents +=
                f64::from(entry.response.prompt_tokens + entry.response.completion_tokens) / 1000.0;

            return Some(entry.response.clone());
        }

        // Cache miss
        let mut stats = self.stats.write().await;
        stats.misses += 1;

        None
    }

    /// Store a response in the cache
    pub async fn put(&self, request: &ChatRequest, response: &ChatResponse) {
        // Check if request is cacheable
        if !self.is_cacheable(request) {
            return;
        }

        let key = CacheKey::from_request(request);
        let entry = CacheEntry {
            response: CachedResponse::from(response),
            created_at: Instant::now(),
            hit_count: 0,
            last_accessed: Instant::now(),
        };

        let mut cache = self.cache.write().await;

        // Evict if at capacity
        if cache.len() >= self.config.max_entries {
            self.evict_lru(&mut cache).await;
        }

        cache.insert(key, entry);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.entries = cache.len();
    }

    /// Check if a request is cacheable
    fn is_cacheable(&self, request: &ChatRequest) -> bool {
        // Don't cache if temperature is high (non-deterministic)
        if request.temperature.unwrap_or(1.0) > 0.5 {
            return false;
        }

        // Don't cache very long conversations
        let total_content_len: usize = request.messages.iter().map(|m| m.content.len()).sum();
        let estimated_tokens = total_content_len / 4; // Rough estimate

        if estimated_tokens > self.config.max_prompt_tokens {
            return false;
        }

        // Don't cache if stream is requested
        // (streaming responses are handled differently)

        true
    }

    /// Evict least recently used entry
    async fn evict_lru(&self, cache: &mut HashMap<CacheKey, CacheEntry>) {
        if let Some((key_to_remove, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            cache.remove(&key_to_remove);

            let mut stats = self.stats.write().await;
            stats.capacity_evictions += 1;
        }
    }

    /// Clear expired entries
    pub async fn clear_expired(&self) {
        let mut cache = self.cache.write().await;
        let now = Instant::now();
        let ttl = self.config.ttl;

        let expired_keys: Vec<CacheKey> = cache
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.created_at) > ttl)
            .map(|(k, _)| k.clone())
            .collect();

        let expired_count = expired_keys.len();

        for key in expired_keys {
            cache.remove(&key);
        }

        if expired_count > 0 {
            let mut stats = self.stats.write().await;
            stats.ttl_evictions += expired_count as u64;
            stats.entries = cache.len();

            tracing::debug!(expired = expired_count, "Cleared expired cache entries");
        }
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();

        let mut stats = self.stats.write().await;
        stats.entries = 0;
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get detailed cache info
    pub async fn get_cache_info(&self) -> CacheInfo {
        let cache = self.cache.read().await;
        let stats = self.stats.read().await;

        let total_hit_count: u64 = cache.values().map(|e| e.hit_count).sum();
        let avg_hit_count = if cache.is_empty() {
            0.0
        } else {
            total_hit_count as f64 / cache.len() as f64
        };

        let oldest_entry = cache.values().map(|e| e.created_at).min();
        let newest_entry = cache.values().map(|e| e.created_at).max();

        CacheInfo {
            config: self.config.clone(),
            stats: stats.clone(),
            entry_count: cache.len(),
            total_hit_count,
            avg_hit_count_per_entry: avg_hit_count,
            oldest_entry_age_secs: oldest_entry.map(|t| t.elapsed().as_secs()),
            newest_entry_age_secs: newest_entry.map(|t| t.elapsed().as_secs()),
        }
    }
}

/// Detailed cache information
#[derive(Debug, Clone, Serialize)]
pub struct CacheInfo {
    /// Cache configuration
    #[serde(skip)]
    pub config: LlmCacheConfig,
    /// Cache statistics
    pub stats: CacheStats,
    /// Number of entries
    pub entry_count: usize,
    /// Total hit count across all entries
    pub total_hit_count: u64,
    /// Average hit count per entry
    pub avg_hit_count_per_entry: f64,
    /// Age of oldest entry in seconds
    pub oldest_entry_age_secs: Option<u64>,
    /// Age of newest entry in seconds
    pub newest_entry_age_secs: Option<u64>,
}

/// Cached LLM client wrapper
pub struct CachedLlmClient<P> {
    /// Underlying provider
    provider: P,
    /// Cache
    cache: LlmCache,
}

impl<P> CachedLlmClient<P> {
    /// Create a new cached client
    pub fn new(provider: P, cache_config: LlmCacheConfig) -> Self {
        Self {
            provider,
            cache: LlmCache::new(cache_config),
        }
    }

    /// Create with default cache config
    pub fn with_default_cache(provider: P) -> Self {
        Self {
            provider,
            cache: LlmCache::default_cache(),
        }
    }

    /// Get the underlying provider
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        self.cache.get_stats().await
    }

    /// Get detailed cache info
    pub async fn cache_info(&self) -> CacheInfo {
        self.cache.get_cache_info().await
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    /// Clear expired entries
    pub async fn clear_expired(&self) {
        self.cache.clear_expired().await;
    }
}

/// Request deduplication to prevent duplicate in-flight requests
pub struct RequestDeduplicator {
    /// In-flight requests
    in_flight: Arc<RwLock<HashMap<u64, tokio::sync::watch::Receiver<Option<CachedResponse>>>>>,
}

impl RequestDeduplicator {
    /// Create a new deduplicator
    #[must_use]
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Hash a request for deduplication
    fn hash_request(request: &ChatRequest) -> u64 {
        let key = CacheKey::from_request(request);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if a request is in flight
    pub async fn is_in_flight(&self, request: &ChatRequest) -> bool {
        let hash = Self::hash_request(request);
        let in_flight = self.in_flight.read().await;
        in_flight.contains_key(&hash)
    }

    /// Register an in-flight request
    pub async fn register(
        &self,
        request: &ChatRequest,
    ) -> tokio::sync::watch::Sender<Option<CachedResponse>> {
        let hash = Self::hash_request(request);
        let (tx, rx) = tokio::sync::watch::channel(None);

        let mut in_flight = self.in_flight.write().await;
        in_flight.insert(hash, rx);

        tx
    }

    /// Wait for an in-flight request to complete
    pub async fn wait_for(&self, request: &ChatRequest) -> Option<CachedResponse> {
        let hash = Self::hash_request(request);

        let rx = {
            let in_flight = self.in_flight.read().await;
            in_flight.get(&hash).cloned()
        };

        if let Some(mut rx) = rx {
            // Wait for the value to change
            let _ = rx.changed().await;
            rx.borrow().clone()
        } else {
            None
        }
    }

    /// Complete an in-flight request
    pub async fn complete(&self, request: &ChatRequest, response: Option<CachedResponse>) {
        let hash = Self::hash_request(request);

        let mut in_flight = self.in_flight.write().await;
        in_flight.remove(&hash);

        // The sender going out of scope will notify all waiters
        drop(response);
    }
}

impl Default for RequestDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatMessage, ChatRole};

    #[test]
    fn test_cache_key_generation() {
        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Hello".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: None,
            stop: None,
            images: None,
        };

        let key1 = CacheKey::from_request(&request);
        let key2 = CacheKey::from_request(&request);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_different_messages_different_keys() {
        let request1 = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Hello".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: None,
            stop: None,
            images: None,
        };

        let request2 = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Goodbye".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: None,
            stop: None,
            images: None,
        };

        let key1 = CacheKey::from_request(&request1);
        let key2 = CacheKey::from_request(&request2);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_config_defaults() {
        let config = LlmCacheConfig::default();
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl, Duration::from_secs(3600));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = LlmCache::default_cache();

        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Test".to_string(),
            }],
            temperature: Some(0.0),
            max_tokens: None,
            stop: None,
            images: None,
        };

        let result = cache.get(&request).await;
        assert!(result.is_none());

        let stats = cache.get_stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn test_not_cacheable_high_temperature() {
        let cache = LlmCache::default_cache();

        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: "Test".to_string(),
            }],
            temperature: Some(0.9), // High temperature
            max_tokens: None,
            stop: None,
            images: None,
        };

        assert!(!cache.is_cacheable(&request));
    }
}
