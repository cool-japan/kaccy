//! Advanced Fee Optimization
//!
//! This module implements dynamic rebate programs, fee optimization algorithms,
//! and gas optimization strategies for the Kaccy Protocol.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Dynamic rebate program configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebateProgram {
    /// Unique identifier of this rebate program
    pub program_id: String,
    /// Category of rebate this program offers
    pub rebate_type: RebateType,
    /// Conditions a user must meet to be eligible
    pub eligibility_criteria: Vec<EligibilityCriterion>,
    /// Percentage of the fee returned as a rebate
    pub rebate_percentage: Decimal,
    /// Maximum rebate a single user can receive in this program
    pub max_rebate_per_user: Option<Decimal>,
    /// Total budget allocated to this program
    pub program_budget: Decimal,
    /// Remaining unspent budget
    pub remaining_budget: Decimal,
    /// When the program becomes active
    pub start_time: SystemTime,
    /// When the program expires (None = indefinite)
    pub end_time: Option<SystemTime>,
}

/// Type of rebate program
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebateType {
    /// Maker rebate for providing liquidity
    MakerRebate,
    /// Volume-based rebate
    VolumeRebate,
    /// Liquidity provision incentive
    LiquidityIncentive,
    /// New user onboarding rebate
    OnboardingRebate,
}

/// Criterion a user must meet to qualify for a rebate program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EligibilityCriterion {
    /// Minimum trading volume required
    MinVolume {
        /// Volume threshold in quote currency.
        threshold: Decimal,
    },
    /// Minimum liquidity provided
    MinLiquidity {
        /// Liquidity threshold.
        threshold: Decimal,
    },
    /// User must be maker (not taker)
    MakerOnly,
    /// Minimum account age
    MinAccountAge {
        /// Minimum age of the account.
        duration: Duration,
    },
    /// Specific token pairs
    TokenPairs {
        /// List of (base, quote) symbol pairs that qualify.
        pairs: Vec<(String, String)>,
    },
}

/// Rebate manager for dynamic rebate programs
pub struct RebateManager {
    /// Active rebate programs indexed by program ID
    programs: HashMap<String, RebateProgram>,
    /// Per-user rebate history indexed by user ID
    user_rebates: HashMap<String, Vec<UserRebate>>,
}

/// Record of a single rebate awarded to a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRebate {
    /// User who received the rebate
    pub user_id: String,
    /// Program that generated the rebate
    pub program_id: String,
    /// Amount rebated
    pub rebate_amount: Decimal,
    /// Original fee amount before rebate
    pub original_fee: Decimal,
    /// When the rebate was issued
    pub timestamp: SystemTime,
    /// Trade that qualified for the rebate
    pub trade_id: String,
}

impl RebateManager {
    /// Create a new rebate manager
    pub fn new() -> Self {
        Self {
            programs: HashMap::new(),
            user_rebates: HashMap::new(),
        }
    }

