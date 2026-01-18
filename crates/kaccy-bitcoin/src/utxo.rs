//! UTXO Management Module
//!
//! This module provides utilities for managing Unspent Transaction Outputs (UTXOs)
//! including consolidation during low-fee periods and optimal input selection.

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// UTXO information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Utxo {
    /// Transaction ID
    pub txid: Txid,
    /// Output index
    pub vout: u32,
    /// Address
    pub address: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Number of confirmations
    pub confirmations: u32,
    /// Whether this is spendable
    pub spendable: bool,
    /// Whether this is safe to spend (not conflicted, not immature coinbase)
    pub safe: bool,
}

/// UTXO selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionStrategy {
    /// Largest first - minimize number of inputs
    LargestFirst,
    /// Smallest first - useful for consolidation
    SmallestFirst,
    /// Branch and bound - optimal selection to minimize fees
    BranchAndBound,
    /// First-fit - quick selection for urgent transactions
    FirstFit,
}

/// UTXO consolidation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    /// Minimum number of UTXOs to trigger consolidation
    pub min_utxo_count: usize,
    /// Maximum fee rate (sat/vB) willing to pay for consolidation
    pub max_fee_rate: f64,
    /// Target number of UTXOs after consolidation
    pub target_utxo_count: usize,
    /// Minimum UTXO value to include (in satoshis)
    pub min_utxo_value_sats: u64,
    /// Target address for consolidated output
    pub consolidation_address: Option<String>,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            min_utxo_count: 50,
            max_fee_rate: 5.0, // 5 sat/vB
            target_utxo_count: 10,
            min_utxo_value_sats: 10_000, // 10k sats minimum (dust threshold)
            consolidation_address: None,
        }
    }
}

/// UTXO selection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSelection {
    /// Selected UTXOs
    pub selected: Vec<Utxo>,
    /// Total input value in satoshis
    pub total_input_sats: u64,
    /// Estimated transaction fee in satoshis
    pub estimated_fee_sats: u64,
    /// Change amount in satoshis (if any)
    pub change_sats: u64,
}

/// UTXO manager for optimal selection and consolidation
pub struct UtxoManager {
    client: Arc<BitcoinClient>,
    config: ConsolidationConfig,
}

impl UtxoManager {
    /// Create a new UTXO manager
    pub fn new(client: Arc<BitcoinClient>, config: ConsolidationConfig) -> Self {
        Self { client, config }
    }

    /// List all UTXOs
    pub fn list_utxos(&self) -> Result<Vec<Utxo>> {
        // Use RPC to get unspent outputs
        let unspent = self.client.list_unspent(None, None, None)?;

        Ok(unspent
            .into_iter()
            .map(|u| Utxo {
                txid: u.txid,
                vout: u.vout,
                address: u
                    .address
                    .map(|a| a.assume_checked().to_string())
                    .unwrap_or_default(),
                amount_sats: u.amount.to_sat(),
                confirmations: u.confirmations,
                spendable: u.spendable,
                safe: u.safe,
            })
            .collect())
    }

    /// Select UTXOs for a given amount using specified strategy
    pub fn select_utxos(
        &self,
        target_amount_sats: u64,
        fee_rate: f64,
        strategy: SelectionStrategy,
    ) -> Result<UtxoSelection> {
        let mut utxos = self.list_utxos()?;

        // Filter to only spendable and safe UTXOs
        utxos.retain(|u| u.spendable && u.safe && u.confirmations > 0);

        if utxos.is_empty() {
            return Err(BitcoinError::InsufficientFunds(
                "No spendable UTXOs available".to_string(),
            ));
        }

        // Sort based on strategy
        match strategy {
            SelectionStrategy::LargestFirst => {
                utxos.sort_by(|a, b| b.amount_sats.cmp(&a.amount_sats));
            }
            SelectionStrategy::SmallestFirst => {
                utxos.sort_by(|a, b| a.amount_sats.cmp(&b.amount_sats));
            }
            SelectionStrategy::FirstFit => {
                // Keep original order (usually sorted by confirmations)
            }
            SelectionStrategy::BranchAndBound => {
                // For now, use largest first as approximation
                // Full B&B would be more complex
                utxos.sort_by(|a, b| b.amount_sats.cmp(&a.amount_sats));
            }
        }

        // Select UTXOs
        let mut selected = Vec::new();
        let mut total_input_sats = 0u64;

        for utxo in utxos {
            selected.push(utxo.clone());
            total_input_sats += utxo.amount_sats;

            // Estimate fee with current selection
            let estimated_fee = self.estimate_tx_fee(selected.len(), 2, fee_rate);

            // Check if we have enough
            if total_input_sats >= target_amount_sats + estimated_fee {
                let change_sats =
                    total_input_sats.saturating_sub(target_amount_sats + estimated_fee);

                return Ok(UtxoSelection {
                    selected,
                    total_input_sats,
                    estimated_fee_sats: estimated_fee,
                    change_sats,
                });
            }
        }

        Err(BitcoinError::InsufficientFunds(format!(
            "Insufficient funds: need {} sats, have {} sats",
            target_amount_sats, total_input_sats
        )))
    }

