//! Automated trading strategies
//!
//! This module implements automated trading strategies including grid trading,
//! dollar-cost averaging (DCA), and portfolio rebalancing.
//!
//! # Strategies
//!
//! - **Grid Trading**: Places buy/sell orders at regular price intervals
//! - **Dollar-Cost Averaging (DCA)**: Periodic purchases of fixed amounts
//! - **Portfolio Rebalancing**: Maintains target asset allocation

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

// ==================== Grid Trading ====================

/// Grid trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridTradingConfig {
    /// Token to trade
    pub token_id: Uuid,
    /// Lower price bound
    pub lower_price: Decimal,
    /// Upper price bound
    pub upper_price: Decimal,
    /// Number of grid levels
    pub grid_levels: usize,
    /// Amount per grid order
    pub order_amount: Decimal,
    /// Take profit percentage (optional)
    pub take_profit_pct: Option<Decimal>,
}

impl GridTradingConfig {
    /// Create a new grid trading configuration
    pub fn new(
        token_id: Uuid,
        lower_price: Decimal,
        upper_price: Decimal,
        grid_levels: usize,
        order_amount: Decimal,
    ) -> Result<Self> {
        if lower_price >= upper_price {
            return Err(CoreError::Validation(
                "Lower price must be less than upper price".to_string(),
            ));
        }

        if grid_levels < 2 {
            return Err(CoreError::Validation(
                "Grid levels must be at least 2".to_string(),
            ));
        }

        if order_amount <= dec!(0) {
            return Err(CoreError::Validation(
                "Order amount must be positive".to_string(),
            ));
        }

        Ok(Self {
            token_id,
            lower_price,
            upper_price,
            grid_levels,
            order_amount,
            take_profit_pct: None,
        })
    }

    /// Calculate grid level prices
    pub fn calculate_grid_prices(&self) -> Vec<Decimal> {
        let price_range = self.upper_price - self.lower_price;
        let step = price_range / Decimal::from(self.grid_levels - 1);

        (0..self.grid_levels)
            .map(|i| self.lower_price + step * Decimal::from(i))
            .collect()
    }
}

/// Grid trading order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridOrder {
    /// Order ID
    pub id: Uuid,
    /// Grid level (0 to grid_levels-1)
    pub level: usize,
    /// Price for this grid level
    pub price: Decimal,
    /// Amount
    pub amount: Decimal,
    /// Is buy order (false = sell)
    pub is_buy: bool,
    /// Whether order has been filled
    pub is_filled: bool,
    /// When filled (if filled)
    pub filled_at: Option<DateTime<Utc>>,
}

/// Grid trading statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridTradingStats {
    /// Total trades executed
    pub total_trades: usize,
    /// Total profit realized
    pub total_profit: Decimal,
    /// Average profit per trade
    pub avg_profit_per_trade: Decimal,
    /// Active grid orders
    pub active_orders: usize,
    /// Current position size
    pub position_size: Decimal,
}

/// Grid trading bot
pub struct GridTradingBot {
    /// Configuration
    pub config: GridTradingConfig,
    /// Grid orders
    pub orders: Vec<GridOrder>,
    /// Filled orders history
    pub filled_orders: Vec<GridOrder>,
    /// Total profit
    pub total_profit: Decimal,
    /// Current position
    pub position: Decimal,
}

impl GridTradingBot {
    /// Create a new grid trading bot
    pub fn new(config: GridTradingConfig) -> Self {
        let prices = config.calculate_grid_prices();
        let mut orders = Vec::new();

        // Create buy orders for lower half, sell orders for upper half
        let mid_level = config.grid_levels / 2;

        for (level, &price) in prices.iter().enumerate() {
            let is_buy = level < mid_level;
            orders.push(GridOrder {
                id: Uuid::new_v4(),
                level,
                price,
                amount: config.order_amount,
                is_buy,
                is_filled: false,
                filled_at: None,
            });
        }

        Self {
            config,
            orders,
            filled_orders: Vec::new(),
            total_profit: dec!(0),
            position: dec!(0),
        }
    }

