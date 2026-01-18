//! Mempool monitoring and block reorganization detection
//!
//! Provides:
//! - Real-time unconfirmed transaction detection
//! - Transaction tracking through mempool
//! - Block reorganization detection and handling

use bitcoin::{BlockHash, Txid};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// Mempool transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolTransaction {
    pub txid: String,
    pub addresses: Vec<String>,
    pub amount_sats: i64,
    pub fee_sats: u64,
    pub size: u64,
    pub time: i64,
    pub first_seen: i64,
}

/// Mempool entry from Bitcoin Core
#[derive(Debug, Clone, Serialize)]
pub struct MempoolEntry {
    pub txid: String,
    pub vsize: u64,
    pub weight: u64,
    pub fee: u64,
    pub time: i64,
    pub descendant_count: u64,
    pub descendant_size: u64,
    pub descendant_fees: u64,
    pub ancestor_count: u64,
    pub ancestor_size: u64,
    pub ancestor_fees: u64,
}

/// Mempool event types
#[derive(Debug, Clone)]
pub enum MempoolEvent {
    /// New transaction detected in mempool matching watched addresses
    TransactionDetected {
        txid: String,
        address: String,
        amount_sats: i64,
        fee_sats: u64,
    },
    /// Transaction confirmed (moved from mempool to block)
    TransactionConfirmed {
        txid: String,
        block_hash: String,
        confirmations: u32,
    },
    /// Transaction removed from mempool (replaced or expired)
    TransactionRemoved { txid: String, reason: RemovalReason },
    /// Block reorganization detected
    Reorganization {
        old_tip: String,
        new_tip: String,
        depth: u32,
        affected_txids: Vec<String>,
    },
}

/// Reason for transaction removal from mempool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemovalReason {
    /// Transaction was included in a block
    Confirmed,
    /// Transaction was replaced (RBF)
    Replaced { replacement_txid: String },
    /// Transaction expired from mempool
    Expired,
    /// Transaction was double-spent
    DoubleSpent,
    /// Unknown reason
    Unknown,
}

/// Configuration for mempool monitor
#[derive(Debug, Clone)]
pub struct MempoolMonitorConfig {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Maximum transactions to track
    pub max_tracked_transactions: usize,
    /// Enable block reorganization detection
    pub detect_reorgs: bool,
    /// Minimum reorg depth to report
    pub min_reorg_depth: u32,
}

impl Default for MempoolMonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 10,
            max_tracked_transactions: 10000,
            detect_reorgs: true,
            min_reorg_depth: 1,
        }
    }
}

/// Block chain tip tracker for reorg detection
#[derive(Debug, Clone)]
struct ChainTip {
    hash: BlockHash,
    height: u64,
    #[allow(dead_code)]
    time: i64,
}

/// Mempool monitor for tracking unconfirmed transactions
pub struct MempoolMonitor {
    client: Arc<BitcoinClient>,
    config: MempoolMonitorConfig,
    /// Addresses being watched
    watched_addresses: Arc<RwLock<HashSet<String>>>,
    /// Transactions we're tracking (txid -> first seen time)
    tracked_transactions: Arc<RwLock<HashMap<String, i64>>>,
    /// Last known mempool txids (for detecting removed transactions)
    #[allow(dead_code)]
    last_mempool_txids: Arc<RwLock<HashSet<String>>>,
    /// Last known chain tip
    last_chain_tip: Arc<RwLock<Option<ChainTip>>>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<MempoolEvent>,
}

