//! Dynamic Parameter Adjustment
//!
//! This module provides tools for dynamically adjusting protocol parameters based on market
//! conditions, including algorithmic fee adjustment, supply elasticity mechanisms, and
//! incentive optimization.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Market conditions for parameter adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    /// Trading volume (24h)
    pub volume_24h: Decimal,
    /// Price volatility (annualized)
    pub volatility: Decimal,
    /// Liquidity depth
    pub liquidity: Decimal,
    /// User activity level (0-1)
    pub activity_level: Decimal,
    /// Network congestion (0-1)
    pub congestion: Decimal,
}

impl MarketConditions {
    /// Create market conditions from metrics
    pub fn new(
        volume_24h: Decimal,
        volatility: Decimal,
        liquidity: Decimal,
        activity_level: Decimal,
        congestion: Decimal,
    ) -> Self {
        Self {
            volume_24h,
            volatility,
            liquidity,
            activity_level,
            congestion,
        }
    }

    /// Classify market state
    pub fn market_state(&self) -> MarketState {
        if self.volatility > dec!(0.5) {
            MarketState::HighVolatility
        } else if self.congestion > dec!(0.8) {
            MarketState::Congested
        } else if self.activity_level < dec!(0.2) {
            MarketState::LowActivity
        } else {
            MarketState::Normal
        }
    }
}

/// Current market state classification used for dynamic parameter adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketState {
    /// Normal market conditions
    Normal,
    /// High volatility
    HighVolatility,
    /// Network congestion
    Congested,
    /// Low activity
    LowActivity,
}

/// Algorithmic fee adjuster
pub struct AlgorithmicFeeAdjuster {
    /// Base fee rate
    base_fee: Decimal,
    /// Minimum fee rate
    min_fee: Decimal,
    /// Maximum fee rate
    max_fee: Decimal,
    /// Historical volumes (for moving average)
    volume_history: VecDeque<Decimal>,
    /// Target volume
    target_volume: Decimal,
}

impl AlgorithmicFeeAdjuster {
    /// Create a new fee adjuster
    pub fn new(base_fee: Decimal, min_fee: Decimal, max_fee: Decimal) -> Self {
        Self {
            base_fee,
            min_fee,
            max_fee,
            volume_history: VecDeque::with_capacity(30),
            target_volume: dec!(0),
        }
    }

    /// Set target volume
    pub fn set_target_volume(&mut self, target: Decimal) {
        self.target_volume = target;
    }

    /// Add volume observation
    pub fn add_volume(&mut self, volume: Decimal) {
        self.volume_history.push_back(volume);
        if self.volume_history.len() > 30 {
            self.volume_history.pop_front();
        }
    }

    /// Calculate optimal fee based on market conditions
    pub fn calculate_optimal_fee(&self, conditions: &MarketConditions) -> Decimal {
        let mut fee = self.base_fee;

        // Adjust based on market state
        fee = match conditions.market_state() {
            MarketState::HighVolatility => {
                // Increase fees during high volatility to reduce risk
                fee * dec!(1.5)
            }
            MarketState::Congested => {
                // Increase fees during congestion (like EIP-1559)
                fee * dec!(2.0)
            }
            MarketState::LowActivity => {
                // Decrease fees to stimulate activity
                fee * dec!(0.7)
            }
            MarketState::Normal => fee,
        };

        // Adjust based on volume vs target
        if self.target_volume > dec!(0) && conditions.volume_24h > dec!(0) {
            let volume_ratio = conditions.volume_24h / self.target_volume;
            if volume_ratio < dec!(0.5) {
                // Low volume -> reduce fees
                fee *= dec!(0.8);
            } else if volume_ratio > dec!(2.0) {
                // High volume -> can increase fees
                fee *= dec!(1.2);
            }
        }

        // Adjust based on liquidity
        if conditions.liquidity < dec!(100000) {
            // Low liquidity -> reduce fees to attract liquidity
            fee *= dec!(0.9);
        }

        // Clamp to min/max
        fee.max(self.min_fee).min(self.max_fee)
    }

    /// Calculate fee adjustment recommendation
    pub fn recommend_adjustment(
        &self,
        current_fee: Decimal,
        conditions: &MarketConditions,
    ) -> FeeAdjustment {
        let optimal_fee = self.calculate_optimal_fee(conditions);
        let diff = optimal_fee - current_fee;
        let diff_pct = if current_fee > dec!(0) {
            (diff / current_fee) * dec!(100)
        } else {
            dec!(0)
        };

        FeeAdjustment {
            current_fee,
            optimal_fee,
            adjustment: diff,
            adjustment_pct: diff_pct,
            reason: self.adjustment_reason(conditions),
        }
    }

