//! Market Making Analytics
//!
//! Provides analytics and performance metrics for market making strategies.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Market making performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerPerformance {
    /// Inventory turnover ratio
    pub inventory_turnover: f64,
    /// Realized spread (bps)
    pub realized_spread_bps: f64,
    /// Adverse selection cost (bps)
    pub adverse_selection_bps: f64,
    /// Gross profit
    pub gross_profit: Decimal,
    /// Net profit (after costs)
    pub net_profit: Decimal,
    /// Sharpe ratio
    pub sharpe_ratio: f64,
}

/// Trade record for market making
#[derive(Debug, Clone)]
pub struct MmTrade {
    /// Whether the market maker bought or sold
    pub side: MmSide,
    /// Execution price of the trade
    pub price: Decimal,
    /// Quantity traded
    pub quantity: Decimal,
    /// Mid-point of bid-ask spread at time of trade
    pub midpoint_at_trade: Decimal,
    /// Mid-point of bid-ask spread when the position was closed, if known
    pub midpoint_at_exit: Option<Decimal>,
}

/// Side of a market-making trade
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmSide {
    /// Market maker bought
    Buy,
    /// Market maker sold
    Sell,
}

/// Market making analytics engine
#[derive(Debug)]
pub struct MarketMakingAnalytics {
    /// All recorded trades
    trades: Vec<MmTrade>,
    /// Historical (inventory, timestamp) snapshots, oldest first
    inventory_history: VecDeque<(Decimal, i64)>,
    /// Maximum number of inventory snapshots to retain
    max_history: usize,
}

