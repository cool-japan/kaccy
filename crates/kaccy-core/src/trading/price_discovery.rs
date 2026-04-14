//! Price discovery mechanisms module
//!
//! This module provides tools for analyzing price discovery in markets, including
//! microprice calculation, price improvement tracking, effective spread measurement,
//! and quote stability metrics.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::trading::OrderSide;

/// Microprice (weighted mid-price based on order book depth)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Microprice {
    /// Best bid price
    pub best_bid: Decimal,
    /// Best ask price
    pub best_ask: Decimal,
    /// Bid size
    pub bid_size: Decimal,
    /// Ask size
    pub ask_size: Decimal,
    /// Calculated microprice (weighted mid)
    pub microprice: Decimal,
    /// Standard mid-price
    pub mid_price: Decimal,
    /// Difference between microprice and mid
    pub price_impact: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl Microprice {
    /// Calculate microprice from bid/ask data
    ///
    /// Formula: microprice = (bid_size * ask + ask_size * bid) / (bid_size + ask_size)
    pub fn calculate(
        best_bid: Decimal,
        best_ask: Decimal,
        bid_size: Decimal,
        ask_size: Decimal,
    ) -> Self {
        let total_size = bid_size + ask_size;
        let microprice = if total_size > dec!(0) {
            (bid_size * best_ask + ask_size * best_bid) / total_size
        } else {
            (best_bid + best_ask) / dec!(2)
        };

        let mid_price = (best_bid + best_ask) / dec!(2);
        let price_impact = microprice - mid_price;

        Self {
            best_bid,
            best_ask,
            bid_size,
            ask_size,
            microprice,
            mid_price,
            price_impact,
            timestamp: Utc::now(),
        }
    }

    /// Check if order book is imbalanced (microprice significantly different from mid)
    pub fn is_imbalanced(&self) -> bool {
        let relative_impact = if self.mid_price > dec!(0) {
            self.price_impact.abs() / self.mid_price
        } else {
            dec!(0)
        };
        relative_impact > dec!(0.001) // 0.1% threshold
    }

    /// Get the direction of the imbalance
    /// When bid_size > ask_size, microprice moves toward ask (up) = buy pressure
    /// When ask_size > bid_size, microprice moves toward bid (down) = sell pressure
    pub fn imbalance_direction(&self) -> Option<OrderSide> {
        if self.bid_size > self.ask_size {
            Some(OrderSide::Buy) // More bids means buy pressure
        } else if self.ask_size > self.bid_size {
            Some(OrderSide::Sell) // More asks means sell pressure
        } else {
            None
        }
    }
}

/// Price improvement record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceImprovement {
    /// Trade ID
    pub trade_id: Uuid,
    /// Order side
    pub side: OrderSide,
    /// Execution price
    pub execution_price: Decimal,
    /// Quote price (best bid/ask at time of order)
    pub quote_price: Decimal,
    /// Improvement amount (positive = better than quote)
    pub improvement: Decimal,
    /// Improvement percentage
    pub improvement_pct: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl PriceImprovement {
    /// Create a price improvement record
    pub fn new(
        trade_id: Uuid,
        side: OrderSide,
        execution_price: Decimal,
        quote_price: Decimal,
    ) -> Self {
        let improvement = match side {
            OrderSide::Buy => quote_price - execution_price, // Bought cheaper than quote
            OrderSide::Sell => execution_price - quote_price, // Sold higher than quote
        };

        let improvement_pct = if quote_price > dec!(0) {
            (improvement / quote_price) * dec!(100)
        } else {
            dec!(0)
        };

        Self {
            trade_id,
            side,
            execution_price,
            quote_price,
            improvement,
            improvement_pct,
            timestamp: Utc::now(),
        }
    }

    /// Check if there was price improvement
    pub fn has_improvement(&self) -> bool {
        self.improvement > dec!(0)
    }

    /// Check if there was price disimprovement
    pub fn has_disimprovement(&self) -> bool {
        self.improvement < dec!(0)
    }
}

