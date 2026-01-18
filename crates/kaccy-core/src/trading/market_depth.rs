//! Market depth analytics module
//!
//! This module provides tools for analyzing order book depth, including
//! skewness calculation, bid-ask spread dynamics, depth imbalance indicators,
//! and liquidity heat maps.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Order book depth level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub volume: Decimal,
    pub order_count: u32,
}

/// Order book skewness metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSkewness {
    /// Bid-side liquidity
    pub bid_liquidity: Decimal,
    /// Ask-side liquidity
    pub ask_liquidity: Decimal,
    /// Skewness ratio: (bid - ask) / (bid + ask)
    pub skewness_ratio: Decimal,
    /// Imbalance direction
    pub imbalance_direction: SkewnessDirection,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Direction of order book skewness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkewnessDirection {
    /// More liquidity on bid side (buying pressure)
    BidHeavy,
    /// More liquidity on ask side (selling pressure)
    AskHeavy,
    /// Balanced liquidity
    Balanced,
}

impl OrderBookSkewness {
    /// Check if skewness is significant
    pub fn is_significant(&self) -> bool {
        self.skewness_ratio.abs() > dec!(0.2)
    }

    /// Get strength of skewness (0-100)
    pub fn strength(&self) -> u8 {
        let score = (self.skewness_ratio.abs() * dec!(100)).min(dec!(100));
        score.round().to_u8().unwrap_or(0)
    }
}

/// Bid-ask spread dynamics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadDynamics {
    /// Best bid price
    pub best_bid: Decimal,
    /// Best ask price
    pub best_ask: Decimal,
    /// Absolute spread
    pub absolute_spread: Decimal,
    /// Relative spread (percentage)
    pub relative_spread: Decimal,
    /// Mid price
    pub mid_price: Decimal,
    /// Spread volatility (rolling std dev)
    pub spread_volatility: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl SpreadDynamics {
    /// Check if spread is tight
    pub fn is_tight(&self) -> bool {
        self.relative_spread < dec!(0.01) // < 1%
    }

    /// Check if spread is wide
    pub fn is_wide(&self) -> bool {
        self.relative_spread > dec!(0.05) // > 5%
    }

    /// Get spread quality score (0-100, higher is better)
    pub fn quality_score(&self) -> u8 {
        let tightness = (dec!(1) - (self.relative_spread * dec!(20))).max(dec!(0));
        let score = (tightness * dec!(100)).min(dec!(100));
        score.round().to_u8().unwrap_or(0)
    }
}

/// Depth imbalance at various levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthImbalance {
    /// Levels analyzed (e.g., top 5 levels)
    pub levels: u32,
    /// Total bid volume
    pub total_bid_volume: Decimal,
    /// Total ask volume
    pub total_ask_volume: Decimal,
    /// Imbalance ratio
    pub imbalance_ratio: Decimal,
    /// Bid order count
    pub bid_order_count: u32,
    /// Ask order count
    pub ask_order_count: u32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl DepthImbalance {
    /// Get pressure direction
    pub fn pressure_direction(&self) -> SkewnessDirection {
        if self.imbalance_ratio > dec!(0.15) {
            SkewnessDirection::BidHeavy
        } else if self.imbalance_ratio < dec!(-0.15) {
            SkewnessDirection::AskHeavy
        } else {
            SkewnessDirection::Balanced
        }
    }

    /// Check if imbalance is extreme
    pub fn is_extreme(&self) -> bool {
        self.imbalance_ratio.abs() > dec!(0.5)
    }
}

/// Liquidity heat map (volume distribution across price levels)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityHeatMap {
    /// Price levels with volume
    pub bid_levels: Vec<PriceLevel>,
    /// Ask levels with volume
    pub ask_levels: Vec<PriceLevel>,
    /// Mid price reference
    pub mid_price: Decimal,
    /// Total bid liquidity
    pub total_bid_liquidity: Decimal,
    /// Total ask liquidity
    pub total_ask_liquidity: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl LiquidityHeatMap {
    /// Find the price level with maximum liquidity on bid side
    pub fn max_bid_liquidity_level(&self) -> Option<&PriceLevel> {
        self.bid_levels
            .iter()
            .max_by(|a, b| a.volume.cmp(&b.volume))
    }

    /// Find the price level with maximum liquidity on ask side
    pub fn max_ask_liquidity_level(&self) -> Option<&PriceLevel> {
        self.ask_levels
            .iter()
            .max_by(|a, b| a.volume.cmp(&b.volume))
    }

    /// Calculate concentration score (0-100, higher means more concentrated)
    pub fn concentration_score(&self) -> u8 {
        let max_bid_vol = self
            .max_bid_liquidity_level()
            .map(|l| l.volume)
            .unwrap_or(dec!(0));
        let max_ask_vol = self
            .max_ask_liquidity_level()
            .map(|l| l.volume)
            .unwrap_or(dec!(0));

        let total_vol = self.total_bid_liquidity + self.total_ask_liquidity;
        if total_vol == dec!(0) {
            return 0;
        }

        let concentration = ((max_bid_vol + max_ask_vol) / total_vol) * dec!(100);
        concentration.round().to_u8().unwrap_or(0)
    }
}

