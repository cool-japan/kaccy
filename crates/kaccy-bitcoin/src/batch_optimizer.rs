//! Batch Transaction Optimizer
//!
//! This module provides utilities for optimizing batch transactions to minimize
//! fees and improve efficiency.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{BitcoinError, Result};

/// Batch withdrawal request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWithdrawal {
    /// User ID
    pub user_id: String,
    /// Recipient address
    pub address: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Priority (higher = more important)
    pub priority: u8,
}

/// Batch optimization strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStrategy {
    /// Minimize transaction count
    MinimizeTransactions,
    /// Minimize total fees
    MinimizeFees,
    /// Balance between count and fees
    Balanced,
    /// Group by priority
    Priority,
}

/// Optimized batch result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedBatch {
    /// Batches of withdrawals
    pub batches: Vec<Vec<BatchWithdrawal>>,
    /// Estimated total fees in satoshis
    pub estimated_total_fees: u64,
    /// Estimated fee savings compared to individual transactions
    pub estimated_savings: u64,
    /// Number of transactions required
    pub transaction_count: usize,
}

/// Batch optimizer
///
/// Optimizes batch withdrawals to minimize fees and transaction count.
///
/// # Examples
///
/// ```
/// use kaccy_bitcoin::{BatchOptimizer, BatchWithdrawal, BatchStrategy};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let optimizer = BatchOptimizer::default();
///
/// let withdrawals = vec![
///     BatchWithdrawal {
///         user_id: "user1".to_string(),
///         address: "bc1qaddr1".to_string(),
///         amount_sats: 50_000,
///         priority: 1,
///     },
///     BatchWithdrawal {
///         user_id: "user2".to_string(),
///         address: "bc1qaddr2".to_string(),
///         amount_sats: 75_000,
///         priority: 1,
///     },
/// ];
///
/// let optimized = optimizer.optimize(withdrawals, BatchStrategy::MinimizeFees)?;
/// println!("Fee savings: {} sats", optimized.estimated_savings);
/// # Ok(())
/// # }
/// ```
pub struct BatchOptimizer {
    /// Maximum outputs per transaction
    max_outputs_per_tx: usize,
    /// Minimum batch size
    min_batch_size: usize,
    /// Fee rate in sat/vB
    fee_rate: f64,
}

impl Default for BatchOptimizer {
    fn default() -> Self {
        Self::new(100, 2, 10.0)
    }
}

impl BatchOptimizer {
    /// Create a new batch optimizer
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_bitcoin::BatchOptimizer;
    ///
    /// let optimizer = BatchOptimizer::new(
    ///     100,  // max outputs per tx
    ///     2,    // min batch size
    ///     10.0, // fee rate sat/vB
    /// );
    /// ```
    pub fn new(max_outputs_per_tx: usize, min_batch_size: usize, fee_rate: f64) -> Self {
        Self {
            max_outputs_per_tx,
            min_batch_size,
            fee_rate,
        }
    }

    /// Optimize a batch of withdrawals
    pub fn optimize(
        &self,
        mut withdrawals: Vec<BatchWithdrawal>,
        strategy: BatchStrategy,
    ) -> Result<OptimizedBatch> {
        if withdrawals.is_empty() {
            return Err(BitcoinError::InvalidTransaction(
                "No withdrawals to batch".to_string(),
            ));
        }

        // Sort based on strategy
        match strategy {
            BatchStrategy::MinimizeTransactions => {
                // No specific sorting needed
            }
            BatchStrategy::MinimizeFees => {
                // Group similar amounts together for better UTXO selection
                withdrawals.sort_by_key(|w| w.amount_sats);
            }
            BatchStrategy::Balanced => {
                // Sort by priority then amount
                withdrawals.sort_by(|a, b| {
                    b.priority
                        .cmp(&a.priority)
                        .then(a.amount_sats.cmp(&b.amount_sats))
                });
            }
            BatchStrategy::Priority => {
                // Sort by priority only
                withdrawals.sort_by_key(|w| std::cmp::Reverse(w.priority));
            }
        }

        let mut batches = Vec::new();
        let mut current_batch = Vec::new();

        for withdrawal in withdrawals {
            current_batch.push(withdrawal);

            // Create a batch when we reach max outputs
            if current_batch.len() >= self.max_outputs_per_tx {
                batches.push(current_batch.clone());
                current_batch.clear();
            }
        }

        // Add remaining withdrawals as final batch
        if !current_batch.is_empty() {
            // Check if the final batch meets minimum size, otherwise merge with previous
            if current_batch.len() < self.min_batch_size && !batches.is_empty() {
                if let Some(last_batch) = batches.last_mut() {
                    last_batch.extend(current_batch);
                }
            } else {
                batches.push(current_batch);
            }
        }

        // Calculate fees
        let estimated_total_fees = self.estimate_batch_fees(&batches);
        let individual_fees = self.estimate_individual_fees(&batches);
        let estimated_savings = individual_fees.saturating_sub(estimated_total_fees);

        Ok(OptimizedBatch {
            transaction_count: batches.len(),
            batches,
            estimated_total_fees,
            estimated_savings,
        })
    }

    /// Estimate fees for batched transactions
    fn estimate_batch_fees(&self, batches: &[Vec<BatchWithdrawal>]) -> u64 {
        batches
            .iter()
            .map(|batch| self.estimate_transaction_fee(batch.len(), 2))
            .sum()
    }

