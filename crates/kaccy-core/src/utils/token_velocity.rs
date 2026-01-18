//! Token Velocity Management
//!
//! This module provides tools for managing token velocity, including velocity reduction
//! mechanisms, staking incentive optimization, and dynamic burn rate adjustment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Token velocity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityMetrics {
    /// Trading volume (period)
    pub volume: Decimal,
    /// Circulating supply
    pub circulating_supply: Decimal,
    /// Market cap
    pub market_cap: Decimal,
    /// Period in days
    pub period_days: i64,
}

impl VelocityMetrics {
    /// Calculate token velocity (volume / market cap)
    pub fn calculate_velocity(&self) -> Decimal {
        if self.market_cap > dec!(0) {
            self.volume / self.market_cap
        } else {
            dec!(0)
        }
    }

    /// Calculate annualized velocity
    pub fn annualized_velocity(&self) -> Decimal {
        let velocity = self.calculate_velocity();
        if self.period_days > 0 {
            velocity * Decimal::from(365) / Decimal::from(self.period_days)
        } else {
            dec!(0)
        }
    }

    /// Calculate average holding period (days)
    pub fn average_holding_period(&self) -> Decimal {
        let velocity = self.calculate_velocity();
        if velocity > dec!(0) {
            Decimal::from(self.period_days) / velocity
        } else {
            dec!(0)
        }
    }

