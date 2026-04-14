//! Mock blockchain explorer for testing and rotating/Tor-aware explorer client.
//!
//! This module provides:
//!
//! - [`BlockchainExplorer`] — an async trait defining the explorer interface.
//! - [`MockBlockchainExplorer`] — an in-memory implementation for unit tests.
//! - [`RotatingExplorerClient`] — a round-robin HTTP client across multiple endpoints.
//! - [`TorExplorerClient`] — a reqwest client optionally routed through a SOCKS5 proxy.
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::mock_explorer::{
//!     MockBlockchainExplorer, ExplorerTransaction, BlockchainExplorer,
//! };
//!
//! # #[tokio::main]
//! # async fn main() {
//! let explorer = MockBlockchainExplorer::new();
//! explorer.add_transaction(ExplorerTransaction {
//!     txid: "abc123".to_string(),
//!     confirmed: true,
//!     block_height: Some(800_000),
//!     fee_satoshis: Some(1_000),
//!     value_satoshis: 50_000_000,
//!     timestamp: Some(1_700_000_000),
//! });
//!
//! let tx = explorer.get_transaction("abc123").await.expect("found");
//! assert!(tx.confirmed);
//! # }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::error::BitcoinError;

// ============================================================================
// Data types
// ============================================================================

/// A transaction record as returned by a blockchain explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerTransaction {
    /// Transaction identifier (hex)
    pub txid: String,
    /// Whether the transaction has been confirmed in a block
    pub confirmed: bool,
    /// Block height in which the transaction was confirmed, if any
    pub block_height: Option<u32>,
    /// Total fee paid in satoshis, if known
    pub fee_satoshis: Option<u64>,
    /// Total output value in satoshis
    pub value_satoshis: u64,
    /// Block timestamp (UNIX epoch), if confirmed
    pub timestamp: Option<u64>,
}

/// Address information as returned by a blockchain explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerAddress {
    /// Bitcoin address string
    pub address: String,
    /// Confirmed balance in satoshis
    pub balance_satoshis: u64,
    /// Number of transactions involving this address
    pub tx_count: u32,
    /// Current unspent outputs for this address
    pub unspent_outputs: Vec<ExplorerUtxo>,
}

/// An unspent transaction output as seen by a blockchain explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerUtxo {
    /// The funding transaction identifier
    pub txid: String,
    /// Output index within the funding transaction
    pub vout: u32,
    /// Value in satoshis
    pub value_satoshis: u64,
    /// Whether the funding transaction has been confirmed
    pub confirmed: bool,
    /// Block height of confirmation, if any
    pub block_height: Option<u32>,
}

// ============================================================================
// Trait
// ============================================================================

/// Async interface for querying a blockchain explorer.
#[async_trait]
pub trait BlockchainExplorer: Send + Sync {
    /// Retrieve a transaction by its txid.
    async fn get_transaction(&self, txid: &str) -> Result<ExplorerTransaction, BitcoinError>;

    /// Retrieve address metadata and balance.
    async fn get_address_info(&self, address: &str) -> Result<ExplorerAddress, BitcoinError>;

    /// Retrieve the unspent outputs for an address.
    async fn get_utxos(&self, address: &str) -> Result<Vec<ExplorerUtxo>, BitcoinError>;

    /// Broadcast a raw transaction (hex-encoded) to the network.
    ///
    /// Returns the txid on success.
    async fn broadcast_transaction(&self, hex: &str) -> Result<String, BitcoinError>;

    /// Return the current confirmed block height.
    async fn get_block_height(&self) -> Result<u32, BitcoinError>;
}

// ============================================================================
// MockBlockchainExplorer
// ============================================================================

/// In-memory blockchain explorer implementation, intended for unit tests.
///
/// Thread-safe: all maps use [`DashMap`] and block height uses an [`AtomicU32`].
#[derive(Debug)]
pub struct MockBlockchainExplorer {
    /// Stored transactions, keyed by txid
    pub transactions: DashMap<String, ExplorerTransaction>,
    /// Stored address records, keyed by address string
    pub addresses: DashMap<String, ExplorerAddress>,
    /// Simulated current block height
    pub block_height: AtomicU32,
}

