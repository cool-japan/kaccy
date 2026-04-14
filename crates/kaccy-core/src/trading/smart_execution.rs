//! Smart Execution Algorithms
//!
//! This module provides advanced order execution algorithms including TWAP, VWAP,
//! implementation shortfall, and iceberg orders.

use crate::error::{CoreError, Result};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// Time-Weighted Average Price (TWAP) execution strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TWAPStrategy {
    /// Strategy ID
    pub id: Uuid,
    /// Token ID
    pub token_id: i64,
    /// Total quantity to execute
    pub total_quantity: Decimal,
    /// Order side
    pub side: OrderSide,
    /// Number of slices
    pub num_slices: usize,
    /// Interval between slices (in seconds)
    pub interval_seconds: u64,
    /// Participation rate (% of market volume, 0-100)
    pub participation_rate: Option<Decimal>,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
    /// Created at
    pub created_at: DateTime<Utc>,
}

impl TWAPStrategy {
    /// Create a new TWAP strategy
    pub fn new(
        token_id: i64,
        total_quantity: Decimal,
        side: OrderSide,
        duration_seconds: u64,
        num_slices: usize,
    ) -> Result<Self> {
        if num_slices == 0 {
            return Err(CoreError::Validation(
                "Number of slices must be > 0".to_string(),
            ));
        }

        let now = Utc::now();
        let interval_seconds = duration_seconds / num_slices as u64;

        Ok(Self {
            id: Uuid::new_v4(),
            token_id,
            total_quantity,
            side,
            num_slices,
            interval_seconds,
            participation_rate: None,
            start_time: now,
            end_time: now + Duration::seconds(duration_seconds as i64),
            created_at: now,
        })
    }

    /// Create adaptive TWAP with participation rate
    pub fn with_participation_rate(mut self, rate: Decimal) -> Result<Self> {
        if rate <= dec!(0) || rate > dec!(100) {
            return Err(CoreError::Validation(
                "Participation rate must be 0-100".to_string(),
            ));
        }
        self.participation_rate = Some(rate);
        Ok(self)
    }

    /// Calculate quantity per slice
    pub fn quantity_per_slice(&self) -> Decimal {
        self.total_quantity / Decimal::from(self.num_slices)
    }

    /// Get slice schedule (timestamps and quantities)
    pub fn get_schedule(&self) -> Vec<(DateTime<Utc>, Decimal)> {
        let mut schedule = Vec::new();
        let quantity_per_slice = self.quantity_per_slice();

        for i in 0..self.num_slices {
            let slice_time =
                self.start_time + Duration::seconds((i as u64 * self.interval_seconds) as i64);
            schedule.push((slice_time, quantity_per_slice));
        }

        schedule
    }

    /// Adjust quantity based on market volume and participation rate
    pub fn adjust_for_market_volume(
        &self,
        market_volume: Decimal,
        slice_quantity: Decimal,
    ) -> Decimal {
        if let Some(rate) = self.participation_rate {
            let max_quantity = market_volume * (rate / dec!(100));
            slice_quantity.min(max_quantity)
        } else {
            slice_quantity
        }
    }

    /// Check if strategy is active
    pub fn is_active(&self, current_time: DateTime<Utc>) -> bool {
        current_time >= self.start_time && current_time <= self.end_time
    }
}

/// Volume-Weighted Average Price (VWAP) execution strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VWAPStrategy {
    /// Strategy ID
    pub id: Uuid,
    /// Token ID
    pub token_id: i64,
    /// Total quantity to execute
    pub total_quantity: Decimal,
    /// Order side
    pub side: OrderSide,
    /// Historical VWAP periods
    pub vwap_periods: Vec<VWAPPeriod>,
    /// Target completion time
    pub target_time: DateTime<Utc>,
    /// Created at
    pub created_at: DateTime<Utc>,
}

/// VWAP period data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VWAPPeriod {
    /// Period start
    pub start_time: DateTime<Utc>,
    /// Period end
    pub end_time: DateTime<Utc>,
    /// Average price
    pub avg_price: Decimal,
    /// Volume
    pub volume: Decimal,
}

