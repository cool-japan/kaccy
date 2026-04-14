//! Order Flow Toxicity Detection
//!
//! Implements advanced metrics to detect toxic order flow that may indicate
//! informed trading, adverse selection, or market manipulation. Market makers
//! can use these metrics to adjust spreads and manage inventory risk.

use crate::error::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Trade classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeDirection {
    /// Buyer-initiated trade.
    Buy,
    /// Seller-initiated trade.
    Sell,
}

/// Trade for toxicity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToxicityTrade {
    /// Unix timestamp of the trade in milliseconds.
    pub timestamp: i64,
    /// Execution price of the trade.
    pub price: Decimal,
    /// Trade volume.
    pub volume: Decimal,
    /// Buyer- or seller-initiated direction.
    pub direction: TradeDirection,
    /// Whether the order was aggressive (crossed the spread).
    pub is_aggressive: bool,
}

/// Toxicity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToxicityMetrics {
    /// Volume-synchronized Probability of Informed Trading (VPIN)
    pub vpin: Decimal,

    /// Order imbalance ratio (-1 to 1)
    pub order_imbalance: Decimal,

    /// Adverse selection score (0 to 1)
    pub adverse_selection: Decimal,

    /// Flow toxicity index (0 to 1)
    pub toxicity_index: Decimal,

    /// Recommended spread adjustment (multiplier)
    pub spread_adjustment: Decimal,
}

/// Order flow toxicity detector
#[derive(Debug)]
pub struct OrderFlowToxicityDetector {
    /// Bucket size for VPIN calculation (in volume)
    pub bucket_size: Decimal,

    /// Number of buckets to analyze
    pub num_buckets: usize,

    /// Trade history
    trades: VecDeque<ToxicityTrade>,

    /// Maximum trades to keep in memory
    max_trades: usize,

    /// Toxicity threshold for spread adjustment
    pub toxicity_threshold: Decimal,
}

impl OrderFlowToxicityDetector {
    /// Create a new toxicity detector
    pub fn new(bucket_size: Decimal, num_buckets: usize, max_trades: usize) -> Self {
        Self {
            bucket_size,
            num_buckets,
            trades: VecDeque::new(),
            max_trades,
            toxicity_threshold: dec!(0.5), // 50% threshold
        }
    }

    /// Add a trade to the history
    pub fn add_trade(&mut self, trade: ToxicityTrade) {
        self.trades.push_back(trade);

        // Keep only recent trades
        while self.trades.len() > self.max_trades {
            self.trades.pop_front();
        }
    }

    /// Calculate Volume-synchronized Probability of Informed Trading (VPIN)
    /// VPIN measures the probability that the next trade is informed
    /// Formula: VPIN = |V_buy - V_sell| / V_total over volume buckets
    pub fn calculate_vpin(&self) -> Result<Decimal> {
        if self.trades.is_empty() {
            return Ok(Decimal::ZERO);
        }

        // Organize trades into volume buckets
        let mut buckets = Vec::new();
        let mut current_bucket_buy = Decimal::ZERO;
        let mut current_bucket_sell = Decimal::ZERO;
        let mut current_bucket_volume = Decimal::ZERO;

        for trade in &self.trades {
            let remaining = self.bucket_size - current_bucket_volume;

            if trade.volume <= remaining {
                // Trade fits in current bucket
                match trade.direction {
                    TradeDirection::Buy => current_bucket_buy += trade.volume,
                    TradeDirection::Sell => current_bucket_sell += trade.volume,
                }
                current_bucket_volume += trade.volume;

                // Check if bucket is complete
                if current_bucket_volume >= self.bucket_size {
                    buckets.push((current_bucket_buy, current_bucket_sell));
                    current_bucket_buy = Decimal::ZERO;
                    current_bucket_sell = Decimal::ZERO;
                    current_bucket_volume = Decimal::ZERO;
                }
            } else {
                // Trade spans multiple buckets
                let mut remaining_volume = trade.volume;

                // Fill current bucket
                if remaining > Decimal::ZERO {
                    match trade.direction {
                        TradeDirection::Buy => current_bucket_buy += remaining,
                        TradeDirection::Sell => current_bucket_sell += remaining,
                    }
                    buckets.push((current_bucket_buy, current_bucket_sell));
                    remaining_volume -= remaining;
                }

                // Create additional buckets as needed
                while remaining_volume >= self.bucket_size {
                    match trade.direction {
                        TradeDirection::Buy => {
                            buckets.push((self.bucket_size, Decimal::ZERO));
                        }
                        TradeDirection::Sell => {
                            buckets.push((Decimal::ZERO, self.bucket_size));
                        }
                    }
                    remaining_volume -= self.bucket_size;
                }

                // Start new bucket with remainder
                current_bucket_buy = Decimal::ZERO;
                current_bucket_sell = Decimal::ZERO;
                current_bucket_volume = remaining_volume;
                match trade.direction {
                    TradeDirection::Buy => current_bucket_buy = remaining_volume,
                    TradeDirection::Sell => current_bucket_sell = remaining_volume,
                }
            }
        }

        // Calculate VPIN over the last N buckets
        let buckets_to_analyze = buckets.len().min(self.num_buckets);
        if buckets_to_analyze == 0 {
            return Ok(Decimal::ZERO);
        }

        let start_idx = buckets.len().saturating_sub(buckets_to_analyze);
        let mut total_imbalance = Decimal::ZERO;
        let mut total_volume = Decimal::ZERO;

        for (buy_vol, sell_vol) in &buckets[start_idx..] {
            let imbalance = (buy_vol - sell_vol).abs();
            total_imbalance += imbalance;
            total_volume += buy_vol + sell_vol;
        }

        let vpin = if total_volume > Decimal::ZERO {
            total_imbalance / total_volume
        } else {
            Decimal::ZERO
        };

        Ok(vpin)
    }

