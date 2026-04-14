//! Token Buyback Automation
//!
//! This module implements automated token buyback mechanisms including
//! scheduled buybacks, price-triggered buybacks, and treasury management.

use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

/// Buyback strategy types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuybackStrategy {
    /// Buy back tokens on a fixed schedule
    Scheduled,
    /// Buy back when price falls below threshold
    PriceTriggered,
    /// Buy back based on treasury balance
    TreasuryBased,
    /// Buy back to maintain specific metrics
    MetricTargeted,
}

/// Buyback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuybackConfig {
    /// Strategy type
    pub strategy: BuybackStrategy,
    /// Minimum time between buybacks
    pub min_interval: Duration,
    /// Maximum amount to buy back per transaction (in BTC)
    pub max_amount_per_buyback: Decimal,
    /// Maximum slippage tolerance (percentage)
    pub max_slippage: Decimal,
    /// Whether to burn bought back tokens
    pub burn_tokens: bool,
}

impl Default for BuybackConfig {
    fn default() -> Self {
        Self {
            strategy: BuybackStrategy::Scheduled,
            min_interval: Duration::from_secs(86400), // 24 hours
            max_amount_per_buyback: Decimal::new(10, 0), // 10 BTC
            max_slippage: Decimal::new(5, 0),         // 5%
            burn_tokens: true,
        }
    }
}

/// Scheduled buyback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledBuybackConfig {
    /// Amount to buy back each interval (in BTC)
    pub amount_per_interval: Decimal,
    /// Interval between buybacks
    pub interval: Duration,
}

impl Default for ScheduledBuybackConfig {
    fn default() -> Self {
        Self {
            amount_per_interval: Decimal::new(5, 0), // 5 BTC
            interval: Duration::from_secs(604800),   // 1 week
        }
    }
}

/// Price-triggered buyback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTriggeredBuybackConfig {
    /// Price threshold below which buyback is triggered
    pub trigger_price: Decimal,
    /// Amount to buy back when triggered (in BTC)
    pub buyback_amount: Decimal,
    /// Cooldown period after a buyback
    pub cooldown: Duration,
}

/// Treasury-based buyback configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryBuybackConfig {
    /// Percentage of treasury to use for buybacks (0-100)
    pub treasury_allocation_pct: Decimal,
    /// Minimum treasury balance required before buybacks (in BTC)
    pub min_treasury_balance: Decimal,
}

/// Buyback execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuybackExecution {
    /// Unique identifier for this execution
    pub execution_id: String,
    /// Time when the buyback was executed
    pub timestamp: SystemTime,
    /// Token that was bought back
    pub token_id: String,
    /// BTC spent on the buyback
    pub btc_spent: Decimal,
    /// Number of tokens purchased
    pub tokens_bought: Decimal,
    /// Weighted average price paid per token
    pub average_price: Decimal,
    /// Whether the bought-back tokens were burned
    pub burned: bool,
    /// Strategy that triggered this buyback
    pub strategy: BuybackStrategy,
}

/// Automated buyback manager
pub struct AutomatedBuybackManager {
    /// Base buyback configuration
    base_config: BuybackConfig,
    /// Scheduled buyback configuration, if enabled
    scheduled_config: Option<ScheduledBuybackConfig>,
    /// Price-triggered buyback configuration, if enabled
    price_triggered_config: Option<PriceTriggeredBuybackConfig>,
    /// Treasury-based buyback configuration, if enabled
    treasury_config: Option<TreasuryBuybackConfig>,
    /// Rolling history of recent executions
    execution_history: VecDeque<BuybackExecution>,
    /// Time of the most recent buyback
    last_buyback_time: Option<SystemTime>,
    /// Monotonically increasing counter for execution IDs
    execution_counter: u64,
}

impl AutomatedBuybackManager {
    /// Create a new buyback manager with the given base configuration
    pub fn new(base_config: BuybackConfig) -> Self {
        Self {
            base_config,
            scheduled_config: None,
            price_triggered_config: None,
            treasury_config: None,
            execution_history: VecDeque::new(),
            last_buyback_time: None,
            execution_counter: 0,
        }
    }

    /// Create a buyback manager with default configuration
    pub fn with_defaults() -> Self {
        Self::new(BuybackConfig::default())
    }

