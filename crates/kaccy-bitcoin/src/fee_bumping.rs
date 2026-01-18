//! Automated fee bumping with RBF and CPFP
//!
//! Provides automation for fee bumping using Replace-By-Fee (RBF) and
//! Child-Pays-For-Parent (CPFP) strategies.

use crate::client::BitcoinClient;
use crate::error::BitcoinError;
use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Fee bumping strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BumpStrategy {
    /// Replace-By-Fee
    RBF,
    /// Child-Pays-For-Parent
    CPFP,
    /// Automatic (choose best strategy)
    Auto,
}

/// Fee bumping policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeBumpingPolicy {
    /// Maximum fee rate willing to pay (sat/vB)
    pub max_fee_rate: f64,
    /// Minimum fee rate increase for RBF
    pub min_rbf_increase: f64,
    /// Maximum number of bump attempts
    pub max_attempts: usize,
    /// Time between bump attempts (seconds)
    pub bump_interval_secs: u64,
    /// Target confirmation blocks
    pub target_confirmations: u32,
}

impl Default for FeeBumpingPolicy {
    fn default() -> Self {
        Self {
            max_fee_rate: 100.0,   // 100 sat/vB max
            min_rbf_increase: 1.0, // Minimum 1 sat/vB increase
            max_attempts: 5,
            bump_interval_secs: 600, // 10 minutes
            target_confirmations: 6,
        }
    }
}

/// Fee bump result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeBumpResult {
    /// New transaction ID
    pub new_txid: Txid,
    /// Old transaction ID
    pub old_txid: Txid,
    /// Strategy used
    pub strategy: BumpStrategy,
    /// New fee rate (sat/vB)
    pub new_fee_rate: f64,
    /// Old fee rate (sat/vB)
    pub old_fee_rate: f64,
    /// Additional fee paid
    pub additional_fee: u64,
}

/// Tracked transaction for auto-bumping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedTransaction {
    /// Transaction ID
    pub txid: Txid,
    /// Original fee rate (sat/vB)
    pub original_fee_rate: f64,
    /// Current fee rate (sat/vB)
    pub current_fee_rate: f64,
    /// Bump attempts made
    pub bump_attempts: usize,
    /// Last bump time
    pub last_bump_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Target confirmation time
    pub target_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Priority level
    pub priority: BumpPriority,
}

/// Bump priority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BumpPriority {
    /// Low priority
    Low,
    /// Normal priority
    Normal,
    /// High priority
    High,
    /// Critical priority
    Critical,
}

impl BumpPriority {
    /// Get multiplier for this priority
    pub fn multiplier(&self) -> f64 {
        match self {
            Self::Low => 1.0,
            Self::Normal => 1.5,
            Self::High => 2.0,
            Self::Critical => 3.0,
        }
    }
}

/// Automated fee bump manager
pub struct AutoFeeBumper {
    client: BitcoinClient,
    policy: FeeBumpingPolicy,
    tracked: HashMap<Txid, TrackedTransaction>,
}

impl AutoFeeBumper {
    /// Create a new auto fee bumper
    pub fn new(client: BitcoinClient, policy: FeeBumpingPolicy) -> Self {
        Self {
            client,
            policy,
            tracked: HashMap::new(),
        }
    }

    /// Track a transaction for auto-bumping
    pub fn track_transaction(
        &mut self,
        txid: Txid,
        fee_rate: f64,
        priority: BumpPriority,
        target_time: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        let tracked = TrackedTransaction {
            txid,
            original_fee_rate: fee_rate,
            current_fee_rate: fee_rate,
            bump_attempts: 0,
            last_bump_time: None,
            target_time,
            priority,
        };

        self.tracked.insert(txid, tracked);
    }

    /// Untrack a transaction
    pub fn untrack_transaction(&mut self, txid: &Txid) -> bool {
        self.tracked.remove(txid).is_some()
    }