    /// Calculate order imbalance ratio
    /// Positive = more buying pressure, Negative = more selling pressure
    pub fn calculate_order_imbalance(&self, window_trades: usize) -> Result<Decimal> {
        if self.trades.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let trades_to_analyze = self.trades.len().min(window_trades);
        let start_idx = self.trades.len() - trades_to_analyze;

        let mut buy_volume = Decimal::ZERO;
        let mut sell_volume = Decimal::ZERO;

        for trade in self.trades.iter().skip(start_idx) {
            match trade.direction {
                TradeDirection::Buy => buy_volume += trade.volume,
                TradeDirection::Sell => sell_volume += trade.volume,
            }
        }

        let total_volume = buy_volume + sell_volume;
        let imbalance = if total_volume > Decimal::ZERO {
            (buy_volume - sell_volume) / total_volume
        } else {
            Decimal::ZERO
        };

        Ok(imbalance)
    }

    /// Calculate adverse selection score
    /// Measures how much price moves against the market maker after a trade
    pub fn calculate_adverse_selection(&self, window_trades: usize) -> Result<Decimal> {
        if self.trades.len() < 2 {
            return Ok(Decimal::ZERO);
        }

        let trades_to_analyze = self.trades.len().min(window_trades);
        let start_idx = self.trades.len() - trades_to_analyze;

        let mut adverse_count = 0;
        let mut total_count = 0;

        for i in start_idx..self.trades.len() - 1 {
            let current_trade = &self.trades[i];
            let next_price = self.trades[i + 1].price;

            // Check if price moved against the liquidity provider
            let is_adverse = match current_trade.direction {
                TradeDirection::Buy => next_price > current_trade.price,
                TradeDirection::Sell => next_price < current_trade.price,
            };

            if is_adverse {
                adverse_count += 1;
            }
            total_count += 1;
        }

        let adverse_selection = if total_count > 0 {
            Decimal::from(adverse_count) / Decimal::from(total_count)
        } else {
            Decimal::ZERO
        };

        Ok(adverse_selection)
    }

    /// Identify informed traders based on trade patterns
    pub fn identify_informed_traders(&self, user_trades: &[(Uuid, &ToxicityTrade)]) -> Vec<Uuid> {
        let mut trader_metrics: std::collections::HashMap<Uuid, (usize, usize)> =
            std::collections::HashMap::new();

        // Count profitable trades for each user
        for (i, (user_id, trade)) in user_trades.iter().enumerate() {
            // Look ahead to see if trade was profitable
            if i + 1 < user_trades.len() {
                let next_price = user_trades[i + 1].1.price;
                let is_profitable = match trade.direction {
                    TradeDirection::Buy => next_price > trade.price,
                    TradeDirection::Sell => next_price < trade.price,
                };

                let stats = trader_metrics.entry(*user_id).or_insert((0, 0));
                stats.1 += 1; // Total trades
                if is_profitable {
                    stats.0 += 1; // Profitable trades
                }
            }
        }

        // Identify traders with high win rate
        let mut informed_traders = Vec::new();
        for (user_id, (profitable, total)) in trader_metrics {
            if total >= 5 {
                // Minimum 5 trades
                let win_rate = Decimal::from(profitable) / Decimal::from(total);
                if win_rate > dec!(0.7) {
                    // >70% win rate
                    informed_traders.push(user_id);
                }
            }
        }

        informed_traders
    }