    /// Process a price update and execute orders
    pub fn process_price(&mut self, current_price: Decimal) -> Vec<GridOrder> {
        let mut executed_orders = Vec::new();

        for order in &mut self.orders {
            if order.is_filled {
                continue;
            }

            let should_execute = if order.is_buy {
                current_price <= order.price
            } else {
                current_price >= order.price
            };

            if should_execute {
                order.is_filled = true;
                order.filled_at = Some(Utc::now());

                // Update position
                if order.is_buy {
                    self.position += order.amount;
                } else {
                    self.position -= order.amount;
                }

                executed_orders.push(order.clone());
                self.filled_orders.push(order.clone());
            }
        }

        // Rebalance: create opposite orders for filled orders
        for filled_order in &executed_orders {
            if let Some(take_profit) = self.config.take_profit_pct {
                let new_price = if filled_order.is_buy {
                    // If we bought, place sell order at take profit price
                    filled_order.price * (dec!(1) + take_profit / dec!(100))
                } else {
                    // If we sold, place buy order at take profit price
                    filled_order.price * (dec!(1) - take_profit / dec!(100))
                };

                // Only create new order if price is within grid bounds
                if new_price >= self.config.lower_price && new_price <= self.config.upper_price {
                    self.orders.push(GridOrder {
                        id: Uuid::new_v4(),
                        level: filled_order.level,
                        price: new_price,
                        amount: filled_order.amount,
                        is_buy: !filled_order.is_buy,
                        is_filled: false,
                        filled_at: None,
                    });
                }
            }
        }

        executed_orders
    }

    /// Get statistics
    pub fn stats(&self) -> GridTradingStats {
        let total_trades = self.filled_orders.len();
        let active_orders = self.orders.iter().filter(|o| !o.is_filled).count();
        let avg_profit_per_trade = if total_trades > 0 {
            self.total_profit / Decimal::from(total_trades)
        } else {
            dec!(0)
        };

        GridTradingStats {
            total_trades,
            total_profit: self.total_profit,
            avg_profit_per_trade,
            active_orders,
            position_size: self.position,
        }
    }
}

// ==================== Dollar-Cost Averaging (DCA) ====================

/// DCA strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcaConfig {
    /// Token to buy
    pub token_id: Uuid,
    /// Amount to buy each period
    pub amount_per_period: Decimal,
    /// Purchase interval
    pub interval: Duration,
    /// Whether to use market orders (true) or limit orders (false)
    pub use_market_orders: bool,
    /// Max price for limit orders (optional)
    pub max_price: Option<Decimal>,
}

/// DCA purchase record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcaPurchase {
    /// Purchase ID
    pub id: Uuid,
    /// Amount purchased
    pub amount: Decimal,
    /// Price paid
    pub price: Decimal,
    /// Total cost
    pub total_cost: Decimal,
    /// When purchased
    pub purchased_at: DateTime<Utc>,
}

/// DCA statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcaStats {
    /// Total purchases made
    pub total_purchases: usize,
    /// Total amount invested
    pub total_invested: Decimal,
    /// Total tokens acquired
    pub total_tokens: Decimal,
    /// Average purchase price
    pub average_price: Decimal,
    /// Next scheduled purchase
    pub next_purchase_at: DateTime<Utc>,
}

/// Dollar-cost averaging bot
pub struct DcaBot {
    /// Configuration
    pub config: DcaConfig,
    /// Purchase history
    pub purchases: Vec<DcaPurchase>,
    /// Last purchase time
    pub last_purchase_at: Option<DateTime<Utc>>,
    /// Total invested
    pub total_invested: Decimal,
    /// Total tokens acquired
    pub total_tokens: Decimal,
}

impl DcaBot {
    /// Create a new DCA bot
    pub fn new(config: DcaConfig) -> Self {
        Self {
            config,
            purchases: Vec::new(),
            last_purchase_at: None,
            total_invested: dec!(0),
            total_tokens: dec!(0),
        }
    }