impl VWAPStrategy {
    /// Create a new VWAP strategy
    pub fn new(
        token_id: i64,
        total_quantity: Decimal,
        side: OrderSide,
        target_time: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            token_id,
            total_quantity,
            side,
            vwap_periods: Vec::new(),
            target_time,
            created_at: Utc::now(),
        }
    }

    /// Add historical VWAP period
    pub fn add_period(&mut self, period: VWAPPeriod) {
        self.vwap_periods.push(period);
    }

    /// Calculate historical VWAP
    pub fn calculate_historical_vwap(&self) -> Option<Decimal> {
        if self.vwap_periods.is_empty() {
            return None;
        }

        let total_pv: Decimal = self
            .vwap_periods
            .iter()
            .map(|p| p.avg_price * p.volume)
            .sum();

        let total_volume: Decimal = self.vwap_periods.iter().map(|p| p.volume).sum();

        if total_volume == dec!(0) {
            return None;
        }

        Some(total_pv / total_volume)
    }

    /// Calculate execution schedule based on volume profile
    pub fn volume_weighted_schedule(&self) -> Vec<(DateTime<Utc>, Decimal)> {
        if self.vwap_periods.is_empty() {
            return Vec::new();
        }

        let total_volume: Decimal = self.vwap_periods.iter().map(|p| p.volume).sum();

        if total_volume == dec!(0) {
            return Vec::new();
        }

        self.vwap_periods
            .iter()
            .map(|period| {
                let volume_weight = period.volume / total_volume;
                let quantity = self.total_quantity * volume_weight;
                (period.start_time, quantity)
            })
            .collect()
    }

    /// Calculate VWAP deviation (percentage)
    pub fn vwap_deviation(&self, execution_price: Decimal) -> Option<Decimal> {
        let vwap = self.calculate_historical_vwap()?;
        if vwap == dec!(0) {
            return None;
        }
        Some(((execution_price - vwap) / vwap) * dec!(100))
    }
}

/// Implementation Shortfall strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationShortfall {
    /// Strategy ID
    pub id: Uuid,
    /// Token ID
    pub token_id: i64,
    /// Total quantity
    pub total_quantity: Decimal,
    /// Order side
    pub side: OrderSide,
    /// Arrival price (decision price)
    pub arrival_price: Decimal,
    /// Executions
    pub executions: Vec<Execution>,
    /// Created at
    pub created_at: DateTime<Utc>,
}

/// Individual execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    /// Execution time
    pub timestamp: DateTime<Utc>,
    /// Executed quantity
    pub quantity: Decimal,
    /// Execution price
    pub price: Decimal,
}

impl ImplementationShortfall {
    /// Create a new implementation shortfall tracker
    pub fn new(
        token_id: i64,
        total_quantity: Decimal,
        side: OrderSide,
        arrival_price: Decimal,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            token_id,
            total_quantity,
            side,
            arrival_price,
            executions: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Add an execution
    pub fn add_execution(&mut self, quantity: Decimal, price: Decimal) {
        self.executions.push(Execution {
            timestamp: Utc::now(),
            quantity,
            price,
        });
    }

    /// Calculate total executed quantity
    pub fn total_executed(&self) -> Decimal {
        self.executions.iter().map(|e| e.quantity).sum()
    }

    /// Calculate average execution price
    pub fn average_execution_price(&self) -> Option<Decimal> {
        let total_quantity = self.total_executed();
        if total_quantity == dec!(0) {
            return None;
        }

        let total_cost: Decimal = self.executions.iter().map(|e| e.quantity * e.price).sum();

        Some(total_cost / total_quantity)
    }

    /// Calculate implementation shortfall (in basis points)
    pub fn calculate_shortfall(&self) -> Option<Decimal> {
        let avg_price = self.average_execution_price()?;

        if self.arrival_price == dec!(0) {
            return None;
        }

        let shortfall = match self.side {
            OrderSide::Buy => (avg_price - self.arrival_price) / self.arrival_price,
            OrderSide::Sell => (self.arrival_price - avg_price) / self.arrival_price,
        };

        Some(shortfall * dec!(10000)) // Convert to basis points
    }

    /// Decompose shortfall into components
    pub fn decompose_shortfall(&self, current_price: Decimal) -> ShortfallDecomposition {
        let total_executed = self.total_executed();
        let remaining = self.total_quantity - total_executed;

        // Delay cost (paper loss on unexecuted portion)
        let delay_cost = if remaining > dec!(0) {
            match self.side {
                OrderSide::Buy => (current_price - self.arrival_price) * remaining,
                OrderSide::Sell => (self.arrival_price - current_price) * remaining,
            }
        } else {
            dec!(0)
        };

        // Execution cost (slippage on executed portion)
        let execution_cost = if let Some(avg_price) = self.average_execution_price() {
            match self.side {
                OrderSide::Buy => (avg_price - self.arrival_price) * total_executed,
                OrderSide::Sell => (self.arrival_price - avg_price) * total_executed,
            }
        } else {
            dec!(0)
        };

        ShortfallDecomposition {
            delay_cost,
            execution_cost,
            total_cost: delay_cost + execution_cost,
        }
    }
}

/// Shortfall decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortfallDecomposition {
    /// Cost from delaying execution
    pub delay_cost: Decimal,
    /// Cost from execution slippage
    pub execution_cost: Decimal,
    /// Total implementation shortfall
    pub total_cost: Decimal,
}

