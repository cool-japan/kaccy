//! Caching layer for Bitcoin data to reduce RPC calls

use bitcoin::{BlockHash, Txid};
use bitcoincore_rpc::json::GetRawTransactionResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace};

use crate::utxo::Utxo;

/// Configuration for the cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Time-to-live for transaction cache entries
    pub transaction_ttl: Duration,
    /// Time-to-live for UTXO cache entries
    pub utxo_ttl: Duration,
    /// Time-to-live for block header cache entries
    pub block_header_ttl: Duration,
    /// Maximum number of transactions to cache
    pub max_transactions: usize,
    /// Maximum number of UTXOs to cache
    pub max_utxos: usize,
    /// Maximum number of block headers to cache
    pub max_block_headers: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            transaction_ttl: Duration::from_secs(300),  // 5 minutes
            utxo_ttl: Duration::from_secs(60),          // 1 minute
            block_header_ttl: Duration::from_secs(600), // 10 minutes
            max_transactions: 1000,
            max_utxos: 5000,
            max_block_headers: 500,
        }
    }
}

/// A cached entry with expiration time
#[derive(Debug, Clone)]
struct CachedEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CachedEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// Transaction cache
pub struct TransactionCache {
    cache: Arc<RwLock<HashMap<Txid, CachedEntry<GetRawTransactionResult>>>>,
    config: CacheConfig,
}

impl TransactionCache {
    /// Create a new transaction cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get a transaction from cache
    pub async fn get(&self, txid: &Txid) -> Option<GetRawTransactionResult> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(txid) {
            if !entry.is_expired() {
                trace!(txid = %txid, "Transaction cache hit");
                return Some(entry.value.clone());
            }
        }
        trace!(txid = %txid, "Transaction cache miss");
        None
    }

    /// Insert a transaction into cache
    pub async fn insert(&self, txid: Txid, tx: GetRawTransactionResult) {
        let mut cache = self.cache.write().await;

        // Evict old entries if we're at capacity
        if cache.len() >= self.config.max_transactions {
            self.evict_expired(&mut cache);

            // If still at capacity, remove oldest entry
            if cache.len() >= self.config.max_transactions {
                if let Some(oldest_key) = cache.keys().next().copied() {
                    cache.remove(&oldest_key);
                }
            }
        }

        cache.insert(txid, CachedEntry::new(tx, self.config.transaction_ttl));
        trace!(txid = %txid, "Transaction cached");
    }

    /// Invalidate a specific transaction
    pub async fn invalidate(&self, txid: &Txid) {
        let mut cache = self.cache.write().await;
        cache.remove(txid);
        debug!(txid = %txid, "Transaction cache invalidated");
    }

    /// Clear all cached transactions
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Transaction cache cleared");
    }

    /// Evict expired entries
    fn evict_expired(&self, cache: &mut HashMap<Txid, CachedEntry<GetRawTransactionResult>>) {
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let expired_count = cache.values().filter(|e| e.is_expired()).count();

        CacheStats {
            total_entries: cache.len(),
            expired_entries: expired_count,
            active_entries: cache.len() - expired_count,
            max_entries: self.config.max_transactions,
        }
    }
}

/// UTXO cache
pub struct UtxoCache {
    cache: Arc<RwLock<HashMap<String, CachedEntry<Vec<Utxo>>>>>,
    config: CacheConfig,
}

impl UtxoCache {
    /// Create a new UTXO cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get UTXOs for an address from cache
    pub async fn get(&self, address: &str) -> Option<Vec<Utxo>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(address) {
            if !entry.is_expired() {
                trace!(address = address, "UTXO cache hit");
                return Some(entry.value.clone());
            }
        }
        trace!(address = address, "UTXO cache miss");
        None
    }

    /// Insert UTXOs for an address into cache
    pub async fn insert(&self, address: String, utxos: Vec<Utxo>) {
        let mut cache = self.cache.write().await;

        // Evict old entries if we're at capacity
        if cache.len() >= self.config.max_utxos {
            self.evict_expired(&mut cache);

            // If still at capacity, remove oldest entry
            if cache.len() >= self.config.max_utxos {
                if let Some(oldest_key) = cache.keys().next().cloned() {
                    cache.remove(&oldest_key);
                }
            }
        }

        cache.insert(
            address.clone(),
            CachedEntry::new(utxos, self.config.utxo_ttl),
        );
        trace!(address = address, "UTXOs cached");
    }

    /// Invalidate UTXOs for a specific address
    pub async fn invalidate(&self, address: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(address);
        debug!(address = address, "UTXO cache invalidated");
    }

    /// Invalidate all UTXOs (e.g., on new block)
    pub async fn invalidate_all(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("All UTXO cache invalidated");
    }

    /// Clear all cached UTXOs
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("UTXO cache cleared");
    }

    /// Evict expired entries
    fn evict_expired(&self, cache: &mut HashMap<String, CachedEntry<Vec<Utxo>>>) {
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let expired_count = cache.values().filter(|e| e.is_expired()).count();

        CacheStats {
            total_entries: cache.len(),
            expired_entries: expired_count,
            active_entries: cache.len() - expired_count,
            max_entries: self.config.max_utxos,
        }
    }
}