    /// Sets scheduled buyback configuration
    pub fn with_scheduled_buyback(mut self, config: ScheduledBuybackConfig) -> Self {
        self.scheduled_config = Some(config);
        self
    }

    /// Sets price-triggered buyback configuration
    pub fn with_price_triggered_buyback(mut self, config: PriceTriggeredBuybackConfig) -> Self {
        self.price_triggered_config = Some(config);
        self
    }

    /// Sets treasury-based buyback configuration
    pub fn with_treasury_buyback(mut self, config: TreasuryBuybackConfig) -> Self {
        self.treasury_config = Some(config);
        self
    }

    /// Checks if a buyback should be executed based on current conditions
    pub fn should_execute_buyback(
        &self,
        current_price: Decimal,
        treasury_balance: Decimal,
    ) -> Option<BuybackRecommendation> {
        // Check minimum interval
        if let Some(last_time) = self.last_buyback_time {
            let elapsed = SystemTime::now()
                .duration_since(last_time)
                .unwrap_or(Duration::ZERO);

            if elapsed < self.base_config.min_interval {
                return None;
            }
        }

        // Check scheduled buyback
        if let Some(config) = &self.scheduled_config {
            if self.is_scheduled_buyback_due(config) {
                return Some(BuybackRecommendation {
                    strategy: BuybackStrategy::Scheduled,
                    recommended_amount: config.amount_per_interval,
                    reason: "Scheduled buyback interval elapsed".to_string(),
                });
            }
        }

        // Check price-triggered buyback
        if let Some(config) = &self.price_triggered_config {
            if current_price < config.trigger_price {
                // Check cooldown
                let cooldown_ok = if let Some(last_time) = self.last_buyback_time {
                    let elapsed = SystemTime::now()
                        .duration_since(last_time)
                        .unwrap_or(Duration::ZERO);
                    elapsed >= config.cooldown
                } else {
                    true
                };

                if cooldown_ok {
                    return Some(BuybackRecommendation {
                        strategy: BuybackStrategy::PriceTriggered,
                        recommended_amount: config.buyback_amount,
                        reason: format!(
                            "Price {} below trigger {}",
                            current_price, config.trigger_price
                        ),
                    });
                }
            }
        }

        // Check treasury-based buyback
        if let Some(config) = &self.treasury_config {
            if treasury_balance >= config.min_treasury_balance {
                let buyback_amount =
                    treasury_balance * config.treasury_allocation_pct / Decimal::new(100, 0);
                if buyback_amount > Decimal::ZERO {
                    return Some(BuybackRecommendation {
                        strategy: BuybackStrategy::TreasuryBased,
                        recommended_amount: buyback_amount
                            .min(self.base_config.max_amount_per_buyback),
                        reason: format!("Treasury balance {} exceeds minimum", treasury_balance),
                    });
                }
            }
        }

        None
    }

    /// Checks if scheduled buyback is due
    fn is_scheduled_buyback_due(&self, config: &ScheduledBuybackConfig) -> bool {
        match self.last_buyback_time {
            None => true, // First buyback
            Some(last_time) => {
                let elapsed = SystemTime::now()
                    .duration_since(last_time)
                    .unwrap_or(Duration::ZERO);
                elapsed >= config.interval
            }
        }
    }