    /// Calculate comprehensive toxicity metrics
    pub fn calculate_toxicity_metrics(&self, window_trades: usize) -> Result<ToxicityMetrics> {
        let vpin = self.calculate_vpin()?;
        let order_imbalance = self.calculate_order_imbalance(window_trades)?;
        let adverse_selection = self.calculate_adverse_selection(window_trades)?;

        // Calculate composite toxicity index
        // Weighted average of different metrics
        let toxicity_index = (vpin * dec!(0.4))
            + (order_imbalance.abs() * dec!(0.3))
            + (adverse_selection * dec!(0.3));

        // Calculate recommended spread adjustment
        // Higher toxicity = wider spread
        let spread_adjustment = if toxicity_index > self.toxicity_threshold {
            dec!(1.0) + (toxicity_index - self.toxicity_threshold) * dec!(2.0)
        } else {
            dec!(1.0)
        };

        Ok(ToxicityMetrics {
            vpin,
            order_imbalance,
            adverse_selection,
            toxicity_index,
            spread_adjustment,
        })
    }

    /// Adjust quote based on toxicity
    pub fn adjust_quote_for_toxicity(
        &self,
        base_bid: Decimal,
        base_ask: Decimal,
        metrics: &ToxicityMetrics,
    ) -> Result<(Decimal, Decimal)> {
        let spread = base_ask - base_bid;
        let mid = (base_bid + base_ask) / dec!(2);

        // Widen spread based on toxicity
        let adjusted_spread = spread * metrics.spread_adjustment;

        // Also adjust mid price slightly based on order imbalance
        let mid_adjustment = mid * metrics.order_imbalance * dec!(0.001); // 0.1% max adjustment

        let adjusted_mid = mid + mid_adjustment;
        let half_spread = adjusted_spread / dec!(2);

        let adjusted_bid = adjusted_mid - half_spread;
        let adjusted_ask = adjusted_mid + half_spread;

        Ok((adjusted_bid, adjusted_ask))
    }
}