/// Block header cache
#[derive(Debug, Clone)]
pub struct BlockHeader {
    /// Block hash
    pub hash: BlockHash,
    /// Block height in the chain
    pub height: u64,
    /// Block timestamp (Unix epoch seconds)
    pub time: u64,
    /// Hash of the previous block
    pub previous_block_hash: Option<BlockHash>,
}

/// Cache for block header data indexed by hash and height
pub struct BlockHeaderCache {
    by_hash: Arc<RwLock<HashMap<BlockHash, CachedEntry<BlockHeader>>>>,
    by_height: Arc<RwLock<HashMap<u64, CachedEntry<BlockHeader>>>>,
    config: CacheConfig,
}

impl BlockHeaderCache {
    /// Create a new block header cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            by_hash: Arc::new(RwLock::new(HashMap::new())),
            by_height: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get a block header by hash
    pub async fn get_by_hash(&self, hash: &BlockHash) -> Option<BlockHeader> {
        let cache = self.by_hash.read().await;
        if let Some(entry) = cache.get(hash) {
            if !entry.is_expired() {
                trace!(hash = %hash, "Block header cache hit (by hash)");
                return Some(entry.value.clone());
            }
        }
        trace!(hash = %hash, "Block header cache miss (by hash)");
        None
    }

    /// Get a block header by height
    pub async fn get_by_height(&self, height: u64) -> Option<BlockHeader> {
        let cache = self.by_height.read().await;
        if let Some(entry) = cache.get(&height) {
            if !entry.is_expired() {
                trace!(height = height, "Block header cache hit (by height)");
                return Some(entry.value.clone());
            }
        }
        trace!(height = height, "Block header cache miss (by height)");
        None
    }

    /// Insert a block header into cache
    pub async fn insert(&self, header: BlockHeader) {
        let mut by_hash = self.by_hash.write().await;
        let mut by_height = self.by_height.write().await;

        // Evict old entries if we're at capacity
        if by_hash.len() >= self.config.max_block_headers {
            Self::evict_expired(&mut by_hash);
            Self::evict_expired_height(&mut by_height);

            // If still at capacity, remove oldest entry
            if by_hash.len() >= self.config.max_block_headers {
                if let Some(oldest_key) = by_hash.keys().next().copied() {
                    by_hash.remove(&oldest_key);
                }
            }
            if by_height.len() >= self.config.max_block_headers {
                if let Some(oldest_key) = by_height.keys().next().copied() {
                    by_height.remove(&oldest_key);
                }
            }
        }

        let hash = header.hash;
        let height = header.height;

        by_hash.insert(
            hash,
            CachedEntry::new(header.clone(), self.config.block_header_ttl),
        );
        by_height.insert(
            height,
            CachedEntry::new(header, self.config.block_header_ttl),
        );

        trace!(hash = %hash, height = height, "Block header cached");
    }

    /// Invalidate a specific block header
    pub async fn invalidate(&self, hash: &BlockHash, height: u64) {
        let mut by_hash = self.by_hash.write().await;
        let mut by_height = self.by_height.write().await;

        by_hash.remove(hash);
        by_height.remove(&height);

        debug!(hash = %hash, height = height, "Block header cache invalidated");
    }

    /// Clear all cached block headers
    pub async fn clear(&self) {
        let mut by_hash = self.by_hash.write().await;
        let mut by_height = self.by_height.write().await;

        by_hash.clear();
        by_height.clear();

        debug!("Block header cache cleared");
    }