    fn adjustment_reason(&self, conditions: &MarketConditions) -> String {
        match conditions.market_state() {
            MarketState::HighVolatility => "High volatility detected".to_string(),
            MarketState::Congested => "Network congestion".to_string(),
            MarketState::LowActivity => "Low activity - stimulating usage".to_string(),
            MarketState::Normal => "Normal market conditions".to_string(),
        }
    }
}

/// Fee adjustment recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeAdjustment {
    /// Current fee
    pub current_fee: Decimal,
    /// Optimal fee
    pub optimal_fee: Decimal,
    /// Adjustment amount
    pub adjustment: Decimal,
    /// Adjustment percentage
    pub adjustment_pct: Decimal,
    /// Reason for adjustment
    pub reason: String,
}

/// Supply elasticity manager
pub struct SupplyElasticityManager {
    /// Target price
    target_price: Decimal,
    /// Price tolerance (percentage)
    tolerance: Decimal,
    /// Expansion rate (per rebase)
    expansion_rate: Decimal,
    /// Contraction rate (per rebase)
    contraction_rate: Decimal,
}

impl SupplyElasticityManager {
    /// Create a new supply elasticity manager
    pub fn new(target_price: Decimal, tolerance: Decimal) -> Self {
        Self {
            target_price,
            tolerance,
            expansion_rate: dec!(0.05),   // 5% default
            contraction_rate: dec!(0.05), // 5% default
        }
    }

    /// Calculate supply adjustment (rebase)
    pub fn calculate_rebase(
        &self,
        current_price: Decimal,
        current_supply: Decimal,
    ) -> SupplyAdjustment {
        let price_deviation = (current_price - self.target_price) / self.target_price;
        let abs_deviation = price_deviation.abs();

        if abs_deviation <= self.tolerance {
            // Within tolerance - no adjustment
            return SupplyAdjustment {
                current_supply,
                new_supply: current_supply,
                adjustment: dec!(0),
                adjustment_pct: dec!(0),
                reason: "Price within target range".to_string(),
            };
        }

        let adjustment_pct = if current_price > self.target_price {
            // Price too high -> expand supply
            self.expansion_rate.min(abs_deviation)
        } else {
            // Price too low -> contract supply
            -self.contraction_rate.max(-abs_deviation)
        };

        let new_supply = current_supply * (dec!(1) + adjustment_pct);
        let adjustment = new_supply - current_supply;

        SupplyAdjustment {
            current_supply,
            new_supply,
            adjustment,
            adjustment_pct: adjustment_pct * dec!(100),
            reason: if adjustment > dec!(0) {
                "Expanding supply to decrease price".to_string()
            } else {
                "Contracting supply to increase price".to_string()
            },
        }
    }

    /// Set expansion rate
    pub fn set_expansion_rate(&mut self, rate: Decimal) {
        self.expansion_rate = rate;
    }

    /// Set contraction rate
    pub fn set_contraction_rate(&mut self, rate: Decimal) {
        self.contraction_rate = rate;
    }
}

/// Supply adjustment (rebase) result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyAdjustment {
    /// Current supply
    pub current_supply: Decimal,
    /// New supply after adjustment
    pub new_supply: Decimal,
    /// Adjustment amount
    pub adjustment: Decimal,
    /// Adjustment percentage
    pub adjustment_pct: Decimal,
    /// Reason for adjustment
    pub reason: String,
}

/// Incentive optimizer
pub struct IncentiveOptimizer {
    /// Total budget for incentives
    budget: Decimal,
    /// Incentive allocation by category
    allocations: Vec<IncentiveAllocation>,
}

/// Allocation of incentives to a specific protocol category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncentiveAllocation {
    /// Category name (e.g., "Liquidity Mining", "Staking Rewards")
    pub category: String,
    /// Allocated amount
    pub amount: Decimal,
    /// Target metric (e.g., TVL, active users)
    pub target_metric: Decimal,
    /// Current metric value
    pub current_metric: Decimal,
    /// Importance weight (0-1)
    pub weight: Decimal,
}

impl IncentiveOptimizer {
    /// Create a new incentive optimizer
    pub fn new(budget: Decimal) -> Self {
        Self {
            budget,
            allocations: Vec::new(),
        }
    }

    /// Add an incentive category
    pub fn add_allocation(&mut self, allocation: IncentiveAllocation) {
        self.allocations.push(allocation);
    }