    /// Executes a buyback
    pub fn execute_buyback(
        &mut self,
        token_id: String,
        btc_amount: Decimal,
        token_amount: Decimal,
        strategy: BuybackStrategy,
    ) -> Result<BuybackExecution, CoreError> {
        // Validate amount
        if btc_amount > self.base_config.max_amount_per_buyback {
            return Err(CoreError::Validation(format!(
                "Buyback amount {} exceeds maximum {}",
                btc_amount, self.base_config.max_amount_per_buyback
            )));
        }

        if btc_amount <= Decimal::ZERO || token_amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Buyback amounts must be positive".to_string(),
            ));
        }

        let average_price = btc_amount / token_amount;

        self.execution_counter += 1;
        let execution = BuybackExecution {
            execution_id: format!("buyback_{}", self.execution_counter),
            timestamp: SystemTime::now(),
            token_id,
            btc_spent: btc_amount,
            tokens_bought: token_amount,
            average_price,
            burned: self.base_config.burn_tokens,
            strategy,
        };

        self.execution_history.push_back(execution.clone());
        self.last_buyback_time = Some(SystemTime::now());

        // Keep history limited to last 100 executions
        if self.execution_history.len() > 100 {
            self.execution_history.pop_front();
        }

        Ok(execution)
    }

    /// Gets buyback statistics
    pub fn get_statistics(&self) -> BuybackStatistics {
        let total_btc_spent: Decimal = self.execution_history.iter().map(|e| e.btc_spent).sum();

        let total_tokens_bought: Decimal =
            self.execution_history.iter().map(|e| e.tokens_bought).sum();

        let total_tokens_burned: Decimal = self
            .execution_history
            .iter()
            .filter(|e| e.burned)
            .map(|e| e.tokens_bought)
            .sum();

        let average_price = if total_tokens_bought > Decimal::ZERO {
            total_btc_spent / total_tokens_bought
        } else {
            Decimal::ZERO
        };

        BuybackStatistics {
            total_executions: self.execution_history.len(),
            total_btc_spent,
            total_tokens_bought,
            total_tokens_burned,
            average_price,
        }
    }

    /// Gets buyback history
    pub fn get_history(&self, limit: usize) -> Vec<&BuybackExecution> {
        self.execution_history.iter().rev().take(limit).collect()
    }

    /// Gets time until next scheduled buyback
    pub fn time_until_next_buyback(&self) -> Option<Duration> {
        self.scheduled_config.as_ref().and_then(|config| {
            self.last_buyback_time.map(|last_time| {
                let elapsed = SystemTime::now()
                    .duration_since(last_time)
                    .unwrap_or(Duration::ZERO);
                config.interval.saturating_sub(elapsed)
            })
        })
    }
}

/// Buyback recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuybackRecommendation {
    /// Strategy that generated the recommendation
    pub strategy: BuybackStrategy,
    /// Recommended amount to spend (in BTC)
    pub recommended_amount: Decimal,
    /// Human-readable explanation for the recommendation
    pub reason: String,
}

/// Buyback statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuybackStatistics {
    /// Number of buyback executions in history
    pub total_executions: usize,
    /// Total BTC spent on buybacks
    pub total_btc_spent: Decimal,
    /// Total tokens bought back
    pub total_tokens_bought: Decimal,
    /// Total tokens burned after buyback
    pub total_tokens_burned: Decimal,
    /// Lifetime average price paid per token
    pub average_price: Decimal,
}

/// Treasury manager for buybacks
pub struct TreasuryManager {
    /// Current treasury balance in BTC
    total_balance: Decimal,
    /// Amount already earmarked for upcoming buybacks
    allocated_for_buyback: Decimal,
    /// Minimum balance to keep in reserve
    min_reserve: Decimal,
}

impl TreasuryManager {
    /// Create a new treasury manager with an initial balance and reserve requirement
    pub fn new(total_balance: Decimal, min_reserve: Decimal) -> Self {
        Self {
            total_balance,
            allocated_for_buyback: Decimal::ZERO,
            min_reserve,
        }
    }