    /// Check if consolidation is recommended
    pub fn should_consolidate(&self) -> Result<bool> {
        let utxos = self.list_utxos()?;

        // Filter to consolidatable UTXOs
        let consolidatable: Vec<_> = utxos
            .into_iter()
            .filter(|u| {
                u.spendable
                    && u.safe
                    && u.confirmations > 0
                    && u.amount_sats >= self.config.min_utxo_value_sats
            })
            .collect();

        Ok(consolidatable.len() >= self.config.min_utxo_count)
    }

    /// Get consolidation recommendation
    pub fn get_consolidation_plan(&self) -> Result<Option<ConsolidationPlan>> {
        if !self.should_consolidate()? {
            return Ok(None);
        }

        // Check current fee rate
        let current_fee_rate = self
            .client
            .estimate_smart_fee(6)?
            .unwrap_or(self.config.max_fee_rate);

        if current_fee_rate > self.config.max_fee_rate {
            return Ok(None); // Fees too high for consolidation
        }

        let utxos = self.list_utxos()?;

        // Select UTXOs for consolidation (smallest first to clean up dust)
        let mut consolidatable: Vec<_> = utxos
            .into_iter()
            .filter(|u| {
                u.spendable
                    && u.safe
                    && u.confirmations > 0
                    && u.amount_sats >= self.config.min_utxo_value_sats
            })
            .collect();

        consolidatable.sort_by(|a, b| a.amount_sats.cmp(&b.amount_sats));

        if consolidatable.len() < self.config.min_utxo_count {
            return Ok(None);
        }

        // Calculate total value
        let total_value: u64 = consolidatable.iter().map(|u| u.amount_sats).sum();

        // Estimate fee for consolidation transaction
        let estimated_fee = self.estimate_tx_fee(consolidatable.len(), 1, current_fee_rate);

        if total_value <= estimated_fee {
            return Ok(None); // Would spend more in fees than the value
        }

        let output_value = total_value - estimated_fee;

        Ok(Some(ConsolidationPlan {
            utxos: consolidatable,
            estimated_fee_sats: estimated_fee,
            output_value_sats: output_value,
            recommended_fee_rate: current_fee_rate,
        }))
    }

    /// Estimate transaction fee
    fn estimate_tx_fee(&self, num_inputs: usize, num_outputs: usize, fee_rate: f64) -> u64 {
        // Approximate transaction size calculation
        // Input: ~148 vbytes for P2WPKH (SegWit)
        // Output: ~31 vbytes for P2WPKH
        // Overhead: ~10.5 vbytes
        let input_size = num_inputs as f64 * 68.0; // Witness data
        let output_size = num_outputs as f64 * 31.0;
        let overhead = 10.5;

        let total_vsize = (input_size + output_size + overhead).ceil();
        (total_vsize * fee_rate).ceil() as u64
    }
}

/// Consolidation plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationPlan {
    /// UTXOs to consolidate
    pub utxos: Vec<Utxo>,
    /// Estimated fee in satoshis
    pub estimated_fee_sats: u64,
    /// Output value after fees
    pub output_value_sats: u64,
    /// Recommended fee rate
    pub recommended_fee_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidation_config_defaults() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.min_utxo_count, 50);
        assert_eq!(config.max_fee_rate, 5.0);
        assert_eq!(config.target_utxo_count, 10);
        assert_eq!(config.min_utxo_value_sats, 10_000);
    }

    #[test]
    fn test_selection_strategy() {
        assert_eq!(
            SelectionStrategy::LargestFirst,
            SelectionStrategy::LargestFirst
        );
        assert_ne!(
            SelectionStrategy::LargestFirst,
            SelectionStrategy::SmallestFirst
        );
    }

    #[test]
    fn test_utxo_selection_result() {
        let selection = UtxoSelection {
            selected: vec![],
            total_input_sats: 100_000,
            estimated_fee_sats: 1_000,
            change_sats: 9_000,
        };

        assert_eq!(selection.total_input_sats, 100_000);
        assert_eq!(selection.estimated_fee_sats, 1_000);
        assert_eq!(selection.change_sats, 9_000);
    }
}
