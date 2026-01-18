//! Order flow analysis module
//!
//! This module provides tools for analyzing order flow and market microstructure,
//! including trade direction detection, order imbalance calculation, and flow toxicity metrics.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::trading::OrderSide;

/// Classification of order aggressiveness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderAggressiveness {
    /// Passive order (limit order that provides liquidity)
    Passive,
    /// Aggressive order (market order or marketable limit order)
    Aggressive,
}

/// Trade flow direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowDirection {
    /// Net buying pressure
    BuyPressure,
    /// Net selling pressure
    SellPressure,
    /// Balanced flow
    Neutral,
}

/// Represents a trade for flow analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowTrade {
    pub trade_id: Uuid,
    pub token_id: Uuid,
    pub side: OrderSide,
    pub price: Decimal,
    pub amount: Decimal,
    pub timestamp: DateTime<Utc>,
    pub aggressiveness: OrderAggressiveness,
}

/// Order imbalance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderImbalance {
    /// Buy volume
    pub buy_volume: Decimal,
    /// Sell volume
    pub sell_volume: Decimal,
    /// Imbalance ratio: (buy - sell) / (buy + sell)
    pub imbalance_ratio: Decimal,
    /// Number of buy orders
    pub buy_count: u64,
    /// Number of sell orders
    pub sell_count: u64,
    /// Calculation timestamp
    pub timestamp: DateTime<Utc>,
}

impl OrderImbalance {
    /// Calculate the flow direction based on imbalance
    pub fn flow_direction(&self) -> FlowDirection {
        if self.imbalance_ratio > dec!(0.2) {
            FlowDirection::BuyPressure
        } else if self.imbalance_ratio < dec!(-0.2) {
            FlowDirection::SellPressure
        } else {
            FlowDirection::Neutral
        }
    }

    /// Check if imbalance is significant
    pub fn is_significant(&self) -> bool {
        self.imbalance_ratio.abs() > dec!(0.3)
    }
}

/// Flow toxicity metrics (measures information content of order flow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowToxicity {
    /// Volume-synchronized probability of informed trading (VPIN)
    pub vpin: Decimal,
    /// Order flow imbalance
    pub order_flow_imbalance: Decimal,
    /// Trade intensity (trades per minute)
    pub trade_intensity: Decimal,
    /// Adverse selection component
    pub adverse_selection: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl FlowToxicity {
    /// Check if flow is toxic (high probability of informed trading)
    pub fn is_toxic(&self) -> bool {
        self.vpin > dec!(0.7) || self.adverse_selection > dec!(0.6)
    }

    /// Get toxicity level (0-10)
    pub fn toxicity_level(&self) -> u8 {
        let score = (self.vpin * dec!(10)).round();
        score.to_string().parse::<u8>().unwrap_or(0).min(10)
    }
}

/// Order flow analyzer
#[derive(Debug, Clone)]
pub struct OrderFlowAnalyzer {
    token_id: Uuid,
    trades: VecDeque<FlowTrade>,
    max_history: usize,
    #[allow(dead_code)]
    bucket_duration_seconds: i64,
}

