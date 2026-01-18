//! Adaptive fee strategies for Bitcoin transactions
//!
//! Provides intelligent fee management based on user preferences, time constraints,
//! and market conditions.

use crate::client::BitcoinClient;
use crate::error::BitcoinError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// User's urgency preference for transaction confirmation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionUrgency {
    /// Low priority - can wait hours or days
    Low,
    /// Medium priority - should confirm within a few hours
    Medium,
    /// High priority - should confirm within 1-2 blocks
    High,
    /// Critical priority - must confirm ASAP
    Critical,
}

impl TransactionUrgency {
    /// Get target confirmation blocks for this urgency level
    pub fn target_blocks(&self) -> u32 {
        match self {
            Self::Low => 144,    // ~1 day
            Self::Medium => 6,   // ~1 hour
            Self::High => 2,     // ~20 minutes
            Self::Critical => 1, // Next block
        }
    }

    /// Get multiplier for base fee rate
    pub fn fee_multiplier(&self) -> f64 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 1.5,
            Self::High => 2.0,
            Self::Critical => 3.0,
        }
    }
}

/// Time-based fee adjustment strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedStrategy {
    /// Target confirmation time (UTC)
    pub target_time: DateTime<Utc>,
    /// Minimum acceptable fee rate (sat/vB)
    pub min_fee_rate: f64,
    /// Maximum acceptable fee rate (sat/vB)
    pub max_fee_rate: f64,
    /// Allow fee bumping if needed
    pub allow_bumping: bool,
}

impl TimeBasedStrategy {
    /// Create a new time-based strategy
    pub fn new(target_time: DateTime<Utc>, min_fee_rate: f64, max_fee_rate: f64) -> Self {
        Self {
            target_time,
            min_fee_rate,
            max_fee_rate,
            allow_bumping: true,
        }
    }

    /// Calculate appropriate fee rate based on time remaining
    pub fn calculate_fee_rate(&self, current_fee_rate: f64) -> f64 {
        let now = Utc::now();
        let time_remaining = self.target_time.signed_duration_since(now);

        if time_remaining.num_seconds() <= 0 {
            // Past deadline - use maximum fee
            return self.max_fee_rate;
        }

        // Calculate urgency multiplier based on time remaining
        let hours_remaining = time_remaining.num_hours() as f64;
        let multiplier = if hours_remaining < 1.0 {
            3.0 // Very urgent
        } else if hours_remaining < 6.0 {
            2.0 // Urgent
        } else if hours_remaining < 24.0 {
            1.5 // Moderate
        } else {
            1.0 // Not urgent
        };

        (current_fee_rate * multiplier)
            .max(self.min_fee_rate)
            .min(self.max_fee_rate)
    }

    /// Check if fee bumping is recommended
    pub fn should_bump_fee(&self) -> bool {
        if !self.allow_bumping {
            return false;
        }

        let now = Utc::now();
        let time_remaining = self.target_time.signed_duration_since(now);

        // Recommend bumping if less than 2 hours remain
        time_remaining.num_hours() < 2
    }
}

/// Budget-constrained fee strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStrategy {
    /// Maximum total fee in satoshis
    pub max_fee_satoshis: u64,
    /// Transaction size in virtual bytes
    pub tx_vbytes: usize,
    /// Minimum acceptable fee rate (sat/vB)
    pub min_fee_rate: f64,
}

impl BudgetStrategy {
    /// Create a new budget-constrained strategy
    pub fn new(max_fee_satoshis: u64, tx_vbytes: usize) -> Self {
        Self {
            max_fee_satoshis,
            tx_vbytes,
            min_fee_rate: 1.0,
        }
    }

    /// Calculate maximum affordable fee rate
    pub fn max_fee_rate(&self) -> f64 {
        self.max_fee_satoshis as f64 / self.tx_vbytes as f64
    }

    /// Calculate recommended fee rate within budget
    pub fn calculate_fee_rate(&self, market_fee_rate: f64) -> Result<f64, BitcoinError> {
        let max_rate = self.max_fee_rate();

        if max_rate < self.min_fee_rate {
            return Err(BitcoinError::InsufficientFunds(
                "Budget too low for minimum fee rate".to_string(),
            ));
        }

        Ok(market_fee_rate.min(max_rate).max(self.min_fee_rate))
    }

    /// Check if transaction fits within budget at given fee rate
    pub fn fits_budget(&self, fee_rate: f64) -> bool {
        let total_fee = (fee_rate * self.tx_vbytes as f64) as u64;
        total_fee <= self.max_fee_satoshis
    }
}

/// Multi-transaction fee planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTxFeeStrategy {
    /// Total budget for all transactions (satoshis)
    pub total_budget: u64,
    /// List of planned transactions with sizes
    pub transactions: Vec<PlannedTransaction>,
    /// Global minimum fee rate (sat/vB)
    pub min_fee_rate: f64,
}