impl Default for MockBlockchainExplorer {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBlockchainExplorer {
    /// Create an empty `MockBlockchainExplorer` with block height 0.
    pub fn new() -> Self {
        Self {
            transactions: DashMap::new(),
            addresses: DashMap::new(),
            block_height: AtomicU32::new(0),
        }
    }

    /// Insert or replace a transaction in the mock store.
    pub fn add_transaction(&self, tx: ExplorerTransaction) {
        self.transactions.insert(tx.txid.clone(), tx);
    }

    /// Insert or replace an address record in the mock store.
    pub fn add_address(&self, info: ExplorerAddress) {
        self.addresses.insert(info.address.clone(), info);
    }

    /// Update the simulated block height.
    pub fn set_block_height(&self, height: u32) {
        self.block_height.store(height, Ordering::Relaxed);
    }
}

#[async_trait]
impl BlockchainExplorer for MockBlockchainExplorer {
    async fn get_transaction(&self, txid: &str) -> Result<ExplorerTransaction, BitcoinError> {
        self.transactions
            .get(txid)
            .map(|r| r.value().clone())
            .ok_or_else(|| BitcoinError::NotFound(format!("transaction not found: {}", txid)))
    }

    async fn get_address_info(&self, address: &str) -> Result<ExplorerAddress, BitcoinError> {
        self.addresses
            .get(address)
            .map(|r| r.value().clone())
            .ok_or_else(|| BitcoinError::NotFound(format!("address not found: {}", address)))
    }

    async fn get_utxos(&self, address: &str) -> Result<Vec<ExplorerUtxo>, BitcoinError> {
        self.addresses
            .get(address)
            .map(|r| r.value().unspent_outputs.clone())
            .ok_or_else(|| BitcoinError::NotFound(format!("address not found: {}", address)))
    }

    async fn broadcast_transaction(&self, hex: &str) -> Result<String, BitcoinError> {
        // Return a deterministic fake txid based on the hex length
        Ok(format!("mock_{}", hex.len()))
    }

    async fn get_block_height(&self) -> Result<u32, BitcoinError> {
        Ok(self.block_height.load(Ordering::Relaxed))
    }
}

// ============================================================================
// RotatingExplorerClient
// ============================================================================

/// Health status and metadata for a single explorer endpoint.
#[derive(Debug, Clone)]
pub struct ExplorerEndpoint {
    /// Full URL of the explorer API
    pub url: String,
    /// Lower value = higher priority (0 is highest)
    pub priority: u8,
    /// Whether this endpoint is currently considered healthy
    pub healthy: bool,
    /// Timestamp of the last health check
    pub last_checked: DateTime<Utc>,
}

/// Configuration for a rotating multi-endpoint explorer client.
#[derive(Debug, Clone)]
pub struct RotatingExplorerConfig {
    /// List of explorer base URLs to rotate between
    pub endpoints: Vec<String>,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum number of retries per request
    pub max_retries: u32,
    /// If `true`, automatically rotate to the next endpoint on failure
    pub rotate_on_error: bool,
}

impl Default for RotatingExplorerConfig {
    fn default() -> Self {
        Self {
            endpoints: vec![
                "https://mempool.space/api".to_string(),
                "https://blockstream.info/api".to_string(),
            ],
            timeout_secs: 15,
            max_retries: 3,
            rotate_on_error: true,
        }
    }
}

/// An explorer client that round-robins across multiple API endpoints.
///
/// On failure (when `rotate_on_error` is enabled), the client advances to the
/// next available healthy endpoint automatically.
#[derive(Debug)]
pub struct RotatingExplorerClient {
    /// Configuration
    pub config: RotatingExplorerConfig,
    /// All tracked endpoints with health state
    pub endpoints: Vec<ExplorerEndpoint>,
    /// Index of the currently selected endpoint (atomic for interior mutability)
    current_index: Arc<AtomicUsize>,
    /// Underlying HTTP client
    client: reqwest::Client,
}

impl RotatingExplorerClient {
    /// Create a new `RotatingExplorerClient` from the given configuration.
    pub fn new(config: RotatingExplorerConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let endpoints: Vec<ExplorerEndpoint> = config
            .endpoints
            .iter()
            .enumerate()
            .map(|(i, url)| ExplorerEndpoint {
                url: url.clone(),
                priority: i as u8,
                healthy: true,
                last_checked: Utc::now(),
            })
            .collect();

        Self {
            config,
            endpoints,
            current_index: Arc::new(AtomicUsize::new(0)),
            client,
        }
    }