/// Iceberg order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcebergOrder {
    /// Order ID
    pub id: Uuid,
    /// Token ID
    pub token_id: i64,
    /// Total quantity (hidden)
    pub total_quantity: Decimal,
    /// Visible quantity per display
    pub visible_quantity: Decimal,
    /// Order side
    pub side: OrderSide,
    /// Limit price
    pub limit_price: Decimal,
    /// Quantity filled so far
    pub filled_quantity: Decimal,
    /// Current visible slice
    pub current_slice: Decimal,
    /// Replenishment strategy
    pub replenishment_strategy: ReplenishmentStrategy,
    /// Created at
    pub created_at: DateTime<Utc>,
}

/// Replenishment strategy for iceberg orders
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ReplenishmentStrategy {
    /// Immediate replenishment after fill
    Immediate,
    /// Random delay (min, max seconds)
    RandomDelay {
        /// Minimum delay in seconds before replenishment.
        min_seconds: u64,
        /// Maximum delay in seconds before replenishment.
        max_seconds: u64,
    },
    /// Volume-based (wait for certain volume)
    VolumeBased {
        /// Cumulative volume threshold that triggers replenishment.
        threshold_volume: u64,
    },
}

impl IcebergOrder {
    /// Create a new iceberg order
    pub fn new(
        token_id: i64,
        total_quantity: Decimal,
        visible_quantity: Decimal,
        side: OrderSide,
        limit_price: Decimal,
        replenishment_strategy: ReplenishmentStrategy,
    ) -> Result<Self> {
        if visible_quantity > total_quantity {
            return Err(CoreError::Validation(
                "Visible quantity cannot exceed total quantity".to_string(),
            ));
        }

        if visible_quantity <= dec!(0) {
            return Err(CoreError::Validation(
                "Visible quantity must be positive".to_string(),
            ));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            token_id,
            total_quantity,
            visible_quantity,
            side,
            limit_price,
            filled_quantity: dec!(0),
            current_slice: visible_quantity,
            replenishment_strategy,
            created_at: Utc::now(),
        })
    }

    /// Get remaining quantity
    pub fn remaining_quantity(&self) -> Decimal {
        self.total_quantity - self.filled_quantity
    }

    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.filled_quantity >= self.total_quantity
    }

    /// Fill current visible slice
    pub fn fill_slice(&mut self, quantity: Decimal) -> Result<()> {
        if quantity > self.current_slice {
            return Err(CoreError::Validation(
                "Fill quantity exceeds visible slice".to_string(),
            ));
        }

        self.filled_quantity += quantity;
        self.current_slice -= quantity;

        // Replenish if current slice is depleted
        if self.current_slice == dec!(0) && !self.is_filled() {
            self.replenish();
        }

        Ok(())
    }

    /// Replenish visible quantity
    fn replenish(&mut self) {
        let remaining = self.remaining_quantity();
        self.current_slice = remaining.min(self.visible_quantity);
    }

    /// Calculate detection risk (lower is better)
    pub fn detection_risk_score(&self) -> Decimal {
        // Risk increases with visibility ratio
        let visibility_ratio = self.visible_quantity / self.total_quantity;

        // Risk decreases with randomization in replenishment
        let randomization_factor = match self.replenishment_strategy {
            ReplenishmentStrategy::Immediate => dec!(1.0),
            ReplenishmentStrategy::RandomDelay { .. } => dec!(0.5),
            ReplenishmentStrategy::VolumeBased { .. } => dec!(0.7),
        };

        visibility_ratio * randomization_factor * dec!(100)
    }
}

