//! Dynamic Supply Mechanisms
//!
//! This module implements elastic supply tokens, rebase mechanisms, and dynamic
//! supply expansion/contraction rules for tokens in the Kaccy Protocol.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Rebase policy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebasePolicy {
    /// Expand supply when price is above target
    TargetPrice,
    /// Rebase based on market cap target
    TargetMarketCap,
    /// Rebase based on volatility dampening
    VolatilityDampening,
    /// Custom rebase policy
    Custom,
}

/// Direction of supply change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupplyChangeDirection {
    /// Supply is increasing
    Expansion,
    /// Supply is decreasing
    Contraction,
    /// No change to supply
    NoChange,
}

/// Rebase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseConfig {
    /// Target price for the token (in BTC)
    pub target_price: Decimal,
    /// Rebase interval (e.g., every 24 hours)
    pub rebase_interval: Duration,
    /// Maximum rebase percentage per period (e.g., 10%)
    pub max_rebase_percentage: Decimal,
    /// Minimum rebase percentage (e.g., 0.1%)
    pub min_rebase_percentage: Decimal,
    /// Rebase policy
    pub policy: RebasePolicy,
    /// Dampening factor (0.0 to 1.0) - how aggressive the rebase is
    pub dampening_factor: Decimal,
}

impl Default for RebaseConfig {
    fn default() -> Self {
        Self {
            target_price: Decimal::ONE,                  // 1 BTC
            rebase_interval: Duration::from_secs(86400), // 24 hours
            max_rebase_percentage: Decimal::new(10, 0),  // 10%
            min_rebase_percentage: Decimal::new(1, 1),   // 0.1%
            policy: RebasePolicy::TargetPrice,
            dampening_factor: Decimal::new(5, 1), // 0.5
        }
    }
}

/// Rebase event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseEvent {
    /// Unique identifier for this rebase event
    pub event_id: String,
    /// Token that was rebased
    pub token_id: String,
    /// When this rebase occurred
    pub timestamp: SystemTime,
    /// Total supply before the rebase
    pub old_supply: Decimal,
    /// Total supply after the rebase
    pub new_supply: Decimal,
    /// Absolute change in supply
    pub supply_change: Decimal,
    /// Percentage change in supply
    pub supply_change_percentage: Decimal,
    /// Whether supply expanded, contracted, or stayed the same
    pub direction: SupplyChangeDirection,
    /// Market price at the time of the rebase
    pub current_price: Decimal,
    /// Target price used for the rebase calculation
    pub target_price: Decimal,
}

/// Elastic supply token manager
///
/// Manages tokens with dynamic supply that adjusts based on market conditions
pub struct ElasticSupplyManager {
    /// Rebase configuration
    config: RebaseConfig,
    /// Timestamp of the last executed rebase
    last_rebase_time: SystemTime,
    /// History of all executed rebase events
    rebase_history: Vec<RebaseEvent>,
    /// Monotonic counter used to generate unique event IDs
    event_counter: u64,
}

impl ElasticSupplyManager {
    /// Create a new elastic supply manager with the given configuration
    pub fn new(config: RebaseConfig) -> Self {
        Self {
            config,
            last_rebase_time: SystemTime::now(),
            rebase_history: Vec::new(),
            event_counter: 0,
        }
    }

    /// Create a new elastic supply manager with default configuration
    pub fn with_defaults() -> Self {
        Self::new(RebaseConfig::default())
    }

    /// Calculates the required rebase based on current market price
    pub fn calculate_rebase(
        &self,
        current_price: Decimal,
        current_supply: Decimal,
    ) -> Result<RebaseCalculation, CoreError> {
        let price_deviation = (current_price - self.config.target_price) / self.config.target_price;

        // Calculate raw rebase percentage
        let raw_rebase_pct = price_deviation * self.config.dampening_factor;

        // Apply limits
        let capped_rebase_pct = if raw_rebase_pct.abs() > self.config.max_rebase_percentage {
            if raw_rebase_pct.is_sign_positive() {
                self.config.max_rebase_percentage
            } else {
                -self.config.max_rebase_percentage
            }
        } else if raw_rebase_pct.abs() < self.config.min_rebase_percentage {
            Decimal::ZERO
        } else {
            raw_rebase_pct
        };

        let direction = if capped_rebase_pct > Decimal::ZERO {
            SupplyChangeDirection::Expansion
        } else if capped_rebase_pct < Decimal::ZERO {
            SupplyChangeDirection::Contraction
        } else {
            SupplyChangeDirection::NoChange
        };

        let supply_change = current_supply * capped_rebase_pct / Decimal::new(100, 0);
        let new_supply = current_supply + supply_change;

        Ok(RebaseCalculation {
            old_supply: current_supply,
            new_supply,
            supply_change,
            supply_change_percentage: capped_rebase_pct,
            direction,
            price_deviation,
        })
    }

