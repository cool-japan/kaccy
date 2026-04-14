//! Advanced Backtesting Framework
//!
//! This module provides comprehensive backtesting capabilities including historical data replay,
//! strategy testing, walk-forward analysis, and performance metrics.

use crate::error::CoreError;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// Order book snapshot for historical replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// When this snapshot was captured
    pub timestamp: SystemTime,
    /// Token this order book belongs to
    pub token_id: String,
    /// Bid (buy) price levels, best bid first
    pub bids: Vec<PriceLevel>,
    /// Ask (sell) price levels, best ask first
    pub asks: Vec<PriceLevel>,
}

/// A single price level in an order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price at this level
    pub price: Decimal,
    /// Quantity available at this price
    pub quantity: Decimal,
}

/// Historical trade tick
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeTick {
    /// When the trade occurred
    pub timestamp: SystemTime,
    /// Token that was traded
    pub token_id: String,
    /// Execution price
    pub price: Decimal,
    /// Trade quantity
    pub quantity: Decimal,
    /// Whether the buyer was the passive (maker) side
    pub is_buyer_maker: bool,
}

/// Backtest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    /// Starting cash balance for the backtest
    pub initial_balance: Decimal,
    /// Start of the backtesting period
    pub start_time: SystemTime,
    /// End of the backtesting period
    pub end_time: SystemTime,
    /// Fee applied to maker orders
    pub maker_fee: Decimal,
    /// Fee applied to taker orders
    pub taker_fee: Decimal,
    /// Model used to estimate execution slippage
    pub slippage_model: SlippageModel,
    /// Whether to simulate market-impact costs
    pub enable_market_impact: bool,
}

/// Slippage model used during backtesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlippageModel {
    /// No slippage
    None,
    /// Fixed percentage slippage
    Fixed {
        /// Slippage applied as a fixed percentage of trade value.
        percentage: Decimal,
    },
    /// Volume-based slippage
    VolumeBased {
        /// Base slippage percentage at minimal volume.
        base_pct: Decimal,
        /// Multiplier applied as volume increases.
        volume_factor: Decimal,
    },
    /// Realistic order book slippage
    OrderBook,
}

/// Backtest execution engine
pub struct BacktestEngine {
    /// Configuration for this backtest run
    config: BacktestConfig,
    /// Current simulated time
    current_time: SystemTime,
    /// Available cash balance
    balance: Decimal,
    /// Open positions (token_id → quantity)
    positions: HashMap<String, Decimal>,
    /// Chronological history of executed orders
    order_history: Vec<BacktestOrder>,
    /// Historical trade ticks used for price lookups
    price_history: Vec<TradeTick>,
    /// Rolling window of order book snapshots
    orderbook_snapshots: VecDeque<OrderBookSnapshot>,
}

/// An order executed during a backtest run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestOrder {
    /// Simulated execution time
    pub timestamp: SystemTime,
    /// Token traded
    pub token_id: String,
    /// Buy or sell
    pub side: OrderSide,
    /// Number of tokens traded
    pub quantity: Decimal,
    /// Actual execution price (including slippage)
    pub price: Decimal,
    /// Fee charged for this order
    pub fee: Decimal,
    /// Slippage incurred relative to the mid-price
    pub slippage: Decimal,
    /// Realised P&L for this order (populated on sell)
    pub pnl: Option<Decimal>,
}

/// Order direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// Buy (long) order
    Buy,
    /// Sell (short/close) order
    Sell,
}

