//! Replace-By-Fee (RBF) handling
//!
//! This module provides utilities for detecting and handling Bitcoin
//! transactions that have been replaced via RBF (BIP 125).

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};
use crate::tx_parser::{ParsedTransaction, TransactionParser};

/// RBF transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RbfStatus {
    /// Transaction does not signal RBF
    NonReplaceable,
    /// Transaction signals RBF and can be replaced
    Replaceable,
    /// Transaction has been replaced by another
    Replaced,
    /// Transaction is confirmed and no longer replaceable
    Confirmed,
}

/// Information about an RBF replacement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbfReplacement {
    /// Original transaction ID
    pub original_txid: String,
    /// Replacement transaction ID
    pub replacement_txid: String,
    /// Old fee in satoshis
    pub old_fee_sats: Option<u64>,
    /// New fee in satoshis
    pub new_fee_sats: Option<u64>,
    /// Fee increase in satoshis
    pub fee_increase_sats: Option<u64>,
    /// Timestamp when replacement was detected
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// Tracked transaction for RBF monitoring
#[derive(Debug, Clone)]
pub struct TrackedRbfTransaction {
    /// Transaction ID
    pub txid: String,
    /// Input outpoints (txid:vout) for conflict detection
    pub input_outpoints: Vec<String>,
    /// RBF status
    pub status: RbfStatus,
    /// Fee in satoshis
    pub fee_sats: Option<u64>,
    /// Order ID associated with this transaction
    pub order_id: Option<String>,
    /// When the transaction was first seen
    pub first_seen: chrono::DateTime<chrono::Utc>,
    /// When status last changed
    pub status_changed: chrono::DateTime<chrono::Utc>,
}

/// Events emitted by the RBF tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RbfEvent {
    /// A transaction was replaced
    TransactionReplaced(RbfReplacement),
    /// A tracked transaction was confirmed
    TransactionConfirmed {
        /// Transaction ID that was confirmed
        txid: String,
        /// Number of confirmations
        confirmations: u32,
    },
    /// A tracked transaction was dropped from mempool (not replaced)
    TransactionDropped {
        /// Transaction ID that was dropped
        txid: String,
        /// Reason for the drop
        reason: String,
    },
}

/// RBF tracker for monitoring transaction replacements
pub struct RbfTracker {
    client: Arc<BitcoinClient>,
    parser: TransactionParser,
    /// Tracked transactions by txid
    tracked: Arc<RwLock<HashMap<String, TrackedRbfTransaction>>>,
    /// Index of outpoints to txids for conflict detection
    outpoint_index: Arc<RwLock<HashMap<String, String>>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<RbfEvent>,
}

