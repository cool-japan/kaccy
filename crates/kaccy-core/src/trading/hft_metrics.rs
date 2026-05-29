//! High-Frequency Trading Metrics
//!
//! Provides metrics and analysis tools for high-frequency trading strategies.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Order event for HFT analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEvent {
    /// Identifier of the order this event belongs to
    pub order_id: Uuid,
    /// Type of lifecycle event
    pub event_type: OrderEventType,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// Price associated with this event (if applicable)
    pub price: Option<Decimal>,
    /// Quantity associated with this event (if applicable)
    pub quantity: Option<Decimal>,
}

/// Type of order lifecycle event
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrderEventType {
    /// Order was submitted to the exchange
    Submitted,
    /// Order was fully filled
    Filled,
    /// Order was partially filled
    PartiallyFilled,
    /// Order was cancelled
    Cancelled,
    /// Order was rejected by the exchange
    Rejected,
}

/// HFT performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HftMetrics {
    /// Fill ratio (fills / submissions)
    pub fill_ratio: f64,
    /// Quote-to-trade ratio
    pub quote_to_trade_ratio: f64,
    /// Cancellation rate
    pub cancellation_rate: f64,
    /// Average order lifespan (milliseconds)
    pub avg_order_lifespan_ms: f64,
    /// Order velocity (orders per second)
    pub order_velocity: f64,
}

/// HFT metrics analyzer
#[derive(Debug)]
pub struct HftAnalyzer {
    /// Sliding window of recent order events
    events: Vec<OrderEvent>,
    /// Maximum number of events to retain
    window_size: usize,
}

impl HftAnalyzer {
    /// Create a new HFT analyzer with the given event window size
    pub fn new(window_size: usize) -> Self {
        Self {
            events: Vec::new(),
            window_size,
        }
    }

    /// Add an order event to the analyzer's window
    pub fn add_event(&mut self, event: OrderEvent) {
        self.events.push(event);

        // Keep only recent events
        if self.events.len() > self.window_size {
            self.events.drain(0..self.events.len() - self.window_size);
        }
    }

    /// Calculate fill ratio
    pub fn fill_ratio(&self) -> f64 {
        let submissions = self.count_event_type(OrderEventType::Submitted);
        let fills = self.count_event_type(OrderEventType::Filled)
            + self.count_event_type(OrderEventType::PartiallyFilled);

        if submissions > 0 {
            fills as f64 / submissions as f64
        } else {
            0.0
        }
    }

    /// Calculate quote-to-trade ratio
    pub fn quote_to_trade_ratio(&self) -> f64 {
        let quotes = self.count_event_type(OrderEventType::Submitted);
        let trades = self.count_event_type(OrderEventType::Filled);

        if trades > 0 {
            quotes as f64 / trades as f64
        } else {
            0.0
        }
    }

    /// Calculate cancellation rate
    pub fn cancellation_rate(&self) -> f64 {
        let submissions = self.count_event_type(OrderEventType::Submitted);
        let cancellations = self.count_event_type(OrderEventType::Cancelled);

        if submissions > 0 {
            cancellations as f64 / submissions as f64
        } else {
            0.0
        }
    }

    /// Calculate average order lifespan
    pub fn average_order_lifespan_ms(&self) -> f64 {
        let mut lifespans = Vec::new();

        // Group events by order_id
        let mut order_events: std::collections::HashMap<Uuid, Vec<&OrderEvent>> =
            std::collections::HashMap::new();

        for event in &self.events {
            order_events.entry(event.order_id).or_default().push(event);
        }

        for (_order_id, events) in order_events {
            if events.len() < 2 {
                continue;
            }

            // Find submission and terminal event
            let submission = events
                .iter()
                .find(|e| e.event_type == OrderEventType::Submitted);
            let terminal = events.iter().find(|e| {
                matches!(
                    e.event_type,
                    OrderEventType::Filled | OrderEventType::Cancelled
                )
            });

            if let (Some(sub), Some(term)) = (submission, terminal) {
                let lifespan = (term.timestamp - sub.timestamp).num_milliseconds();
                lifespans.push(lifespan as f64);
            }
        }

        if !lifespans.is_empty() {
            lifespans.iter().sum::<f64>() / lifespans.len() as f64
        } else {
            0.0
        }
    }

    /// Calculate order velocity (orders per second)
    pub fn order_velocity(&self) -> f64 {
        if self.events.len() < 2 {
            return 0.0;
        }

        let first_time = self
            .events
            .first()
            .expect("events has >= 2 elements per len check above")
            .timestamp;
        let last_time = self
            .events
            .last()
            .expect("events has >= 2 elements per len check above")
            .timestamp;
        let duration_secs = (last_time - first_time).num_seconds() as f64;

        if duration_secs > 0.0 {
            self.events.len() as f64 / duration_secs
        } else {
            0.0
        }
    }

    /// Get comprehensive metrics
    pub fn get_metrics(&self) -> HftMetrics {
        HftMetrics {
            fill_ratio: self.fill_ratio(),
            quote_to_trade_ratio: self.quote_to_trade_ratio(),
            cancellation_rate: self.cancellation_rate(),
            avg_order_lifespan_ms: self.average_order_lifespan_ms(),
            order_velocity: self.order_velocity(),
        }
    }

    fn count_event_type(&self, event_type: OrderEventType) -> usize {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }
}

/// Latency tracker
#[derive(Debug)]
pub struct LatencyTracker {
    /// Sliding window of latency measurements in microseconds
    samples: VecDeque<f64>,
    /// Maximum number of samples to retain
    max_samples: usize,
}