/// Market depth analyzer
#[derive(Debug, Clone)]
pub struct MarketDepthAnalyzer {
    #[allow(dead_code)]
    token_id: Uuid,
    bid_levels: BTreeMap<Decimal, PriceLevel>,
    ask_levels: BTreeMap<Decimal, PriceLevel>,
    spread_history: Vec<Decimal>,
    max_history: usize,
}

impl MarketDepthAnalyzer {
    /// Create a new market depth analyzer
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            bid_levels: BTreeMap::new(),
            ask_levels: BTreeMap::new(),
            spread_history: Vec::new(),
            max_history: 100,
        }
    }

    /// Update bid levels
    pub fn update_bid_level(&mut self, price: Decimal, volume: Decimal, order_count: u32) {
        if volume > dec!(0) {
            self.bid_levels.insert(
                price,
                PriceLevel {
                    price,
                    volume,
                    order_count,
                },
            );
        } else {
            self.bid_levels.remove(&price);
        }
    }

    /// Update ask levels
    pub fn update_ask_level(&mut self, price: Decimal, volume: Decimal, order_count: u32) {
        if volume > dec!(0) {
            self.ask_levels.insert(
                price,
                PriceLevel {
                    price,
                    volume,
                    order_count,
                },
            );
        } else {
            self.ask_levels.remove(&price);
        }
    }

    /// Get best bid price
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bid_levels.keys().next_back().copied()
    }

    /// Get best ask price
    pub fn best_ask(&self) -> Option<Decimal> {
        self.ask_levels.keys().next().copied()
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some((bid + ask) / dec!(2))
    }

    /// Calculate order book skewness
    pub fn calculate_skewness(&self, depth_levels: usize) -> OrderBookSkewness {
        let bid_liquidity: Decimal = self
            .bid_levels
            .values()
            .rev()
            .take(depth_levels)
            .map(|l| l.volume)
            .sum();

        let ask_liquidity: Decimal = self
            .ask_levels
            .values()
            .take(depth_levels)
            .map(|l| l.volume)
            .sum();

        let total_liquidity = bid_liquidity + ask_liquidity;
        let skewness_ratio = if total_liquidity > dec!(0) {
            (bid_liquidity - ask_liquidity) / total_liquidity
        } else {
            dec!(0)
        };

        let imbalance_direction = if skewness_ratio > dec!(0.15) {
            SkewnessDirection::BidHeavy
        } else if skewness_ratio < dec!(-0.15) {
            SkewnessDirection::AskHeavy
        } else {
            SkewnessDirection::Balanced
        };

        OrderBookSkewness {
            bid_liquidity,
            ask_liquidity,
            skewness_ratio,
            imbalance_direction,
            timestamp: Utc::now(),
        }
    }

    /// Calculate spread dynamics
    pub fn calculate_spread_dynamics(&mut self) -> Option<SpreadDynamics> {
        let best_bid = self.best_bid()?;
        let best_ask = self.best_ask()?;
        let absolute_spread = best_ask - best_bid;
        let mid_price = (best_bid + best_ask) / dec!(2);
        let relative_spread = if mid_price > dec!(0) {
            absolute_spread / mid_price
        } else {
            dec!(0)
        };

        // Track spread history
        self.spread_history.push(relative_spread);
        if self.spread_history.len() > self.max_history {
            self.spread_history.remove(0);
        }

        // Calculate spread volatility (standard deviation)
        let spread_volatility = if self.spread_history.len() > 1 {
            let mean: Decimal = self.spread_history.iter().sum::<Decimal>()
                / Decimal::from(self.spread_history.len());
            let variance: Decimal = self
                .spread_history
                .iter()
                .map(|&s| {
                    let diff = s - mean;
                    diff * diff
                })
                .sum::<Decimal>()
                / Decimal::from(self.spread_history.len());
            // Approximate square root
            variance.sqrt().unwrap_or(dec!(0))
        } else {
            dec!(0)
        };

        Some(SpreadDynamics {
            best_bid,
            best_ask,
            absolute_spread,
            relative_spread,
            mid_price,
            spread_volatility,
            timestamp: Utc::now(),
        })
    }

    /// Calculate depth imbalance
    pub fn calculate_depth_imbalance(&self, levels: u32) -> DepthImbalance {
        let bid_data: Vec<_> = self
            .bid_levels
            .values()
            .rev()
            .take(levels as usize)
            .collect();
        let ask_data: Vec<_> = self.ask_levels.values().take(levels as usize).collect();

        let total_bid_volume: Decimal = bid_data.iter().map(|l| l.volume).sum();
        let total_ask_volume: Decimal = ask_data.iter().map(|l| l.volume).sum();
        let bid_order_count: u32 = bid_data.iter().map(|l| l.order_count).sum();
        let ask_order_count: u32 = ask_data.iter().map(|l| l.order_count).sum();

        let total_volume = total_bid_volume + total_ask_volume;
        let imbalance_ratio = if total_volume > dec!(0) {
            (total_bid_volume - total_ask_volume) / total_volume
        } else {
            dec!(0)
        };

        DepthImbalance {
            levels,
            total_bid_volume,
            total_ask_volume,
            imbalance_ratio,
            bid_order_count,
            ask_order_count,
            timestamp: Utc::now(),
        }
    }

    /// Generate liquidity heat map
    pub fn generate_heat_map(&self, levels: usize) -> LiquidityHeatMap {
        let bid_levels: Vec<PriceLevel> = self
            .bid_levels
            .values()
            .rev()
            .take(levels)
            .cloned()
            .collect();

        let ask_levels: Vec<PriceLevel> = self.ask_levels.values().take(levels).cloned().collect();

        let total_bid_liquidity: Decimal = bid_levels.iter().map(|l| l.volume).sum();
        let total_ask_liquidity: Decimal = ask_levels.iter().map(|l| l.volume).sum();

        let mid_price = self.mid_price().unwrap_or(dec!(0));

        LiquidityHeatMap {
            bid_levels,
            ask_levels,
            mid_price,
            total_bid_liquidity,
            total_ask_liquidity,
            timestamp: Utc::now(),
        }
    }

    /// Clear all depth data
    pub fn clear(&mut self) {
        self.bid_levels.clear();
        self.ask_levels.clear();
        self.spread_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_skewness_direction() {
        let skewness = OrderBookSkewness {
            bid_liquidity: dec!(1000),
            ask_liquidity: dec!(500),
            skewness_ratio: dec!(0.333),
            imbalance_direction: SkewnessDirection::BidHeavy,
            timestamp: Utc::now(),
        };

        assert_eq!(skewness.imbalance_direction, SkewnessDirection::BidHeavy);
        assert!(skewness.is_significant());
        assert_eq!(skewness.strength(), 33);
    }

    #[test]
    fn test_spread_dynamics_quality() {
        let spread = SpreadDynamics {
            best_bid: dec!(99.8),
            best_ask: dec!(100.2),
            absolute_spread: dec!(0.4),
            relative_spread: dec!(0.004),
            mid_price: dec!(100),
            spread_volatility: dec!(0.001),
            timestamp: Utc::now(),
        };

        assert!(spread.is_tight());
        assert!(!spread.is_wide());
        assert!(spread.quality_score() >= 80);
    }

    #[test]
    fn test_depth_imbalance_pressure() {
        let imbalance = DepthImbalance {
            levels: 5,
            total_bid_volume: dec!(1000),
            total_ask_volume: dec!(500),
            imbalance_ratio: dec!(0.333),
            bid_order_count: 10,
            ask_order_count: 5,
            timestamp: Utc::now(),
        };

        assert_eq!(imbalance.pressure_direction(), SkewnessDirection::BidHeavy);
        assert!(!imbalance.is_extreme());
    }

    #[test]
    fn test_market_depth_analyzer_basic() {
        let token_id = Uuid::new_v4();
        let mut analyzer = MarketDepthAnalyzer::new(token_id);

        // Add bid levels
        analyzer.update_bid_level(dec!(99), dec!(100), 5);
        analyzer.update_bid_level(dec!(98), dec!(200), 10);

        // Add ask levels
        analyzer.update_ask_level(dec!(101), dec!(150), 7);
        analyzer.update_ask_level(dec!(102), dec!(250), 12);

        assert_eq!(analyzer.best_bid(), Some(dec!(99)));
        assert_eq!(analyzer.best_ask(), Some(dec!(101)));
        assert_eq!(analyzer.mid_price(), Some(dec!(100)));
    }

    #[test]
    fn test_calculate_skewness() {
        let token_id = Uuid::new_v4();
        let mut analyzer = MarketDepthAnalyzer::new(token_id);

        // More bids than asks
        analyzer.update_bid_level(dec!(99), dec!(1000), 10);
        analyzer.update_bid_level(dec!(98), dec!(500), 5);
        analyzer.update_ask_level(dec!(101), dec!(200), 2);
        analyzer.update_ask_level(dec!(102), dec!(100), 1);

        let skewness = analyzer.calculate_skewness(2);
        assert!(skewness.skewness_ratio > dec!(0));
        assert_eq!(skewness.imbalance_direction, SkewnessDirection::BidHeavy);
    }

    #[test]
    fn test_calculate_spread_dynamics() {
        let token_id = Uuid::new_v4();
        let mut analyzer = MarketDepthAnalyzer::new(token_id);

        analyzer.update_bid_level(dec!(99), dec!(100), 5);
        analyzer.update_ask_level(dec!(101), dec!(100), 5);

        let spread = analyzer.calculate_spread_dynamics().unwrap();
        assert_eq!(spread.best_bid, dec!(99));
        assert_eq!(spread.best_ask, dec!(101));
        assert_eq!(spread.absolute_spread, dec!(2));
        assert_eq!(spread.mid_price, dec!(100));
    }

    #[test]
    fn test_calculate_depth_imbalance() {
        let token_id = Uuid::new_v4();
        let mut analyzer = MarketDepthAnalyzer::new(token_id);

        analyzer.update_bid_level(dec!(99), dec!(500), 5);
        analyzer.update_bid_level(dec!(98), dec!(300), 3);
        analyzer.update_ask_level(dec!(101), dec!(200), 2);
        analyzer.update_ask_level(dec!(102), dec!(100), 1);

        let imbalance = analyzer.calculate_depth_imbalance(2);
        assert_eq!(imbalance.total_bid_volume, dec!(800));
        assert_eq!(imbalance.total_ask_volume, dec!(300));
        assert!(imbalance.imbalance_ratio > dec!(0));
    }

    #[test]
    fn test_generate_heat_map() {
        let token_id = Uuid::new_v4();
        let mut analyzer = MarketDepthAnalyzer::new(token_id);

        analyzer.update_bid_level(dec!(99), dec!(100), 5);
        analyzer.update_bid_level(dec!(98), dec!(200), 10);
        analyzer.update_ask_level(dec!(101), dec!(150), 7);
        analyzer.update_ask_level(dec!(102), dec!(250), 12);

        let heat_map = analyzer.generate_heat_map(2);
        assert_eq!(heat_map.bid_levels.len(), 2);
        assert_eq!(heat_map.ask_levels.len(), 2);
        assert_eq!(heat_map.total_bid_liquidity, dec!(300));
        assert_eq!(heat_map.total_ask_liquidity, dec!(400));
    }

    #[test]
    fn test_liquidity_heat_map_concentration() {
        let heat_map = LiquidityHeatMap {
            bid_levels: vec![
                PriceLevel {
                    price: dec!(99),
                    volume: dec!(900),
                    order_count: 10,
                },
                PriceLevel {
                    price: dec!(98),
                    volume: dec!(100),
                    order_count: 5,
                },
            ],
            ask_levels: vec![
                PriceLevel {
                    price: dec!(101),
                    volume: dec!(800),
                    order_count: 8,
                },
                PriceLevel {
                    price: dec!(102),
                    volume: dec!(200),
                    order_count: 3,
                },
            ],
            mid_price: dec!(100),
            total_bid_liquidity: dec!(1000),
            total_ask_liquidity: dec!(1000),
            timestamp: Utc::now(),
        };

        let concentration = heat_map.concentration_score();
        // (900 + 800) / 2000 = 0.85 = 85%
        assert_eq!(concentration, 85);
    }
}