    /// Evict expired entries
    fn evict_expired(cache: &mut HashMap<BlockHash, CachedEntry<BlockHeader>>) {
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// Evict expired entries (by height)
    fn evict_expired_height(cache: &mut HashMap<u64, CachedEntry<BlockHeader>>) {
        cache.retain(|_, entry| !entry.is_expired());
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let by_hash = self.by_hash.read().await;
        let expired_count = by_hash.values().filter(|e| e.is_expired()).count();

        CacheStats {
            total_entries: by_hash.len(),
            expired_entries: expired_count,
            active_entries: by_hash.len() - expired_count,
            max_entries: self.config.max_block_headers,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of entries (active + expired)
    pub total_entries: usize,
    /// Number of entries that have expired but not yet been evicted
    pub expired_entries: usize,
    /// Number of non-expired entries
    pub active_entries: usize,
    /// Maximum allowed entries
    pub max_entries: usize,
}

impl CacheStats {
    /// Calculate hit rate (requires tracking hits/misses externally)
    pub fn utilization(&self) -> f64 {
        if self.max_entries == 0 {
            0.0
        } else {
            self.total_entries as f64 / self.max_entries as f64
        }
    }
}

/// Unified cache manager
pub struct CacheManager {
    /// Transaction data cache
    pub transactions: TransactionCache,
    /// UTXO data cache
    pub utxos: UtxoCache,
    /// Block header cache
    pub block_headers: BlockHeaderCache,
    #[allow(dead_code)]
    config: CacheConfig,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(config: CacheConfig) -> Self {
        Self {
            transactions: TransactionCache::new(config.clone()),
            utxos: UtxoCache::new(config.clone()),
            block_headers: BlockHeaderCache::new(config.clone()),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Clear all caches
    pub async fn clear_all(&self) {
        self.transactions.clear().await;
        self.utxos.clear().await;
        self.block_headers.clear().await;
        debug!("All caches cleared");
    }

    /// Get overall cache statistics
    pub async fn overall_stats(&self) -> OverallCacheStats {
        OverallCacheStats {
            transaction_stats: self.transactions.stats().await,
            utxo_stats: self.utxos.stats().await,
            block_header_stats: self.block_headers.stats().await,
        }
    }
}

/// Overall cache statistics
#[derive(Debug, Clone)]
pub struct OverallCacheStats {
    /// Statistics for the transaction cache
    pub transaction_stats: CacheStats,
    /// Statistics for the UTXO cache
    pub utxo_stats: CacheStats,
    /// Statistics for the block header cache
    pub block_header_stats: CacheStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[tokio::test]
    async fn test_cache_config_defaults() {
        let config = CacheConfig::default();
        assert!(config.transaction_ttl.as_secs() > 0);
        assert!(config.max_transactions > 0);
    }

    #[tokio::test]
    async fn test_transaction_cache() {
        let config = CacheConfig {
            transaction_ttl: Duration::from_secs(1),
            max_transactions: 2,
            ..Default::default()
        };

        let cache = TransactionCache::new(config);
        let txid = Txid::all_zeros();

        // Initially empty
        assert!(cache.get(&txid).await.is_none());

        // Insert and retrieve (would need a proper GetRawTransactionResult to test fully)
        // Skipping full test as it requires complex setup
    }

    #[tokio::test]
    async fn test_utxo_cache() {
        let config = CacheConfig {
            utxo_ttl: Duration::from_secs(1),
            max_utxos: 2,
            ..Default::default()
        };

        let cache = UtxoCache::new(config);
        let address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";

        // Initially empty
        assert!(cache.get(address).await.is_none());

        // Insert and retrieve
        let utxos = vec![];
        cache.insert(address.to_string(), utxos.clone()).await;
        assert_eq!(cache.get(address).await, Some(utxos));

        // Invalidate
        cache.invalidate(address).await;
        assert!(cache.get(address).await.is_none());
    }

    #[tokio::test]
    async fn test_block_header_cache() {
        let config = CacheConfig {
            block_header_ttl: Duration::from_secs(1),
            max_block_headers: 2,
            ..Default::default()
        };

        let cache = BlockHeaderCache::new(config);
        let hash = BlockHash::all_zeros();
        let header = BlockHeader {
            hash,
            height: 100,
            time: 1234567890,
            previous_block_hash: None,
        };

        // Initially empty
        assert!(cache.get_by_hash(&hash).await.is_none());
        assert!(cache.get_by_height(100).await.is_none());

        // Insert and retrieve
        cache.insert(header.clone()).await;
        assert!(cache.get_by_hash(&hash).await.is_some());
        assert!(cache.get_by_height(100).await.is_some());

        // Invalidate
        cache.invalidate(&hash, 100).await;
        assert!(cache.get_by_hash(&hash).await.is_none());
        assert!(cache.get_by_height(100).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_manager() {
        let manager = CacheManager::with_defaults();
        manager.clear_all().await;

        let stats = manager.overall_stats().await;
        assert_eq!(stats.transaction_stats.total_entries, 0);
        assert_eq!(stats.utxo_stats.total_entries, 0);
        assert_eq!(stats.block_header_stats.total_entries, 0);
    }

    #[test]
    fn test_cache_stats_utilization() {
        let stats = CacheStats {
            total_entries: 50,
            expired_entries: 10,
            active_entries: 40,
            max_entries: 100,
        };

        assert_eq!(stats.utilization(), 0.5);
    }
}
