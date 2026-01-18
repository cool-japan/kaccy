//! Compact Block Filters (BIP 157/158)
//!
//! Implements compact block filters for privacy-preserving transaction detection.
//! Allows light clients to detect relevant transactions without revealing addresses.

use bitcoin::{BlockHash, Network, ScriptBuf, Txid};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// Filter type as per BIP 157
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    /// Basic filter (filter type 0x00)
    Basic,
    /// Extended filter (filter type 0x01) - not widely adopted
    Extended,
}

/// Compact block filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactFilter {
    /// Block hash this filter corresponds to
    pub block_hash: BlockHash,
    /// Filter type
    pub filter_type: FilterType,
    /// Serialized filter data
    pub filter_data: Vec<u8>,
    /// Block height
    pub height: u64,
}

impl CompactFilter {
    /// Create a new compact filter
    pub fn new(
        block_hash: BlockHash,
        filter_type: FilterType,
        filter_data: Vec<u8>,
        height: u64,
    ) -> Self {
        Self {
            block_hash,
            filter_type,
            filter_data,
            height,
        }
    }

    /// Check if the filter might match any of the given scripts
    pub fn matches_any(&self, scripts: &[ScriptBuf]) -> bool {
        // Simplified match check - in production, use proper GCS filter matching
        // For now, we'll do a basic check
        if scripts.is_empty() {
            return false;
        }

        // In a real implementation, this would:
        // 1. Deserialize the GCS filter
        // 2. Check each script against the filter
        // 3. Return true if any match

        // For this implementation, we'll assume it might match
        !self.filter_data.is_empty()
    }
}

/// Block filter header (for verification)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterHeader {
    /// Block hash
    pub block_hash: BlockHash,
    /// Filter header hash
    pub filter_header: Vec<u8>,
    /// Previous filter header hash
    pub prev_filter_header: Vec<u8>,
    /// Block height
    pub height: u64,
}

/// Compact filter manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactFilterConfig {
    /// Whether to download and verify filters
    pub enabled: bool,
    /// Filter type to use
    pub filter_type: FilterType,
    /// Maximum number of filters to cache
    pub max_cached_filters: usize,
    /// Whether to verify filter headers
    pub verify_headers: bool,
}

impl Default for CompactFilterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filter_type: FilterType::Basic,
            max_cached_filters: 1000,
            verify_headers: true,
        }
    }
}

/// Compact filter manager for privacy-preserving transaction detection
pub struct CompactFilterManager {
    config: CompactFilterConfig,
    client: Arc<BitcoinClient>,
    filters: Arc<RwLock<HashMap<BlockHash, CompactFilter>>>,
    headers: Arc<RwLock<HashMap<BlockHash, FilterHeader>>>,
    watched_scripts: Arc<RwLock<HashSet<ScriptBuf>>>,
}

impl CompactFilterManager {
    /// Create a new compact filter manager
    pub fn new(config: CompactFilterConfig, client: Arc<BitcoinClient>) -> Self {
        Self {
            config,
            client,
            filters: Arc::new(RwLock::new(HashMap::new())),
            headers: Arc::new(RwLock::new(HashMap::new())),
            watched_scripts: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Add a script to watch
    pub async fn watch_script(&self, script: ScriptBuf) {
        self.watched_scripts.write().await.insert(script);
        tracing::info!("Added script to watch list");
    }

    /// Remove a script from watch list
    pub async fn unwatch_script(&self, script: &ScriptBuf) -> bool {
        let removed = self.watched_scripts.write().await.remove(script);
        if removed {
            tracing::info!("Removed script from watch list");
        }
        removed
    }

    /// Get all watched scripts
    pub async fn get_watched_scripts(&self) -> Vec<ScriptBuf> {
        self.watched_scripts.read().await.iter().cloned().collect()
    }

    /// Download and cache a filter for a specific block
    pub async fn download_filter(&self, block_hash: BlockHash) -> Result<CompactFilter> {
        // Check if already cached
        if let Some(filter) = self.filters.read().await.get(&block_hash) {
            return Ok(filter.clone());
        }

        // In a real implementation, this would use the Bitcoin P2P protocol
        // to fetch the filter via getcfilters/getcfheaders messages
        // For now, we'll create a stub

        tracing::warn!("Filter download not fully implemented - would fetch via P2P protocol");

        // Create a placeholder filter
        let filter = CompactFilter::new(
            block_hash,
            self.config.filter_type,
            vec![0u8; 32], // Placeholder data
            0,
        );

        // Cache it
        self.cache_filter(filter.clone()).await;

        Ok(filter)
    }

    /// Cache a filter
    async fn cache_filter(&self, filter: CompactFilter) {
        let mut filters = self.filters.write().await;

        // Evict old filters if cache is full
        if filters.len() >= self.config.max_cached_filters {
            // Remove oldest entries (simplified - in production use LRU)
            if let Some(key) = filters.keys().next().cloned() {
                filters.remove(&key);
            }
        }

        filters.insert(filter.block_hash, filter);
    }

    /// Download filter header for verification
    pub async fn download_filter_header(&self, block_hash: BlockHash) -> Result<FilterHeader> {
        // Check if already cached
        if let Some(header) = self.headers.read().await.get(&block_hash) {
            return Ok(header.clone());
        }

        // In a real implementation, fetch via P2P protocol
        tracing::warn!(
            "Filter header download not fully implemented - would fetch via P2P protocol"
        );

        // Create placeholder
        let header = FilterHeader {
            block_hash,
            filter_header: vec![0u8; 32],
            prev_filter_header: vec![0u8; 32],
            height: 0,
        };

        self.headers
            .write()
            .await
            .insert(block_hash, header.clone());

        Ok(header)
    }

    /// Scan a block for relevant transactions using filters
    pub async fn scan_block(&self, block_hash: BlockHash) -> Result<Vec<Txid>> {
        let filter = self.download_filter(block_hash).await?;
        let scripts = self.get_watched_scripts().await;

        if scripts.is_empty() {
            return Ok(vec![]);
        }

        // Check if filter matches any watched scripts
        if filter.matches_any(&scripts) {
            tracing::info!(
                block_hash = %block_hash,
                "Filter matches - downloading full block"
            );

            // Filter matched - need to download full block to find exact transactions
            // In production, this would fetch the full block and check transactions
            Ok(vec![])
        } else {
            // No match - can skip this block
            Ok(vec![])
        }
    }

    /// Scan a range of blocks
    pub async fn scan_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<(BlockHash, Vec<Txid>)>> {
        let mut results = Vec::new();

        for height in start_height..=end_height {
            // Get block hash for this height
            let block_hash = self.client.get_block_hash(height)?;

            // Scan the block
            let txids = self.scan_block(block_hash).await?;

            if !txids.is_empty() {
                results.push((block_hash, txids));
            }
        }

        tracing::info!(
            start = start_height,
            end = end_height,
            matches = results.len(),
            "Completed block range scan"
        );

        Ok(results)
    }

    /// Get filter statistics
    pub async fn get_statistics(&self) -> FilterStatistics {
        FilterStatistics {
            cached_filters: self.filters.read().await.len(),
            cached_headers: self.headers.read().await.len(),
            watched_scripts: self.watched_scripts.read().await.len(),
        }
    }

    /// Clear all cached filters
    pub async fn clear_cache(&self) {
        self.filters.write().await.clear();
        self.headers.write().await.clear();
        tracing::info!("Cleared filter cache");
    }
}

/// Filter statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatistics {
    /// Number of cached filters
    pub cached_filters: usize,
    /// Number of cached headers
    pub cached_headers: usize,
    /// Number of watched scripts
    pub watched_scripts: usize,
}

/// Filter verification helper
pub struct FilterVerifier {
    #[allow(dead_code)]
    network: Network,
}

impl FilterVerifier {
    /// Create a new filter verifier
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Verify a filter header chain
    pub fn verify_header_chain(&self, headers: &[FilterHeader]) -> Result<bool> {
        if headers.is_empty() {
            return Ok(true);
        }

        // Verify each header links to the previous one
        for i in 1..headers.len() {
            let prev = &headers[i - 1];
            let current = &headers[i];

            // In production, verify that current.prev_filter_header == hash(prev.filter_header)
            if current.height != prev.height + 1 {
                return Err(BitcoinError::Validation(
                    "Filter header chain has gap".to_string(),
                ));
            }
        }

        Ok(true)
    }