    /// Classify velocity level
    pub fn velocity_level(&self) -> VelocityLevel {
        let annual_velocity = self.annualized_velocity();

        if annual_velocity > dec!(50) {
            VelocityLevel::VeryHigh
        } else if annual_velocity > dec!(20) {
            VelocityLevel::High
        } else if annual_velocity > dec!(5) {
            VelocityLevel::Moderate
        } else {
            VelocityLevel::Low
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VelocityLevel {
    /// Low velocity (good for store of value)
    Low,
    /// Moderate velocity
    Moderate,
    /// High velocity
    High,
    /// Very high velocity (may indicate speculation)
    VeryHigh,
}

/// Velocity reduction mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VelocityReductionMechanism {
    /// Staking with lock periods
    Staking {
        /// Annual percentage yield
        apy: Decimal,
        /// Lock period in days
        lock_days: i64,
    },
    /// Transaction fees
    TransactionFees {
        /// Fee percentage
        fee_pct: Decimal,
    },
    /// Holding rewards
    HoldingRewards {
        /// Reward rate per day
        daily_reward_rate: Decimal,
        /// Minimum holding period for rewards
        min_hold_days: i64,
    },
    /// Utility requirements (token needed for platform access)
    UtilityRequirements {
        /// Minimum balance required
        min_balance: Decimal,
    },
    /// Buyback and burn
    BuybackBurn {
        /// Percentage of revenue used for buyback
        buyback_pct: Decimal,
    },
}

/// Token velocity manager
pub struct TokenVelocityManager {
    /// Current velocity metrics
    metrics: VelocityMetrics,
    /// Target velocity
    target_velocity: Decimal,
    /// Active mechanisms
    mechanisms: Vec<VelocityReductionMechanism>,
}

impl TokenVelocityManager {
    /// Create a new token velocity manager
    pub fn new(metrics: VelocityMetrics, target_velocity: Decimal) -> Self {
        Self {
            metrics,
            target_velocity,
            mechanisms: Vec::new(),
        }
    }

    /// Add a velocity reduction mechanism
    pub fn add_mechanism(&mut self, mechanism: VelocityReductionMechanism) {
        self.mechanisms.push(mechanism);
    }

    /// Calculate expected velocity reduction from mechanisms
    pub fn calculate_velocity_reduction(&self) -> Decimal {
        let mut total_reduction = dec!(0);

        for mechanism in &self.mechanisms {
            let reduction = match mechanism {
                VelocityReductionMechanism::Staking { apy, lock_days } => {
                    // Staking reduces velocity proportional to APY and lock period
                    let lock_factor = Decimal::from(*lock_days) / dec!(365);
                    apy * lock_factor * dec!(0.5) // 50% of staked tokens effectively locked
                }
                VelocityReductionMechanism::TransactionFees { fee_pct } => {
                    // Fees discourage trading
                    fee_pct * dec!(2) // 2x multiplier for velocity reduction
                }
                VelocityReductionMechanism::HoldingRewards {
                    daily_reward_rate,
                    min_hold_days,
                } => {
                    // Holding rewards encourage holding
                    let annual_rate = daily_reward_rate * dec!(365);
                    let hold_factor = Decimal::from(*min_hold_days) / dec!(365);
                    annual_rate * hold_factor
                }
                VelocityReductionMechanism::UtilityRequirements { min_balance } => {
                    // Utility requirements lock up tokens
                    (min_balance / self.metrics.circulating_supply) * dec!(100)
                }
                VelocityReductionMechanism::BuybackBurn { buyback_pct } => {
                    // Buyback and burn reduces supply
                    buyback_pct * dec!(0.5)
                }
            };

            total_reduction += reduction;
        }

        total_reduction
    }

    /// Calculate new velocity with mechanisms
    pub fn projected_velocity(&self) -> Decimal {
        let current = self.metrics.annualized_velocity();
        let reduction = self.calculate_velocity_reduction();

        (current * (dec!(1) - reduction / dec!(100))).max(dec!(0))
    }

    /// Check if velocity is within target
    pub fn is_velocity_optimal(&self) -> bool {
        let projected = self.projected_velocity();
        let tolerance = self.target_velocity * dec!(0.20); // 20% tolerance

        projected >= (self.target_velocity - tolerance)
            && projected <= (self.target_velocity + tolerance)
    }

    /// Recommend adjustments to reach target velocity
    pub fn recommend_adjustments(&self) -> Vec<VelocityAdjustment> {
        let current = self.metrics.annualized_velocity();
        let target = self.target_velocity;

        if current <= target * dec!(1.2) {
            return vec![]; // Close enough
        }

        let mut recommendations = Vec::new();

        // If velocity too high, recommend velocity reduction mechanisms
        if current > target {
            recommendations.push(VelocityAdjustment {
                mechanism_type: "Increase Staking APY".to_string(),
                current_value: None,
                recommended_value: dec!(0.20), // 20% APY
                expected_impact: dec!(10.0),   // 10% velocity reduction
                reason: "High velocity detected - increase staking incentives".to_string(),
            });

            recommendations.push(VelocityAdjustment {
                mechanism_type: "Implement Transaction Fees".to_string(),
                current_value: None,
                recommended_value: dec!(0.005), // 0.5% fee
                expected_impact: dec!(5.0),     // 5% velocity reduction
                reason: "Discourage high-frequency trading".to_string(),
            });

            recommendations.push(VelocityAdjustment {
                mechanism_type: "Increase Burn Rate".to_string(),
                current_value: None,
                recommended_value: dec!(0.02), // 2% of revenue
                expected_impact: dec!(3.0),    // 3% velocity reduction
                reason: "Reduce circulating supply".to_string(),
            });
        }

        recommendations
    }

    /// Update metrics
    pub fn update_metrics(&mut self, metrics: VelocityMetrics) {
        self.metrics = metrics;
    }
}

/// Velocity adjustment recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityAdjustment {
    /// Type of mechanism
    pub mechanism_type: String,
    /// Current value (if exists)
    pub current_value: Option<Decimal>,
    /// Recommended value
    pub recommended_value: Decimal,
    /// Expected impact (percentage)
    pub expected_impact: Decimal,
    /// Reason for adjustment
    pub reason: String,
}

/// Staking incentive optimizer
pub struct StakingIncentiveOptimizer {
    /// Total staking budget
    budget: Decimal,
    /// Current staking data
    stakes: HashMap<i64, StakePosition>,
    /// Target staking ratio (percentage of supply staked)
    target_ratio: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakePosition {
    /// User ID
    pub user_id: i64,
    /// Staked amount
    pub amount: Decimal,
    /// Lock period (days)
    pub lock_days: i64,
    /// Start date
    pub start_date: DateTime<Utc>,
    /// APY
    pub apy: Decimal,
}

impl StakePosition {
    /// Calculate rewards accrued
    pub fn calculate_rewards(&self, current_date: DateTime<Utc>) -> Decimal {
        let days_elapsed = (current_date - self.start_date).num_days();
        if days_elapsed <= 0 {
            return dec!(0);
        }

        let years = Decimal::from(days_elapsed) / dec!(365);
        self.amount * self.apy * years
    }