    /// Check if it's time to make a purchase
    pub fn should_purchase(&self) -> bool {
        match self.last_purchase_at {
            None => true, // First purchase
            Some(last) => Utc::now() >= last + self.config.interval,
        }
    }

    /// Execute a purchase
    pub fn execute_purchase(&mut self, current_price: Decimal) -> Result<DcaPurchase> {
        // Check max price if set
        if let Some(max_price) = self.config.max_price {
            if current_price > max_price {
                return Err(CoreError::InvalidPrice(format!(
                    "Current price {} exceeds max price {}",
                    current_price, max_price
                )));
            }
        }

        let total_cost = self.config.amount_per_period;
        let amount = total_cost / current_price;

        let purchase = DcaPurchase {
            id: Uuid::new_v4(),
            amount,
            price: current_price,
            total_cost,
            purchased_at: Utc::now(),
        };

        self.purchases.push(purchase.clone());
        self.last_purchase_at = Some(Utc::now());
        self.total_invested += total_cost;
        self.total_tokens += amount;

        Ok(purchase)
    }

    /// Get statistics
    pub fn stats(&self) -> DcaStats {
        let average_price = if !self.total_tokens.is_zero() {
            self.total_invested / self.total_tokens
        } else {
            dec!(0)
        };

        let next_purchase_at = match self.last_purchase_at {
            Some(last) => last + self.config.interval,
            None => Utc::now(),
        };

        DcaStats {
            total_purchases: self.purchases.len(),
            total_invested: self.total_invested,
            total_tokens: self.total_tokens,
            average_price,
            next_purchase_at,
        }
    }
}

// ==================== Portfolio Rebalancing ====================

/// Target allocation for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAllocation {
    /// Token ID
    pub token_id: Uuid,
    /// Target percentage (0-100)
    pub target_pct: Decimal,
}

/// Portfolio rebalancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceConfig {
    /// Target allocations (must sum to 100%)
    pub allocations: Vec<TokenAllocation>,
    /// Rebalance threshold (percentage deviation that triggers rebalance)
    pub threshold_pct: Decimal,
    /// Minimum rebalancing interval
    pub min_interval: Duration,
}

impl RebalanceConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        let total: Decimal = self.allocations.iter().map(|a| a.target_pct).sum();

        if (total - dec!(100)).abs() > dec!(0.01) {
            return Err(CoreError::Validation(format!(
                "Allocations must sum to 100%, got {}",
                total
            )));
        }

        if self.threshold_pct <= dec!(0) || self.threshold_pct > dec!(50) {
            return Err(CoreError::Validation(
                "Threshold must be between 0 and 50%".to_string(),
            ));
        }

        Ok(())
    }
}

/// Current portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    /// Token ID
    pub token_id: Uuid,
    /// Current amount
    pub amount: Decimal,
    /// Current price
    pub price: Decimal,
    /// Current value
    pub value: Decimal,
    /// Current percentage of portfolio
    pub current_pct: Decimal,
    /// Target percentage
    pub target_pct: Decimal,
    /// Deviation from target
    pub deviation_pct: Decimal,
}

/// Portfolio rebalancing action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRebalanceAction {
    /// Token to trade
    pub token_id: Uuid,
    /// Amount to buy (positive) or sell (negative)
    pub amount: Decimal,
    /// Estimated value of trade
    pub value: Decimal,
}

/// Portfolio rebalancing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceStats {
    /// Total rebalances performed
    pub total_rebalances: usize,
    /// Last rebalance time
    pub last_rebalance_at: Option<DateTime<Utc>>,
    /// Next rebalance eligible time
    pub next_rebalance_at: DateTime<Utc>,
    /// Current portfolio value
    pub portfolio_value: Decimal,
    /// Maximum deviation from target
    pub max_deviation_pct: Decimal,
}

/// Portfolio rebalancing bot
pub struct PortfolioRebalancer {
    /// Configuration
    pub config: RebalanceConfig,
    /// Last rebalance time
    pub last_rebalance_at: Option<DateTime<Utc>>,
    /// Rebalance count
    pub rebalance_count: usize,
}