    /// Check and bump transactions that need it
    pub fn check_and_bump(&mut self) -> Result<Vec<FeeBumpResult>, BitcoinError> {
        let mut results = Vec::new();
        let now = chrono::Utc::now();

        // Get current mempool fee rates
        let current_mempool_fee_rate = self.get_mempool_fee_rate()?;

        let txids: Vec<Txid> = self.tracked.keys().copied().collect();

        for txid in txids {
            if let Some(tracked) = self.tracked.get(&txid) {
                // Check if transaction is confirmed
                if self.is_confirmed(&txid)? {
                    self.tracked.remove(&txid);
                    continue;
                }

                // Check if we should bump
                if self.should_bump(tracked, current_mempool_fee_rate, now)? {
                    match self.bump_transaction(&txid, tracked.priority) {
                        Ok(result) => {
                            results.push(result.clone());

                            // Update tracked transaction
                            if let Some(tracked) = self.tracked.get_mut(&txid) {
                                tracked.current_fee_rate = result.new_fee_rate;
                                tracked.bump_attempts += 1;
                                tracked.last_bump_time = Some(now);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to bump transaction {}: {}", txid, e);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Check if a transaction should be bumped
    fn should_bump(
        &self,
        tracked: &TrackedTransaction,
        mempool_fee_rate: f64,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, BitcoinError> {
        // Check max attempts
        if tracked.bump_attempts >= self.policy.max_attempts {
            return Ok(false);
        }

        // Check time since last bump
        if let Some(last_bump) = tracked.last_bump_time {
            let elapsed = now.signed_duration_since(last_bump);
            if elapsed.num_seconds() < self.policy.bump_interval_secs as i64 {
                return Ok(false);
            }
        }

        // Check if fee rate is below mempool average
        if tracked.current_fee_rate < mempool_fee_rate {
            return Ok(true);
        }

        // Check target time
        if let Some(target_time) = tracked.target_time {
            let time_remaining = target_time.signed_duration_since(now);
            // If less than 1 hour remains, bump aggressively
            if time_remaining.num_hours() < 1 {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Bump a transaction
    pub fn bump_transaction(
        &self,
        txid: &Txid,
        priority: BumpPriority,
    ) -> Result<FeeBumpResult, BitcoinError> {
        // Get transaction info
        let _tx_info = self.client.get_transaction(txid)?;
        let old_fee_rate = self.calculate_current_fee_rate(txid)?;

        // Calculate new fee rate
        let mempool_rate = self.get_mempool_fee_rate()?;
        let priority_multiplier = priority.multiplier();
        let new_fee_rate = (mempool_rate * priority_multiplier)
            .max(old_fee_rate + self.policy.min_rbf_increase)
            .min(self.policy.max_fee_rate);

        // For now, return a placeholder result
        // In production, this would create and broadcast the actual RBF transaction
        Ok(FeeBumpResult {
            new_txid: *txid,
            old_txid: *txid,
            strategy: BumpStrategy::RBF,
            new_fee_rate,
            old_fee_rate,
            additional_fee: ((new_fee_rate - old_fee_rate) * 200.0) as u64, // Estimate
        })
    }

    /// Check if a transaction is confirmed
    fn is_confirmed(&self, txid: &Txid) -> Result<bool, BitcoinError> {
        match self.client.get_transaction(txid) {
            Ok(tx_info) => Ok(tx_info.info.confirmations > 0),
            Err(_) => Ok(false),
        }
    }

    /// Get current mempool fee rate
    fn get_mempool_fee_rate(&self) -> Result<f64, BitcoinError> {
        self.client
            .estimate_smart_fee(self.policy.target_confirmations as u16)
            .map(|opt| opt.unwrap_or(1.0))
    }

    /// Calculate current fee rate for a transaction
    fn calculate_current_fee_rate(&self, _txid: &Txid) -> Result<f64, BitcoinError> {
        // Placeholder - would calculate from actual transaction data
        Ok(1.0)
    }

    /// Get all tracked transactions
    pub fn tracked_transactions(&self) -> Vec<&TrackedTransaction> {
        self.tracked.values().collect()
    }

    /// Get number of tracked transactions
    pub fn tracked_count(&self) -> usize {
        self.tracked.len()
    }
}

/// CPFP transaction builder
#[allow(dead_code)]
pub struct CpfpBuilder {
    client: BitcoinClient,
}

impl CpfpBuilder {
    /// Create a new CPFP builder
    pub fn new(client: BitcoinClient) -> Self {
        Self { client }
    }

    /// Create a CPFP transaction to bump parent
    pub fn create_cpfp(
        &self,
        parent_txid: Txid,
        target_fee_rate: f64,
    ) -> Result<String, BitcoinError> {
        // Placeholder - would build actual CPFP transaction
        Ok(format!(
            "CPFP transaction for parent {} with fee rate {}",
            parent_txid, target_fee_rate
        ))
    }
}

/// Fee budget manager
pub struct FeeBudgetManager {
    /// Total budget in satoshis
    pub total_budget: u64,
    /// Spent so far
    pub spent: u64,
    /// Reserved for pending transactions
    pub reserved: u64,
}

impl FeeBudgetManager {
    /// Create a new fee budget manager
    pub fn new(total_budget: u64) -> Self {
        Self {
            total_budget,
            spent: 0,
            reserved: 0,
        }
    }

    /// Check if budget allows spending an amount
    pub fn can_spend(&self, amount: u64) -> bool {
        self.spent + self.reserved + amount <= self.total_budget
    }

    /// Reserve an amount
    pub fn reserve(&mut self, amount: u64) -> Result<(), BitcoinError> {
        if !self.can_spend(amount) {
            return Err(BitcoinError::InsufficientFunds(
                "Fee budget exceeded".to_string(),
            ));
        }
        self.reserved += amount;
        Ok(())
    }

    /// Confirm spending (move from reserved to spent)
    pub fn confirm_spend(&mut self, amount: u64) {
        self.reserved = self.reserved.saturating_sub(amount);
        self.spent += amount;
    }

    /// Cancel reservation
    pub fn cancel_reservation(&mut self, amount: u64) {
        self.reserved = self.reserved.saturating_sub(amount);
    }

    /// Get remaining budget
    pub fn remaining(&self) -> u64 {
        self.total_budget.saturating_sub(self.spent + self.reserved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_bumping_policy_default() {
        let policy = FeeBumpingPolicy::default();
        assert_eq!(policy.max_fee_rate, 100.0);
        assert_eq!(policy.min_rbf_increase, 1.0);
        assert_eq!(policy.max_attempts, 5);
    }

    #[test]
    fn test_bump_priority_multiplier() {
        assert_eq!(BumpPriority::Low.multiplier(), 1.0);
        assert_eq!(BumpPriority::Normal.multiplier(), 1.5);
        assert_eq!(BumpPriority::High.multiplier(), 2.0);
        assert_eq!(BumpPriority::Critical.multiplier(), 3.0);
    }

    #[test]
    fn test_fee_budget_manager() {
        let mut manager = FeeBudgetManager::new(10_000);

        assert!(manager.can_spend(5_000));
        assert_eq!(manager.remaining(), 10_000);

        manager.reserve(3_000).unwrap();
        assert_eq!(manager.remaining(), 7_000);

        manager.confirm_spend(3_000);
        assert_eq!(manager.spent, 3_000);
        assert_eq!(manager.reserved, 0);
        assert_eq!(manager.remaining(), 7_000);
    }

    #[test]
    fn test_fee_budget_exceeded() {
        let mut manager = FeeBudgetManager::new(1_000);
        let result = manager.reserve(2_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_bump_strategy() {
        let strategy = BumpStrategy::RBF;
        assert_eq!(strategy, BumpStrategy::RBF);
        assert_ne!(strategy, BumpStrategy::CPFP);
    }
}