/// Effective spread measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveSpread {
    /// Trade price
    pub trade_price: Decimal,
    /// Mid-price at time of trade
    pub mid_price: Decimal,
    /// Order side
    pub side: OrderSide,
    /// Effective half-spread
    pub effective_half_spread: Decimal,
    /// Quoted half-spread
    pub quoted_half_spread: Decimal,
    /// Realized spread (effective minus quoted)
    pub realized_spread: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl EffectiveSpread {
    /// Calculate effective spread
    ///
    /// Effective spread = 2 * |trade_price - mid_price|
    pub fn calculate(
        trade_price: Decimal,
        mid_price: Decimal,
        side: OrderSide,
        best_bid: Decimal,
        best_ask: Decimal,
    ) -> Self {
        let price_deviation = match side {
            OrderSide::Buy => trade_price - mid_price,
            OrderSide::Sell => mid_price - trade_price,
        };

        let effective_half_spread = price_deviation.abs();
        let quoted_half_spread = (best_ask - best_bid) / dec!(2);
        let realized_spread = effective_half_spread - quoted_half_spread;

        Self {
            trade_price,
            mid_price,
            side,
            effective_half_spread,
            quoted_half_spread,
            realized_spread,
            timestamp: Utc::now(),
        }
    }

    /// Get effective spread as percentage of mid-price
    pub fn effective_spread_pct(&self) -> Decimal {
        if self.mid_price > dec!(0) {
            (self.effective_half_spread * dec!(2) / self.mid_price) * dec!(100)
        } else {
            dec!(0)
        }
    }

    /// Check if effective spread is lower than quoted (price improvement)
    pub fn has_price_improvement(&self) -> bool {
        self.realized_spread < dec!(0)
    }
}

/// Quote stability metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteStability {
    /// Number of quote updates in period
    pub update_count: u64,
    /// Average time between updates (seconds)
    pub avg_update_interval: Decimal,
    /// Quote volatility (std dev of mid-price)
    pub quote_volatility: Decimal,
    /// Max price deviation from mean
    pub max_deviation: Decimal,
    /// Quote persistence score (0-100, higher = more stable)
    pub stability_score: u8,
    /// Time period analyzed (seconds)
    pub period_seconds: i64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl QuoteStability {
    /// Check if quotes are stable
    pub fn is_stable(&self) -> bool {
        self.stability_score >= 70
    }

    /// Check if quotes are volatile
    pub fn is_volatile(&self) -> bool {
        self.stability_score < 40
    }
}

/// Quote update record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteUpdate {
    /// Best bid price.
    pub bid: Decimal,
    /// Best ask price.
    pub ask: Decimal,
    /// Mid-point price between bid and ask.
    pub mid: Decimal,
    /// Time of the quote update.
    pub timestamp: DateTime<Utc>,
}

/// Price discovery analyzer
#[derive(Debug, Clone)]
pub struct PriceDiscoveryAnalyzer {
    token_id: Uuid,
    quote_history: VecDeque<QuoteUpdate>,
    price_improvements: VecDeque<PriceImprovement>,
    effective_spreads: VecDeque<EffectiveSpread>,
    max_history: usize,
}