impl PortfolioRebalancer {
    /// Create a new portfolio rebalancer
    pub fn new(config: RebalanceConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            last_rebalance_at: None,
            rebalance_count: 0,
        })
    }

    /// Calculate current portfolio positions
    pub fn calculate_positions(
        &self,
        holdings: &HashMap<Uuid, Decimal>,
        prices: &HashMap<Uuid, Decimal>,
    ) -> Vec<PortfolioPosition> {
        let total_value: Decimal = holdings
            .iter()
            .map(|(token_id, &amount)| {
                let price = prices.get(token_id).copied().unwrap_or(dec!(0));
                amount * price
            })
            .sum();

        self.config
            .allocations
            .iter()
            .map(|allocation| {
                let amount = holdings
                    .get(&allocation.token_id)
                    .copied()
                    .unwrap_or(dec!(0));
                let price = prices.get(&allocation.token_id).copied().unwrap_or(dec!(0));
                let value = amount * price;
                let current_pct = if !total_value.is_zero() {
                    (value / total_value) * dec!(100)
                } else {
                    dec!(0)
                };
                let deviation_pct = current_pct - allocation.target_pct;

                PortfolioPosition {
                    token_id: allocation.token_id,
                    amount,
                    price,
                    value,
                    current_pct,
                    target_pct: allocation.target_pct,
                    deviation_pct,
                }
            })
            .collect()
    }

    /// Check if rebalancing is needed
    pub fn needs_rebalance(&self, positions: &[PortfolioPosition]) -> bool {
        // Check time constraint
        if let Some(last) = self.last_rebalance_at {
            if Utc::now() < last + self.config.min_interval {
                return false;
            }
        }

        // Check if any position exceeds threshold
        positions
            .iter()
            .any(|pos| pos.deviation_pct.abs() > self.config.threshold_pct)
    }

    /// Calculate rebalancing actions
    pub fn calculate_rebalance_actions(
        &self,
        positions: &[PortfolioPosition],
    ) -> Vec<PortfolioRebalanceAction> {
        let total_value: Decimal = positions.iter().map(|p| p.value).sum();

        positions
            .iter()
            .filter_map(|pos| {
                if pos.deviation_pct.abs() < dec!(0.01) {
                    return None; // Skip if deviation is negligible
                }

                let target_value = (pos.target_pct / dec!(100)) * total_value;
                let value_diff = target_value - pos.value;

                if value_diff.abs() < dec!(0.01) {
                    return None;
                }

                let amount = if !pos.price.is_zero() {
                    value_diff / pos.price
                } else {
                    dec!(0)
                };

                Some(PortfolioRebalanceAction {
                    token_id: pos.token_id,
                    amount,
                    value: value_diff,
                })
            })
            .collect()
    }

    /// Execute rebalancing
    pub fn execute_rebalance(&mut self) {
        self.last_rebalance_at = Some(Utc::now());
        self.rebalance_count += 1;
    }

    /// Get statistics
    pub fn stats(&self, positions: &[PortfolioPosition]) -> RebalanceStats {
        let portfolio_value: Decimal = positions.iter().map(|p| p.value).sum();
        let max_deviation_pct = positions
            .iter()
            .map(|p| p.deviation_pct.abs())
            .max()
            .unwrap_or(dec!(0));

        let next_rebalance_at = match self.last_rebalance_at {
            Some(last) => last + self.config.min_interval,
            None => Utc::now(),
        };

        RebalanceStats {
            total_rebalances: self.rebalance_count,
            last_rebalance_at: self.last_rebalance_at,
            next_rebalance_at,
            portfolio_value,
            max_deviation_pct,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_trading_config() {
        let token_id = Uuid::new_v4();
        let config = GridTradingConfig::new(token_id, dec!(90), dec!(110), 5, dec!(10)).unwrap();

        let prices = config.calculate_grid_prices();
        assert_eq!(prices.len(), 5);
        assert_eq!(prices[0], dec!(90));
        assert_eq!(prices[4], dec!(110));
    }

    #[test]
    fn test_grid_trading_bot() {
        let token_id = Uuid::new_v4();
        let config = GridTradingConfig::new(token_id, dec!(90), dec!(110), 5, dec!(10)).unwrap();

        let mut bot = GridTradingBot::new(config);

        // Process a price that should trigger buy orders
        let executed = bot.process_price(dec!(92));
        assert!(!executed.is_empty());
    }

    #[test]
    fn test_dca_bot() {
        let token_id = Uuid::new_v4();
        let config = DcaConfig {
            token_id,
            amount_per_period: dec!(100),
            interval: Duration::days(7),
            use_market_orders: true,
            max_price: Some(dec!(150)),
        };

        let mut bot = DcaBot::new(config);
        assert!(bot.should_purchase());

        let purchase = bot.execute_purchase(dec!(100)).unwrap();
        assert_eq!(purchase.amount, dec!(1));
        assert_eq!(purchase.total_cost, dec!(100));
    }

    #[test]
    fn test_dca_max_price() {
        let token_id = Uuid::new_v4();
        let config = DcaConfig {
            token_id,
            amount_per_period: dec!(100),
            interval: Duration::days(7),
            use_market_orders: false,
            max_price: Some(dec!(100)),
        };

        let mut bot = DcaBot::new(config);

        // Should fail when price exceeds max
        let result = bot.execute_purchase(dec!(150));
        assert!(result.is_err());
    }

    #[test]
    fn test_portfolio_rebalancer() {
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let config = RebalanceConfig {
            allocations: vec![
                TokenAllocation {
                    token_id: token1,
                    target_pct: dec!(60),
                },
                TokenAllocation {
                    token_id: token2,
                    target_pct: dec!(40),
                },
            ],
            threshold_pct: dec!(5),
            min_interval: Duration::days(7),
        };

        let rebalancer = PortfolioRebalancer::new(config).unwrap();

        let mut holdings = HashMap::new();
        holdings.insert(token1, dec!(100));
        holdings.insert(token2, dec!(100));

        let mut prices = HashMap::new();
        prices.insert(token1, dec!(1));
        prices.insert(token2, dec!(1));

        let positions = rebalancer.calculate_positions(&holdings, &prices);
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn test_rebalance_validation() {
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        // Invalid: doesn't sum to 100%
        let config = RebalanceConfig {
            allocations: vec![
                TokenAllocation {
                    token_id: token1,
                    target_pct: dec!(60),
                },
                TokenAllocation {
                    token_id: token2,
                    target_pct: dec!(30),
                },
            ],
            threshold_pct: dec!(5),
            min_interval: Duration::days(7),
        };

        let result = PortfolioRebalancer::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_rebalance_actions() {
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let config = RebalanceConfig {
            allocations: vec![
                TokenAllocation {
                    token_id: token1,
                    target_pct: dec!(50),
                },
                TokenAllocation {
                    token_id: token2,
                    target_pct: dec!(50),
                },
            ],
            threshold_pct: dec!(5),
            min_interval: Duration::days(7),
        };

        let rebalancer = PortfolioRebalancer::new(config).unwrap();

        // Portfolio is 70% token1, 30% token2
        let mut holdings = HashMap::new();
        holdings.insert(token1, dec!(70));
        holdings.insert(token2, dec!(30));

        let mut prices = HashMap::new();
        prices.insert(token1, dec!(1));
        prices.insert(token2, dec!(1));

        let positions = rebalancer.calculate_positions(&holdings, &prices);
        let actions = rebalancer.calculate_rebalance_actions(&positions);

        assert!(!actions.is_empty());

        // Should sell token1 and buy token2
        let token1_action = actions.iter().find(|a| a.token_id == token1).unwrap();
        let token2_action = actions.iter().find(|a| a.token_id == token2).unwrap();

        assert!(token1_action.amount < dec!(0)); // Sell
        assert!(token2_action.amount > dec!(0)); // Buy
    }
}