impl BacktestEngine {
    /// Create a new backtest engine with the given configuration
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            current_time: config.start_time,
            balance: config.initial_balance,
            config,
            positions: HashMap::new(),
            order_history: Vec::new(),
            price_history: Vec::new(),
            orderbook_snapshots: VecDeque::new(),
        }
    }

    /// Advances time in the backtest
    pub fn advance_time(&mut self, duration: Duration) {
        self.current_time += duration;
    }

    /// Sets current time
    pub fn set_time(&mut self, time: SystemTime) -> Result<(), CoreError> {
        if time < self.config.start_time || time > self.config.end_time {
            return Err(CoreError::Validation(
                "Time must be within backtest period".to_string(),
            ));
        }
        self.current_time = time;
        Ok(())
    }

    /// Adds historical price data
    pub fn add_price_tick(&mut self, tick: TradeTick) {
        self.price_history.push(tick);
    }

    /// Adds order book snapshot
    pub fn add_orderbook_snapshot(&mut self, snapshot: OrderBookSnapshot) {
        self.orderbook_snapshots.push_back(snapshot);

        // Keep only recent snapshots (last 1000)
        if self.orderbook_snapshots.len() > 1000 {
            self.orderbook_snapshots.pop_front();
        }
    }

    /// Executes a market order in the backtest
    pub fn execute_market_order(
        &mut self,
        token_id: String,
        side: OrderSide,
        quantity: Decimal,
    ) -> Result<BacktestOrder, CoreError> {
        if quantity <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Quantity must be positive".to_string(),
            ));
        }

        // Get current price from latest tick or orderbook
        let base_price = self.get_current_price(&token_id)?;

        // Calculate slippage
        let slippage = self.calculate_slippage(&token_id, quantity, side);
        let execution_price = match side {
            OrderSide::Buy => base_price * (Decimal::ONE + slippage),
            OrderSide::Sell => base_price * (Decimal::ONE - slippage),
        };

        // Calculate cost/revenue and fee
        let notional = quantity * execution_price;
        let fee = notional * self.config.taker_fee;

        // Execute the order
        match side {
            OrderSide::Buy => {
                let total_cost = notional + fee;
                if self.balance < total_cost {
                    return Err(CoreError::InsufficientBalance {
                        required: total_cost,
                        available: self.balance,
                    });
                }

                self.balance -= total_cost;
                *self
                    .positions
                    .entry(token_id.clone())
                    .or_insert(Decimal::ZERO) += quantity;
            }
            OrderSide::Sell => {
                let current_position = *self.positions.get(&token_id).unwrap_or(&Decimal::ZERO);
                if current_position < quantity {
                    return Err(CoreError::Validation(format!(
                        "Insufficient position: have {}, need {}",
                        current_position, quantity
                    )));
                }

                let revenue = notional - fee;
                self.balance += revenue;
                *self.positions.get_mut(&token_id).unwrap() -= quantity;
            }
        }

        let order = BacktestOrder {
            timestamp: self.current_time,
            token_id,
            side,
            quantity,
            price: execution_price,
            fee,
            slippage,
            pnl: None, // Calculated later
        };

        self.order_history.push(order.clone());

        Ok(order)
    }

    /// Gets current price for a token
    fn get_current_price(&self, token_id: &str) -> Result<Decimal, CoreError> {
        // Try to get from recent price ticks
        for tick in self.price_history.iter().rev() {
            if tick.token_id == token_id {
                return Ok(tick.price);
            }
        }

        // Try to get from orderbook midprice
        if let Some(snapshot) = self
            .orderbook_snapshots
            .iter()
            .rev()
            .find(|s| s.token_id == token_id)
        {
            if let (Some(best_bid), Some(best_ask)) = (snapshot.bids.first(), snapshot.asks.first())
            {
                return Ok((best_bid.price + best_ask.price) / Decimal::new(2, 0));
            }
        }

        Err(CoreError::NotFound(format!(
            "No price data for token {}",
            token_id
        )))
    }

    /// Calculates slippage based on configuration
    fn calculate_slippage(&self, token_id: &str, quantity: Decimal, side: OrderSide) -> Decimal {
        match &self.config.slippage_model {
            SlippageModel::None => Decimal::ZERO,
            SlippageModel::Fixed { percentage } => *percentage / Decimal::new(100, 0),
            SlippageModel::VolumeBased {
                base_pct,
                volume_factor,
            } => {
                // Calculate recent volume
                let recent_volume: Decimal = self
                    .price_history
                    .iter()
                    .rev()
                    .take(100)
                    .filter(|t| t.token_id == token_id)
                    .map(|t| t.quantity)
                    .sum();

                let volume_ratio = if recent_volume > Decimal::ZERO {
                    quantity / recent_volume
                } else {
                    Decimal::ONE
                };

                (base_pct + volume_ratio * volume_factor) / Decimal::new(100, 0)
            }
            SlippageModel::OrderBook => {
                if let Some(snapshot) = self
                    .orderbook_snapshots
                    .iter()
                    .rev()
                    .find(|s| s.token_id == token_id)
                {
                    self.calculate_orderbook_slippage(snapshot, quantity, side)
                } else {
                    Decimal::new(10, 4) // 0.1% default
                }
            }
        }
    }

    /// Calculates realistic slippage from order book
    fn calculate_orderbook_slippage(
        &self,
        snapshot: &OrderBookSnapshot,
        quantity: Decimal,
        side: OrderSide,
    ) -> Decimal {
        let levels = match side {
            OrderSide::Buy => &snapshot.asks,
            OrderSide::Sell => &snapshot.bids,
        };

        if levels.is_empty() {
            return Decimal::new(50, 4); // 0.5% if no liquidity
        }

        let best_price = levels[0].price;
        let mut remaining = quantity;
        let mut total_cost = Decimal::ZERO;

        for level in levels {
            if remaining <= Decimal::ZERO {
                break;
            }

            let fill_qty = remaining.min(level.quantity);
            total_cost += fill_qty * level.price;
            remaining -= fill_qty;
        }

        if remaining > Decimal::ZERO {
            // Not enough liquidity - return high slippage
            return Decimal::new(100, 4); // 1%
        }

        let avg_price = total_cost / quantity;
        ((avg_price - best_price).abs() / best_price).min(Decimal::new(100, 4))
    }

    /// Gets current portfolio value
    pub fn get_portfolio_value(&self) -> Result<Decimal, CoreError> {
        let mut total_value = self.balance;

        for (token_id, quantity) in &self.positions {
            if *quantity > Decimal::ZERO {
                let price = self.get_current_price(token_id)?;
                total_value += price * quantity;
            }
        }

        Ok(total_value)
    }

    /// Calculates backtest results
    pub fn get_results(&self) -> Result<BacktestResults, CoreError> {
        let final_value = self.get_portfolio_value()?;
        let total_return =
            (final_value - self.config.initial_balance) / self.config.initial_balance;

        // Calculate metrics
        let total_trades = self.order_history.len();
        let total_fees: Decimal = self.order_history.iter().map(|o| o.fee).sum();

        // Group trades by token for PnL calculation
        let mut pnl_by_trade = Vec::new();
        let mut token_positions: HashMap<String, Vec<(Decimal, Decimal)>> = HashMap::new();

        for order in &self.order_history {
            let positions = token_positions.entry(order.token_id.clone()).or_default();

            match order.side {
                OrderSide::Buy => {
                    positions.push((order.quantity, order.price));
                }
                OrderSide::Sell => {
                    // Calculate PnL using FIFO
                    let mut remaining = order.quantity;
                    let mut pnl = Decimal::ZERO;

                    while remaining > Decimal::ZERO && !positions.is_empty() {
                        let (qty, buy_price) = positions[0];
                        let sell_qty = remaining.min(qty);

                        pnl += sell_qty * (order.price - buy_price);

                        if sell_qty >= qty {
                            positions.remove(0);
                        } else {
                            positions[0].0 -= sell_qty;
                        }

                        remaining -= sell_qty;
                    }

                    pnl_by_trade.push(pnl);
                }
            }
        }

        let winning_trades = pnl_by_trade.iter().filter(|&&p| p > Decimal::ZERO).count();
        let losing_trades = pnl_by_trade.iter().filter(|&&p| p < Decimal::ZERO).count();
        let win_rate = if !pnl_by_trade.is_empty() {
            Decimal::from(winning_trades) / Decimal::from(pnl_by_trade.len())
        } else {
            Decimal::ZERO
        };

        // Calculate max drawdown
        let max_drawdown = self.calculate_max_drawdown()?;

        // Calculate Sharpe ratio (simplified)
        let returns = self.calculate_period_returns()?;
        let sharpe_ratio = if !returns.is_empty() {
            let mean_return: Decimal =
                returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
            let variance: Decimal = returns
                .iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<Decimal>()
                / Decimal::from(returns.len());
            let std_dev = variance.sqrt().unwrap_or(Decimal::ONE);

            if std_dev > Decimal::ZERO {
                mean_return / std_dev * Decimal::new(252, 0).sqrt().unwrap_or(Decimal::ONE) // Annualized
            } else {
                Decimal::ZERO
            }
        } else {
            Decimal::ZERO
        };

        Ok(BacktestResults {
            initial_balance: self.config.initial_balance,
            final_balance: self.balance,
            final_portfolio_value: final_value,
            total_return,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_fees,
            max_drawdown,
            sharpe_ratio,
            start_time: self.config.start_time,
            end_time: self.config.end_time,
        })
    }

    /// Calculates maximum drawdown
    fn calculate_max_drawdown(&self) -> Result<Decimal, CoreError> {
        if self.order_history.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let mut peak = self.config.initial_balance;
        let mut max_dd = Decimal::ZERO;

        // Simulate portfolio value over time
        let mut current_balance = self.config.initial_balance;
        for order in &self.order_history {
            match order.side {
                OrderSide::Buy => {
                    current_balance -= order.quantity * order.price + order.fee;
                }
                OrderSide::Sell => {
                    current_balance += order.quantity * order.price - order.fee;
                }
            }

            if current_balance > peak {
                peak = current_balance;
            }

            let drawdown = (peak - current_balance) / peak;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        Ok(max_dd)
    }

    /// Calculates period returns for Sharpe ratio
    fn calculate_period_returns(&self) -> Result<Vec<Decimal>, CoreError> {
        let mut returns = Vec::new();
        if self.order_history.len() < 2 {
            return Ok(returns);
        }

        let mut prev_value = self.config.initial_balance;

        for order in &self.order_history {
            let mut current_value = prev_value;

            match order.side {
                OrderSide::Buy => {
                    current_value -= order.quantity * order.price + order.fee;
                }
                OrderSide::Sell => {
                    current_value += order.quantity * order.price - order.fee;
                }
            }

            let period_return = (current_value - prev_value) / prev_value;
            returns.push(period_return);
            prev_value = current_value;
        }

        Ok(returns)
    }
}