impl Default for OrderFlowToxicityDetector {
    fn default() -> Self {
        Self::new(
            dec!(1000), // 1000 units per bucket
            50,         // Analyze 50 buckets
            10000,      // Keep 10k trades
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_trade() {
        let mut detector = OrderFlowToxicityDetector::default();

        detector.add_trade(ToxicityTrade {
            timestamp: 1000,
            price: dec!(100),
            volume: dec!(10),
            direction: TradeDirection::Buy,
            is_aggressive: true,
        });

        assert_eq!(detector.trades.len(), 1);
    }

    #[test]
    fn test_order_imbalance() {
        let mut detector = OrderFlowToxicityDetector::default();

        // Add more buy trades
        for i in 0..7 {
            detector.add_trade(ToxicityTrade {
                timestamp: 1000 + i,
                price: dec!(100),
                volume: dec!(10),
                direction: TradeDirection::Buy,
                is_aggressive: true,
            });
        }

        // Add fewer sell trades
        for i in 0..3 {
            detector.add_trade(ToxicityTrade {
                timestamp: 2000 + i,
                price: dec!(100),
                volume: dec!(10),
                direction: TradeDirection::Sell,
                is_aggressive: true,
            });
        }

        let imbalance = detector.calculate_order_imbalance(100).unwrap();

        // Should be positive (more buying)
        assert!(imbalance > Decimal::ZERO);
        // 70 buy - 30 sell = 40 / 100 = 0.4
        assert_eq!(imbalance, dec!(0.4));
    }

    #[test]
    fn test_adverse_selection() {
        let mut detector = OrderFlowToxicityDetector::default();

        // Add trades where price moves against liquidity provider
        detector.add_trade(ToxicityTrade {
            timestamp: 1000,
            price: dec!(100),
            volume: dec!(10),
            direction: TradeDirection::Buy,
            is_aggressive: true,
        });

        detector.add_trade(ToxicityTrade {
            timestamp: 1001,
            price: dec!(105), // Price went up after buy (adverse)
            volume: dec!(10),
            direction: TradeDirection::Sell,
            is_aggressive: true,
        });

        detector.add_trade(ToxicityTrade {
            timestamp: 1002,
            price: dec!(103), // Price went down after sell (adverse)
            volume: dec!(10),
            direction: TradeDirection::Buy,
            is_aggressive: true,
        });

        let adverse_selection = detector.calculate_adverse_selection(100).unwrap();

        // Should be 100% adverse selection
        assert_eq!(adverse_selection, dec!(1.0));
    }

    #[test]
    fn test_vpin_calculation() {
        let mut detector = OrderFlowToxicityDetector::new(dec!(100), 5, 1000);

        // Add trades to create imbalance
        for i in 0..10 {
            detector.add_trade(ToxicityTrade {
                timestamp: 1000 + i,
                price: dec!(100),
                volume: dec!(15), // Will create multiple buckets
                direction: if i < 7 {
                    TradeDirection::Buy
                } else {
                    TradeDirection::Sell
                },
                is_aggressive: true,
            });
        }

        let vpin = detector.calculate_vpin().unwrap();

        // VPIN should be positive indicating order flow imbalance
        assert!(vpin > Decimal::ZERO);
        assert!(vpin <= dec!(1.0));
    }

    #[test]
    fn test_toxicity_metrics() {
        let mut detector = OrderFlowToxicityDetector::default();

        // Add some trades
        for i in 0..20 {
            detector.add_trade(ToxicityTrade {
                timestamp: 1000 + i,
                price: dec!(100) + Decimal::from(i % 5),
                volume: dec!(10),
                direction: if i % 3 == 0 {
                    TradeDirection::Buy
                } else {
                    TradeDirection::Sell
                },
                is_aggressive: i % 2 == 0,
            });
        }

        let metrics = detector.calculate_toxicity_metrics(20).unwrap();

        assert!(metrics.vpin >= Decimal::ZERO);
        assert!(metrics.vpin <= dec!(1.0));
        assert!(metrics.toxicity_index >= Decimal::ZERO);
        assert!(metrics.toxicity_index <= dec!(1.0));
        assert!(metrics.spread_adjustment >= dec!(1.0));
    }

    #[test]
    fn test_quote_adjustment() {
        let detector = OrderFlowToxicityDetector::default();

        let metrics = ToxicityMetrics {
            vpin: dec!(0.7),
            order_imbalance: dec!(0.3),
            adverse_selection: dec!(0.5),
            toxicity_index: dec!(0.6),
            spread_adjustment: dec!(1.5),
        };

        let (adjusted_bid, adjusted_ask) = detector
            .adjust_quote_for_toxicity(dec!(99), dec!(101), &metrics)
            .unwrap();

        // Spread should be wider than original
        let original_spread = dec!(2);
        let adjusted_spread = adjusted_ask - adjusted_bid;
        assert!(adjusted_spread > original_spread);
    }

    #[test]
    fn test_identify_informed_traders() {
        let detector = OrderFlowToxicityDetector::default();

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let mut user_trades = Vec::new();

        // User1: High win rate (informed)
        for i in 0..10 {
            let trade = ToxicityTrade {
                timestamp: 1000 + i,
                price: dec!(100) + Decimal::from(i),
                volume: dec!(10),
                direction: TradeDirection::Buy,
                is_aggressive: true,
            };
            user_trades.push((user1, trade));
        }

        // User2: Low win rate (not informed)
        for i in 0..5 {
            let trade = ToxicityTrade {
                timestamp: 2000 + i,
                price: dec!(100) - Decimal::from(i),
                volume: dec!(10),
                direction: TradeDirection::Sell,
                is_aggressive: true,
            };
            user_trades.push((user2, trade));
        }

        let user_trades_ref: Vec<(Uuid, &ToxicityTrade)> =
            user_trades.iter().map(|(id, trade)| (*id, trade)).collect();

        let informed = detector.identify_informed_traders(&user_trades_ref);

        // User1 should be identified as informed (consistent profitable pattern)
        assert!(informed.contains(&user1));
    }

    #[test]
    fn test_max_trades_limit() {
        let mut detector = OrderFlowToxicityDetector::new(dec!(100), 5, 100);

        // Add more than max trades
        for i in 0..150 {
            detector.add_trade(ToxicityTrade {
                timestamp: 1000 + i,
                price: dec!(100),
                volume: dec!(10),
                direction: TradeDirection::Buy,
                is_aggressive: true,
            });
        }

        // Should only keep max_trades
        assert_eq!(detector.trades.len(), 100);
    }
}