impl OrderFlowAnalyzer {
    /// Create a new order flow analyzer
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            trades: VecDeque::new(),
            max_history: 1000,
            bucket_duration_seconds: 60, // 1 minute buckets
        }
    }

    /// Create with custom settings
    pub fn with_settings(token_id: Uuid, max_history: usize, bucket_duration_seconds: i64) -> Self {
        Self {
            token_id,
            trades: VecDeque::new(),
            max_history,
            bucket_duration_seconds,
        }
    }

    /// Add a trade to the analyzer
    pub fn add_trade(&mut self, trade: FlowTrade) {
        // Ensure trade is for the correct token
        if trade.token_id != self.token_id {
            return;
        }

        self.trades.push_back(trade);

        // Trim old trades
        while self.trades.len() > self.max_history {
            self.trades.pop_front();
        }
    }

    /// Calculate order imbalance for recent trades
    pub fn calculate_imbalance(&self, window_seconds: i64) -> OrderImbalance {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        let mut buy_volume = dec!(0);
        let mut sell_volume = dec!(0);
        let mut buy_count = 0u64;
        let mut sell_count = 0u64;

        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }

            match trade.side {
                OrderSide::Buy => {
                    buy_volume += trade.amount;
                    buy_count += 1;
                }
                OrderSide::Sell => {
                    sell_volume += trade.amount;
                    sell_count += 1;
                }
            }
        }

        let total_volume = buy_volume + sell_volume;
        let imbalance_ratio = if total_volume > dec!(0) {
            (buy_volume - sell_volume) / total_volume
        } else {
            dec!(0)
        };

        OrderImbalance {
            buy_volume,
            sell_volume,
            imbalance_ratio,
            buy_count,
            sell_count,
            timestamp: now,
        }
    }

    /// Calculate flow toxicity metrics (VPIN-based)
    pub fn calculate_toxicity(&self, window_seconds: i64, bucket_count: usize) -> FlowToxicity {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        // Split trades into volume buckets
        let mut buckets: Vec<(Decimal, Decimal)> = vec![(dec!(0), dec!(0)); bucket_count];
        let mut total_volume = dec!(0);
        let mut trade_count = 0;

        // Calculate total volume first
        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }
            total_volume += trade.amount;
            trade_count += 1;
        }

        if total_volume == dec!(0) || trade_count == 0 {
            return FlowToxicity {
                vpin: dec!(0),
                order_flow_imbalance: dec!(0),
                trade_intensity: dec!(0),
                adverse_selection: dec!(0),
                timestamp: now,
            };
        }

        let volume_per_bucket = total_volume / Decimal::from(bucket_count);
        let mut current_bucket = 0;
        let mut bucket_volume = dec!(0);

        // Distribute trades into volume buckets
        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }

            if current_bucket >= bucket_count {
                break;
            }

            let remaining_in_bucket = volume_per_bucket - bucket_volume;
            let trade_volume = trade.amount.min(remaining_in_bucket);

            match trade.side {
                OrderSide::Buy => buckets[current_bucket].0 += trade_volume,
                OrderSide::Sell => buckets[current_bucket].1 += trade_volume,
            }

            bucket_volume += trade_volume;

            if bucket_volume >= volume_per_bucket {
                current_bucket += 1;
                bucket_volume = dec!(0);
            }
        }

        // Calculate VPIN (average absolute order imbalance across buckets)
        let mut vpin_sum = dec!(0);
        let mut valid_buckets = 0;

        for (buy_vol, sell_vol) in &buckets {
            let bucket_total = buy_vol + sell_vol;
            if bucket_total > dec!(0) {
                let imbalance = (buy_vol - sell_vol).abs() / bucket_total;
                vpin_sum += imbalance;
                valid_buckets += 1;
            }
        }

        let vpin = if valid_buckets > 0 {
            vpin_sum / Decimal::from(valid_buckets)
        } else {
            dec!(0)
        };

        // Calculate overall order flow imbalance
        let total_buy: Decimal = buckets.iter().map(|(b, _)| b).sum();
        let total_sell: Decimal = buckets.iter().map(|(_, s)| s).sum();
        let ofi = if total_volume > dec!(0) {
            (total_buy - total_sell) / total_volume
        } else {
            dec!(0)
        };

        // Calculate trade intensity (trades per minute)
        let duration_minutes = Decimal::from(window_seconds) / dec!(60);
        let trade_intensity = if duration_minutes > dec!(0) {
            Decimal::from(trade_count) / duration_minutes
        } else {
            dec!(0)
        };

        // Adverse selection is approximated by VPIN and trade intensity
        let adverse_selection = (vpin * dec!(0.7)) + (trade_intensity / dec!(100) * dec!(0.3));

        FlowToxicity {
            vpin,
            order_flow_imbalance: ofi,
            trade_intensity,
            adverse_selection: adverse_selection.min(dec!(1.0)),
            timestamp: now,
        }
    }

    /// Classify order aggressiveness based on price movement
    pub fn classify_aggressiveness(
        &self,
        price: Decimal,
        side: OrderSide,
        mid_price: Decimal,
    ) -> OrderAggressiveness {
        // If the order is on the "wrong" side of the mid price, it's aggressive
        match side {
            OrderSide::Buy => {
                if price >= mid_price {
                    OrderAggressiveness::Aggressive
                } else {
                    OrderAggressiveness::Passive
                }
            }
            OrderSide::Sell => {
                if price <= mid_price {
                    OrderAggressiveness::Aggressive
                } else {
                    OrderAggressiveness::Passive
                }
            }
        }
    }

    /// Get aggressive vs passive trade statistics
    pub fn get_aggressiveness_stats(&self, window_seconds: i64) -> (Decimal, Decimal, Decimal) {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        let mut aggressive_volume = dec!(0);
        let mut passive_volume = dec!(0);

        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }

            match trade.aggressiveness {
                OrderAggressiveness::Aggressive => {
                    aggressive_volume += trade.amount;
                }
                OrderAggressiveness::Passive => {
                    passive_volume += trade.amount;
                }
            }
        }

        let total_volume = aggressive_volume + passive_volume;
        let aggressive_ratio = if total_volume > dec!(0) {
            aggressive_volume / total_volume
        } else {
            dec!(0)
        };

        (aggressive_volume, passive_volume, aggressive_ratio)
    }

    /// Get the number of stored trades
    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Clear all stored trades
    pub fn clear(&mut self) {
        self.trades.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_imbalance_flow_direction() {
        let imbalance = OrderImbalance {
            buy_volume: dec!(100),
            sell_volume: dec!(50),
            imbalance_ratio: dec!(0.333),
            buy_count: 10,
            sell_count: 5,
            timestamp: Utc::now(),
        };

        assert_eq!(imbalance.flow_direction(), FlowDirection::BuyPressure);
        assert!(imbalance.is_significant());
    }

    #[test]
    fn test_flow_toxicity_detection() {
        let toxicity = FlowToxicity {
            vpin: dec!(0.8),
            order_flow_imbalance: dec!(0.5),
            trade_intensity: dec!(50),
            adverse_selection: dec!(0.7),
            timestamp: Utc::now(),
        };

        assert!(toxicity.is_toxic());
        assert!(toxicity.toxicity_level() >= 8);
    }

    #[test]
    fn test_order_flow_analyzer_add_trade() {
        let token_id = Uuid::new_v4();
        let mut analyzer = OrderFlowAnalyzer::new(token_id);

        let trade = FlowTrade {
            trade_id: Uuid::new_v4(),
            token_id,
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
            timestamp: Utc::now(),
            aggressiveness: OrderAggressiveness::Aggressive,
        };

        analyzer.add_trade(trade);
        assert_eq!(analyzer.trade_count(), 1);
    }

    #[test]
    fn test_calculate_imbalance() {
        let token_id = Uuid::new_v4();
        let mut analyzer = OrderFlowAnalyzer::new(token_id);

        // Add buy trades
        for _ in 0..3 {
            analyzer.add_trade(FlowTrade {
                trade_id: Uuid::new_v4(),
                token_id,
                side: OrderSide::Buy,
                price: dec!(100),
                amount: dec!(10),
                timestamp: Utc::now(),
                aggressiveness: OrderAggressiveness::Aggressive,
            });
        }

        // Add sell trade
        analyzer.add_trade(FlowTrade {
            trade_id: Uuid::new_v4(),
            token_id,
            side: OrderSide::Sell,
            price: dec!(100),
            amount: dec!(10),
            timestamp: Utc::now(),
            aggressiveness: OrderAggressiveness::Aggressive,
        });

        let imbalance = analyzer.calculate_imbalance(60);
        assert_eq!(imbalance.buy_count, 3);
        assert_eq!(imbalance.sell_count, 1);
        assert!(imbalance.imbalance_ratio > dec!(0));
    }

    #[test]
    fn test_classify_aggressiveness() {
        let token_id = Uuid::new_v4();
        let analyzer = OrderFlowAnalyzer::new(token_id);

        // Buy above mid is aggressive
        assert_eq!(
            analyzer.classify_aggressiveness(dec!(105), OrderSide::Buy, dec!(100)),
            OrderAggressiveness::Aggressive
        );

        // Buy below mid is passive
        assert_eq!(
            analyzer.classify_aggressiveness(dec!(95), OrderSide::Buy, dec!(100)),
            OrderAggressiveness::Passive
        );

        // Sell below mid is aggressive
        assert_eq!(
            analyzer.classify_aggressiveness(dec!(95), OrderSide::Sell, dec!(100)),
            OrderAggressiveness::Aggressive
        );

        // Sell above mid is passive
        assert_eq!(
            analyzer.classify_aggressiveness(dec!(105), OrderSide::Sell, dec!(100)),
            OrderAggressiveness::Passive
        );
    }

    #[test]
    fn test_aggressiveness_stats() {
        let token_id = Uuid::new_v4();
        let mut analyzer = OrderFlowAnalyzer::new(token_id);

        // Add aggressive trades
        for _ in 0..2 {
            analyzer.add_trade(FlowTrade {
                trade_id: Uuid::new_v4(),
                token_id,
                side: OrderSide::Buy,
                price: dec!(100),
                amount: dec!(10),
                timestamp: Utc::now(),
                aggressiveness: OrderAggressiveness::Aggressive,
            });
        }

        // Add passive trade
        analyzer.add_trade(FlowTrade {
            trade_id: Uuid::new_v4(),
            token_id,
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
            timestamp: Utc::now(),
            aggressiveness: OrderAggressiveness::Passive,
        });

        let (aggressive_vol, passive_vol, ratio) = analyzer.get_aggressiveness_stats(60);
        assert_eq!(aggressive_vol, dec!(20));
        assert_eq!(passive_vol, dec!(10));
        assert!(ratio > dec!(0.6));
    }

    #[test]
    fn test_calculate_toxicity() {
        let token_id = Uuid::new_v4();
        let mut analyzer = OrderFlowAnalyzer::new(token_id);

        // Add imbalanced trades (more buys than sells)
        for i in 0..10 {
            let side = if i < 7 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };

            analyzer.add_trade(FlowTrade {
                trade_id: Uuid::new_v4(),
                token_id,
                side,
                price: dec!(100),
                amount: dec!(10),
                timestamp: Utc::now(),
                aggressiveness: OrderAggressiveness::Aggressive,
            });
        }

        let toxicity = analyzer.calculate_toxicity(60, 5);
        assert!(toxicity.vpin >= dec!(0));
        assert!(toxicity.trade_intensity > dec!(0));
    }

    #[test]
    fn test_max_history_limit() {
        let token_id = Uuid::new_v4();
        let mut analyzer = OrderFlowAnalyzer::with_settings(token_id, 10, 60);

        // Add 20 trades
        for _ in 0..20 {
            analyzer.add_trade(FlowTrade {
                trade_id: Uuid::new_v4(),
                token_id,
                side: OrderSide::Buy,
                price: dec!(100),
                amount: dec!(10),
                timestamp: Utc::now(),
                aggressiveness: OrderAggressiveness::Aggressive,
            });
        }

        // Should only keep 10 most recent trades
        assert_eq!(analyzer.trade_count(), 10);
    }
}