    /// Estimate fees if all transactions were individual
    fn estimate_individual_fees(&self, batches: &[Vec<BatchWithdrawal>]) -> u64 {
        let total_withdrawals: usize = batches.iter().map(|b| b.len()).sum();
        total_withdrawals as u64 * self.estimate_transaction_fee(1, 2)
    }

    /// Estimate fee for a single transaction
    fn estimate_transaction_fee(&self, num_outputs: usize, num_inputs: usize) -> u64 {
        // Approximate transaction size calculation
        // Input: ~148 vbytes for P2WPKH (SegWit)
        // Output: ~31 vbytes for P2WPKH
        // Overhead: ~10.5 vbytes
        let input_size = num_inputs as f64 * 68.0; // Witness data
        let output_size = num_outputs as f64 * 31.0;
        let overhead = 10.5;

        let total_vsize = (input_size + output_size + overhead).ceil();
        (total_vsize * self.fee_rate).ceil() as u64
    }

    /// Group withdrawals by user
    pub fn group_by_user(
        withdrawals: Vec<BatchWithdrawal>,
    ) -> HashMap<String, Vec<BatchWithdrawal>> {
        let mut groups: HashMap<String, Vec<BatchWithdrawal>> = HashMap::new();

        for withdrawal in withdrawals {
            groups
                .entry(withdrawal.user_id.clone())
                .or_default()
                .push(withdrawal);
        }

        groups
    }

    /// Analyze batch efficiency
    pub fn analyze_efficiency(&self, batches: &OptimizedBatch) -> BatchEfficiency {
        let avg_batch_size = if batches.batches.is_empty() {
            0.0
        } else {
            batches.batches.iter().map(|b| b.len()).sum::<usize>() as f64
                / batches.batches.len() as f64
        };

        let fee_per_withdrawal = if batches.batches.iter().map(|b| b.len()).sum::<usize>() == 0 {
            0
        } else {
            batches.estimated_total_fees
                / batches.batches.iter().map(|b| b.len()).sum::<usize>() as u64
        };

        let savings_percentage = if batches.estimated_total_fees > 0 {
            (batches.estimated_savings as f64
                / (batches.estimated_total_fees + batches.estimated_savings) as f64
                * 100.0) as u32
        } else {
            0
        };

        BatchEfficiency {
            avg_batch_size,
            fee_per_withdrawal,
            total_withdrawals: batches.batches.iter().map(|b| b.len()).sum(),
            savings_percentage,
        }
    }
}

/// Batch efficiency metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEfficiency {
    /// Average batch size
    pub avg_batch_size: f64,
    /// Fee per withdrawal in satoshis
    pub fee_per_withdrawal: u64,
    /// Total number of withdrawals
    pub total_withdrawals: usize,
    /// Savings percentage compared to individual transactions
    pub savings_percentage: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_optimizer_creation() {
        let optimizer = BatchOptimizer::default();
        assert_eq!(optimizer.max_outputs_per_tx, 100);
        assert_eq!(optimizer.min_batch_size, 2);
    }

    #[test]
    fn test_batch_withdrawal_grouping() {
        let withdrawals = vec![
            BatchWithdrawal {
                user_id: "user1".to_string(),
                address: "addr1".to_string(),
                amount_sats: 100_000,
                priority: 5,
            },
            BatchWithdrawal {
                user_id: "user1".to_string(),
                address: "addr2".to_string(),
                amount_sats: 200_000,
                priority: 5,
            },
            BatchWithdrawal {
                user_id: "user2".to_string(),
                address: "addr3".to_string(),
                amount_sats: 150_000,
                priority: 3,
            },
        ];

        let groups = BatchOptimizer::group_by_user(withdrawals);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get("user1").unwrap().len(), 2);
        assert_eq!(groups.get("user2").unwrap().len(), 1);
    }

    #[test]
    fn test_batch_optimization() {
        let optimizer = BatchOptimizer::new(3, 2, 10.0);

        let withdrawals = vec![
            BatchWithdrawal {
                user_id: "user1".to_string(),
                address: "addr1".to_string(),
                amount_sats: 100_000,
                priority: 5,
            },
            BatchWithdrawal {
                user_id: "user2".to_string(),
                address: "addr2".to_string(),
                amount_sats: 200_000,
                priority: 3,
            },
            BatchWithdrawal {
                user_id: "user3".to_string(),
                address: "addr3".to_string(),
                amount_sats: 150_000,
                priority: 4,
            },
        ];

        let result = optimizer.optimize(withdrawals, BatchStrategy::MinimizeFees);
        assert!(result.is_ok());

        let batch = result.unwrap();
        assert_eq!(batch.transaction_count, 1);
        assert!(batch.estimated_savings > 0);
    }

    #[test]
    fn test_efficiency_analysis() {
        let optimizer = BatchOptimizer::default();

        let batch = OptimizedBatch {
            batches: vec![vec![
                BatchWithdrawal {
                    user_id: "user1".to_string(),
                    address: "addr1".to_string(),
                    amount_sats: 100_000,
                    priority: 5,
                },
                BatchWithdrawal {
                    user_id: "user2".to_string(),
                    address: "addr2".to_string(),
                    amount_sats: 200_000,
                    priority: 3,
                },
            ]],
            estimated_total_fees: 5_000,
            estimated_savings: 3_000,
            transaction_count: 1,
        };

        let efficiency = optimizer.analyze_efficiency(&batch);
        assert_eq!(efficiency.total_withdrawals, 2);
        assert_eq!(efficiency.avg_batch_size, 2.0);
        assert!(efficiency.savings_percentage > 0);
    }
}