/// Smart execution manager
pub struct SmartExecutionManager {
    /// Active TWAP strategies
    twap_strategies: Vec<TWAPStrategy>,
    /// Active VWAP strategies
    vwap_strategies: Vec<VWAPStrategy>,
    /// Implementation shortfall trackers
    shortfall_trackers: Vec<ImplementationShortfall>,
    /// Active iceberg orders
    iceberg_orders: VecDeque<IcebergOrder>,
}

impl SmartExecutionManager {
    /// Create a new smart execution manager
    pub fn new() -> Self {
        Self {
            twap_strategies: Vec::new(),
            vwap_strategies: Vec::new(),
            shortfall_trackers: Vec::new(),
            iceberg_orders: VecDeque::new(),
        }
    }

    /// Add TWAP strategy
    pub fn add_twap(&mut self, strategy: TWAPStrategy) {
        self.twap_strategies.push(strategy);
    }

    /// Add VWAP strategy
    pub fn add_vwap(&mut self, strategy: VWAPStrategy) {
        self.vwap_strategies.push(strategy);
    }

    /// Add implementation shortfall tracker
    pub fn add_shortfall_tracker(&mut self, tracker: ImplementationShortfall) {
        self.shortfall_trackers.push(tracker);
    }

    /// Add iceberg order
    pub fn add_iceberg(&mut self, order: IcebergOrder) {
        self.iceberg_orders.push_back(order);
    }

    /// Get active TWAP strategies
    pub fn get_active_twap(&self) -> Vec<&TWAPStrategy> {
        let now = Utc::now();
        self.twap_strategies
            .iter()
            .filter(|s| s.is_active(now))
            .collect()
    }

    /// Get TWAP strategy by ID
    pub fn get_twap(&self, id: Uuid) -> Option<&TWAPStrategy> {
        self.twap_strategies.iter().find(|s| s.id == id)
    }

    /// Get VWAP strategy by ID
    pub fn get_vwap(&self, id: Uuid) -> Option<&VWAPStrategy> {
        self.vwap_strategies.iter().find(|s| s.id == id)
    }

    /// Get implementation shortfall tracker by ID
    pub fn get_shortfall_tracker(&self, id: Uuid) -> Option<&ImplementationShortfall> {
        self.shortfall_trackers.iter().find(|s| s.id == id)
    }

    /// Get mutable implementation shortfall tracker
    pub fn get_shortfall_tracker_mut(&mut self, id: Uuid) -> Option<&mut ImplementationShortfall> {
        self.shortfall_trackers.iter_mut().find(|s| s.id == id)
    }

    /// Get iceberg order by ID
    pub fn get_iceberg(&self, id: Uuid) -> Option<&IcebergOrder> {
        self.iceberg_orders.iter().find(|o| o.id == id)
    }

    /// Get mutable iceberg order
    pub fn get_iceberg_mut(&mut self, id: Uuid) -> Option<&mut IcebergOrder> {
        self.iceberg_orders.iter_mut().find(|o| o.id == id)
    }

    /// Remove completed iceberg orders
    pub fn cleanup_filled_icebergs(&mut self) {
        self.iceberg_orders.retain(|o| !o.is_filled());
    }
}

impl Default for SmartExecutionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twap_strategy_creation() {
        let strategy = TWAPStrategy::new(
            1,
            dec!(1000),
            OrderSide::Buy,
            3600, // 1 hour
            10,   // 10 slices
        )
        .unwrap();