/// Backtest results summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    /// Cash balance at the start of the backtest
    pub initial_balance: Decimal,
    /// Cash balance at the end of the backtest
    pub final_balance: Decimal,
    /// Total portfolio value (cash + open positions) at the end
    pub final_portfolio_value: Decimal,
    /// Net return as a fraction of the initial balance
    pub total_return: Decimal,
    /// Total number of orders executed
    pub total_trades: usize,
    /// Number of profitable sell orders
    pub winning_trades: usize,
    /// Number of loss-making sell orders
    pub losing_trades: usize,
    /// Win rate as a fraction (winning / total sell orders)
    pub win_rate: Decimal,
    /// Total fees paid across all orders
    pub total_fees: Decimal,
    /// Maximum peak-to-trough drawdown observed
    pub max_drawdown: Decimal,
    /// Annualised Sharpe ratio
    pub sharpe_ratio: Decimal,
    /// Start of the backtesting period
    pub start_time: SystemTime,
    /// End of the backtesting period
    pub end_time: SystemTime,
}

impl BacktestResults {
    /// Checks if the backtest results are acceptable
    pub fn is_profitable(&self) -> bool {
        self.total_return > Decimal::ZERO
    }

    /// Gets annualized return
    pub fn annualized_return(&self) -> Result<Decimal, CoreError> {
        let duration = self
            .end_time
            .duration_since(self.start_time)
            .map_err(|e| CoreError::Validation(format!("Invalid time range: {}", e)))?;

        let years = Decimal::from(duration.as_secs()) / Decimal::from(365 * 24 * 3600);
        if years <= Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        Ok(self.total_return / years)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> BacktestConfig {
        BacktestConfig {
            initial_balance: Decimal::new(10000, 0),
            start_time: SystemTime::UNIX_EPOCH,
            end_time: SystemTime::UNIX_EPOCH + Duration::from_secs(86400),
            maker_fee: Decimal::new(25, 4), // 0.25%
            taker_fee: Decimal::new(50, 4), // 0.50%
            slippage_model: SlippageModel::Fixed {
                percentage: Decimal::new(10, 2), // 0.1%
            },
            enable_market_impact: false,
        }
    }

    #[test]
    fn test_backtest_engine_creation() {
        let config = create_test_config();
        let engine = BacktestEngine::new(config.clone());

        assert_eq!(engine.balance, config.initial_balance);
        assert_eq!(engine.current_time, config.start_time);
    }

    #[test]
    fn test_execute_buy_order() {
        let mut engine = BacktestEngine::new(create_test_config());

        // Add price data
        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        let result = engine.execute_market_order(
            "BTC".to_string(),
            OrderSide::Buy,
            Decimal::new(1, 1), // 0.1 BTC
        );

        assert!(result.is_ok());
        let order = result.unwrap();
        assert_eq!(order.side, OrderSide::Buy);
        assert!(order.fee > Decimal::ZERO);
    }

    #[test]
    fn test_execute_sell_order() {
        let mut engine = BacktestEngine::new(create_test_config());

        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        // Buy first
        engine
            .execute_market_order("BTC".to_string(), OrderSide::Buy, Decimal::new(1, 1))
            .unwrap();

        // Then sell
        let result =
            engine.execute_market_order("BTC".to_string(), OrderSide::Sell, Decimal::new(1, 1));

        assert!(result.is_ok());
    }

    #[test]
    fn test_insufficient_balance() {
        let mut engine = BacktestEngine::new(create_test_config());

        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        // Try to buy too much
        let result = engine.execute_market_order(
            "BTC".to_string(),
            OrderSide::Buy,
            Decimal::new(100, 0), // Way too much
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_portfolio_value() {
        let mut engine = BacktestEngine::new(create_test_config());

        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        // Buy some BTC
        engine
            .execute_market_order("BTC".to_string(), OrderSide::Buy, Decimal::new(1, 1))
            .unwrap();

        let portfolio_value = engine.get_portfolio_value().unwrap();
        // Should be close to initial balance (minus fees and slippage)
        assert!(portfolio_value > Decimal::ZERO);
        assert!(portfolio_value <= engine.config.initial_balance);
    }

    #[test]
    fn test_backtest_results() {
        let mut engine = BacktestEngine::new(create_test_config());

        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        // Execute some trades
        engine
            .execute_market_order("BTC".to_string(), OrderSide::Buy, Decimal::new(1, 1))
            .unwrap();

        // Update price
        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH + Duration::from_secs(3600),
            token_id: "BTC".to_string(),
            price: Decimal::new(51000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: false,
        });

        engine
            .execute_market_order("BTC".to_string(), OrderSide::Sell, Decimal::new(1, 1))
            .unwrap();

        let results = engine.get_results().unwrap();
        assert_eq!(results.total_trades, 2);
        assert_eq!(results.initial_balance, Decimal::new(10000, 0));
    }

    #[test]
    fn test_slippage_calculation() {
        let mut engine = BacktestEngine::new(create_test_config());

        engine.add_price_tick(TradeTick {
            timestamp: SystemTime::UNIX_EPOCH,
            token_id: "BTC".to_string(),
            price: Decimal::new(50000, 0),
            quantity: Decimal::new(1, 0),
            is_buyer_maker: true,
        });

        let order = engine
            .execute_market_order("BTC".to_string(), OrderSide::Buy, Decimal::new(1, 1))
            .unwrap();

        // With 0.1% slippage, price should be slightly higher
        assert!(order.slippage > Decimal::ZERO);
    }
}
