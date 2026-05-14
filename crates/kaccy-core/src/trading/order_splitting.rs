//! Intelligent order splitting algorithms for optimal execution
//!
//! This module provides various order splitting strategies to minimize
//! market impact and achieve better execution prices.

use crate::error::Result;
use crate::models::Order;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Split order configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitOrder {
    /// Quantity to execute in this split.
    pub amount: Decimal,
    /// Scheduled execution time for this split.
    pub execution_time: DateTime<Utc>,
    /// Maximum acceptable slippage (optional).
    pub max_slippage: Option<Decimal>,
    /// Human-readable description of this split.
    pub description: String,
}

/// Order splitting strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplittingStrategy {
    /// Equal-sized chunks
    Uniform,
    /// Volume-weighted splitting based on historical patterns
    VolumeWeighted,
    /// Time-weighted splitting (TWAP-style)
    TimeWeighted,
    /// Adaptive splitting based on market conditions
    Adaptive,
}

/// Volume-weighted order splitter
#[derive(Debug, Clone)]
pub struct VolumeWeightedSplitter {
    volume_profile: Vec<(DateTime<Utc>, Decimal)>,
}

impl VolumeWeightedSplitter {
    /// Create a new volume-weighted splitter from a historical volume profile.
    pub fn new(volume_profile: Vec<(DateTime<Utc>, Decimal)>) -> Self {
        Self { volume_profile }
    }

    /// Split order based on historical volume patterns
    pub fn split(
        &self,
        order: &Order,
        num_splits: usize,
        duration: Duration,
    ) -> Result<Vec<SplitOrder>> {
        if num_splits == 0 {
            return Ok(vec![]);
        }

        if num_splits == 1 {
            return Ok(vec![SplitOrder {
                amount: order.amount,
                execution_time: Utc::now(),
                max_slippage: None,
                description: "Single order execution".to_string(),
            }]);
        }

        let total_volume: Decimal = self.volume_profile.iter().map(|(_, vol)| vol).sum();

        if total_volume == Decimal::ZERO {
            // Fallback to uniform splitting
            return self.uniform_split(order, num_splits, duration);
        }

        let start_time = Utc::now();
        let time_per_split = duration / num_splits as i32;
        let mut splits = Vec::new();
        let mut allocated = Decimal::ZERO;

        for i in 0..num_splits {
            let execution_time = start_time + time_per_split * i as i32;

            // Find closest volume data point
            let closest_volume = self
                .volume_profile
                .iter()
                .min_by_key(|(time, _)| (*time - execution_time).num_seconds().abs())
                .map(|(_, vol)| *vol)
                .unwrap_or(Decimal::ONE);

            let weight = closest_volume / total_volume;
            let amount = if i == num_splits - 1 {
                // Last split gets remainder to avoid rounding errors
                order.amount - allocated
            } else {
                order.amount * weight
            };

            allocated += amount;

            splits.push(SplitOrder {
                amount,
                execution_time,
                max_slippage: None,
                description: format!("Volume-weighted split {}/{}", i + 1, num_splits),
            });
        }

        Ok(splits)
    }

    fn uniform_split(
        &self,
        order: &Order,
        num_splits: usize,
        duration: Duration,
    ) -> Result<Vec<SplitOrder>> {
        let start_time = Utc::now();
        let time_per_split = duration / num_splits as i32;
        let base_amount = order.amount / Decimal::from(num_splits);
        let mut splits = Vec::new();
        let mut allocated = Decimal::ZERO;

        for i in 0..num_splits {
            let amount = if i == num_splits - 1 {
                order.amount - allocated
            } else {
                base_amount
            };

            allocated += amount;

            splits.push(SplitOrder {
                amount,
                execution_time: start_time + time_per_split * i as i32,
                max_slippage: None,
                description: format!("Uniform split {}/{}", i + 1, num_splits),
            });
        }

        Ok(splits)
    }
}

/// Time-weighted order splitter (TWAP-style)
#[derive(Debug, Clone)]
pub struct TimeWeightedSplitter {
    #[allow(dead_code)]
    participation_rate: Decimal,
}

impl TimeWeightedSplitter {
    /// Create a new time-weighted splitter with the given participation rate.
    pub fn new(participation_rate: Decimal) -> Self {
        Self { participation_rate }
    }