        assert_eq!(strategy.token_id, 1);
        assert_eq!(strategy.total_quantity, dec!(1000));
        assert_eq!(strategy.num_slices, 10);
        assert_eq!(strategy.quantity_per_slice(), dec!(100));
        assert_eq!(strategy.interval_seconds, 360);
    }

    #[test]
    fn test_twap_schedule() {
        let strategy = TWAPStrategy::new(1, dec!(1000), OrderSide::Buy, 1000, 5).unwrap();

        let schedule = strategy.get_schedule();
        assert_eq!(schedule.len(), 5);

        for (_, quantity) in &schedule {
            assert_eq!(*quantity, dec!(200));
        }
    }

    #[test]
    fn test_twap_participation_rate() {
        let strategy = TWAPStrategy::new(1, dec!(1000), OrderSide::Buy, 3600, 10)
            .unwrap()
            .with_participation_rate(dec!(20))
            .unwrap();

        assert_eq!(strategy.participation_rate, Some(dec!(20)));

        // With market volume of 1000, max execution is 20% = 200
        let adjusted = strategy.adjust_for_market_volume(dec!(1000), dec!(300));
        assert_eq!(adjusted, dec!(200));
    }

    #[test]
    fn test_vwap_calculation() {
        let mut strategy = VWAPStrategy::new(
            1,
            dec!(1000),
            OrderSide::Buy,
            Utc::now() + Duration::hours(1),
        );

        strategy.add_period(VWAPPeriod {
            start_time: Utc::now(),
            end_time: Utc::now() + Duration::minutes(15),
            avg_price: dec!(100),
            volume: dec!(500),
        });

        strategy.add_period(VWAPPeriod {
            start_time: Utc::now() + Duration::minutes(15),
            end_time: Utc::now() + Duration::minutes(30),
            avg_price: dec!(110),
            volume: dec!(300),
        });

        // VWAP = (100*500 + 110*300) / (500 + 300) = 83000 / 800 = 103.75
        let vwap = strategy.calculate_historical_vwap().unwrap();
        assert_eq!(vwap, dec!(103.75));
    }

    #[test]
    fn test_implementation_shortfall() {
        let mut tracker = ImplementationShortfall::new(1, dec!(1000), OrderSide::Buy, dec!(100));

        tracker.add_execution(dec!(500), dec!(101));
        tracker.add_execution(dec!(500), dec!(103));

        assert_eq!(tracker.total_executed(), dec!(1000));

        // Average price = (500*101 + 500*103) / 1000 = 102
        let avg_price = tracker.average_execution_price().unwrap();
        assert_eq!(avg_price, dec!(102));

        // Shortfall = (102 - 100) / 100 * 10000 = 200 bps
        let shortfall = tracker.calculate_shortfall().unwrap();
        assert_eq!(shortfall, dec!(200));
    }

    #[test]
    fn test_iceberg_order() {
        let mut order = IcebergOrder::new(
            1,
            dec!(1000),
            dec!(100),
            OrderSide::Buy,
            dec!(50),
            ReplenishmentStrategy::Immediate,
        )
        .unwrap();

        assert_eq!(order.remaining_quantity(), dec!(1000));
        assert_eq!(order.current_slice, dec!(100));

        // Fill the visible slice
        order.fill_slice(dec!(100)).unwrap();

        assert_eq!(order.filled_quantity, dec!(100));
        assert_eq!(order.remaining_quantity(), dec!(900));
        // Should auto-replenish
        assert_eq!(order.current_slice, dec!(100));
    }

    #[test]
    fn test_iceberg_partial_fill() {
        let mut order = IcebergOrder::new(
            1,
            dec!(1000),
            dec!(100),
            OrderSide::Buy,
            dec!(50),
            ReplenishmentStrategy::Immediate,
        )
        .unwrap();

        // Partial fill
        order.fill_slice(dec!(30)).unwrap();

        assert_eq!(order.filled_quantity, dec!(30));
        assert_eq!(order.current_slice, dec!(70));
    }

    #[test]
    fn test_smart_execution_manager() {
        let mut manager = SmartExecutionManager::new();

        let twap = TWAPStrategy::new(1, dec!(1000), OrderSide::Buy, 3600, 10).unwrap();

        let twap_id = twap.id;
        manager.add_twap(twap);

        assert!(manager.get_twap(twap_id).is_some());
    }

    #[test]
    fn test_shortfall_decomposition() {
        let mut tracker = ImplementationShortfall::new(1, dec!(1000), OrderSide::Buy, dec!(100));

        tracker.add_execution(dec!(600), dec!(102));

        // Current price is 105, remaining 400
        let decomposition = tracker.decompose_shortfall(dec!(105));

        // Execution cost = (102 - 100) * 600 = 1200
        assert_eq!(decomposition.execution_cost, dec!(1200));

        // Delay cost = (105 - 100) * 400 = 2000
        assert_eq!(decomposition.delay_cost, dec!(2000));

        // Total = 3200
        assert_eq!(decomposition.total_cost, dec!(3200));
    }
}