impl PriceDiscoveryAnalyzer {
    /// Create a new price discovery analyzer
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            quote_history: VecDeque::new(),
            price_improvements: VecDeque::new(),
            effective_spreads: VecDeque::new(),
            max_history: 1000,
        }
    }

    /// Add a quote update
    pub fn add_quote(&mut self, bid: Decimal, ask: Decimal) {
        let mid = (bid + ask) / dec!(2);
        self.quote_history.push_back(QuoteUpdate {
            bid,
            ask,
            mid,
            timestamp: Utc::now(),
        });

        while self.quote_history.len() > self.max_history {
            self.quote_history.pop_front();
        }
    }

    /// Record a price improvement
    pub fn record_price_improvement(&mut self, improvement: PriceImprovement) {
        self.price_improvements.push_back(improvement);

        while self.price_improvements.len() > self.max_history {
            self.price_improvements.pop_front();
        }
    }

    /// Record an effective spread
    pub fn record_effective_spread(&mut self, spread: EffectiveSpread) {
        self.effective_spreads.push_back(spread);

        while self.effective_spreads.len() > self.max_history {
            self.effective_spreads.pop_front();
        }
    }

    /// Calculate current microprice
    pub fn calculate_microprice(
        &self,
        best_bid: Decimal,
        best_ask: Decimal,
        bid_size: Decimal,
        ask_size: Decimal,
    ) -> Microprice {
        Microprice::calculate(best_bid, best_ask, bid_size, ask_size)
    }

    /// Calculate quote stability metrics
    pub fn calculate_quote_stability(&self, period_seconds: i64) -> QuoteStability {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(period_seconds);

        let recent_quotes: Vec<_> = self
            .quote_history
            .iter()
            .filter(|q| q.timestamp >= cutoff)
            .collect();

        if recent_quotes.is_empty() {
            return QuoteStability {
                update_count: 0,
                avg_update_interval: dec!(0),
                quote_volatility: dec!(0),
                max_deviation: dec!(0),
                stability_score: 0,
                period_seconds,
                timestamp: now,
            };
        }

        let update_count = recent_quotes.len() as u64;

        // Calculate average update interval
        let avg_update_interval = if update_count > 1 {
            Decimal::from(period_seconds) / Decimal::from(update_count - 1)
        } else {
            dec!(0)
        };

        // Calculate mid-price volatility
        let mids: Vec<Decimal> = recent_quotes.iter().map(|q| q.mid).collect();
        let mean_mid: Decimal = mids.iter().sum::<Decimal>() / Decimal::from(mids.len());

        let variance: Decimal = mids
            .iter()
            .map(|&m| {
                let diff = m - mean_mid;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(mids.len());

        let quote_volatility = variance.sqrt().unwrap_or(dec!(0));

        // Calculate max deviation
        let max_deviation = mids
            .iter()
            .map(|&m| (m - mean_mid).abs())
            .max()
            .unwrap_or(dec!(0));

        // Calculate stability score (inverse of volatility, scaled 0-100)
        let volatility_ratio = if mean_mid > dec!(0) {
            quote_volatility / mean_mid
        } else {
            dec!(0)
        };

        let stability_score = if volatility_ratio < dec!(0.001) {
            100
        } else if volatility_ratio > dec!(0.1) {
            0
        } else {
            // Linear scale between 0.001 and 0.1
            let normalized = (dec!(0.1) - volatility_ratio) / dec!(0.099);
            (normalized * dec!(100)).round().to_u8().unwrap_or(50)
        };

        QuoteStability {
            update_count,
            avg_update_interval,
            quote_volatility,
            max_deviation,
            stability_score,
            period_seconds,
            timestamp: now,
        }
    }

    /// Get average price improvement over recent trades
    pub fn avg_price_improvement(&self, count: usize) -> Decimal {
        if self.price_improvements.is_empty() {
            return dec!(0);
        }

        let recent: Vec<_> = self.price_improvements.iter().rev().take(count).collect();

        if recent.is_empty() {
            return dec!(0);
        }

        let total: Decimal = recent.iter().map(|pi| pi.improvement).sum();
        total / Decimal::from(recent.len())
    }

    /// Get average effective spread
    pub fn avg_effective_spread(&self, count: usize) -> Decimal {
        if self.effective_spreads.is_empty() {
            return dec!(0);
        }

        let recent: Vec<_> = self.effective_spreads.iter().rev().take(count).collect();

        if recent.is_empty() {
            return dec!(0);
        }

        let total: Decimal = recent.iter().map(|es| es.effective_half_spread).sum();
        (total / Decimal::from(recent.len())) * dec!(2) // Return full spread
    }

    /// Get token ID
    pub fn token_id(&self) -> Uuid {
        self.token_id
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.quote_history.clear();
        self.price_improvements.clear();
        self.effective_spreads.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microprice_calculation() {
        let microprice = Microprice::calculate(dec!(99), dec!(101), dec!(1000), dec!(500));

        // Microprice = (1000 * 101 + 500 * 99) / 1500 = (101000 + 49500) / 1500 = 100.333...
        assert!(microprice.microprice > dec!(100));
        assert!(microprice.microprice < dec!(101));
        assert_eq!(microprice.mid_price, dec!(100));
    }

    #[test]
    fn test_microprice_imbalance() {
        // More bid liquidity = buy pressure
        let microprice = Microprice::calculate(dec!(99), dec!(101), dec!(2000), dec!(500));
        assert!(microprice.is_imbalanced());
        assert_eq!(microprice.imbalance_direction(), Some(OrderSide::Buy));

        // Balanced
        let microprice = Microprice::calculate(dec!(99), dec!(101), dec!(1000), dec!(1000));
        assert!(!microprice.is_imbalanced());
    }

    #[test]
    fn test_price_improvement_buy() {
        let improvement =
            PriceImprovement::new(Uuid::new_v4(), OrderSide::Buy, dec!(99), dec!(100));

        assert!(improvement.has_improvement());
        assert_eq!(improvement.improvement, dec!(1));
        assert_eq!(improvement.improvement_pct, dec!(1));
    }

    #[test]
    fn test_price_improvement_sell() {
        let improvement =
            PriceImprovement::new(Uuid::new_v4(), OrderSide::Sell, dec!(101), dec!(100));

        assert!(improvement.has_improvement());
        assert_eq!(improvement.improvement, dec!(1));
        assert_eq!(improvement.improvement_pct, dec!(1));
    }

    #[test]
    fn test_effective_spread_calculation() {
        let spread = EffectiveSpread::calculate(
            dec!(100.5), // trade price
            dec!(100),   // mid price
            OrderSide::Buy,
            dec!(99.5),  // best bid
            dec!(100.5), // best ask
        );

        assert_eq!(spread.effective_half_spread, dec!(0.5));
        assert_eq!(spread.quoted_half_spread, dec!(0.5));
        assert_eq!(spread.realized_spread, dec!(0));
    }

    #[test]
    fn test_effective_spread_with_improvement() {
        let spread = EffectiveSpread::calculate(
            dec!(100.2), // trade price (better than ask)
            dec!(100),   // mid price
            OrderSide::Buy,
            dec!(99.5),  // best bid
            dec!(100.5), // best ask
        );

        assert!(spread.has_price_improvement());
    }

    #[test]
    fn test_price_discovery_analyzer() {
        let token_id = Uuid::new_v4();
        let mut analyzer = PriceDiscoveryAnalyzer::new(token_id);

        analyzer.add_quote(dec!(99), dec!(101));
        analyzer.add_quote(dec!(99.5), dec!(100.5));
        analyzer.add_quote(dec!(100), dec!(102));

        assert_eq!(analyzer.quote_history.len(), 3);
    }

    #[test]
    fn test_quote_stability() {
        let token_id = Uuid::new_v4();
        let mut analyzer = PriceDiscoveryAnalyzer::new(token_id);

        // Add stable quotes
        for _ in 0..10 {
            analyzer.add_quote(dec!(99.9), dec!(100.1));
        }

        let stability = analyzer.calculate_quote_stability(60);
        assert!(stability.is_stable());
        assert!(!stability.is_volatile());
    }

    #[test]
    fn test_avg_price_improvement() {
        let token_id = Uuid::new_v4();
        let mut analyzer = PriceDiscoveryAnalyzer::new(token_id);

        analyzer.record_price_improvement(PriceImprovement::new(
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(99),
            dec!(100),
        ));

        analyzer.record_price_improvement(PriceImprovement::new(
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(98),
            dec!(100),
        ));

        let avg = analyzer.avg_price_improvement(2);
        assert_eq!(avg, dec!(1.5));
    }

    #[test]
    fn test_avg_effective_spread() {
        let token_id = Uuid::new_v4();
        let mut analyzer = PriceDiscoveryAnalyzer::new(token_id);

        analyzer.record_effective_spread(EffectiveSpread::calculate(
            dec!(100.5),
            dec!(100),
            OrderSide::Buy,
            dec!(99.5),
            dec!(100.5),
        ));

        analyzer.record_effective_spread(EffectiveSpread::calculate(
            dec!(101),
            dec!(100),
            OrderSide::Buy,
            dec!(99),
            dec!(101),
        ));

        let avg = analyzer.avg_effective_spread(2);
        assert!(avg > dec!(0));
    }
}