    /// Allocates funds for buyback
    pub fn allocate_for_buyback(&mut self, amount: Decimal) -> Result<(), CoreError> {
        let available = self.total_balance - self.min_reserve - self.allocated_for_buyback;

        if amount > available {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available,
            });
        }

        self.allocated_for_buyback += amount;
        Ok(())
    }

    /// Executes buyback and updates balances
    pub fn execute_buyback(&mut self, btc_spent: Decimal) -> Result<(), CoreError> {
        if btc_spent > self.allocated_for_buyback {
            return Err(CoreError::InsufficientBalance {
                required: btc_spent,
                available: self.allocated_for_buyback,
            });
        }

        self.total_balance -= btc_spent;
        self.allocated_for_buyback -= btc_spent;
        Ok(())
    }

    /// Gets available balance for buybacks
    pub fn available_for_buyback(&self) -> Decimal {
        (self.total_balance - self.min_reserve - self.allocated_for_buyback).max(Decimal::ZERO)
    }

    /// Gets total treasury balance
    pub fn total_balance(&self) -> Decimal {
        self.total_balance
    }

    /// Adds to treasury balance
    pub fn add_to_treasury(&mut self, amount: Decimal) {
        self.total_balance += amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduled_buyback() {
        let config = BuybackConfig {
            strategy: BuybackStrategy::Scheduled,
            ..Default::default()
        };

        let manager =
            AutomatedBuybackManager::new(config).with_scheduled_buyback(ScheduledBuybackConfig {
                amount_per_interval: Decimal::new(5, 0),
                interval: Duration::from_secs(60),
            });

        // First buyback should be due
        let recommendation = manager.should_execute_buyback(Decimal::ONE, Decimal::new(100, 0));

        assert!(recommendation.is_some());
        if let Some(rec) = recommendation {
            assert_eq!(rec.strategy, BuybackStrategy::Scheduled);
            assert_eq!(rec.recommended_amount, Decimal::new(5, 0));
        }
    }

    #[test]
    fn test_price_triggered_buyback() {
        let config = BuybackConfig {
            strategy: BuybackStrategy::PriceTriggered,
            ..Default::default()
        };

        let manager = AutomatedBuybackManager::new(config).with_price_triggered_buyback(
            PriceTriggeredBuybackConfig {
                trigger_price: Decimal::new(10, 0),
                buyback_amount: Decimal::new(3, 0),
                cooldown: Duration::from_secs(3600),
            },
        );

        // Price above trigger - no buyback
        let recommendation1 =
            manager.should_execute_buyback(Decimal::new(15, 0), Decimal::new(100, 0));
        assert!(recommendation1.is_none());

        // Price below trigger - should recommend buyback
        let recommendation2 =
            manager.should_execute_buyback(Decimal::new(8, 0), Decimal::new(100, 0));
        assert!(recommendation2.is_some());
    }

    #[test]
    fn test_execute_buyback() {
        let mut manager = AutomatedBuybackManager::with_defaults();

        let result = manager.execute_buyback(
            "token1".to_string(),
            Decimal::new(5, 0),
            Decimal::new(100, 0),
            BuybackStrategy::Scheduled,
        );

        assert!(result.is_ok());
        let execution = result.unwrap();
        assert_eq!(execution.btc_spent, Decimal::new(5, 0));
        assert_eq!(execution.tokens_bought, Decimal::new(100, 0));
        assert_eq!(execution.average_price, Decimal::new(5, 2)); // 0.05
    }

    #[test]
    fn test_buyback_statistics() {
        let mut manager = AutomatedBuybackManager::with_defaults();

        manager
            .execute_buyback(
                "token1".to_string(),
                Decimal::new(5, 0),
                Decimal::new(100, 0),
                BuybackStrategy::Scheduled,
            )
            .unwrap();

        manager
            .execute_buyback(
                "token1".to_string(),
                Decimal::new(3, 0),
                Decimal::new(60, 0),
                BuybackStrategy::Scheduled,
            )
            .unwrap();

        let stats = manager.get_statistics();
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.total_btc_spent, Decimal::new(8, 0));
        assert_eq!(stats.total_tokens_bought, Decimal::new(160, 0));
    }

    #[test]
    fn test_treasury_manager() {
        let mut treasury = TreasuryManager::new(
            Decimal::new(100, 0), // 100 BTC total
            Decimal::new(20, 0),  // 20 BTC reserve
        );

        // Should be able to allocate 80 BTC
        assert!(treasury.allocate_for_buyback(Decimal::new(50, 0)).is_ok());
        assert_eq!(treasury.available_for_buyback(), Decimal::new(30, 0));

        // Execute buyback
        assert!(treasury.execute_buyback(Decimal::new(25, 0)).is_ok());
        assert_eq!(treasury.total_balance(), Decimal::new(75, 0));
        assert_eq!(treasury.allocated_for_buyback, Decimal::new(25, 0));
    }

    #[test]
    fn test_treasury_insufficient_balance() {
        let mut treasury = TreasuryManager::new(Decimal::new(100, 0), Decimal::new(20, 0));

        // Try to allocate more than available
        let result = treasury.allocate_for_buyback(Decimal::new(90, 0));
        assert!(result.is_err());
    }

    #[test]
    fn test_max_buyback_amount_validation() {
        let mut manager = AutomatedBuybackManager::with_defaults();

        // Try to execute buyback exceeding max
        let result = manager.execute_buyback(
            "token1".to_string(),
            Decimal::new(20, 0), // Exceeds default max of 10
            Decimal::new(100, 0),
            BuybackStrategy::Scheduled,
        );

        assert!(result.is_err());
    }
}