    /// Adds a new rebate program
    pub fn add_program(&mut self, program: RebateProgram) -> Result<(), CoreError> {
        if self.programs.contains_key(&program.program_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Rebate program {} already exists",
                program.program_id
            )));
        }

        self.programs.insert(program.program_id.clone(), program);
        Ok(())
    }

    /// Calculates applicable rebate for a user
    pub fn calculate_rebate(
        &self,
        _user_id: &str,
        fee_amount: Decimal,
        is_maker: bool,
        trading_volume: Decimal,
        liquidity_provided: Decimal,
        account_age: Duration,
    ) -> Decimal {
        let mut total_rebate = Decimal::ZERO;

        for program in self.programs.values() {
            // Check if program is active
            if let Some(end_time) = program.end_time {
                if SystemTime::now() > end_time {
                    continue;
                }
            }

            // Check eligibility
            if !self.check_eligibility(
                program,
                is_maker,
                trading_volume,
                liquidity_provided,
                account_age,
            ) {
                continue;
            }

            // Check budget
            if program.remaining_budget <= Decimal::ZERO {
                continue;
            }

            // Calculate rebate
            let rebate = fee_amount * program.rebate_percentage / Decimal::new(100, 0);

            // Apply max rebate per user limit
            let limited_rebate = if let Some(max_rebate) = program.max_rebate_per_user {
                rebate.min(max_rebate)
            } else {
                rebate
            };

            // Check against remaining budget
            let final_rebate = limited_rebate.min(program.remaining_budget);
            total_rebate += final_rebate;
        }

        total_rebate.min(fee_amount) // Can't rebate more than the fee
    }

    fn check_eligibility(
        &self,
        program: &RebateProgram,
        is_maker: bool,
        trading_volume: Decimal,
        liquidity_provided: Decimal,
        account_age: Duration,
    ) -> bool {
        for criterion in &program.eligibility_criteria {
            match criterion {
                EligibilityCriterion::MinVolume { threshold } => {
                    if trading_volume < *threshold {
                        return false;
                    }
                }
                EligibilityCriterion::MinLiquidity { threshold } => {
                    if liquidity_provided < *threshold {
                        return false;
                    }
                }
                EligibilityCriterion::MakerOnly => {
                    if !is_maker {
                        return false;
                    }
                }
                EligibilityCriterion::MinAccountAge { duration } => {
                    if account_age < *duration {
                        return false;
                    }
                }
                EligibilityCriterion::TokenPairs { .. } => {
                    // Would need token pair info to check this
                    continue;
                }
            }
        }
        true
    }

    /// Records a rebate distribution
    pub fn record_rebate(
        &mut self,
        user_id: String,
        program_id: String,
        rebate_amount: Decimal,
        original_fee: Decimal,
        trade_id: String,
    ) -> Result<(), CoreError> {
        // Update program budget
        if let Some(program) = self.programs.get_mut(&program_id) {
            if program.remaining_budget < rebate_amount {
                return Err(CoreError::InsufficientBalance {
                    required: rebate_amount,
                    available: program.remaining_budget,
                });
            }
            program.remaining_budget -= rebate_amount;
        }

        // Record user rebate
        let rebate = UserRebate {
            user_id: user_id.clone(),
            program_id,
            rebate_amount,
            original_fee,
            timestamp: SystemTime::now(),
            trade_id,
        };

        self.user_rebates.entry(user_id).or_default().push(rebate);

        Ok(())
    }

    /// Gets total rebates for a user
    pub fn get_user_total_rebates(&self, user_id: &str) -> Decimal {
        self.user_rebates
            .get(user_id)
            .map(|rebates| rebates.iter().map(|r| r.rebate_amount).sum())
            .unwrap_or(Decimal::ZERO)
    }
}

impl Default for RebateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee optimization engine
///
/// Dynamically optimizes fees to maximize revenue while maintaining competitiveness
pub struct FeeOptimizer {
    /// Historical fee performance data
    fee_history: Vec<FeePerformance>,
    /// Current fee configuration
    current_fees: FeeConfiguration,
    /// Optimization strategy
    strategy: OptimizationStrategy,
}

/// Historical snapshot of fee performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeePerformance {
    /// When this snapshot was recorded
    pub timestamp: SystemTime,
    /// Fee rate in effect at the time
    pub fee_rate: Decimal,
    /// Total trading volume during this period
    pub trading_volume: Decimal,
    /// Revenue collected during this period
    pub revenue: Decimal,
    /// Number of active users during this period
    pub user_count: usize,
}

/// Fee rate configuration for the exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConfiguration {
    /// Base fee rate applied by default
    pub base_fee: Decimal,
    /// Fee rate charged to market makers
    pub maker_fee: Decimal,
    /// Fee rate charged to market takers
    pub taker_fee: Decimal,
    /// Minimum allowable fee rate
    pub min_fee: Decimal,
    /// Maximum allowable fee rate
    pub max_fee: Decimal,
}

/// Fee optimisation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationStrategy {
    /// Maximize total revenue
    MaximizeRevenue,
    /// Maximize trading volume
    MaximizeVolume,
    /// Balance between revenue and volume
    Balanced,
    /// Match competitor fees
    Competitive,
}

impl FeeOptimizer {
    /// Create a new fee optimizer with the given initial configuration and strategy
    pub fn new(initial_config: FeeConfiguration, strategy: OptimizationStrategy) -> Self {
        Self {
            fee_history: Vec::new(),
            current_fees: initial_config,
            strategy,
        }
    }

