//! Prepared statement caching for improved query performance.
//!
//! This module provides:
//! - Statement pool with LRU eviction
//! - Query fingerprinting for cache key generation
//! - Statement reuse metrics

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Configuration for statement cache
#[derive(Debug, Clone)]
pub struct StatementCacheConfig {
    /// Maximum number of cached statements
    pub max_size: usize,
    /// Time-to-live for cached statements (in seconds)
    pub ttl_secs: u64,
    /// Whether to enable query fingerprinting
    pub enable_fingerprinting: bool,
}

impl Default for StatementCacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            ttl_secs: 3600, // 1 hour
            enable_fingerprinting: true,
        }
    }
}

/// Cached statement metadata
#[derive(Debug, Clone)]
struct CachedStatement {
    /// SQL statement text
    sql: String,
    /// Query fingerprint (normalized SQL)
    #[allow(dead_code)]
    fingerprint: String,
    /// Timestamp when statement was cached
    cached_at: Instant,
    /// Number of times this statement was reused
    reuse_count: u64,
    /// Last access time
    last_accessed: Instant,
}

impl CachedStatement {
    fn new(sql: String, fingerprint: String) -> Self {
        let now = Instant::now();
        Self {
            sql,
            fingerprint,
            cached_at: now,
            reuse_count: 0,
            last_accessed: now,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }

    fn touch(&mut self) {
        self.reuse_count += 1;
        self.last_accessed = Instant::now();
    }
}

/// Statement cache with LRU eviction
#[derive(Debug, Clone)]
pub struct StatementCache {
    config: StatementCacheConfig,
    cache: Arc<RwLock<HashMap<String, CachedStatement>>>,
    lru_queue: Arc<RwLock<VecDeque<String>>>,
    stats: Arc<RwLock<CacheStats>>,
}

/// Statement cache performance statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Number of expired entries removed
    pub expirations: u64,
    /// Total statements in cache
    pub cached_statements: usize,
}

impl CacheStats {
    /// Calculate cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.evictions = 0;
        self.expirations = 0;
    }
}

impl Default for StatementCache {
    fn default() -> Self {
        Self::new(StatementCacheConfig::default())
    }
}

impl StatementCache {
    /// Create a new statement cache
    pub fn new(config: StatementCacheConfig) -> Self {
        Self {
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            lru_queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Generate a fingerprint for a SQL query
    pub fn fingerprint(&self, sql: &str) -> String {
        if !self.config.enable_fingerprinting {
            return sql.to_string();
        }

        // Normalize SQL for fingerprinting
        let normalized = normalize_sql(sql);

        // Generate SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        let result = hasher.finalize();

        hex::encode(result.as_slice())
    }

    /// Get a cached statement
    pub fn get(&self, sql: &str) -> Option<String> {
        let fingerprint = self.fingerprint(sql);

        let mut cache = self.cache.write();
        let mut stats = self.stats.write();

        if let Some(stmt) = cache.get_mut(&fingerprint) {
            // Check if expired
            if stmt.is_expired(Duration::from_secs(self.config.ttl_secs)) {
                cache.remove(&fingerprint);
                self.remove_from_lru(&fingerprint);
                stats.expirations += 1;
                stats.misses += 1;
                debug!(fingerprint = %fingerprint, "Statement expired");
                return None;
            }

            stmt.touch();
            stats.hits += 1;

            // Move to front of LRU queue
            self.update_lru(&fingerprint);

            debug!(
                fingerprint = %fingerprint,
                reuse_count = stmt.reuse_count,
                "Statement cache hit"
            );

            Some(stmt.sql.clone())
        } else {
            stats.misses += 1;
            debug!(fingerprint = %fingerprint, "Statement cache miss");
            None
        }
    }

    /// Cache a statement
    pub fn put(&self, sql: String) {
        let fingerprint = self.fingerprint(&sql);

        let mut cache = self.cache.write();
        let mut stats = self.stats.write();

        // Evict if cache is full
        while cache.len() >= self.config.max_size {
            if let Some(oldest) = self.lru_queue.write().pop_back() {
                cache.remove(&oldest);
                stats.evictions += 1;
                debug!(fingerprint = %oldest, "Statement evicted from cache");
            } else {
                break;
            }
        }

        // Add to cache
        let stmt = CachedStatement::new(sql, fingerprint.clone());
        cache.insert(fingerprint.clone(), stmt);

        // Add to LRU queue
        self.lru_queue.write().push_front(fingerprint.clone());

        stats.cached_statements = cache.len();

        debug!(
            fingerprint = %fingerprint,
            cache_size = cache.len(),
            "Statement cached"
        );
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.cache.write().clear();
        self.lru_queue.write().clear();
        self.stats.write().cached_statements = 0;

        info!("Statement cache cleared");
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().clone()
    }

    /// Remove expired entries
    pub fn remove_expired(&self) {
        let ttl = Duration::from_secs(self.config.ttl_secs);
        let mut cache = self.cache.write();
        let mut stats = self.stats.write();

        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, stmt)| stmt.is_expired(ttl))
            .map(|(fp, _)| fp.clone())
            .collect();

        for fp in &expired {
            cache.remove(fp);
            self.remove_from_lru(fp);
            stats.expirations += 1;
        }

        stats.cached_statements = cache.len();

        if !expired.is_empty() {
            info!(count = expired.len(), "Removed expired statements");
        }
    }