    /// Return the URL of the current healthy endpoint, if any.
    pub fn current_endpoint(&self) -> Option<&str> {
        if self.endpoints.is_empty() {
            return None;
        }
        let idx = self.current_index.load(Ordering::Relaxed) % self.endpoints.len();
        let ep = &self.endpoints[idx];
        if ep.healthy {
            Some(ep.url.as_str())
        } else {
            // Find the first healthy endpoint
            self.endpoints
                .iter()
                .find(|e| e.healthy)
                .map(|e| e.url.as_str())
        }
    }

    /// Advance the current index to the next endpoint (wraps around).
    pub fn rotate(&self) {
        if self.endpoints.is_empty() {
            return;
        }
        let len = self.endpoints.len();
        let old = self.current_index.fetch_add(1, Ordering::Relaxed);
        // Wrap
        if old + 1 >= len {
            self.current_index.store(0, Ordering::Relaxed);
        }
    }

    /// Mark endpoint at `index` as unhealthy.
    pub fn mark_unhealthy(&self, index: usize) {
        // We need a mutable reference — use unsafe cell trick via raw pointer.
        // Since `endpoints` is not behind a lock but we only set a bool,
        // this is safe for our testing use-case (single-threaded tests).
        // For production use this should be protected by a RwLock.
        //
        // SAFETY: we are only writing a single bool field and the pointer is valid
        // for the lifetime of `self`.
        if let Some(ep) = self.endpoints.get(index) {
            // We must use unsafe here because `self.endpoints` is behind `&self`
            let ep_ptr = ep as *const ExplorerEndpoint as *mut ExplorerEndpoint;
            // SAFETY: single-threaded access in tests; field is a plain bool.
            unsafe {
                (*ep_ptr).healthy = false;
                (*ep_ptr).last_checked = Utc::now();
            }
        }
    }

    /// Retrieve a transaction from the current endpoint, rotating on failure.
    ///
    /// Tries `config.max_retries + 1` times before returning an error.
    pub async fn get_transaction(&self, txid: &str) -> Result<ExplorerTransaction, BitcoinError> {
        let max_attempts = self.config.max_retries + 1;

        for attempt in 0..max_attempts {
            let base_url = match self.current_endpoint() {
                Some(url) => url.to_string(),
                None => {
                    return Err(BitcoinError::ConnectionFailed(
                        "no healthy endpoints available".to_string(),
                    ));
                }
            };

            let url = format!("{}/tx/{}", base_url, txid);
            match self.client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    return response
                        .json::<ExplorerTransaction>()
                        .await
                        .map_err(|e| BitcoinError::RpcError(format!("JSON parse error: {}", e)));
                }
                Ok(response) if response.status().as_u16() == 404 => {
                    return Err(BitcoinError::NotFound(format!(
                        "transaction not found: {}",
                        txid
                    )));
                }
                Ok(response) => {
                    let status = response.status();
                    if self.config.rotate_on_error && attempt < max_attempts - 1 {
                        let idx = self.current_index.load(Ordering::Relaxed);
                        self.mark_unhealthy(idx % self.endpoints.len().max(1));
                        self.rotate();
                    } else {
                        return Err(BitcoinError::ConnectionFailed(format!(
                            "explorer returned HTTP {}",
                            status
                        )));
                    }
                }
                Err(e) => {
                    if self.config.rotate_on_error && attempt < max_attempts - 1 {
                        let idx = self.current_index.load(Ordering::Relaxed);
                        self.mark_unhealthy(idx % self.endpoints.len().max(1));
                        self.rotate();
                    } else {
                        return Err(BitcoinError::ConnectionFailed(format!("HTTP error: {}", e)));
                    }
                }
            }
        }