    /// Checks if it's time for a rebase
    pub fn is_rebase_due(&self) -> bool {
        let elapsed = SystemTime::now()
            .duration_since(self.last_rebase_time)
            .unwrap_or(Duration::ZERO);

        elapsed >= self.config.rebase_interval
    }

    /// Executes a rebase
    pub fn execute_rebase(
        &mut self,
        token_id: String,
        current_price: Decimal,
        current_supply: Decimal,
    ) -> Result<RebaseEvent, CoreError> {
        if !self.is_rebase_due() {
            return Err(CoreError::InvalidState(
                "Rebase interval has not elapsed".to_string(),
            ));
        }

        let calculation = self.calculate_rebase(current_price, current_supply)?;

        if calculation.direction == SupplyChangeDirection::NoChange {
            return Err(CoreError::InvalidState(
                "Price deviation too small for rebase".to_string(),
            ));
        }

        self.event_counter += 1;
        let event = RebaseEvent {
            event_id: format!("rebase_{}", self.event_counter),
            token_id,
            timestamp: SystemTime::now(),
            old_supply: calculation.old_supply,
            new_supply: calculation.new_supply,
            supply_change: calculation.supply_change,
            supply_change_percentage: calculation.supply_change_percentage,
            direction: calculation.direction,
            current_price,
            target_price: self.config.target_price,
        };

        self.last_rebase_time = SystemTime::now();
        self.rebase_history.push(event.clone());

        Ok(event)
    }

    /// Gets rebase history
    pub fn get_rebase_history(&self) -> &[RebaseEvent] {
        &self.rebase_history
    }

    /// Gets time until next rebase
    pub fn time_until_next_rebase(&self) -> Duration {
        let elapsed = SystemTime::now()
            .duration_since(self.last_rebase_time)
            .unwrap_or(Duration::ZERO);

        self.config.rebase_interval.saturating_sub(elapsed)
    }

    /// Calculates the effective balance after all rebases for a holder
    pub fn calculate_holder_balance(
        &self,
        initial_balance: Decimal,
        since_timestamp: SystemTime,
    ) -> Decimal {
        let mut effective_balance = initial_balance;

        for event in &self.rebase_history {
            if event.timestamp >= since_timestamp {
                let rebase_multiplier = event.new_supply / event.old_supply;
                effective_balance *= rebase_multiplier;
            }
        }

        effective_balance
    }
}

/// Result of a rebase calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseCalculation {
    /// Supply before the rebase
    pub old_supply: Decimal,
    /// Proposed supply after the rebase
    pub new_supply: Decimal,
    /// Absolute change in supply
    pub supply_change: Decimal,
    /// Percentage change in supply
    pub supply_change_percentage: Decimal,
    /// Whether supply would expand, contract, or stay the same
    pub direction: SupplyChangeDirection,
    /// Relative deviation of current price from target
    pub price_deviation: Decimal,
}

/// Supply expansion rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyExpansionRules {
    /// Maximum total supply cap
    pub max_total_supply: Option<Decimal>,
    /// Conditions that trigger expansion
    pub expansion_triggers: Vec<ExpansionTrigger>,
    /// Rate limit for expansion (max per day)
    pub max_expansion_per_day: Decimal,
}

/// Condition that causes a supply expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpansionTrigger {
    /// Expand when price exceeds threshold
    PriceThreshold {
        /// Price level that triggers expansion
        threshold: Decimal,
    },
    /// Expand when trading volume exceeds threshold
    VolumeThreshold {
        /// Volume level that triggers expansion
        threshold: Decimal,
    },
    /// Expand when liquidity ratio falls below threshold
    LiquidityRatio {
        /// Minimum acceptable liquidity ratio
        min_ratio: Decimal,
    },
    /// Scheduled expansion
    Scheduled {
        /// How often to expand supply
        interval: Duration,
    },
}