    /// Split order evenly over time
    pub fn split(
        &self,
        order: &Order,
        num_splits: usize,
        duration: Duration,
    ) -> Result<Vec<SplitOrder>> {
        if num_splits == 0 {
            return Ok(vec![]);
        }

        let start_time = Utc::now();
        let time_per_split = duration / num_splits as i32;
        let base_amount = order.amount / Decimal::from(num_splits);
        let mut splits = Vec::new();
        let mut allocated = Decimal::ZERO;

        for i in 0..num_splits {
            let amount = if i == num_splits - 1 {
                order.amount - allocated
            } else {
                base_amount
            };

            allocated += amount;

            splits.push(SplitOrder {
                amount,
                execution_time: start_time + time_per_split * i as i32,
                max_slippage: Some(Decimal::from_f64_retain(0.005).expect("0.005 is a valid f64 for Decimal")), // 0.5% max slippage
                description: format!("Time-weighted split {}/{}", i + 1, num_splits),
            });
        }

        Ok(splits)
    }
}

/// Adaptive order splitter based on market conditions
#[derive(Debug, Clone)]
pub struct AdaptiveSplitter {
    volatility: Decimal,
    liquidity_score: Decimal,
}

impl AdaptiveSplitter {
    /// Create a new adaptive splitter using current volatility and liquidity score.
    pub fn new(volatility: Decimal, liquidity_score: Decimal) -> Self {
        Self {
            volatility,
            liquidity_score,
        }
    }

    /// Adaptively split order based on market conditions
    pub fn split(&self, order: &Order, duration: Duration) -> Result<Vec<SplitOrder>> {
        // Determine optimal number of splits based on conditions
        let num_splits = self.calculate_optimal_splits(order.amount);

        let start_time = Utc::now();
        let mut splits = Vec::new();
        let mut allocated = Decimal::ZERO;

        for i in 0..num_splits {
            // Vary timing based on volatility - trade faster in low vol
            let time_factor = if self.volatility > Decimal::from(3) {
                2.0 // Slower in high volatility
            } else {
                1.0
            };

            let base_time_per_split = duration / num_splits as i32;
            let adjusted_time = Duration::milliseconds(
                (base_time_per_split.num_milliseconds() as f64 * time_factor) as i64,
            );

            let execution_time = start_time + adjusted_time * i as i32;

            // Vary size based on liquidity - smaller chunks in low liquidity
            let size_factor = if self.liquidity_score < Decimal::from(5) {
                Decimal::from_f64_retain(0.8).expect("0.8 is a valid f64 for Decimal")
            } else {
                Decimal::ONE
            };

            let base_amount = order.amount / Decimal::from(num_splits);
            let amount = if i == num_splits - 1 {
                order.amount - allocated
            } else {
                base_amount * size_factor
            };

            allocated += amount;

            // Tighter slippage tolerance in high volatility
            let max_slippage = if self.volatility > Decimal::from(3) {
                Decimal::from_f64_retain(0.002).expect("0.002 is a valid f64 for Decimal") // 0.2%
            } else {
                Decimal::from_f64_retain(0.01).expect("0.01 is a valid f64 for Decimal") // 1%
            };

            splits.push(SplitOrder {
                amount,
                execution_time,
                max_slippage: Some(max_slippage),
                description: format!("Adaptive split {}/{}", i + 1, num_splits),
            });
        }

        Ok(splits)
    }

    fn calculate_optimal_splits(&self, amount: Decimal) -> usize {
        // More splits for larger orders and lower liquidity
        let base_splits = if amount > Decimal::from(1000) {
            20
        } else if amount > Decimal::from(100) {
            10
        } else {
            5
        };

        // Adjust for liquidity
        let liquidity_adjustment = if self.liquidity_score < Decimal::from(3) {
            1.5
        } else if self.liquidity_score > Decimal::from(7) {
            0.7
        } else {
            1.0
        };

        ((base_splits as f64 * liquidity_adjustment).ceil() as usize).max(1)
    }
}

/// Order splitting scheduler
#[derive(Debug, Clone)]
pub struct OrderSplitScheduler {
    splits: Vec<SplitOrder>,
    executed_splits: Vec<usize>,
}

impl OrderSplitScheduler {
    /// Create a new scheduler from a list of pre-computed split orders.
    pub fn new(splits: Vec<SplitOrder>) -> Self {
        Self {
            splits,
            executed_splits: Vec::new(),
        }
    }