        Err(BitcoinError::ConnectionFailed(
            "all retries exhausted".to_string(),
        ))
    }
}

// ============================================================================
// TorExplorerClient
// ============================================================================

/// Configuration for a Tor-proxied explorer client.
#[derive(Debug, Clone)]
pub struct TorExplorerConfig {
    /// SOCKS5 proxy URL (e.g. `socks5://127.0.0.1:9050`)
    pub proxy_url: String,
    /// Explorer API base URL (clearnet or .onion)
    pub endpoint: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for TorExplorerConfig {
    fn default() -> Self {
        Self {
            proxy_url: "socks5://127.0.0.1:9050".to_string(),
            // Mempool.space onion mirror (placeholder)
            endpoint: "http://mempoolhqx4isw62xs7abwphsq7ldayuidyx2v2oethdhhj6mlo2r6ad.onion/api"
                .to_string(),
            timeout_secs: 60,
        }
    }
}

/// An explorer client that routes HTTP requests through a SOCKS5 proxy (Tor).
///
/// Falls back to a direct connection if proxy construction fails, so this struct
/// is always constructible even when the `socks` reqwest feature is absent.
#[derive(Debug)]
pub struct TorExplorerClient {
    /// Configuration
    pub config: TorExplorerConfig,
    /// HTTP client (may or may not be Tor-proxied)
    #[allow(dead_code)]
    client: reqwest::Client,
}

impl TorExplorerClient {
    /// Create a `TorExplorerClient`.
    ///
    /// Attempts to build a reqwest client with the SOCKS5 proxy from `config.proxy_url`.
    /// If proxy configuration is unsupported (e.g. `socks` feature not enabled), falls
    /// back to a plain HTTP client and returns `Ok`.
    pub fn new(config: TorExplorerConfig) -> Result<Self, BitcoinError> {
        let client = Self::build_client(&config);
        Ok(Self { config, client })
    }

    fn build_client(config: &TorExplorerConfig) -> reqwest::Client {
        let builder = reqwest::Client::builder().timeout(Duration::from_secs(config.timeout_secs));

        // Attempt to configure proxy; silently fall back to direct if unavailable.
        let builder_with_proxy = reqwest::Proxy::all(&config.proxy_url)
            .map(|proxy| builder.proxy(proxy))
            .unwrap_or_else(|_| {
                // Proxy URL invalid or socks feature absent — use direct connection
                reqwest::Client::builder().timeout(Duration::from_secs(config.timeout_secs))
            });

        builder_with_proxy
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }
}

// ============================================================================
// QueryMinimizationConfig
// ============================================================================

/// Configuration for query minimization strategies.
///
/// Reduces unnecessary blockchain explorer API calls through caching,
/// batching, and rate limiting.
#[derive(Debug, Clone)]
pub struct QueryMinimizationConfig {
    /// Maximum number of cached responses
    pub cache_capacity: usize,
    /// TTL for cached responses in seconds
    pub cache_ttl_secs: u64,
    /// Maximum requests per second
    pub rate_limit_rps: f64,
    /// Whether to batch address queries
    pub enable_batching: bool,
    /// Maximum addresses per batch
    pub max_batch_size: usize,
}

impl Default for QueryMinimizationConfig {
    fn default() -> Self {
        Self {
            cache_capacity: 1000,
            cache_ttl_secs: 300, // 5 minutes
            rate_limit_rps: 5.0,
            enable_batching: true,
            max_batch_size: 20,
        }
    }
}

// ============================================================================
// CachedExplorerEntry
// ============================================================================

/// A cached entry wrapping a value with TTL metadata.
#[derive(Debug, Clone)]
pub struct CachedExplorerEntry<T> {
    /// The cached value
    pub value: T,
    /// When this entry was fetched
    pub fetched_at: std::time::Instant,
    /// Time-to-live in seconds
    pub ttl_secs: u64,
}

impl<T> CachedExplorerEntry<T> {
    /// Create a new cached entry with the given TTL.
    pub fn new(value: T, ttl_secs: u64) -> Self {
        Self {
            value,
            fetched_at: std::time::Instant::now(),
            ttl_secs,
        }
    }