    /// Check if lock period is complete
    pub fn is_unlocked(&self, current_date: DateTime<Utc>) -> bool {
        let days_elapsed = (current_date - self.start_date).num_days();
        days_elapsed >= self.lock_days
    }

    /// Calculate effective lock value (amount × remaining lock time)
    pub fn effective_lock_value(&self, current_date: DateTime<Utc>) -> Decimal {
        let days_elapsed = (current_date - self.start_date).num_days();
        let remaining_days = (self.lock_days - days_elapsed).max(0);
        let lock_factor = Decimal::from(remaining_days) / dec!(365);

        self.amount * lock_factor
    }
}

impl StakingIncentiveOptimizer {
    /// Create a new staking incentive optimizer
    pub fn new(budget: Decimal, target_ratio: Decimal) -> Self {
        Self {
            budget,
            stakes: HashMap::new(),
            target_ratio,
        }
    }

    /// Add a stake position
    pub fn add_stake(&mut self, stake: StakePosition) {
        self.stakes.insert(stake.user_id, stake);
    }

    /// Calculate current staking ratio
    pub fn current_staking_ratio(&self, total_supply: Decimal) -> Decimal {
        if total_supply == dec!(0) {
            return dec!(0);
        }

        let total_staked: Decimal = self.stakes.values().map(|s| s.amount).sum();
        (total_staked / total_supply) * dec!(100)
    }

    /// Calculate optimal APY to reach target ratio
    pub fn optimal_apy(&self, total_supply: Decimal, current_apy: Decimal) -> Decimal {
        let current_ratio = self.current_staking_ratio(total_supply);
        let target = self.target_ratio;

        if current_ratio >= target {
            return current_apy; // Already at target
        }

        // Increase APY proportionally to gap
        let gap = target - current_ratio;
        let multiplier = dec!(1) + (gap / dec!(100));

        (current_apy * multiplier).min(dec!(1.0)) // Cap at 100% APY
    }

    /// Calculate total rewards to distribute
    pub fn calculate_total_rewards(&self, current_date: DateTime<Utc>) -> Decimal {
        self.stakes
            .values()
            .map(|s| s.calculate_rewards(current_date))
            .sum()
    }

    /// Check if budget is sufficient
    pub fn is_budget_sufficient(&self, current_date: DateTime<Utc>) -> bool {
        let total_rewards = self.calculate_total_rewards(current_date);
        total_rewards <= self.budget
    }