    /// Records fee performance
    pub fn record_performance(&mut self, performance: FeePerformance) {
        self.fee_history.push(performance);

        // Keep only last 1000 records
        if self.fee_history.len() > 1000 {
            self.fee_history.remove(0);
        }
    }

    /// Optimizes fees based on historical performance
    pub fn optimize_fees(&mut self) -> FeeConfiguration {
        if self.fee_history.len() < 10 {
            // Not enough data for optimization
            return self.current_fees.clone();
        }

        match self.strategy {
            OptimizationStrategy::MaximizeRevenue => self.optimize_for_revenue(),
            OptimizationStrategy::MaximizeVolume => self.optimize_for_volume(),
            OptimizationStrategy::Balanced => self.optimize_balanced(),
            OptimizationStrategy::Competitive => self.current_fees.clone(), // Would need competitor data
        }
    }

    fn optimize_for_revenue(&self) -> FeeConfiguration {
        // Find the fee rate with highest revenue
        let best_performance = self.fee_history.iter().max_by_key(|p| p.revenue).unwrap();

        let mut config = self.current_fees.clone();
        config.base_fee = best_performance.fee_rate;
        config.taker_fee = best_performance.fee_rate;
        config.maker_fee = best_performance.fee_rate * Decimal::new(80, 2); // 80% of taker fee

        config
    }

    fn optimize_for_volume(&self) -> FeeConfiguration {
        // Find the fee rate with highest volume
        let best_performance = self
            .fee_history
            .iter()
            .max_by_key(|p| p.trading_volume)
            .unwrap();

        let mut config = self.current_fees.clone();
        config.base_fee = best_performance.fee_rate;
        config.taker_fee = best_performance.fee_rate;
        config.maker_fee = best_performance.fee_rate * Decimal::new(70, 2); // 70% of taker fee

        config
    }

    fn optimize_balanced(&self) -> FeeConfiguration {
        // Calculate revenue-per-volume ratio and optimize for that
        let mut best_ratio = Decimal::ZERO;
        let mut best_fee_rate = self.current_fees.base_fee;

        for performance in &self.fee_history {
            if performance.trading_volume > Decimal::ZERO {
                let ratio = performance.revenue / performance.trading_volume;
                if ratio > best_ratio {
                    best_ratio = ratio;
                    best_fee_rate = performance.fee_rate;
                }
            }
        }

        let mut config = self.current_fees.clone();
        config.base_fee = best_fee_rate;
        config.taker_fee = best_fee_rate;
        config.maker_fee = best_fee_rate * Decimal::new(75, 2); // 75% of taker fee

        config
    }

    /// Suggests fee adjustment based on market conditions
    pub fn suggest_adjustment(
        &self,
        current_volume: Decimal,
        target_volume: Decimal,
    ) -> FeeAdjustment {
        if current_volume < target_volume * Decimal::new(80, 2) / Decimal::new(100, 0) {
            // Volume is too low - suggest lowering fees
            FeeAdjustment {
                direction: AdjustmentDirection::Decrease,
                magnitude: Decimal::new(5, 0), // 5% decrease
                reason: "Trading volume below target".to_string(),
            }
        } else if current_volume > target_volume * Decimal::new(120, 2) / Decimal::new(100, 0) {
            // Volume is high - can potentially increase fees
            FeeAdjustment {
                direction: AdjustmentDirection::Increase,
                magnitude: Decimal::new(3, 0), // 3% increase
                reason: "Trading volume above target".to_string(),
            }
        } else {
            FeeAdjustment {
                direction: AdjustmentDirection::NoChange,
                magnitude: Decimal::ZERO,
                reason: "Trading volume within target range".to_string(),
            }
        }
    }
}

/// Suggested fee adjustment from the optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeAdjustment {
    /// Whether to raise, lower, or hold fees
    pub direction: AdjustmentDirection,
    /// Percentage magnitude of the suggested change
    pub magnitude: Decimal,
    /// Human-readable justification for the suggestion
    pub reason: String,
}

/// Direction of a fee adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdjustmentDirection {
    /// Increase the fee rate
    Increase,
    /// Decrease the fee rate
    Decrease,
    /// Keep the fee rate unchanged
    NoChange,
}