impl RbfTracker {
    /// Create a new RBF tracker
    pub fn new(client: Arc<BitcoinClient>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            parser: TransactionParser::new(client.clone()),
            client,
            tracked: Arc::new(RwLock::new(HashMap::new())),
            outpoint_index: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to RBF events
    pub fn subscribe(&self) -> broadcast::Receiver<RbfEvent> {
        self.event_tx.subscribe()
    }

    /// Track a transaction for RBF
    pub async fn track_transaction(
        &self,
        txid: &Txid,
        order_id: Option<String>,
    ) -> Result<TrackedRbfTransaction> {
        let parsed = self.parser.parse_transaction(txid)?;

        let status = if parsed.confirmations > 0 {
            RbfStatus::Confirmed
        } else if parsed.is_rbf {
            RbfStatus::Replaceable
        } else {
            RbfStatus::NonReplaceable
        };

        // Build list of input outpoints
        let input_outpoints: Vec<String> = parsed
            .inputs
            .iter()
            .map(|input| format!("{}:{}", input.prev_txid, input.prev_vout))
            .collect();

        let now = chrono::Utc::now();
        let tracked = TrackedRbfTransaction {
            txid: txid.to_string(),
            input_outpoints: input_outpoints.clone(),
            status,
            fee_sats: parsed.fee_sats,
            order_id,
            first_seen: now,
            status_changed: now,
        };

        // Add to tracking
        {
            let mut tracked_map = self.tracked.write().await;
            tracked_map.insert(txid.to_string(), tracked.clone());
        }

        // Index outpoints for conflict detection
        {
            let mut index = self.outpoint_index.write().await;
            for outpoint in input_outpoints {
                index.insert(outpoint, txid.to_string());
            }
        }

        tracing::info!(
            txid = %txid,
            status = ?status,
            is_rbf = parsed.is_rbf,
            "Tracking transaction for RBF"
        );

        Ok(tracked)
    }

    /// Check if a transaction has been replaced
    pub async fn check_replacement(&self, txid: &Txid) -> Result<Option<RbfReplacement>> {
        let txid_str = txid.to_string();

        // Get tracked transaction
        let tracked = {
            let tracked_map = self.tracked.read().await;
            tracked_map.get(&txid_str).cloned()
        };

        let tracked = match tracked {
            Some(t) => t,
            None => return Ok(None),
        };

        // If already confirmed or replaced, return cached status
        if matches!(tracked.status, RbfStatus::Confirmed | RbfStatus::Replaced) {
            return Ok(None);
        }

        // Try to get the transaction from the node
        match self.client.get_raw_transaction(txid) {
            Ok(raw_tx) => {
                // Transaction still exists
                if raw_tx.confirmations.unwrap_or(0) > 0 {
                    // Transaction confirmed
                    self.update_status(&txid_str, RbfStatus::Confirmed).await;

                    let _ = self.event_tx.send(RbfEvent::TransactionConfirmed {
                        txid: txid_str,
                        confirmations: raw_tx.confirmations.unwrap_or(0),
                    });
                }
                Ok(None)
            }
            Err(BitcoinError::Rpc(_)) => {
                // Transaction not found - might be replaced or dropped
                self.detect_replacement(&tracked).await
            }
            Err(e) => Err(e),
        }
    }

    /// Detect if a transaction was replaced by checking for conflicting transactions
    async fn detect_replacement(
        &self,
        tracked: &TrackedRbfTransaction,
    ) -> Result<Option<RbfReplacement>> {
        // Look for transactions spending the same inputs
        for _outpoint in &tracked.input_outpoints {
            // Check mempool for conflicts
            // This requires iterating through recent transactions
            // In practice, this would be done via ZMQ notifications or mempool scanning
        }

        // For now, mark as dropped if we can't find it
        self.update_status(&tracked.txid, RbfStatus::Replaced).await;

        let _ = self.event_tx.send(RbfEvent::TransactionDropped {
            txid: tracked.txid.clone(),
            reason: "Transaction no longer in mempool".to_string(),
        });

        Ok(None)
    }

    /// Update transaction status
    async fn update_status(&self, txid: &str, new_status: RbfStatus) {
        let mut tracked_map = self.tracked.write().await;
        if let Some(tracked) = tracked_map.get_mut(txid) {
            tracked.status = new_status;
            tracked.status_changed = chrono::Utc::now();
        }
    }

    /// Get status of a tracked transaction
    pub async fn get_status(&self, txid: &str) -> Option<RbfStatus> {
        let tracked_map = self.tracked.read().await;
        tracked_map.get(txid).map(|t| t.status)
    }

    /// Remove a transaction from tracking
    pub async fn untrack(&self, txid: &str) {
        let tracked = {
            let mut tracked_map = self.tracked.write().await;
            tracked_map.remove(txid)
        };

        if let Some(tracked) = tracked {
            let mut index = self.outpoint_index.write().await;
            for outpoint in tracked.input_outpoints {
                index.remove(&outpoint);
            }
        }
    }

    /// Check all tracked transactions for updates
    pub async fn check_all(&self) -> Vec<RbfEvent> {
        let txids: Vec<String> = {
            let tracked_map = self.tracked.read().await;
            tracked_map.keys().cloned().collect()
        };

        let mut events = Vec::new();

        for txid_str in txids {
            if let Ok(txid) = txid_str.parse::<Txid>() {
                if let Ok(Some(replacement)) = self.check_replacement(&txid).await {
                    events.push(RbfEvent::TransactionReplaced(replacement));
                }
            }
        }

        events
    }

    /// Check if a new transaction conflicts with any tracked transactions
    pub async fn check_conflict(&self, new_tx: &ParsedTransaction) -> Option<String> {
        let index = self.outpoint_index.read().await;

        for input in &new_tx.inputs {
            let outpoint = format!("{}:{}", input.prev_txid, input.prev_vout);
            if let Some(existing_txid) = index.get(&outpoint) {
                // Found a conflict
                if existing_txid != &new_tx.txid {
                    return Some(existing_txid.clone());
                }
            }
        }

        None
    }

    /// Handle a potential RBF replacement
    pub async fn handle_replacement(
        &self,
        original_txid: &str,
        replacement_txid: &Txid,
    ) -> Result<RbfReplacement> {
        let tracked = {
            let tracked_map = self.tracked.read().await;
            tracked_map.get(original_txid).cloned()
        };

        let tracked =
            tracked.ok_or_else(|| BitcoinError::TransactionNotFound(original_txid.to_string()))?;

        // Parse the replacement transaction
        let replacement = self.parser.parse_transaction(replacement_txid)?;

        let fee_increase = match (tracked.fee_sats, replacement.fee_sats) {
            (Some(old), Some(new)) => Some(new.saturating_sub(old)),
            _ => None,
        };

        let rbf_replacement = RbfReplacement {
            original_txid: original_txid.to_string(),
            replacement_txid: replacement_txid.to_string(),
            old_fee_sats: tracked.fee_sats,
            new_fee_sats: replacement.fee_sats,
            fee_increase_sats: fee_increase,
            detected_at: chrono::Utc::now(),
        };

        // Update status
        self.update_status(original_txid, RbfStatus::Replaced).await;

        // Track the replacement
        self.track_transaction(replacement_txid, tracked.order_id.clone())
            .await?;

        // Emit event
        let _ = self
            .event_tx
            .send(RbfEvent::TransactionReplaced(rbf_replacement.clone()));

        tracing::warn!(
            original_txid = original_txid,
            replacement_txid = %replacement_txid,
            fee_increase = ?fee_increase,
            "Transaction replaced via RBF"
        );

        Ok(rbf_replacement)
    }
}

/// RBF configuration
#[derive(Debug, Clone)]
pub struct RbfConfig {
    /// Minimum fee bump required (in percent)
    pub min_fee_bump_percent: u32,
    /// Check interval in seconds
    pub check_interval_secs: u64,
    /// Maximum age to track (in seconds)
    pub max_track_age_secs: u64,
}

impl Default for RbfConfig {
    fn default() -> Self {
        Self {
            min_fee_bump_percent: 10,
            check_interval_secs: 60,
            max_track_age_secs: 86400 * 7, // 7 days
        }
    }
}

/// Builder for creating RBF-enabled replacement transactions
pub struct RbfTransactionBuilder {
    /// Original transaction details
    #[allow(dead_code)]
    original: Option<ParsedTransaction>,
    /// New outputs (can be modified)
    #[allow(dead_code)]
    outputs: Vec<(String, u64)>,
    /// Target fee rate in sat/vB
    target_fee_rate: f64,
}

impl RbfTransactionBuilder {
    /// Create a new RBF transaction builder
    pub fn new() -> Self {
        Self {
            original: None,
            outputs: Vec::new(),
            target_fee_rate: 10.0,
        }
    }