    /// Optimize staking tiers (lock duration → APY mapping)
    pub fn optimize_tiers(&self, total_supply: Decimal) -> Vec<StakingTier> {
        let optimal_base_apy = self.optimal_apy(total_supply, dec!(0.10));

        vec![
            StakingTier {
                name: "Flexible".to_string(),
                lock_days: 0,
                apy: optimal_base_apy * dec!(0.5), // 50% of base
                min_stake: dec!(100),
            },
            StakingTier {
                name: "1 Month".to_string(),
                lock_days: 30,
                apy: optimal_base_apy,
                min_stake: dec!(100),
            },
            StakingTier {
                name: "3 Months".to_string(),
                lock_days: 90,
                apy: optimal_base_apy * dec!(1.3), // 30% bonus
                min_stake: dec!(100),
            },
            StakingTier {
                name: "6 Months".to_string(),
                lock_days: 180,
                apy: optimal_base_apy * dec!(1.6), // 60% bonus
                min_stake: dec!(100),
            },
            StakingTier {
                name: "1 Year".to_string(),
                lock_days: 365,
                apy: optimal_base_apy * dec!(2.0), // 100% bonus
                min_stake: dec!(100),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingTier {
    /// Tier name
    pub name: String,
    /// Lock period in days
    pub lock_days: i64,
    /// Annual percentage yield
    pub apy: Decimal,
    /// Minimum stake amount
    pub min_stake: Decimal,
}

/// Dynamic burn rate manager
pub struct BurnRateManager {
    /// Current burn rate (percentage of supply per year)
    burn_rate: Decimal,
    /// Target supply (if known)
    target_supply: Option<Decimal>,
    /// Burn history
    burn_history: Vec<BurnEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnEvent {
    /// Burn date
    pub date: DateTime<Utc>,
    /// Amount burned
    pub amount: Decimal,
    /// Reason for burn
    pub reason: String,
}

impl BurnRateManager {
    /// Create a new burn rate manager
    pub fn new(burn_rate: Decimal) -> Self {
        Self {
            burn_rate,
            target_supply: None,
            burn_history: Vec::new(),
        }
    }

    /// Set target supply
    pub fn set_target_supply(&mut self, target: Decimal) {
        self.target_supply = Some(target);
    }

    /// Record a burn event
    pub fn record_burn(&mut self, event: BurnEvent) {
        self.burn_history.push(event);
    }

    /// Calculate total burned
    pub fn total_burned(&self) -> Decimal {
        self.burn_history.iter().map(|e| e.amount).sum()
    }

    /// Calculate burn rate needed to reach target supply
    pub fn calculate_target_burn_rate(
        &self,
        current_supply: Decimal,
        years_to_target: Decimal,
    ) -> Option<Decimal> {
        let target = self.target_supply?;

        if current_supply <= target || years_to_target <= dec!(0) {
            return None;
        }

        let total_to_burn = current_supply - target;
        let annual_burn = total_to_burn / years_to_target;

        Some((annual_burn / current_supply) * dec!(100))
    }

    /// Adjust burn rate based on velocity
    pub fn adjust_for_velocity(&mut self, velocity_metrics: &VelocityMetrics) {
        let velocity_level = velocity_metrics.velocity_level();

        match velocity_level {
            VelocityLevel::VeryHigh => {
                // Increase burn rate to counter high velocity
                self.burn_rate *= dec!(1.5);
            }
            VelocityLevel::High => {
                self.burn_rate *= dec!(1.2);
            }
            VelocityLevel::Low => {
                // Can reduce burn rate
                self.burn_rate *= dec!(0.8);
            }
            VelocityLevel::Moderate => {
                // Keep current rate
            }
        }

        // Reasonable bounds
        self.burn_rate = self.burn_rate.max(dec!(0)).min(dec!(0.20)); // Max 20% annual burn
    }

    /// Calculate expected supply after n years
    pub fn project_supply(&self, current_supply: Decimal, years: Decimal) -> Decimal {
        // Supply(t) = Supply(0) × (1 - burn_rate)^t
        let mut supply = current_supply;

        // Approximate using discrete steps
        let steps = (years * dec!(10)).to_i64().unwrap_or(10);
        let step_rate = self.burn_rate / Decimal::from(steps);

        for _ in 0..steps {
            supply *= dec!(1) - step_rate / dec!(100);
        }

        supply
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_velocity_calculation() {
        let metrics = VelocityMetrics {
            volume: dec!(1000000),
            circulating_supply: dec!(10000000),
            market_cap: dec!(5000000),
            period_days: 30,
        };

        // Velocity = Volume / Market Cap = 1000000 / 5000000 = 0.2
        assert_eq!(metrics.calculate_velocity(), dec!(0.2));

        // Annualized = 0.2 × (365 / 30) ≈ 2.43
        let annual = metrics.annualized_velocity();
        assert!(annual > dec!(2) && annual < dec!(3));
    }

    #[test]
    fn test_velocity_level() {
        let low = VelocityMetrics {
            volume: dec!(100000),
            market_cap: dec!(10000000),
            circulating_supply: dec!(1000000),
            period_days: 30,
        };
        // velocity = 100000/10000000 = 0.01, annualized = 0.01 * 12.17 ≈ 0.12 (< 5, so Low)
        assert_eq!(low.velocity_level(), VelocityLevel::Low);

        let high = VelocityMetrics {
            volume: dec!(10000000),
            market_cap: dec!(5000000),
            circulating_supply: dec!(1000000),
            period_days: 30,
        };
        // velocity = 10M/5M = 2, annualized = 2 * 12.17 ≈ 24.3 (> 20, <= 50, so High)
        assert_eq!(high.velocity_level(), VelocityLevel::High);
    }

    #[test]
    fn test_velocity_manager() {
        let metrics = VelocityMetrics {
            volume: dec!(5000000),
            circulating_supply: dec!(10000000),
            market_cap: dec!(3000000),
            period_days: 30,
        };

        let manager = TokenVelocityManager::new(metrics, dec!(10.0));
        let current = manager.metrics.annualized_velocity();

        assert!(current > dec!(0));
    }

    #[test]
    fn test_velocity_reduction_mechanisms() {
        let metrics = VelocityMetrics {
            volume: dec!(5000000),
            circulating_supply: dec!(10000000),
            market_cap: dec!(3000000),
            period_days: 30,
        };

        let mut manager = TokenVelocityManager::new(metrics, dec!(10.0));

        manager.add_mechanism(VelocityReductionMechanism::Staking {
            apy: dec!(0.20),
            lock_days: 365,
        });

        manager.add_mechanism(VelocityReductionMechanism::TransactionFees {
            fee_pct: dec!(0.005),
        });

        let reduction = manager.calculate_velocity_reduction();
        assert!(reduction > dec!(0));

        let projected = manager.projected_velocity();
        let current = manager.metrics.annualized_velocity();
        assert!(projected < current);
    }

    #[test]
    fn test_staking_position() {
        let stake = StakePosition {
            user_id: 1,
            amount: dec!(10000),
            lock_days: 365,
            start_date: Utc::now() - Duration::days(180),
            apy: dec!(0.20),
        };

        let rewards = stake.calculate_rewards(Utc::now());
        // ~180 days ≈ 0.5 years, rewards ≈ 10000 × 0.20 × 0.5 ≈ 1000
        assert!(rewards > dec!(900) && rewards < dec!(1100));

        assert!(!stake.is_unlocked(Utc::now()));
        assert!(stake.is_unlocked(Utc::now() + Duration::days(200)));
    }

    #[test]
    fn test_staking_optimizer() {
        let mut optimizer = StakingIncentiveOptimizer::new(dec!(100000), dec!(50.0));

        optimizer.add_stake(StakePosition {
            user_id: 1,
            amount: dec!(100000),
            lock_days: 365,
            start_date: Utc::now(),
            apy: dec!(0.15),
        });

        let ratio = optimizer.current_staking_ratio(dec!(1000000));
        // 100000 / 1000000 × 100 = 10%
        assert_eq!(ratio, dec!(10.0));

        let optimal = optimizer.optimal_apy(dec!(1000000), dec!(0.15));
        // Should be higher than current since we're below target (50%)
        assert!(optimal > dec!(0.15));
    }

    #[test]
    fn test_staking_tiers() {
        let optimizer = StakingIncentiveOptimizer::new(dec!(100000), dec!(50.0));

        let tiers = optimizer.optimize_tiers(dec!(1000000));

        assert_eq!(tiers.len(), 5);
        // Longer locks should have higher APY
        assert!(tiers[4].apy > tiers[0].apy);
    }

    #[test]
    fn test_burn_rate_manager() {
        let mut manager = BurnRateManager::new(dec!(2.0)); // 2% annual burn

        manager.record_burn(BurnEvent {
            date: Utc::now(),
            amount: dec!(10000),
            reason: "Protocol fee burn".to_string(),
        });

        assert_eq!(manager.total_burned(), dec!(10000));
    }

    #[test]
    fn test_target_burn_rate() {
        let mut manager = BurnRateManager::new(dec!(2.0));
        manager.set_target_supply(dec!(5000000));

        let target_rate = manager.calculate_target_burn_rate(dec!(10000000), dec!(5.0));

        assert!(target_rate.is_some());
        // Need to burn 5M over 5 years = 1M/year = 10% of 10M
        let rate = target_rate.unwrap();
        assert!(rate > dec!(9) && rate < dec!(11));
    }

    #[test]
    fn test_supply_projection() {
        let manager = BurnRateManager::new(dec!(5.0)); // 5% annual burn

        let projected = manager.project_supply(dec!(10000000), dec!(1.0));

        // After 1 year with 5% burn: 10M × 0.95 = 9.5M
        assert!(projected > dec!(9400000) && projected < dec!(9600000));
    }

    #[test]
    fn test_burn_rate_velocity_adjustment() {
        let mut manager = BurnRateManager::new(dec!(0.05)); // Start with 0.05 (5%)

        let high_velocity = VelocityMetrics {
            volume: dec!(100000000),
            market_cap: dec!(5000000),
            circulating_supply: dec!(10000000),
            period_days: 30,
        };

        let initial_rate = manager.burn_rate;
        manager.adjust_for_velocity(&high_velocity);

        // Burn rate should increase for high velocity (0.05 * 1.5 = 0.075)
        assert!(manager.burn_rate > initial_rate);
    }
}