/// Supply contraction rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyContractionRules {
    /// Minimum total supply floor
    pub min_total_supply: Option<Decimal>,
    /// Conditions that trigger contraction
    pub contraction_triggers: Vec<ContractionTrigger>,
    /// Rate limit for contraction (max per day)
    pub max_contraction_per_day: Decimal,
}

/// Condition that causes a supply contraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractionTrigger {
    /// Contract when price falls below threshold
    PriceThreshold {
        /// Price level below which contraction triggers
        threshold: Decimal,
    },
    /// Contract when trading volume falls below threshold
    VolumeThreshold {
        /// Volume level below which contraction triggers
        threshold: Decimal,
    },
    /// Burn mechanism on transactions
    TransactionBurn {
        /// Fraction of each transaction amount to burn
        burn_rate: Decimal,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebase_expansion() {
        let manager = ElasticSupplyManager::with_defaults();

        let current_supply = Decimal::new(1000000, 0);
        let current_price = Decimal::new(15, 1); // 1.5 BTC (above target of 1.0)

        let calculation = manager
            .calculate_rebase(current_price, current_supply)
            .unwrap();

        assert_eq!(calculation.direction, SupplyChangeDirection::Expansion);
        assert!(calculation.new_supply > calculation.old_supply);
        assert!(calculation.supply_change > Decimal::ZERO);
    }

    #[test]
    fn test_rebase_contraction() {
        let manager = ElasticSupplyManager::with_defaults();

        let current_supply = Decimal::new(1000000, 0);
        let current_price = Decimal::new(7, 1); // 0.7 BTC (below target of 1.0)

        let calculation = manager
            .calculate_rebase(current_price, current_supply)
            .unwrap();

        assert_eq!(calculation.direction, SupplyChangeDirection::Contraction);
        assert!(calculation.new_supply < calculation.old_supply);
        assert!(calculation.supply_change < Decimal::ZERO);
    }

    #[test]
    fn test_rebase_no_change() {
        let manager = ElasticSupplyManager::with_defaults();

        let current_supply = Decimal::new(1000000, 0);
        let current_price = Decimal::new(10005, 4); // 1.0005 BTC (very close to target)

        let calculation = manager
            .calculate_rebase(current_price, current_supply)
            .unwrap();

        // Should be no change due to min_rebase_percentage
        assert_eq!(calculation.direction, SupplyChangeDirection::NoChange);
        assert_eq!(calculation.supply_change, Decimal::ZERO);
    }

    #[test]
    fn test_rebase_capping() {
        let manager = ElasticSupplyManager::with_defaults();

        let current_supply = Decimal::new(1000000, 0);
        let current_price = Decimal::new(100, 0); // 100 BTC (way above target)

        let calculation = manager
            .calculate_rebase(current_price, current_supply)
            .unwrap();

        // Should be capped at max_rebase_percentage (10%)
        assert_eq!(calculation.supply_change_percentage, Decimal::new(10, 0));
    }

    #[test]
    fn test_is_rebase_due() {
        let manager = ElasticSupplyManager::with_defaults();

        // Just created, should not be due yet
        assert!(!manager.is_rebase_due());
    }

    #[test]
    fn test_holder_balance_calculation() {
        let mut manager = ElasticSupplyManager::with_defaults();
        let initial_time = SystemTime::now() - Duration::from_secs(10);

        // Simulate a rebase event that doubles the supply
        let event = RebaseEvent {
            event_id: "test_rebase_1".to_string(),
            token_id: "token1".to_string(),
            timestamp: SystemTime::now(),
            old_supply: Decimal::new(1000000, 0),
            new_supply: Decimal::new(2000000, 0),
            supply_change: Decimal::new(1000000, 0),
            supply_change_percentage: Decimal::new(100, 0),
            direction: SupplyChangeDirection::Expansion,
            current_price: Decimal::new(2, 0),
            target_price: Decimal::ONE,
        };

        manager.rebase_history.push(event);

        let initial_balance = Decimal::new(100, 0);
        let effective_balance = manager.calculate_holder_balance(initial_balance, initial_time);

        // Balance should double
        assert_eq!(effective_balance, Decimal::new(200, 0));
    }

    #[test]
    fn test_execute_rebase_not_due() {
        let mut manager = ElasticSupplyManager::with_defaults();

        let result = manager.execute_rebase(
            "token1".to_string(),
            Decimal::new(15, 1),
            Decimal::new(1000000, 0),
        );

        assert!(result.is_err());
    }
}