impl MempoolMonitor {
    /// Create a new mempool monitor
    pub fn new(client: Arc<BitcoinClient>, config: MempoolMonitorConfig) -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        Self {
            client,
            config,
            watched_addresses: Arc::new(RwLock::new(HashSet::new())),
            tracked_transactions: Arc::new(RwLock::new(HashMap::new())),
            last_mempool_txids: Arc::new(RwLock::new(HashSet::new())),
            last_chain_tip: Arc::new(RwLock::new(None)),
            event_tx,
        }
    }

    /// Subscribe to mempool events
    pub fn subscribe(&self) -> broadcast::Receiver<MempoolEvent> {
        self.event_tx.subscribe()
    }

    /// Add an address to watch
    pub async fn watch_address(&self, address: &str) {
        let mut addresses = self.watched_addresses.write().await;
        addresses.insert(address.to_string());
        tracing::debug!(
            address = address,
            "Watching address for mempool transactions"
        );
    }

    /// Remove an address from watch list
    pub async fn unwatch_address(&self, address: &str) {
        let mut addresses = self.watched_addresses.write().await;
        addresses.remove(address);
    }

    /// Track a specific transaction
    pub async fn track_transaction(&self, txid: &str) {
        let mut tracked = self.tracked_transactions.write().await;
        if tracked.len() < self.config.max_tracked_transactions {
            tracked.insert(txid.to_string(), chrono::Utc::now().timestamp());
            tracing::debug!(txid = txid, "Tracking transaction");
        }
    }

    /// Stop tracking a transaction
    pub async fn untrack_transaction(&self, txid: &str) {
        let mut tracked = self.tracked_transactions.write().await;
        tracked.remove(txid);
    }

    /// Get current mempool statistics
    pub fn get_mempool_stats(&self) -> Result<MempoolStats> {
        let info = self.client.get_mempool_info()?;

        Ok(MempoolStats {
            size: info.size as u64,
            bytes: info.bytes as u64,
            usage: info.usage as u64,
            max_mempool: info.max_mempool as u64,
            mempool_min_fee: info.mempool_min_fee.to_sat() as f64 / 1000.0,
            min_relay_fee: info.min_relay_tx_fee.to_sat() as f64 / 1000.0,
        })
    }

    /// Check if a transaction is in the mempool
    pub fn is_in_mempool(&self, txid: &str) -> Result<bool> {
        let txid_parsed: Txid = txid
            .parse()
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Invalid txid: {}", e)))?;

        // Try to get raw mempool entry
        match self.client.get_raw_transaction(&txid_parsed) {
            Ok(tx_info) => Ok(tx_info.confirmations.is_none() || tx_info.confirmations == Some(0)),
            Err(BitcoinError::Rpc(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Poll mempool for changes
    pub async fn poll(&self) -> Result<Vec<MempoolEvent>> {
        let mut events = Vec::new();

        // Check for chain reorgs first
        if self.config.detect_reorgs {
            if let Some(reorg_event) = self.check_for_reorg().await? {
                events.push(reorg_event);
            }
        }

        // Get watched addresses
        let watched = self.watched_addresses.read().await.clone();
        if watched.is_empty() {
            return Ok(events);
        }

        // Check for new transactions to watched addresses
        // Use list_since_block with 0 confirmations to get mempool transactions
        let blockchain_info = self.client.get_blockchain_info()?;
        let since_result = self
            .client
            .list_since_block(Some(&blockchain_info.best_block_hash), Some(0))?;

        for tx in since_result.transactions {
            if let Some(ref addr) = tx.address {
                if watched.contains(addr) && tx.confirmations == 0 {
                    // New unconfirmed transaction to watched address
                    let event = MempoolEvent::TransactionDetected {
                        txid: tx.txid.to_string(),
                        address: addr.clone(),
                        amount_sats: tx.amount,
                        fee_sats: 0, // Would need additional lookup
                    };
                    events.push(event.clone());

                    // Track this transaction
                    self.track_transaction(&tx.txid.to_string()).await;
                }
            }
        }

        // Broadcast events
        for event in &events {
            let _ = self.event_tx.send(event.clone());
        }

        Ok(events)
    }

    /// Check for block reorganization
    async fn check_for_reorg(&self) -> Result<Option<MempoolEvent>> {
        let blockchain_info = self.client.get_blockchain_info()?;
        let current_tip = ChainTip {
            hash: blockchain_info.best_block_hash,
            height: blockchain_info.blocks,
            time: chrono::Utc::now().timestamp(),
        };

        let mut last_tip_guard = self.last_chain_tip.write().await;

        // Take the previous tip to avoid borrow conflicts
        let previous_tip = last_tip_guard.take();

        let result = if let Some(prev) = previous_tip {
            // Check if we're on a different chain
            if prev.hash != current_tip.hash && current_tip.height <= prev.height {
                let depth = (prev.height - current_tip.height + 1) as u32;

                if depth >= self.config.min_reorg_depth {
                    // Get affected transactions from our tracked list
                    let tracked = self.tracked_transactions.read().await;
                    let affected_txids: Vec<String> = tracked.keys().cloned().collect();

                    tracing::warn!(
                        old_tip = %prev.hash,
                        new_tip = %current_tip.hash,
                        depth = depth,
                        "Block reorganization detected"
                    );

                    Some(MempoolEvent::Reorganization {
                        old_tip: prev.hash.to_string(),
                        new_tip: blockchain_info.best_block_hash.to_string(),
                        depth,
                        affected_txids,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        *last_tip_guard = Some(current_tip);
        Ok(result)
    }

    /// Start the background monitoring task
    pub async fn run(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let poll_interval = std::time::Duration::from_secs(self.config.poll_interval_secs);

        tracing::info!(
            poll_interval_secs = self.config.poll_interval_secs,
            "Mempool monitor started"
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {
                    if let Err(e) = self.poll().await {
                        tracing::warn!(error = %e, "Mempool poll failed");
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("Mempool monitor shutting down");
                        break;
                    }
                }
            }
        }
    }
}

/// Mempool statistics
#[derive(Debug, Clone, Serialize)]
pub struct MempoolStats {
    /// Number of transactions in mempool
    pub size: u64,
    /// Total size in bytes
    pub bytes: u64,
    /// Memory usage
    pub usage: u64,
    /// Maximum mempool size
    pub max_mempool: u64,
    /// Minimum fee rate (sat/vB)
    pub mempool_min_fee: f64,
    /// Minimum relay fee (sat/vB)
    pub min_relay_fee: f64,
}

/// Block reorganization tracker
pub struct ReorgTracker {
    /// Recent block hashes by height
    block_history: HashMap<u64, BlockHash>,
    /// Maximum history depth
    max_depth: usize,
    /// Confirmed transactions that might be affected by reorg
    confirmed_transactions: HashMap<String, u64>, // txid -> confirmed_height
}

impl ReorgTracker {
    /// Create a new reorg tracker
    pub fn new(max_depth: usize) -> Self {
        Self {
            block_history: HashMap::new(),
            max_depth,
            confirmed_transactions: HashMap::new(),
        }
    }

    /// Record a block at a specific height
    pub fn record_block(&mut self, height: u64, hash: BlockHash) {
        self.block_history.insert(height, hash);

        // Prune old blocks beyond max_depth
        let min_height = height.saturating_sub(self.max_depth as u64);
        self.block_history.retain(|h, _| *h >= min_height);
    }

    /// Record a confirmed transaction
    pub fn record_confirmation(&mut self, txid: &str, height: u64) {
        self.confirmed_transactions.insert(txid.to_string(), height);

        // Prune old confirmations
        let min_height = height.saturating_sub(self.max_depth as u64);
        self.confirmed_transactions.retain(|_, h| *h >= min_height);
    }

    /// Check if a block hash matches our recorded hash for that height
    pub fn verify_block(&self, height: u64, hash: &BlockHash) -> bool {
        self.block_history.get(&height).is_none_or(|h| h == hash)
    }

    /// Get transactions that would be affected by a reorg to a specific height
    pub fn get_affected_transactions(&self, reorg_height: u64) -> Vec<String> {
        self.confirmed_transactions
            .iter()
            .filter(|(_, h)| **h >= reorg_height)
            .map(|(txid, _)| txid.clone())
            .collect()
    }

    /// Detect if a reorganization occurred
    pub fn detect_reorg(
        &self,
        client: &BitcoinClient,
        current_height: u64,
    ) -> Result<Option<ReorgInfo>> {
        // Check our recorded blocks against the chain
        for (height, expected_hash) in &self.block_history {
            if *height <= current_height {
                let actual_hash = client.get_block_hash(*height)?;
                if actual_hash != *expected_hash {
                    // Found a divergence point
                    let depth = (current_height - height + 1) as u32;
                    let affected = self.get_affected_transactions(*height);

                    return Ok(Some(ReorgInfo {
                        divergence_height: *height,
                        depth,
                        expected_hash: expected_hash.to_string(),
                        actual_hash: actual_hash.to_string(),
                        affected_transactions: affected,
                    }));
                }
            }
        }

        Ok(None)
    }
}

/// Reorganization information
#[derive(Debug, Clone, Serialize)]
pub struct ReorgInfo {
    /// Height where chain diverged
    pub divergence_height: u64,
    /// Depth of reorganization
    pub depth: u32,
    /// Expected block hash at divergence
    pub expected_hash: String,
    /// Actual block hash at divergence
    pub actual_hash: String,
    /// Transactions that may have been unconfirmed
    pub affected_transactions: Vec<String>,
}

/// Unconfirmed transaction watcher for specific addresses
pub struct AddressWatcher {
    client: Arc<BitcoinClient>,
    addresses: HashSet<String>,
    /// Last known unconfirmed amounts by address
    last_unconfirmed: HashMap<String, u64>,
}

impl AddressWatcher {
    /// Create a new address watcher
    pub fn new(client: Arc<BitcoinClient>) -> Self {
        Self {
            client,
            addresses: HashSet::new(),
            last_unconfirmed: HashMap::new(),
        }
    }

    /// Add an address to watch
    pub fn watch(&mut self, address: &str) {
        self.addresses.insert(address.to_string());
    }

    /// Remove an address from watch
    pub fn unwatch(&mut self, address: &str) {
        self.addresses.remove(address);
        self.last_unconfirmed.remove(address);
    }

    /// Check for new unconfirmed payments to watched addresses
    pub fn check_unconfirmed(&mut self) -> Result<Vec<UnconfirmedPayment>> {
        let mut new_payments = Vec::new();

        for address in &self.addresses {
            let addr: bitcoin::Address<bitcoin::address::NetworkUnchecked> = address
                .parse()
                .map_err(|e| BitcoinError::InvalidAddress(format!("{:?}", e)))?;

            let checked_addr = addr.assume_checked();

            // Get unconfirmed amount (0 confirmations)
            let unconfirmed = self
                .client
                .get_received_by_address(&checked_addr, Some(0))?
                .to_sat();

            // Get confirmed amount
            let confirmed = self
                .client
                .get_received_by_address(&checked_addr, Some(1))?
                .to_sat();

            // Unconfirmed balance is total minus confirmed
            let unconfirmed_balance = unconfirmed.saturating_sub(confirmed);

            // Check if this is new unconfirmed balance
            let last = self.last_unconfirmed.get(address).copied().unwrap_or(0);
            if unconfirmed_balance > last {
                let new_amount = unconfirmed_balance - last;
                new_payments.push(UnconfirmedPayment {
                    address: address.clone(),
                    amount_sats: new_amount,
                    detected_at: chrono::Utc::now().timestamp(),
                });

                tracing::info!(
                    address = address,
                    amount_sats = new_amount,
                    "New unconfirmed payment detected"
                );
            }

            self.last_unconfirmed
                .insert(address.clone(), unconfirmed_balance);
        }

        Ok(new_payments)
    }

    /// Get list of watched addresses
    pub fn watched_addresses(&self) -> Vec<String> {
        self.addresses.iter().cloned().collect()
    }
}

/// Unconfirmed payment notification
#[derive(Debug, Clone, Serialize)]
pub struct UnconfirmedPayment {
    pub address: String,
    pub amount_sats: u64,
    pub detected_at: i64,
}