/// A planned transaction in a multi-transaction batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTransaction {
    /// Transaction ID or label
    pub id: String,
    /// Transaction size in vbytes
    pub vbytes: usize,
    /// Priority level
    pub urgency: TransactionUrgency,
    /// Optional maximum fee for this transaction
    pub max_fee: Option<u64>,
}

impl MultiTxFeeStrategy {
    /// Create a new multi-transaction fee strategy
    pub fn new(total_budget: u64, min_fee_rate: f64) -> Self {
        Self {
            total_budget,
            transactions: Vec::new(),
            min_fee_rate,
        }
    }

    /// Add a transaction to the plan
    pub fn add_transaction(&mut self, tx: PlannedTransaction) {
        self.transactions.push(tx);
    }

    /// Calculate optimal fee allocation across all transactions
    pub fn calculate_fee_allocation(
        &self,
        market_fee_rate: f64,
    ) -> Result<Vec<TxFeeAllocation>, BitcoinError> {
        if self.transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate total size and priority-weighted size
        let mut allocations = Vec::new();

        // Calculate base allocation proportional to size and priority
        let mut remaining_budget = self.total_budget;

        for tx in &self.transactions {
            let priority_multiplier = tx.urgency.fee_multiplier();
            let base_fee = (tx.vbytes as f64 * market_fee_rate * priority_multiplier) as u64;

            // Apply transaction-specific max fee if set
            let allocated_fee = if let Some(max_fee) = tx.max_fee {
                base_fee.min(max_fee)
            } else {
                base_fee
            };

            let allocated_fee = allocated_fee.min(remaining_budget);
            remaining_budget = remaining_budget.saturating_sub(allocated_fee);

            let fee_rate = allocated_fee as f64 / tx.vbytes as f64;

            allocations.push(TxFeeAllocation {
                tx_id: tx.id.clone(),
                allocated_fee,
                fee_rate,
                vbytes: tx.vbytes,
                urgency: tx.urgency,
            });
        }

        // Check if minimum fee rate is met for all transactions
        for alloc in &allocations {
            if alloc.fee_rate < self.min_fee_rate {
                return Err(BitcoinError::InsufficientFunds(format!(
                    "Insufficient budget to meet minimum fee rate for tx: {}",
                    alloc.tx_id
                )));
            }
        }

        Ok(allocations)
    }

    /// Calculate total cost at market rate
    pub fn total_cost_at_rate(&self, fee_rate: f64) -> u64 {
        self.transactions
            .iter()
            .map(|tx| {
                let multiplier = tx.urgency.fee_multiplier();
                (tx.vbytes as f64 * fee_rate * multiplier) as u64
            })
            .sum()
    }
}

/// Fee allocation for a specific transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxFeeAllocation {
    /// Transaction ID
    pub tx_id: String,
    /// Allocated fee in satoshis
    pub allocated_fee: u64,
    /// Resulting fee rate (sat/vB)
    pub fee_rate: f64,
    /// Transaction size in vbytes
    pub vbytes: usize,
    /// Transaction urgency
    pub urgency: TransactionUrgency,
}

/// Adaptive fee manager
pub struct AdaptiveFeeManager {
    client: BitcoinClient,
}

impl AdaptiveFeeManager {
    /// Create a new adaptive fee manager
    pub fn new(client: BitcoinClient) -> Self {
        Self { client }
    }

    /// Get current market fee rate estimate
    pub fn get_market_fee_rate(&self, target_blocks: u32) -> Result<f64, BitcoinError> {
        let estimate = self.client.estimate_smart_fee(target_blocks as u16)?;
        Ok(estimate.unwrap_or(1.0))
    }

    /// Calculate fee rate based on urgency
    pub fn calculate_urgency_fee_rate(
        &self,
        urgency: TransactionUrgency,
    ) -> Result<f64, BitcoinError> {
        let target_blocks = urgency.target_blocks();
        let market_rate = self.get_market_fee_rate(target_blocks)?;
        let multiplier = urgency.fee_multiplier();
        Ok(market_rate * multiplier)
    }

    /// Calculate fee rate for time-based strategy
    pub fn calculate_time_based_fee_rate(
        &self,
        strategy: &TimeBasedStrategy,
    ) -> Result<f64, BitcoinError> {
        let market_rate = self.get_market_fee_rate(6)?;
        Ok(strategy.calculate_fee_rate(market_rate))
    }

    /// Calculate fee rate for budget strategy
    pub fn calculate_budget_fee_rate(
        &self,
        strategy: &BudgetStrategy,
    ) -> Result<f64, BitcoinError> {
        let market_rate = self.get_market_fee_rate(6)?;
        strategy.calculate_fee_rate(market_rate)
    }