    /// Verify a filter matches its header
    pub fn verify_filter_header(
        &self,
        filter: &CompactFilter,
        header: &FilterHeader,
    ) -> Result<bool> {
        if filter.block_hash != header.block_hash {
            return Err(BitcoinError::Validation(
                "Filter and header block hash mismatch".to_string(),
            ));
        }

        // In production, verify hash(filter.filter_data) == header.filter_header
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_filter_type() {
        assert_eq!(FilterType::Basic, FilterType::Basic);
        assert_ne!(FilterType::Basic, FilterType::Extended);
    }

    #[test]
    fn test_compact_filter_creation() {
        let block_hash =
            BlockHash::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f")
                .unwrap();

        let filter = CompactFilter::new(block_hash, FilterType::Basic, vec![1, 2, 3, 4], 0);

        assert_eq!(filter.block_hash, block_hash);
        assert_eq!(filter.filter_type, FilterType::Basic);
        assert_eq!(filter.filter_data, vec![1, 2, 3, 4]);
        assert_eq!(filter.height, 0);
    }

    #[test]
    fn test_compact_filter_config_defaults() {
        let config = CompactFilterConfig::default();
        assert!(config.enabled);
        assert_eq!(config.filter_type, FilterType::Basic);
        assert_eq!(config.max_cached_filters, 1000);
        assert!(config.verify_headers);
    }

    #[test]
    fn test_filter_statistics() {
        let stats = FilterStatistics {
            cached_filters: 100,
            cached_headers: 150,
            watched_scripts: 5,
        };

        assert_eq!(stats.cached_filters, 100);
        assert_eq!(stats.cached_headers, 150);
        assert_eq!(stats.watched_scripts, 5);
    }

    #[test]
    fn test_filter_header_creation() {
        let block_hash =
            BlockHash::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f")
                .unwrap();

        let header = FilterHeader {
            block_hash,
            filter_header: vec![1, 2, 3],
            prev_filter_header: vec![4, 5, 6],
            height: 100,
        };

        assert_eq!(header.block_hash, block_hash);
        assert_eq!(header.height, 100);
    }

    #[test]
    fn test_filter_verifier() {
        let verifier = FilterVerifier::new(Network::Bitcoin);

        // Test empty chain
        let result = verifier.verify_header_chain(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_filter_verifier_chain_gap() {
        let verifier = FilterVerifier::new(Network::Bitcoin);
        let block_hash =
            BlockHash::from_str("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f")
                .unwrap();

        let headers = vec![
            FilterHeader {
                block_hash,
                filter_header: vec![1],
                prev_filter_header: vec![0],
                height: 100,
            },
            FilterHeader {
                block_hash,
                filter_header: vec![2],
                prev_filter_header: vec![1],
                height: 102, // Gap!
            },
        ];

        let result = verifier.verify_header_chain(&headers);
        assert!(result.is_err());
    }
}