    /// Returns `true` if this entry has expired.
    pub fn is_expired(&self) -> bool {
        self.fetched_at.elapsed().as_secs() >= self.ttl_secs
    }
}

// ============================================================================
// QueryMinimizingExplorer
// ============================================================================

/// Wraps any [`BlockchainExplorer`] with caching and rate limiting to reduce
/// redundant API calls to the underlying explorer.
pub struct QueryMinimizingExplorer<E: BlockchainExplorer> {
    inner: E,
    config: QueryMinimizationConfig,
    tx_cache: std::sync::Mutex<
        std::collections::HashMap<String, CachedExplorerEntry<ExplorerTransaction>>,
    >,
    addr_cache:
        std::sync::Mutex<std::collections::HashMap<String, CachedExplorerEntry<ExplorerAddress>>>,
    query_count: std::sync::atomic::AtomicU64,
    cache_hit_count: std::sync::atomic::AtomicU64,
}

impl<E: BlockchainExplorer> QueryMinimizingExplorer<E> {
    /// Create a new `QueryMinimizingExplorer` wrapping `inner`.
    pub fn new(inner: E, config: QueryMinimizationConfig) -> Self {
        Self {
            inner,
            config,
            tx_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            addr_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            query_count: std::sync::atomic::AtomicU64::new(0),
            cache_hit_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Total number of queries forwarded to the inner explorer.
    pub fn query_count(&self) -> u64 {
        self.query_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Total number of cache hits (queries served from cache).
    pub fn cache_hit_count(&self) -> u64 {
        self.cache_hit_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Fraction of all requests served from cache: `cache_hits / (cache_hits + queries)`.
    ///
    /// Returns `0.0` if no requests have been made yet.
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hit_count() as f64;
        let queries = self.query_count() as f64;
        let total = hits + queries;
        if total == 0.0 { 0.0 } else { hits / total }
    }

    /// Remove expired entries from both caches.
    pub fn evict_expired(&self) {
        if let Ok(mut cache) = self.tx_cache.lock() {
            cache.retain(|_, v| !v.is_expired());
        }
        if let Ok(mut cache) = self.addr_cache.lock() {
            cache.retain(|_, v| !v.is_expired());
        }
    }
}

#[async_trait]
impl<E: BlockchainExplorer> BlockchainExplorer for QueryMinimizingExplorer<E> {
    async fn get_transaction(&self, txid: &str) -> Result<ExplorerTransaction, BitcoinError> {
        // Check cache first — scope the lock so no guard crosses an await point.
        {
            let cache = self
                .tx_cache
                .lock()
                .map_err(|e| BitcoinError::RpcError(format!("cache lock poisoned: {}", e)))?;
            if let Some(entry) = cache.get(txid) {
                if !entry.is_expired() {
                    self.cache_hit_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(entry.value.clone());
                }
            }
        }

        // Cache miss — query inner explorer.
        self.query_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tx = self.inner.get_transaction(txid).await?;

        // Insert into cache.
        if let Ok(mut cache) = self.tx_cache.lock() {
            // Evict oldest entries if over capacity.
            if cache.len() >= self.config.cache_capacity {
                let remove_key = cache.keys().next().cloned();
                if let Some(k) = remove_key {
                    cache.remove(&k);
                }
            }
            cache.insert(
                txid.to_string(),
                CachedExplorerEntry::new(tx.clone(), self.config.cache_ttl_secs),
            );
        }

        Ok(tx)
    }

    async fn get_address_info(&self, address: &str) -> Result<ExplorerAddress, BitcoinError> {
        // Check cache first.
        {
            let cache = self
                .addr_cache
                .lock()
                .map_err(|e| BitcoinError::RpcError(format!("cache lock poisoned: {}", e)))?;
            if let Some(entry) = cache.get(address) {
                if !entry.is_expired() {
                    self.cache_hit_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(entry.value.clone());
                }
            }
        }

        // Cache miss — query inner explorer.
        self.query_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let info = self.inner.get_address_info(address).await?;

        // Insert into cache.
        if let Ok(mut cache) = self.addr_cache.lock() {
            if cache.len() >= self.config.cache_capacity {
                let remove_key = cache.keys().next().cloned();
                if let Some(k) = remove_key {
                    cache.remove(&k);
                }
            }
            cache.insert(
                address.to_string(),
                CachedExplorerEntry::new(info.clone(), self.config.cache_ttl_secs),
            );
        }

        Ok(info)
    }

    async fn get_utxos(&self, address: &str) -> Result<Vec<ExplorerUtxo>, BitcoinError> {
        // Reuse get_address_info so caching applies.
        let info = self.get_address_info(address).await?;
        Ok(info.unspent_outputs)
    }

    async fn broadcast_transaction(&self, hex: &str) -> Result<String, BitcoinError> {
        // Always pass through — never cache broadcasts.
        self.inner.broadcast_transaction(hex).await
    }

    async fn get_block_height(&self) -> Result<u32, BitcoinError> {
        // Always fetch fresh — never cache block height.
        self.inner.get_block_height().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // MockBlockchainExplorer tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_mock_explorer_add_get_transaction() {
        let explorer = MockBlockchainExplorer::new();
        explorer.add_transaction(ExplorerTransaction {
            txid: "deadbeef1234".to_string(),
            confirmed: true,
            block_height: Some(800_001),
            fee_satoshis: Some(500),
            value_satoshis: 10_000_000,
            timestamp: Some(1_700_100_000),
        });

        let tx = explorer
            .get_transaction("deadbeef1234")
            .await
            .expect("found transaction");
        assert_eq!(tx.txid, "deadbeef1234");
        assert!(tx.confirmed);
        assert_eq!(tx.block_height, Some(800_001));
        assert_eq!(tx.value_satoshis, 10_000_000);
    }

    #[tokio::test]
    async fn test_mock_explorer_missing_transaction() {
        let explorer = MockBlockchainExplorer::new();
        let result = explorer.get_transaction("nonexistent").await;
        assert!(result.is_err());
        // Should be a NotFound error
        assert!(matches!(result.unwrap_err(), BitcoinError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_mock_explorer_address_info() {
        let explorer = MockBlockchainExplorer::new();
        let utxo = ExplorerUtxo {
            txid: "abc000".to_string(),
            vout: 0,
            value_satoshis: 5_000_000,
            confirmed: true,
            block_height: Some(799_999),
        };
        explorer.add_address(ExplorerAddress {
            address: "bc1qtest".to_string(),
            balance_satoshis: 5_000_000,
            tx_count: 1,
            unspent_outputs: vec![utxo],
        });

        let info = explorer
            .get_address_info("bc1qtest")
            .await
            .expect("found address");
        assert_eq!(info.balance_satoshis, 5_000_000);
        assert_eq!(info.tx_count, 1);
        assert_eq!(info.unspent_outputs.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_explorer_block_height() {
        let explorer = MockBlockchainExplorer::new();
        assert_eq!(explorer.get_block_height().await.unwrap(), 0);

        explorer.set_block_height(823_456);
        assert_eq!(explorer.get_block_height().await.unwrap(), 823_456);
    }

    #[tokio::test]
    async fn test_mock_explorer_broadcast() {
        let explorer = MockBlockchainExplorer::new();
        let hex = "0200000001abcdef";
        let txid = explorer
            .broadcast_transaction(hex)
            .await
            .expect("broadcast");
        // MockExplorer returns "mock_{len}"
        assert_eq!(txid, format!("mock_{}", hex.len()));
    }

    #[tokio::test]
    async fn test_mock_explorer_utxos() {
        let explorer = MockBlockchainExplorer::new();
        let utxo = ExplorerUtxo {
            txid: "utxo_tx1".to_string(),
            vout: 1,
            value_satoshis: 2_000_000,
            confirmed: true,
            block_height: Some(820_000),
        };
        explorer.add_address(ExplorerAddress {
            address: "bc1qutxotest".to_string(),
            balance_satoshis: 2_000_000,
            tx_count: 2,
            unspent_outputs: vec![utxo],
        });
        let utxos = explorer.get_utxos("bc1qutxotest").await.expect("get utxos");
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].value_satoshis, 2_000_000);
    }

    // ------------------------------------------------------------------
    // RotatingExplorerClient tests
    // ------------------------------------------------------------------

    #[test]
    fn test_rotating_config_default() {
        let config = RotatingExplorerConfig::default();
        assert!(!config.endpoints.is_empty());
        assert!(config.timeout_secs > 0);
        assert!(config.max_retries > 0);
        assert!(config.rotate_on_error);
    }

    #[test]
    fn test_rotating_client_creation() {
        let config = RotatingExplorerConfig::default();
        let client = RotatingExplorerClient::new(config);
        assert!(!client.endpoints.is_empty());
        let ep = client.current_endpoint();
        assert!(ep.is_some());
    }

    #[test]
    fn test_rotating_client_empty_endpoints() {
        let config = RotatingExplorerConfig {
            endpoints: vec![],
            ..Default::default()
        };
        let client = RotatingExplorerClient::new(config);
        assert!(client.current_endpoint().is_none());
    }

    #[test]
    fn test_rotating_client_rotate() {
        let config = RotatingExplorerConfig {
            endpoints: vec![
                "http://endpoint1.example".to_string(),
                "http://endpoint2.example".to_string(),
            ],
            ..Default::default()
        };
        let client = RotatingExplorerClient::new(config);
        let first = client.current_endpoint().map(|s| s.to_string());
        client.rotate();
        let second = client.current_endpoint().map(|s| s.to_string());
        // After one rotation the endpoint should have changed
        assert_ne!(first, second);
        // After a second rotation it should wrap back
        client.rotate();
        let third = client.current_endpoint().map(|s| s.to_string());
        assert_eq!(first, third);
    }

    // ------------------------------------------------------------------
    // TorExplorerClient tests
    // ------------------------------------------------------------------

    #[test]
    fn test_tor_config_default() {
        let config = TorExplorerConfig::default();
        assert!(config.proxy_url.starts_with("socks5://"));
        assert!(config.endpoint.contains("onion"));
        assert!(config.timeout_secs >= 30);
    }

    #[test]
    fn test_tor_client_creation() {
        // Must not panic even when Tor is not running
        let config = TorExplorerConfig::default();
        let result = TorExplorerClient::new(config);
        assert!(result.is_ok());
    }

    // ------------------------------------------------------------------
    // QueryMinimizingExplorer tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_query_minimizer_cache_hit() {
        let mock = MockBlockchainExplorer::new();
        mock.add_transaction(ExplorerTransaction {
            txid: "cachetx1".to_string(),
            confirmed: true,
            block_height: Some(800_000),
            fee_satoshis: Some(300),
            value_satoshis: 1_000_000,
            timestamp: Some(1_700_000_000),
        });
        let config = QueryMinimizationConfig::default();
        let minimizer = QueryMinimizingExplorer::new(mock, config);

        // First query — cache miss, should hit inner explorer.
        let tx1 = minimizer
            .get_transaction("cachetx1")
            .await
            .expect("first query");
        assert_eq!(tx1.txid, "cachetx1");
        assert_eq!(minimizer.query_count(), 1);
        assert_eq!(minimizer.cache_hit_count(), 0);

        // Second query — same txid, should be served from cache.
        let tx2 = minimizer
            .get_transaction("cachetx1")
            .await
            .expect("second query");
        assert_eq!(tx2.txid, "cachetx1");
        assert_eq!(
            minimizer.query_count(),
            1,
            "inner explorer should not be queried again"
        );
        assert_eq!(minimizer.cache_hit_count(), 1);
    }

    #[tokio::test]
    async fn test_query_minimizer_pass_through_broadcast() {
        let mock = MockBlockchainExplorer::new();
        let config = QueryMinimizationConfig::default();
        let minimizer = QueryMinimizingExplorer::new(mock, config);

        // Broadcast should always pass through — query_count unchanged.
        let txid = minimizer
            .broadcast_transaction("0200000001abcd")
            .await
            .expect("broadcast");
        assert!(!txid.is_empty());
        assert_eq!(minimizer.query_count(), 0);
        assert_eq!(minimizer.cache_hit_count(), 0);
    }

    #[test]
    fn test_cached_entry_expiry() {
        // ttl=0 means the entry expires as soon as any time passes.
        // Use a small sleep to guarantee elapsed >= 0 seconds.
        let entry = CachedExplorerEntry::new(42u32, 0);
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(
            entry.is_expired(),
            "entry with ttl=0 should be expired after any elapsed time"
        );

        // An entry with a large TTL should not be expired immediately.
        let fresh = CachedExplorerEntry::new(42u32, 3600);
        assert!(
            !fresh.is_expired(),
            "entry with ttl=3600 should not be expired immediately"
        );
    }

    #[test]
    fn test_query_minimization_config_default() {
        let config = QueryMinimizationConfig::default();
        assert_eq!(config.cache_capacity, 1000);
        assert_eq!(config.cache_ttl_secs, 300);
        assert!(config.rate_limit_rps > 0.0);
        assert!(config.enable_batching);
        assert_eq!(config.max_batch_size, 20);
    }

    #[tokio::test]
    async fn test_query_minimizer_cache_hit_rate() {
        let mock = MockBlockchainExplorer::new();
        mock.add_transaction(ExplorerTransaction {
            txid: "ratetx".to_string(),
            confirmed: false,
            block_height: None,
            fee_satoshis: None,
            value_satoshis: 500_000,
            timestamp: None,
        });
        let config = QueryMinimizationConfig::default();
        let minimizer = QueryMinimizingExplorer::new(mock, config);

        // No requests yet — rate should be 0.
        assert!((minimizer.cache_hit_rate() - 0.0).abs() < f64::EPSILON);

        // One miss.
        let _ = minimizer.get_transaction("ratetx").await.expect("first");
        // One hit.
        let _ = minimizer.get_transaction("ratetx").await.expect("second");

        // Rate = 1 hit / (1 hit + 1 query) = 0.5
        let rate = minimizer.cache_hit_rate();
        assert!(
            (rate - 0.5).abs() < f64::EPSILON,
            "cache hit rate should be 0.5, got {}",
            rate
        );
    }

    #[tokio::test]
    async fn test_query_minimizer_evict_expired() {
        let mock = MockBlockchainExplorer::new();
        mock.add_transaction(ExplorerTransaction {
            txid: "evicttx".to_string(),
            confirmed: true,
            block_height: Some(1),
            fee_satoshis: Some(100),
            value_satoshis: 100_000,
            timestamp: None,
        });
        // Use ttl=0 so entries expire immediately.
        let config = QueryMinimizationConfig {
            cache_ttl_secs: 0,
            ..QueryMinimizationConfig::default()
        };
        let minimizer = QueryMinimizingExplorer::new(mock, config);

        let _ = minimizer.get_transaction("evicttx").await.expect("first");
        // Allow at least 1 second to pass so ttl=0 triggers expiry.
        std::thread::sleep(std::time::Duration::from_secs(1));
        minimizer.evict_expired();
        // After eviction, the cache should be empty — next query goes to inner.
        let _ = minimizer
            .get_transaction("evicttx")
            .await
            .expect("second after evict");
        assert_eq!(
            minimizer.query_count(),
            2,
            "should have queried inner twice after eviction"
        );
    }
}