impl LatencyTracker {
    /// Create a new latency tracker with the given sample window
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Record a latency sample in microseconds
    pub fn record(&mut self, latency_us: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_us);
    }

    /// Return the mean latency of all samples
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    /// Return the `p`th percentile latency (p is 0–100)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let idx = ((sorted.len() as f64) * p / 100.0) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Return the maximum latency sample
    pub fn max(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    /// Return the minimum latency sample
    pub fn min(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }
}

/// Adverse selection detector
#[derive(Debug)]
pub struct AdverseSelectionDetector {
    /// Historical trade records used to compute adverse selection cost
    trades: Vec<TradeRecord>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TradeRecord {
    price: Decimal,
    side: TradeSide,
    timestamp: DateTime<Utc>,
    midpoint_before: Decimal,
    midpoint_after: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum TradeSide {
    Buy,
    Sell,
}

impl AdverseSelectionDetector {
    /// Create a new adverse selection detector
    pub fn new() -> Self {
        Self { trades: Vec::new() }
    }

    /// Calculate adverse selection cost
    pub fn adverse_selection_cost(&self) -> f64 {
        if self.trades.is_empty() {
            return 0.0;
        }

        let mut total_cost = 0.0;

        for trade in &self.trades {
            let price_move = match trade.side {
                TradeSide::Buy => trade.midpoint_after - trade.midpoint_before,
                TradeSide::Sell => trade.midpoint_before - trade.midpoint_after,
            };

            let cost_bps = if trade.midpoint_before > Decimal::ZERO {
                ((price_move / trade.midpoint_before) * Decimal::from(10000))
                    .to_string()
                    .parse()
                    .unwrap_or(0.0)
            } else {
                0.0
            };

            total_cost += cost_bps;
        }

        total_cost / self.trades.len() as f64
    }
}

impl Default for AdverseSelectionDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Inventory risk calculator
#[derive(Debug)]
pub struct InventoryRiskCalculator {
    /// Target inventory level
    pub target_inventory: Decimal,
    /// Maximum inventory deviation
    pub max_deviation: Decimal,
}

impl InventoryRiskCalculator {
    /// Create a new inventory risk calculator
    pub fn new(target_inventory: Decimal, max_deviation: Decimal) -> Self {
        Self {
            target_inventory,
            max_deviation,
        }
    }

    /// Calculate inventory risk score (0-1)
    pub fn risk_score(&self, current_inventory: Decimal) -> f64 {
        let deviation = (current_inventory - self.target_inventory).abs();

        if self.max_deviation == Decimal::ZERO {
            return 0.0;
        }

        let risk: f64 = (deviation / self.max_deviation)
            .to_string()
            .parse()
            .unwrap_or(0.0);

        risk.min(1.0)
    }

    /// Check if inventory is within acceptable range
    pub fn is_acceptable(&self, current_inventory: Decimal) -> bool {
        let deviation = (current_inventory - self.target_inventory).abs();
        deviation <= self.max_deviation
    }

    /// Suggest rebalance amount
    pub fn suggest_rebalance(&self, current_inventory: Decimal) -> Decimal {
        if self.is_acceptable(current_inventory) {
            Decimal::ZERO
        } else {
            self.target_inventory - current_inventory
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(event_type: OrderEventType) -> OrderEvent {
        OrderEvent {
            order_id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            price: Some(Decimal::from(100)),
            quantity: Some(Decimal::from(10)),
        }
    }

    #[test]
    fn test_fill_ratio() {
        let mut analyzer = HftAnalyzer::new(1000);

        // Add 10 submissions, 7 fills
        for _ in 0..10 {
            analyzer.add_event(create_test_event(OrderEventType::Submitted));
        }
        for _ in 0..7 {
            analyzer.add_event(create_test_event(OrderEventType::Filled));
        }

        let fill_ratio = analyzer.fill_ratio();
        assert!((fill_ratio - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_cancellation_rate() {
        let mut analyzer = HftAnalyzer::new(1000);

        // Add 10 submissions, 3 cancellations
        for _ in 0..10 {
            analyzer.add_event(create_test_event(OrderEventType::Submitted));
        }
        for _ in 0..3 {
            analyzer.add_event(create_test_event(OrderEventType::Cancelled));
        }

        let cancel_rate = analyzer.cancellation_rate();
        assert!((cancel_rate - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_latency_tracker() {
        let mut tracker = LatencyTracker::new(100);

        tracker.record(100.0);
        tracker.record(200.0);
        tracker.record(300.0);

        assert!((tracker.mean() - 200.0).abs() < 0.01);
        assert_eq!(tracker.max(), 300.0);
        assert_eq!(tracker.min(), 100.0);
    }

    #[test]
    fn test_latency_percentile() {
        let mut tracker = LatencyTracker::new(100);

        for i in 1..=100 {
            tracker.record(i as f64);
        }

        let p50 = tracker.percentile(50.0);
        let p95 = tracker.percentile(95.0);

        assert!((p50 - 50.0).abs() < 5.0);
        assert!((p95 - 95.0).abs() < 5.0);
    }

    #[test]
    fn test_inventory_risk() {
        let calculator = InventoryRiskCalculator::new(Decimal::from(1000), Decimal::from(100));

        // Within range
        assert!(calculator.is_acceptable(Decimal::from(1050)));
        assert_eq!(calculator.risk_score(Decimal::from(1050)), 0.5);

        // Outside range
        assert!(!calculator.is_acceptable(Decimal::from(1200)));
        assert!(calculator.risk_score(Decimal::from(1200)) > 0.9);

        // Rebalance suggestion
        let rebalance = calculator.suggest_rebalance(Decimal::from(1200));
        assert_eq!(rebalance, Decimal::from(-200));
    }
}