    /// Optimize incentive distribution based on performance
    pub fn optimize_distribution(&mut self) -> Vec<IncentiveAllocation> {
        if self.allocations.is_empty() {
            return Vec::new();
        }

        // Calculate efficiency scores
        let scores: Vec<(usize, Decimal)> = self
            .allocations
            .iter()
            .enumerate()
            .map(|(i, alloc)| {
                let efficiency = if alloc.amount > dec!(0) {
                    alloc.current_metric / alloc.amount
                } else {
                    dec!(0)
                };
                (i, efficiency * alloc.weight)
            })
            .collect();

        // Normalize scores
        let total_score: Decimal = scores.iter().map(|(_, s)| s).sum();
        if total_score == dec!(0) {
            // Equal distribution if no scores
            let equal_amount = self.budget / Decimal::from(self.allocations.len() as i64);
            return self
                .allocations
                .iter()
                .map(|alloc| {
                    let mut new_alloc = alloc.clone();
                    new_alloc.amount = equal_amount;
                    new_alloc
                })
                .collect();
        }

        // Allocate budget proportional to weighted efficiency
        let mut optimized = Vec::new();
        for (i, score) in scores {
            let mut alloc = self.allocations[i].clone();
            alloc.amount = (score / total_score) * self.budget;
            optimized.push(alloc);
        }

        optimized
    }

    /// Calculate total incentive efficiency
    pub fn overall_efficiency(&self) -> Decimal {
        if self.allocations.is_empty() {
            return dec!(0);
        }

        let total_output: Decimal = self.allocations.iter().map(|a| a.current_metric).sum();
        let total_input: Decimal = self.allocations.iter().map(|a| a.amount).sum();

        if total_input > dec!(0) {
            total_output / total_input
        } else {
            dec!(0)
        }
    }