    /// Calculate fee allocation for multi-transaction strategy
    pub fn calculate_multi_tx_allocation(
        &self,
        strategy: &MultiTxFeeStrategy,
    ) -> Result<Vec<TxFeeAllocation>, BitcoinError> {
        let market_rate = self.get_market_fee_rate(6)?;
        strategy.calculate_fee_allocation(market_rate)
    }

    /// Get recommended fee rate with fallback logic
    pub fn get_recommended_fee_rate(
        &self,
        urgency: TransactionUrgency,
        max_fee_rate: Option<f64>,
    ) -> Result<f64, BitcoinError> {
        let mut fee_rate = self.calculate_urgency_fee_rate(urgency)?;

        // Apply maximum cap if provided
        if let Some(max_rate) = max_fee_rate {
            fee_rate = fee_rate.min(max_rate);
        }

        // Ensure minimum of 1 sat/vB
        Ok(fee_rate.max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_urgency_target_blocks() {
        assert_eq!(TransactionUrgency::Low.target_blocks(), 144);
        assert_eq!(TransactionUrgency::Medium.target_blocks(), 6);
        assert_eq!(TransactionUrgency::High.target_blocks(), 2);
        assert_eq!(TransactionUrgency::Critical.target_blocks(), 1);
    }

    #[test]
    fn test_transaction_urgency_multiplier() {
        assert_eq!(TransactionUrgency::Low.fee_multiplier(), 1.0);
        assert_eq!(TransactionUrgency::Medium.fee_multiplier(), 1.5);
        assert_eq!(TransactionUrgency::High.fee_multiplier(), 2.0);
        assert_eq!(TransactionUrgency::Critical.fee_multiplier(), 3.0);
    }

    #[test]
    fn test_time_based_strategy() {
        let target_time = Utc::now() + chrono::Duration::hours(12);
        let strategy = TimeBasedStrategy::new(target_time, 1.0, 100.0);

        let fee_rate = strategy.calculate_fee_rate(10.0);
        assert!(fee_rate >= 1.0);
        assert!(fee_rate <= 100.0);
    }

    #[test]
    fn test_budget_strategy() {
        let strategy = BudgetStrategy::new(10_000, 200);

        assert_eq!(strategy.max_fee_rate(), 50.0);
        assert!(strategy.fits_budget(40.0));
        assert!(!strategy.fits_budget(60.0));
    }

    #[test]
    fn test_budget_strategy_calculate_fee_rate() {
        let strategy = BudgetStrategy::new(5_000, 200);

        // Market rate below max - should use market rate
        let result = strategy.calculate_fee_rate(20.0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 20.0);

        // Market rate above max - should cap at max
        let result = strategy.calculate_fee_rate(30.0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 25.0); // max_fee_rate = 5000/200 = 25
    }

    #[test]
    fn test_multi_tx_strategy() {
        let mut strategy = MultiTxFeeStrategy::new(50_000, 1.0);

        strategy.add_transaction(PlannedTransaction {
            id: "tx1".to_string(),
            vbytes: 200,
            urgency: TransactionUrgency::High,
            max_fee: None,
        });

        strategy.add_transaction(PlannedTransaction {
            id: "tx2".to_string(),
            vbytes: 150,
            urgency: TransactionUrgency::Low,
            max_fee: None,
        });

        let total_cost = strategy.total_cost_at_rate(10.0);
        assert!(total_cost > 0);
    }

    #[test]
    fn test_multi_tx_fee_allocation() {
        let mut strategy = MultiTxFeeStrategy::new(10_000, 1.0);

        strategy.add_transaction(PlannedTransaction {
            id: "tx1".to_string(),
            vbytes: 200,
            urgency: TransactionUrgency::Medium,
            max_fee: Some(4_000),
        });

        strategy.add_transaction(PlannedTransaction {
            id: "tx2".to_string(),
            vbytes: 200,
            urgency: TransactionUrgency::Low,
            max_fee: None,
        });

        let result = strategy.calculate_fee_allocation(10.0);
        assert!(result.is_ok());

        let allocations = result.unwrap();
        assert_eq!(allocations.len(), 2);

        let total_allocated: u64 = allocations.iter().map(|a| a.allocated_fee).sum();
        assert!(total_allocated <= 10_000);
    }

    #[test]
    fn test_planned_transaction() {
        let tx = PlannedTransaction {
            id: "test_tx".to_string(),
            vbytes: 250,
            urgency: TransactionUrgency::High,
            max_fee: Some(10_000),
        };

        assert_eq!(tx.id, "test_tx");
        assert_eq!(tx.vbytes, 250);
        assert_eq!(tx.urgency, TransactionUrgency::High);
        assert_eq!(tx.max_fee, Some(10_000));
    }
}