impl MarketMakingAnalytics {
    /// Create a new market-making analytics engine
    pub fn new(max_history: usize) -> Self {
        Self {
            trades: Vec::new(),
            inventory_history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Record a completed market-making trade
    pub fn record_trade(&mut self, trade: MmTrade) {
        self.trades.push(trade);
    }

    /// Record an inventory snapshot at the given Unix timestamp
    pub fn record_inventory(&mut self, inventory: Decimal, timestamp: i64) {
        if self.inventory_history.len() >= self.max_history {
            self.inventory_history.pop_front();
        }
        self.inventory_history.push_back((inventory, timestamp));
    }

    /// Calculate inventory turnover
    pub fn inventory_turnover(&self) -> f64 {
        if self.inventory_history.is_empty() {
            return 0.0;
        }

        // Average inventory
        let avg_inventory = self
            .inventory_history
            .iter()
            .map(|(inv, _)| *inv)
            .sum::<Decimal>()
            / Decimal::from(self.inventory_history.len());

        // Total volume traded
        let total_volume: Decimal = self.trades.iter().map(|t| t.quantity).sum();

        if avg_inventory > Decimal::ZERO {
            (total_volume / avg_inventory)
                .to_string()
                .parse()
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Calculate realized spread
    pub fn realized_spread_bps(&self) -> f64 {
        let mut total_spread = 0.0;
        let mut count = 0;

        for trade in &self.trades {
            if trade.midpoint_at_exit.is_none() {
                continue;
            }

            let midpoint_exit = trade.midpoint_at_exit.expect("midpoint_at_exit is Some per is_none check above");

            // Realized spread = 2 * (midpoint_at_exit - execution_price) for buy
            //                  = 2 * (execution_price - midpoint_at_exit) for sell
            let spread = match trade.side {
                MmSide::Buy => (midpoint_exit - trade.price) * dec!(2),
                MmSide::Sell => (trade.price - midpoint_exit) * dec!(2),
            };

            if trade.midpoint_at_trade > Decimal::ZERO {
                let spread_bps = ((spread / trade.midpoint_at_trade) * dec!(10000))
                    .to_string()
                    .parse()
                    .unwrap_or(0.0);
                total_spread += spread_bps;
                count += 1;
            }
        }

        if count > 0 {
            total_spread / count as f64
        } else {
            0.0
        }
    }

    /// Calculate adverse selection cost
    pub fn adverse_selection_cost_bps(&self) -> f64 {
        let mut total_cost = 0.0;
        let mut count = 0;

        for trade in &self.trades {
            if trade.midpoint_at_exit.is_none() {
                continue;
            }

            let midpoint_exit = trade.midpoint_at_exit.expect("midpoint_at_exit is Some per is_none check above");
            let midpoint_trade = trade.midpoint_at_trade;

            // Adverse selection = price movement against the trade
            let price_move = match trade.side {
                MmSide::Buy => midpoint_exit - midpoint_trade,
                MmSide::Sell => midpoint_trade - midpoint_exit,
            };

            if midpoint_trade > Decimal::ZERO {
                let cost_bps = ((price_move / midpoint_trade) * dec!(10000))
                    .to_string()
                    .parse()
                    .unwrap_or(0.0);
                total_cost += cost_bps;
                count += 1;
            }
        }

        if count > 0 {
            total_cost / count as f64
        } else {
            0.0
        }
    }

    /// Calculate gross profit
    pub fn gross_profit(&self) -> Decimal {
        let mut profit = Decimal::ZERO;

        for trade in &self.trades {
            let trade_pnl = match trade.side {
                MmSide::Buy => {
                    // Bought below midpoint (negative cost)
                    (trade.midpoint_at_trade - trade.price) * trade.quantity
                }
                MmSide::Sell => {
                    // Sold above midpoint (positive gain)
                    (trade.price - trade.midpoint_at_trade) * trade.quantity
                }
            };

            profit += trade_pnl;
        }

        profit
    }

    /// Calculate Sharpe ratio
    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> f64 {
        if self.trades.len() < 2 {
            return 0.0;
        }

        // Calculate returns per trade
        let mut returns = Vec::new();

        for trade in &self.trades {
            let ret = match trade.side {
                MmSide::Buy => (trade.midpoint_at_trade - trade.price) / trade.price,
                MmSide::Sell => (trade.price - trade.midpoint_at_trade) / trade.price,
            };

            returns.push(ret.to_string().parse::<f64>().unwrap_or(0.0));
        }

        // Mean return
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;

        // Standard deviation
        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            (mean_return - risk_free_rate) / std_dev
        } else {
            0.0
        }
    }

    /// Get comprehensive performance metrics
    pub fn get_performance(&self, transaction_costs: Decimal) -> MarketMakerPerformance {
        let gross_profit = self.gross_profit();
        let total_trades = Decimal::from(self.trades.len());
        let total_costs = transaction_costs * total_trades;
        let net_profit = gross_profit - total_costs;

        MarketMakerPerformance {
            inventory_turnover: self.inventory_turnover(),
            realized_spread_bps: self.realized_spread_bps(),
            adverse_selection_bps: self.adverse_selection_cost_bps(),
            gross_profit,
            net_profit,
            sharpe_ratio: self.sharpe_ratio(0.0),
        }
    }
}

/// Spread decomposition analyzer
#[derive(Debug)]
pub struct SpreadDecomposition;

impl SpreadDecomposition {
    /// Decompose spread into components
    pub fn decompose(
        quoted_spread: Decimal,
        realized_spread: Decimal,
        adverse_selection: Decimal,
    ) -> SpreadComponents {
        // Order processing cost = Realized spread - Adverse selection
        let order_processing = realized_spread - adverse_selection;

        SpreadComponents {
            quoted_spread,
            realized_spread,
            adverse_selection,
            order_processing_cost: order_processing,
        }
    }
}

/// Components of the bid-ask spread
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadComponents {
    /// Quoted spread (ask - bid)
    pub quoted_spread: Decimal,
    /// Realized spread (revenue)
    pub realized_spread: Decimal,
    /// Adverse selection cost
    pub adverse_selection: Decimal,
    /// Order processing cost
    pub order_processing_cost: Decimal,
}

/// Inventory management analyzer
#[derive(Debug)]
pub struct InventoryAnalyzer {
    /// Desired inventory level around which to quote symmetrically
    target_inventory: Decimal,
}

impl InventoryAnalyzer {
    /// Create a new inventory analyzer targeting the given inventory level
    pub fn new(target_inventory: Decimal) -> Self {
        Self { target_inventory }
    }

    /// Calculate inventory skew adjustment for quotes
    pub fn calculate_skew(&self, current_inventory: Decimal) -> f64 {
        if self.target_inventory == Decimal::ZERO {
            return 0.0;
        }

        let deviation = current_inventory - self.target_inventory;
        let skew: f64 = (deviation / self.target_inventory)
            .to_string()
            .parse()
            .unwrap_or(0.0);

        // Cap skew at ±50%
        skew.clamp(-0.5, 0.5)
    }

    /// Suggest quote adjustments based on inventory
    pub fn suggest_adjustments(
        &self,
        current_inventory: Decimal,
        base_spread: Decimal,
    ) -> QuoteAdjustments {
        let skew = self.calculate_skew(current_inventory);

        // If inventory is high, widen bid-ask on sell side
        // If inventory is low, widen bid-ask on buy side
        let bid_adjustment = Decimal::from_f64_retain(-skew * 0.5).unwrap_or(Decimal::ZERO);
        let ask_adjustment = Decimal::from_f64_retain(skew * 0.5).unwrap_or(Decimal::ZERO);

        QuoteAdjustments {
            bid_adjustment: bid_adjustment * base_spread,
            ask_adjustment: ask_adjustment * base_spread,
            suggested_spread: base_spread
                * (Decimal::ONE
                    + Decimal::from_f64_retain(skew.abs() * 0.2).unwrap_or(Decimal::ZERO)),
        }
    }
}

/// Quote adjustment recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteAdjustments {
    /// Adjustment to apply to the bid price (negative means tighten)
    pub bid_adjustment: Decimal,
    /// Adjustment to apply to the ask price (positive means widen)
    pub ask_adjustment: Decimal,
    /// Suggested total bid-ask spread
    pub suggested_spread: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_turnover() {
        let mut analytics = MarketMakingAnalytics::new(100);

        // Record inventory
        analytics.record_inventory(dec!(1000), 0);
        analytics.record_inventory(dec!(1000), 1);

        // Record trades
        for _ in 0..10 {
            analytics.record_trade(MmTrade {
                side: MmSide::Buy,
                price: dec!(100),
                quantity: dec!(100),
                midpoint_at_trade: dec!(100),
                midpoint_at_exit: Some(dec!(100)),
            });
        }

        let turnover = analytics.inventory_turnover();
        // Total volume 1000, avg inventory 1000 = turnover of 1.0
        assert!((turnover - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_realized_spread() {
        let mut analytics = MarketMakingAnalytics::new(100);

        // Buy at 99, midpoint at exit 100
        analytics.record_trade(MmTrade {
            side: MmSide::Buy,
            price: dec!(99),
            quantity: dec!(100),
            midpoint_at_trade: dec!(100),
            midpoint_at_exit: Some(dec!(100)),
        });

        // Sell at 101, midpoint at exit 100
        analytics.record_trade(MmTrade {
            side: MmSide::Sell,
            price: dec!(101),
            quantity: dec!(100),
            midpoint_at_trade: dec!(100),
            midpoint_at_exit: Some(dec!(100)),
        });

        let spread = analytics.realized_spread_bps();
        // Both trades should have positive realized spread
        assert!(spread > 0.0);
    }

    #[test]
    fn test_adverse_selection() {
        let mut analytics = MarketMakingAnalytics::new(100);

        // Buy at 100, midpoint moves against us to 99
        analytics.record_trade(MmTrade {
            side: MmSide::Buy,
            price: dec!(100),
            quantity: dec!(100),
            midpoint_at_trade: dec!(100),
            midpoint_at_exit: Some(dec!(99)),
        });

        let cost = analytics.adverse_selection_cost_bps();
        // Should be positive (negative for us)
        assert!(cost < 0.0);
    }

    #[test]
    fn test_gross_profit() {
        let mut analytics = MarketMakingAnalytics::new(100);

        // Buy below midpoint
        analytics.record_trade(MmTrade {
            side: MmSide::Buy,
            price: dec!(99),
            quantity: dec!(100),
            midpoint_at_trade: dec!(100),
            midpoint_at_exit: None,
        });

        // Sell above midpoint
        analytics.record_trade(MmTrade {
            side: MmSide::Sell,
            price: dec!(101),
            quantity: dec!(100),
            midpoint_at_trade: dec!(100),
            midpoint_at_exit: None,
        });

        let profit = analytics.gross_profit();
        // (100-99)*100 + (101-100)*100 = 100 + 100 = 200
        assert_eq!(profit, dec!(200));
    }

    #[test]
    fn test_inventory_skew() {
        let analyzer = InventoryAnalyzer::new(dec!(1000));

        // High inventory (1500) should give positive skew
        let skew_high = analyzer.calculate_skew(dec!(1500));
        assert!(skew_high > 0.0);

        // Low inventory (500) should give negative skew
        let skew_low = analyzer.calculate_skew(dec!(500));
        assert!(skew_low < 0.0);

        // Target inventory should give zero skew
        let skew_target = analyzer.calculate_skew(dec!(1000));
        assert_eq!(skew_target, 0.0);
    }

    #[test]
    fn test_quote_adjustments() {
        let analyzer = InventoryAnalyzer::new(dec!(1000));

        // High inventory - should widen ask, narrow bid
        let adj = analyzer.suggest_adjustments(dec!(1500), dec!(1.0));
        assert!(adj.ask_adjustment > Decimal::ZERO);
        assert!(adj.bid_adjustment < Decimal::ZERO);
    }
}