    /// Get splits that should be executed now or before
    pub fn get_due_splits(&mut self, current_time: DateTime<Utc>) -> Vec<&SplitOrder> {
        self.splits
            .iter()
            .enumerate()
            .filter(|(idx, split)| {
                !self.executed_splits.contains(idx) && split.execution_time <= current_time
            })
            .map(|(_, split)| split)
            .collect()
    }

    /// Mark a split as executed
    pub fn mark_executed(&mut self, index: usize) {
        if !self.executed_splits.contains(&index) {
            self.executed_splits.push(index);
        }
    }

    /// Get execution progress
    pub fn progress(&self) -> (usize, usize) {
        (self.executed_splits.len(), self.splits.len())
    }

    /// Check if all splits are executed
    pub fn is_complete(&self) -> bool {
        self.executed_splits.len() == self.splits.len()
    }

    /// Get next scheduled split
    pub fn next_split(&self) -> Option<&SplitOrder> {
        self.splits
            .iter()
            .enumerate()
            .find(|(idx, _)| !self.executed_splits.contains(idx))
            .map(|(_, split)| split)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OrderType;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn create_test_order() -> Order {
        Order {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            order_type: OrderType::Buy,
            amount: dec!(1000),
            price_btc: dec!(10),
            total_btc: dec!(10000),
            status: crate::models::OrderStatus::Pending,
            btc_address: None,
            btc_txid: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    #[test]
    fn test_volume_weighted_splitter() {
        let volume_profile = vec![
            (Utc::now(), dec!(100)),
            (Utc::now() + Duration::hours(1), dec!(200)),
            (Utc::now() + Duration::hours(2), dec!(150)),
        ];

        let splitter = VolumeWeightedSplitter::new(volume_profile);
        let order = create_test_order();
        let splits = splitter.split(&order, 3, Duration::hours(3)).unwrap();

        assert_eq!(splits.len(), 3);
        let total: Decimal = splits.iter().map(|s| s.amount).sum();
        assert_eq!(total, order.amount);
    }

    #[test]
    fn test_time_weighted_splitter() {
        let splitter = TimeWeightedSplitter::new(dec!(0.1));
        let order = create_test_order();
        let splits = splitter.split(&order, 5, Duration::hours(5)).unwrap();

        assert_eq!(splits.len(), 5);
        let total: Decimal = splits.iter().map(|s| s.amount).sum();
        assert_eq!(total, order.amount);

        // Each split should have max slippage set
        for split in &splits {
            assert!(split.max_slippage.is_some());
        }
    }

    #[test]
    fn test_adaptive_splitter() {
        let splitter = AdaptiveSplitter::new(dec!(2.5), dec!(6.0));
        let order = create_test_order();
        let splits = splitter.split(&order, Duration::hours(2)).unwrap();

        assert!(!splits.is_empty());
        let total: Decimal = splits.iter().map(|s| s.amount).sum();
        assert!(total <= order.amount);
    }

    #[test]
    fn test_order_split_scheduler() {
        let splits = vec![
            SplitOrder {
                amount: dec!(100),
                execution_time: Utc::now() - Duration::minutes(5),
                max_slippage: None,
                description: "Split 1".to_string(),
            },
            SplitOrder {
                amount: dec!(100),
                execution_time: Utc::now() + Duration::minutes(5),
                max_slippage: None,
                description: "Split 2".to_string(),
            },
        ];

        let mut scheduler = OrderSplitScheduler::new(splits);
        let due = scheduler.get_due_splits(Utc::now());

        assert_eq!(due.len(), 1);
        assert!(!scheduler.is_complete());

        scheduler.mark_executed(0);
        let (done, total) = scheduler.progress();
        assert_eq!(done, 1);
        assert_eq!(total, 2);
    }

    #[test]
    fn test_single_split() {
        let splitter = TimeWeightedSplitter::new(dec!(0.1));
        let order = create_test_order();
        let splits = splitter.split(&order, 1, Duration::hours(1)).unwrap();

        assert_eq!(splits.len(), 1);
        assert_eq!(splits[0].amount, order.amount);
    }

    #[test]
    fn test_zero_splits() {
        let splitter = TimeWeightedSplitter::new(dec!(0.1));
        let order = create_test_order();
        let splits = splitter.split(&order, 0, Duration::hours(1)).unwrap();

        assert_eq!(splits.len(), 0);
    }
}