    /// Set the original transaction to replace
    pub fn replace(mut self, tx: ParsedTransaction) -> Self {
        self.original = Some(tx);
        self
    }

    /// Set target fee rate
    pub fn fee_rate(mut self, sat_per_vb: f64) -> Self {
        self.target_fee_rate = sat_per_vb;
        self
    }

    /// Add output
    pub fn add_output(mut self, address: String, amount_sats: u64) -> Self {
        self.outputs.push((address, amount_sats));
        self
    }

    /// Calculate required fee for replacement
    pub fn calculate_replacement_fee(&self) -> Option<u64> {
        let original = self.original.as_ref()?;
        let original_fee = original.fee_sats?;

        // BIP 125 requires the replacement to pay for its own bandwidth
        // plus the original transaction's fee
        let min_fee = original_fee + original.vsize; // +1 sat/vB for bandwidth

        // Also ensure we meet our target fee rate
        let target_fee = (self.target_fee_rate * original.vsize as f64) as u64;

        Some(std::cmp::max(min_fee, target_fee))
    }
}

impl Default for RbfTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rbf_config_defaults() {
        let config = RbfConfig::default();
        assert_eq!(config.min_fee_bump_percent, 10);
        assert_eq!(config.check_interval_secs, 60);
    }

    #[test]
    fn test_rbf_status() {
        assert_ne!(RbfStatus::Replaceable, RbfStatus::Confirmed);
        assert_ne!(RbfStatus::NonReplaceable, RbfStatus::Replaced);
    }
}