/// Gas optimization utilities
pub struct GasOptimizer {
    /// Historical gas price data
    gas_price_history: Vec<GasPrice>,
    /// Pending transactions
    #[allow(dead_code)]
    pending_transactions: Vec<PendingTransaction>,
}

/// Observed gas price at a specific block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasPrice {
    /// When this gas price was observed
    pub timestamp: SystemTime,
    /// Gas price in gwei
    pub price: Decimal,
    /// Block number at which this was observed
    pub block_number: u64,
}

/// A transaction waiting to be submitted on-chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    /// Unique identifier of the transaction
    pub tx_id: String,
    /// Urgency level for ordering
    pub priority: TransactionPriority,
    /// Maximum gas units this transaction may consume
    pub gas_limit: u64,
    /// When the transaction was enqueued
    pub created_at: SystemTime,
}

/// Transaction submission priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum TransactionPriority {
    /// Low urgency, can wait for cheaper gas
    Low = 1,
    /// Normal urgency
    Medium = 2,
    /// High urgency, should be included soon
    High = 3,
    /// Must be included in the next block
    Critical = 4,
}

impl GasOptimizer {
    /// Create a new gas optimizer
    pub fn new() -> Self {
        Self {
            gas_price_history: Vec::new(),
            pending_transactions: Vec::new(),
        }
    }

    /// Records gas price observation
    pub fn record_gas_price(&mut self, price: Decimal, block_number: u64) {
        self.gas_price_history.push(GasPrice {
            timestamp: SystemTime::now(),
            price,
            block_number,
        });

        // Keep only last 1000 observations
        if self.gas_price_history.len() > 1000 {
            self.gas_price_history.remove(0);
        }
    }

    /// Predicts optimal gas price based on priority
    pub fn predict_gas_price(&self, priority: TransactionPriority) -> Decimal {
        if self.gas_price_history.is_empty() {
            return Decimal::new(50, 0); // Default fallback
        }

        // Calculate recent average
        let recent_prices: Vec<Decimal> = self
            .gas_price_history
            .iter()
            .rev()
            .take(20)
            .map(|g| g.price)
            .collect();

        let avg_price: Decimal =
            recent_prices.iter().sum::<Decimal>() / Decimal::from(recent_prices.len());

        // Adjust based on priority
        match priority {
            TransactionPriority::Low => avg_price * Decimal::new(90, 2), // 90%
            TransactionPriority::Medium => avg_price,                    // 100%
            TransactionPriority::High => avg_price * Decimal::new(115, 2), // 115%
            TransactionPriority::Critical => avg_price * Decimal::new(150, 2), // 150%
        }
    }

    /// Batches transactions for gas optimization
    pub fn batch_transactions(
        &self,
        transactions: Vec<PendingTransaction>,
    ) -> Vec<Vec<PendingTransaction>> {
        let mut batches: Vec<Vec<PendingTransaction>> = Vec::new();
        let mut current_batch: Vec<PendingTransaction> = Vec::new();
        let mut current_gas: u64 = 0;
        const MAX_GAS_PER_BATCH: u64 = 10_000_000;

        for tx in transactions {
            if current_gas + tx.gas_limit > MAX_GAS_PER_BATCH && !current_batch.is_empty() {
                batches.push(current_batch);
                current_batch = Vec::new();
                current_gas = 0;
            }

            current_gas += tx.gas_limit;
            current_batch.push(tx);
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }

    /// Prioritizes transactions for execution
    pub fn prioritize_transactions(
        &self,
        mut transactions: Vec<PendingTransaction>,
    ) -> Vec<PendingTransaction> {
        // Sort by priority (high to low), then by creation time (old to new)
        transactions.sort_by(|a, b| match b.priority.cmp(&a.priority) {
            std::cmp::Ordering::Equal => a.created_at.cmp(&b.created_at),
            other => other,
        });

        transactions
    }
}

impl Default for GasOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebate_calculation() {
        let mut manager = RebateManager::new();

        let program = RebateProgram {
            program_id: "maker_rebate_1".to_string(),
            rebate_type: RebateType::MakerRebate,
            eligibility_criteria: vec![
                EligibilityCriterion::MakerOnly,
                EligibilityCriterion::MinVolume {
                    threshold: Decimal::new(10, 0),
                },
            ],
            rebate_percentage: Decimal::new(25, 0), // 25% rebate
            max_rebate_per_user: Some(Decimal::new(1, 0)),
            program_budget: Decimal::new(100, 0),
            remaining_budget: Decimal::new(100, 0),
            start_time: SystemTime::now(),
            end_time: None,
        };