    /// Update LRU queue (move to front)
    fn update_lru(&self, fingerprint: &str) {
        let mut queue = self.lru_queue.write();

        // Remove from current position
        if let Some(pos) = queue.iter().position(|fp| fp == fingerprint) {
            queue.remove(pos);
        }

        // Add to front
        queue.push_front(fingerprint.to_string());
    }

    /// Remove from LRU queue
    fn remove_from_lru(&self, fingerprint: &str) {
        let mut queue = self.lru_queue.write();
        if let Some(pos) = queue.iter().position(|fp| fp == fingerprint) {
            queue.remove(pos);
        }
    }
}

/// Normalize SQL for fingerprinting
fn normalize_sql(sql: &str) -> String {
    // Remove extra whitespace and normalize to lowercase
    let normalized = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    // Replace literal values with placeholders
    // This is a simple approach; a more sophisticated version would use a SQL parser

    replace_literals(&normalized)
}

static RE_LITERAL_STRING: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"'[^']*'")
        .expect("static regex pattern for SQL string literals is always valid")
});

static RE_LITERAL_NUMBER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\b\d+\b")
        .expect("static regex pattern for SQL numeric literals is always valid")
});

/// Replace literal values with placeholders
fn replace_literals(sql: &str) -> String {
    // Replace string literals
    let sql = RE_LITERAL_STRING.replace_all(sql, "?");

    // Replace numeric literals
    let sql = RE_LITERAL_NUMBER.replace_all(&sql, "?");

    sql.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statement_cache_config_default() {
        let config = StatementCacheConfig::default();
        assert_eq!(config.max_size, 1000);
        assert_eq!(config.ttl_secs, 3600);
        assert!(config.enable_fingerprinting);
    }

    #[test]
    fn test_fingerprint_generation() {
        let cache = StatementCache::default();

        let sql1 = "SELECT * FROM users WHERE id = 1";
        let sql2 = "SELECT * FROM users WHERE id = 2";

        let fp1 = cache.fingerprint(sql1);
        let fp2 = cache.fingerprint(sql2);

        // Should generate the same fingerprint (normalized)
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_cache_put_and_get() {
        let cache = StatementCache::default();
        let sql = "SELECT * FROM users WHERE id = 1".to_string();

        cache.put(sql.clone());

        let result = cache.get(&sql);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), sql);
    }

    #[test]
    fn test_cache_miss() {
        let cache = StatementCache::default();
        let sql = "SELECT * FROM users WHERE id = 1";

        let result = cache.get(sql);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = StatementCache::default();
        let sql = "SELECT * FROM users WHERE id = 1".to_string();

        // Miss
        cache.get(&sql);

        // Put
        cache.put(sql.clone());

        // Hit
        cache.get(&sql);
        cache.get(&sql);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.cached_statements, 1);
    }

    #[test]
    fn test_cache_hit_rate() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            evictions: 0,
            expirations: 0,
            cached_statements: 100,
        };

        assert!((stats.hit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_cache_eviction() {
        let config = StatementCacheConfig {
            max_size: 3,
            enable_fingerprinting: false, // Disable fingerprinting for this test
            ..Default::default()
        };

        let cache = StatementCache::new(config);

        cache.put("SELECT 1".to_string());
        cache.put("SELECT 2".to_string());
        cache.put("SELECT 3".to_string());
        cache.put("SELECT 4".to_string()); // Should evict oldest

        let stats = cache.stats();
        assert_eq!(stats.cached_statements, 3);
        assert_eq!(stats.evictions, 1);
    }

    #[test]
    fn test_cache_clear() {
        let cache = StatementCache::default();

        cache.put("SELECT 1".to_string());
        cache.put("SELECT 2".to_string());

        cache.clear();

        let stats = cache.stats();
        assert_eq!(stats.cached_statements, 0);
    }

    #[test]
    fn test_normalize_sql() {
        let sql = "SELECT  *  FROM  users  WHERE  id  =  1";
        let normalized = normalize_sql(sql);

        assert_eq!(normalized, "select * from users where id = ?");
    }

    #[test]
    fn test_replace_literals() {
        let sql = "SELECT * FROM users WHERE id = 123 AND name = 'john'";
        let replaced = replace_literals(sql);

        assert!(replaced.contains("?"));
        assert!(!replaced.contains("123"));
        assert!(!replaced.contains("john"));
    }

    #[test]
    fn test_statement_reuse_count() {
        let cache = StatementCache::default();
        let sql = "SELECT * FROM users WHERE id = 1".to_string();

        cache.put(sql.clone());

        // Access multiple times
        for _ in 0..5 {
            cache.get(&sql);
        }

        let fingerprint = cache.fingerprint(&sql);
        let cached_cache = cache.cache.read();
        let stmt = cached_cache.get(&fingerprint).unwrap();

        assert_eq!(stmt.reuse_count, 5);
    }

    #[test]
    fn test_stats_reset() {
        let mut stats = CacheStats {
            hits: 100,
            misses: 50,
            evictions: 10,
            expirations: 5,
            cached_statements: 200,
        };

        stats.reset();

        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.expirations, 0);
    }
}