    /// Suggest reallocation to improve efficiency
    pub fn suggest_reallocation(&self) -> Vec<(String, Decimal, String)> {
        let avg_efficiency = self.overall_efficiency();
        let mut suggestions = Vec::new();

        for alloc in &self.allocations {
            let efficiency = if alloc.amount > dec!(0) {
                alloc.current_metric / alloc.amount
            } else {
                dec!(0)
            };

            if efficiency < avg_efficiency * dec!(0.5) {
                // Underperforming category
                suggestions.push((
                    alloc.category.clone(),
                    -alloc.amount * dec!(0.25), // Reduce by 25%
                    "Underperforming - reduce allocation".to_string(),
                ));
            } else if efficiency > avg_efficiency * dec!(1.5) {
                // Overperforming category
                suggestions.push((
                    alloc.category.clone(),
                    alloc.amount * dec!(0.25), // Increase by 25%
                    "Overperforming - increase allocation".to_string(),
                ));
            }
        }

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_conditions() {
        let conditions = MarketConditions::new(
            dec!(1000000), // volume
            dec!(0.30),    // volatility
            dec!(500000),  // liquidity
            dec!(0.75),    // activity
            dec!(0.50),    // congestion
        );

        assert_eq!(conditions.market_state(), MarketState::Normal);
    }

    #[test]
    fn test_market_state_classification() {
        let high_vol = MarketConditions::new(
            dec!(1000000),
            dec!(0.60), // High volatility
            dec!(500000),
            dec!(0.75),
            dec!(0.50),
        );
        assert_eq!(high_vol.market_state(), MarketState::HighVolatility);

        let congested = MarketConditions::new(
            dec!(1000000),
            dec!(0.30),
            dec!(500000),
            dec!(0.75),
            dec!(0.85), // High congestion
        );
        assert_eq!(congested.market_state(), MarketState::Congested);

        let low_activity = MarketConditions::new(
            dec!(1000000),
            dec!(0.30),
            dec!(500000),
            dec!(0.15), // Low activity
            dec!(0.50),
        );
        assert_eq!(low_activity.market_state(), MarketState::LowActivity);
    }

    #[test]
    fn test_fee_adjuster() {
        let adjuster = AlgorithmicFeeAdjuster::new(dec!(0.025), dec!(0.001), dec!(0.10));

        let conditions = MarketConditions::new(
            dec!(1000000),
            dec!(0.30),
            dec!(500000),
            dec!(0.75),
            dec!(0.50),
        );

        let fee = adjuster.calculate_optimal_fee(&conditions);
        assert!(fee >= dec!(0.001));
        assert!(fee <= dec!(0.10));
    }

    #[test]
    fn test_fee_adjustment_high_volatility() {
        let adjuster = AlgorithmicFeeAdjuster::new(dec!(0.025), dec!(0.001), dec!(0.10));

        let high_vol = MarketConditions::new(
            dec!(1000000),
            dec!(0.60), // High volatility
            dec!(500000),
            dec!(0.75),
            dec!(0.50),
        );

        let normal = MarketConditions::new(
            dec!(1000000),
            dec!(0.30),
            dec!(500000),
            dec!(0.75),
            dec!(0.50),
        );

        let fee_high_vol = adjuster.calculate_optimal_fee(&high_vol);
        let fee_normal = adjuster.calculate_optimal_fee(&normal);

        // Fee should be higher in high volatility
        assert!(fee_high_vol > fee_normal);
    }

    #[test]
    fn test_fee_recommendation() {
        let adjuster = AlgorithmicFeeAdjuster::new(dec!(0.025), dec!(0.001), dec!(0.10));

        let conditions = MarketConditions::new(
            dec!(1000000),
            dec!(0.30),
            dec!(500000),
            dec!(0.75),
            dec!(0.50),
        );

        let recommendation = adjuster.recommend_adjustment(dec!(0.025), &conditions);

        assert_eq!(recommendation.current_fee, dec!(0.025));
        assert!(recommendation.optimal_fee > dec!(0));
    }

    #[test]
    fn test_supply_elasticity() {
        let manager = SupplyElasticityManager::new(dec!(1.0), dec!(0.05));

        // Price above target
        let adjustment = manager.calculate_rebase(dec!(1.20), dec!(1000000));
        assert!(adjustment.adjustment > dec!(0)); // Should expand supply

        // Price below target
        let adjustment = manager.calculate_rebase(dec!(0.80), dec!(1000000));
        assert!(adjustment.adjustment < dec!(0)); // Should contract supply

        // Price within tolerance
        let adjustment = manager.calculate_rebase(dec!(1.03), dec!(1000000));
        assert_eq!(adjustment.adjustment, dec!(0)); // No adjustment
    }

    #[test]
    fn test_supply_expansion() {
        let manager = SupplyElasticityManager::new(dec!(1.0), dec!(0.05));

        let adjustment = manager.calculate_rebase(dec!(1.50), dec!(1000000));

        assert!(adjustment.new_supply > adjustment.current_supply);
        assert!(adjustment.adjustment_pct > dec!(0));
        assert!(adjustment.reason.contains("Expanding"));
    }

    #[test]
    fn test_supply_contraction() {
        let manager = SupplyElasticityManager::new(dec!(1.0), dec!(0.05));

        let adjustment = manager.calculate_rebase(dec!(0.70), dec!(1000000));

        assert!(adjustment.new_supply < adjustment.current_supply);
        assert!(adjustment.adjustment_pct < dec!(0));
        assert!(adjustment.reason.contains("Contracting"));
    }

    #[test]
    fn test_incentive_optimizer() {
        let mut optimizer = IncentiveOptimizer::new(dec!(100000));

        optimizer.add_allocation(IncentiveAllocation {
            category: "Liquidity Mining".to_string(),
            amount: dec!(50000),
            target_metric: dec!(1000000),
            current_metric: dec!(800000),
            weight: dec!(1.0),
        });

        optimizer.add_allocation(IncentiveAllocation {
            category: "Staking Rewards".to_string(),
            amount: dec!(50000),
            target_metric: dec!(500000),
            current_metric: dec!(600000), // Overperforming
            weight: dec!(1.0),
        });

        let efficiency = optimizer.overall_efficiency();
        assert!(efficiency > dec!(0));
    }

    #[test]
    fn test_incentive_optimization() {
        let mut optimizer = IncentiveOptimizer::new(dec!(100000));

        optimizer.add_allocation(IncentiveAllocation {
            category: "Category A".to_string(),
            amount: dec!(60000),
            target_metric: dec!(100),
            current_metric: dec!(120), // Good efficiency
            weight: dec!(1.0),
        });

        optimizer.add_allocation(IncentiveAllocation {
            category: "Category B".to_string(),
            amount: dec!(40000),
            target_metric: dec!(100),
            current_metric: dec!(80), // Poor efficiency
            weight: dec!(1.0),
        });

        let optimized = optimizer.optimize_distribution();

        assert_eq!(optimized.len(), 2);
        // Total should equal budget
        let total: Decimal = optimized.iter().map(|a| a.amount).sum();
        assert!(total > dec!(99000) && total <= dec!(100000));
    }

    #[test]
    fn test_reallocation_suggestions() {
        let mut optimizer = IncentiveOptimizer::new(dec!(100000));

        optimizer.add_allocation(IncentiveAllocation {
            category: "High Performer".to_string(),
            amount: dec!(30000),
            target_metric: dec!(100),
            current_metric: dec!(200), // Very efficient
            weight: dec!(1.0),
        });

        optimizer.add_allocation(IncentiveAllocation {
            category: "Low Performer".to_string(),
            amount: dec!(70000),
            target_metric: dec!(100),
            current_metric: dec!(50), // Inefficient
            weight: dec!(1.0),
        });

        let suggestions = optimizer.suggest_reallocation();

        assert!(!suggestions.is_empty());
        // Should suggest reducing low performer and increasing high performer
        assert!(
            suggestions
                .iter()
                .any(|(cat, amt, _)| cat == "Low Performer" && *amt < dec!(0))
        );
        assert!(
            suggestions
                .iter()
                .any(|(cat, amt, _)| cat == "High Performer" && *amt > dec!(0))
        );
    }
}