        manager.add_program(program).unwrap();

        // Eligible user (maker with sufficient volume)
        let rebate = manager.calculate_rebate(
            "user1",
            Decimal::new(4, 0),  // 4 BTC fee
            true,                // is maker
            Decimal::new(20, 0), // trading volume
            Decimal::ZERO,
            Duration::from_secs(3600),
        );

        assert_eq!(rebate, Decimal::new(1, 0)); // 25% of 4 = 1 BTC
    }

    #[test]
    fn test_fee_optimizer_revenue() {
        let config = FeeConfiguration {
            base_fee: Decimal::new(25, 2), // 0.25%
            maker_fee: Decimal::new(20, 2),
            taker_fee: Decimal::new(30, 2),
            min_fee: Decimal::new(10, 2),
            max_fee: Decimal::new(100, 2),
        };

        let mut optimizer = FeeOptimizer::new(config, OptimizationStrategy::MaximizeRevenue);

        // Add some performance data
        optimizer.record_performance(FeePerformance {
            timestamp: SystemTime::now(),
            fee_rate: Decimal::new(25, 2),
            trading_volume: Decimal::new(1000, 0),
            revenue: Decimal::new(25, 1), // 2.5 BTC
            user_count: 100,
        });

        optimizer.record_performance(FeePerformance {
            timestamp: SystemTime::now(),
            fee_rate: Decimal::new(30, 2),
            trading_volume: Decimal::new(800, 0),
            revenue: Decimal::new(24, 1), // 2.4 BTC
            user_count: 80,
        });

        // Should not optimize with < 10 records
        let optimized = optimizer.optimize_fees();
        assert_eq!(optimized.base_fee, Decimal::new(25, 2));
    }

    #[test]
    fn test_gas_price_prediction() {
        let mut optimizer = GasOptimizer::new();

        optimizer.record_gas_price(Decimal::new(50, 0), 1000);
        optimizer.record_gas_price(Decimal::new(60, 0), 1001);
        optimizer.record_gas_price(Decimal::new(55, 0), 1002);

        let low_priority = optimizer.predict_gas_price(TransactionPriority::Low);
        let high_priority = optimizer.predict_gas_price(TransactionPriority::High);

        assert!(low_priority < high_priority);
    }

    #[test]
    fn test_transaction_batching() {
        let optimizer = GasOptimizer::new();

        let transactions = vec![
            PendingTransaction {
                tx_id: "tx1".to_string(),
                priority: TransactionPriority::Medium,
                gas_limit: 8_000_000,
                created_at: SystemTime::now(),
            },
            PendingTransaction {
                tx_id: "tx2".to_string(),
                priority: TransactionPriority::Medium,
                gas_limit: 3_000_000,
                created_at: SystemTime::now(),
            },
            PendingTransaction {
                tx_id: "tx3".to_string(),
                priority: TransactionPriority::Low,
                gas_limit: 2_000_000,
                created_at: SystemTime::now(),
            },
        ];

        let batches = optimizer.batch_transactions(transactions);
        assert_eq!(batches.len(), 2); // Should split into 2 batches
    }

    #[test]
    fn test_transaction_prioritization() {
        let optimizer = GasOptimizer::new();

        let transactions = vec![
            PendingTransaction {
                tx_id: "tx1".to_string(),
                priority: TransactionPriority::Low,
                gas_limit: 1_000_000,
                created_at: SystemTime::now(),
            },
            PendingTransaction {
                tx_id: "tx2".to_string(),
                priority: TransactionPriority::Critical,
                gas_limit: 1_000_000,
                created_at: SystemTime::now(),
            },
            PendingTransaction {
                tx_id: "tx3".to_string(),
                priority: TransactionPriority::Medium,
                gas_limit: 1_000_000,
                created_at: SystemTime::now(),
            },
        ];

        let prioritized = optimizer.prioritize_transactions(transactions);
        assert_eq!(prioritized[0].tx_id, "tx2"); // Critical first
        assert_eq!(prioritized[2].tx_id, "tx1"); // Low last
    }
}
