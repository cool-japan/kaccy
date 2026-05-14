//! Order Flow Prediction Module
//!
//! This module provides predictive models for order flow including:
//! - Next order prediction (buy/sell direction)
//! - Order size distribution analysis
//! - Inter-arrival time modeling
//! - Order cancellation prediction

use crate::CoreError;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Order direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderDirection {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// Historical order event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEvent {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Direction
    pub direction: OrderDirection,
    /// Size
    pub size: Decimal,
    /// Price
    pub price: Decimal,
    /// Was cancelled (if applicable)
    pub cancelled: bool,
}

/// Next order prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPrediction {
    /// Predicted direction
    pub direction: OrderDirection,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Predicted size range (min, max)
    pub size_range: (Decimal, Decimal),
    /// Predicted time until next order
    pub time_until_next: Duration,
}

/// Order flow predictor
#[derive(Debug, Clone)]
pub struct OrderFlowPredictor {
    /// Historical events for training
    history: VecDeque<OrderEvent>,
    /// Maximum history size
    max_history: usize,
    /// Buy/sell ratio window
    direction_window: usize,
}

impl OrderFlowPredictor {
    /// Create a new order flow predictor
    pub fn new(max_history: usize, direction_window: usize) -> Self {
        Self {
            history: VecDeque::new(),
            max_history,
            direction_window,
        }
    }

    /// Add an order event to history
    pub fn add_event(&mut self, event: OrderEvent) {
        self.history.push_back(event);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }

    /// Predict next order characteristics
    pub fn predict_next(&self) -> anyhow::Result<OrderPrediction> {
        if self.history.len() < self.direction_window {
            return Err(CoreError::Validation("Insufficient history".to_string()).into());
        }

        // Analyze recent direction bias
        let recent = self
            .history
            .iter()
            .rev()
            .take(self.direction_window)
            .collect::<Vec<_>>();

        let buy_count = recent
            .iter()
            .filter(|e| e.direction == OrderDirection::Buy)
            .count();
        let sell_count = recent.len() - buy_count;

        // Predict direction (mean reversion assumption)
        let (direction, confidence) = if buy_count > sell_count {
            let imbalance = (buy_count as f64 - sell_count as f64) / recent.len() as f64;
            (OrderDirection::Sell, 0.5 + (imbalance * 0.3))
        } else if sell_count > buy_count {
            let imbalance = (sell_count as f64 - buy_count as f64) / recent.len() as f64;
            (OrderDirection::Buy, 0.5 + (imbalance * 0.3))
        } else {
            // Equal, use momentum from last order
            let last_dir = self.history.back().expect("history is non-empty (recent is non-empty which is a slice of history)").direction;
            (last_dir, 0.5)
        };

        // Predict size based on recent average and std dev
        let sizes: Vec<f64> = recent
            .iter()
            .map(|e| e.size.to_f64().unwrap_or(0.0))
            .collect();
        let avg_size = sizes.iter().sum::<f64>() / sizes.len() as f64;
        let variance =
            sizes.iter().map(|s| (s - avg_size).powi(2)).sum::<f64>() / sizes.len() as f64;
        let std_dev = variance.sqrt();

        let size_min =
            Decimal::from_f64_retain((avg_size - std_dev).max(0.0)).unwrap_or(Decimal::ZERO);
        let size_max = Decimal::from_f64_retain(avg_size + std_dev).unwrap_or(Decimal::ZERO);

        // Predict inter-arrival time
        let time_until_next = self.predict_inter_arrival_time()?;

        Ok(OrderPrediction {
            direction,
            confidence,
            size_range: (size_min, size_max),
            time_until_next,
        })
    }

    /// Predict inter-arrival time
    fn predict_inter_arrival_time(&self) -> anyhow::Result<Duration> {
        if self.history.len() < 2 {
            return Ok(Duration::seconds(60));
        }

        // Calculate recent inter-arrival times
        let mut intervals: Vec<i64> = Vec::new();
        for i in 1..self.history.len().min(20) {
            let prev = &self.history[self.history.len() - i - 1];
            let curr = &self.history[self.history.len() - i];
            let interval = (curr.timestamp - prev.timestamp).num_seconds();
            intervals.push(interval);
        }

        // Use exponential moving average for prediction
        let mut ema = intervals[0] as f64;
        let alpha = 0.3;
        for &interval in &intervals[1..] {
            ema = alpha * interval as f64 + (1.0 - alpha) * ema;
        }

        Ok(Duration::seconds(ema as i64))
    }
}

/// Order size distribution analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSizeDistribution {
    /// Mean size
    pub mean: Decimal,
    /// Median size
    pub median: Decimal,
    /// Standard deviation
    pub std_dev: f64,
    /// Percentiles (p10, p25, p75, p90)
    pub percentiles: (Decimal, Decimal, Decimal, Decimal),
    /// Total orders analyzed
    pub sample_size: usize,
}

/// Order size analyzer
#[derive(Debug, Clone)]
pub struct OrderSizeAnalyzer;

impl OrderSizeAnalyzer {
    /// Analyze order size distribution
    pub fn analyze(events: &[OrderEvent]) -> anyhow::Result<OrderSizeDistribution> {
        if events.is_empty() {
            return Err(CoreError::Validation("No events provided".to_string()).into());
        }

        let mut sizes: Vec<f64> = events
            .iter()
            .map(|e| e.size.to_f64().unwrap_or(0.0))
            .collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mean_f64 = sizes.iter().sum::<f64>() / sizes.len() as f64;
        let mean = Decimal::from_f64_retain(mean_f64).unwrap_or(Decimal::ZERO);

        let median_idx = sizes.len() / 2;
        let median = Decimal::from_f64_retain(sizes[median_idx]).unwrap_or(Decimal::ZERO);

        let variance =
            sizes.iter().map(|s| (s - mean_f64).powi(2)).sum::<f64>() / sizes.len() as f64;
        let std_dev = variance.sqrt();

        let p10_idx = (sizes.len() as f64 * 0.10) as usize;
        let p25_idx = (sizes.len() as f64 * 0.25) as usize;
        let p75_idx = (sizes.len() as f64 * 0.75) as usize;
        let p90_idx = (sizes.len() as f64 * 0.90) as usize;

        let p10 = Decimal::from_f64_retain(sizes[p10_idx]).unwrap_or(Decimal::ZERO);
        let p25 = Decimal::from_f64_retain(sizes[p25_idx]).unwrap_or(Decimal::ZERO);
        let p75 = Decimal::from_f64_retain(sizes[p75_idx]).unwrap_or(Decimal::ZERO);
        let p90 = Decimal::from_f64_retain(sizes[p90_idx]).unwrap_or(Decimal::ZERO);

        Ok(OrderSizeDistribution {
            mean,
            median,
            std_dev,
            percentiles: (p10, p25, p75, p90),
            sample_size: sizes.len(),
        })
    }
}

/// Inter-arrival time model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterArrivalTimeModel {
    /// Mean inter-arrival time (seconds)
    pub mean_seconds: f64,
    /// Exponential decay parameter (lambda)
    pub lambda: f64,
    /// Recent average (last 20 events)
    pub recent_avg_seconds: f64,
}

/// Inter-arrival time analyzer
#[derive(Debug, Clone)]
pub struct InterArrivalTimeAnalyzer;

impl InterArrivalTimeAnalyzer {
    /// Build inter-arrival time model
    pub fn build_model(events: &[OrderEvent]) -> anyhow::Result<InterArrivalTimeModel> {
        if events.len() < 2 {
            return Err(CoreError::Validation("Need at least 2 events".to_string()).into());
        }

        // Calculate all inter-arrival times
        let mut intervals: Vec<f64> = Vec::new();
        for i in 1..events.len() {
            let interval = (events[i].timestamp - events[i - 1].timestamp).num_seconds() as f64;
            if interval > 0.0 {
                intervals.push(interval);
            }
        }

        if intervals.is_empty() {
            return Err(CoreError::Validation("No valid intervals".to_string()).into());
        }

        let mean_seconds = intervals.iter().sum::<f64>() / intervals.len() as f64;

        // Exponential distribution parameter
        let lambda = 1.0 / mean_seconds;

        // Recent average (last 20 intervals)
        let recent_avg_seconds = if intervals.len() > 20 {
            intervals[intervals.len() - 20..].iter().sum::<f64>() / 20.0
        } else {
            mean_seconds
        };

        Ok(InterArrivalTimeModel {
            mean_seconds,
            lambda,
            recent_avg_seconds,
        })
    }

    /// Predict probability that next order arrives within given seconds
    pub fn predict_probability(model: &InterArrivalTimeModel, within_seconds: f64) -> f64 {
        // Exponential distribution CDF: P(T <= t) = 1 - e^(-lambda * t)
        1.0 - (-model.lambda * within_seconds).exp()
    }
}

/// Order cancellation prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationPrediction {
    /// Probability of cancellation (0.0 to 1.0)
    pub probability: f64,
    /// Factors contributing to prediction
    pub factors: Vec<String>,
}

/// Order cancellation predictor
#[derive(Debug, Clone)]
pub struct CancellationPredictor {
    /// Historical cancellation rate
    cancellation_rate: f64,
    /// Size threshold for increased cancellation
    large_order_threshold: Decimal,
}

impl CancellationPredictor {
    /// Create a new cancellation predictor
    pub fn new(events: &[OrderEvent]) -> Self {
        let cancelled_count = events.iter().filter(|e| e.cancelled).count();
        let cancellation_rate = if events.is_empty() {
            0.1
        } else {
            cancelled_count as f64 / events.len() as f64
        };

        // Calculate 90th percentile as large order threshold
        let mut sizes: Vec<Decimal> = events.iter().map(|e| e.size).collect();
        sizes.sort();
        let large_order_threshold = if sizes.len() > 10 {
            let idx = (sizes.len() as f64 * 0.90) as usize;
            sizes[idx]
        } else {
            Decimal::MAX
        };

        Self {
            cancellation_rate,
            large_order_threshold,
        }
    }

    /// Predict cancellation probability for a new order
    pub fn predict(&self, size: Decimal, time_in_force_seconds: i64) -> CancellationPrediction {
        let mut probability = self.cancellation_rate;
        let mut factors = Vec::new();

        // Large orders are more likely to be cancelled
        if size > self.large_order_threshold {
            probability += 0.2;
            factors.push("Large order size".to_string());
        }

        // Longer time in force increases cancellation probability
        if time_in_force_seconds > 3600 {
            probability += 0.15;
            factors.push("Long time in force".to_string());
        } else if time_in_force_seconds > 600 {
            probability += 0.05;
            factors.push("Medium time in force".to_string());
        }

        // Cap probability at 0.95
        probability = probability.min(0.95);

        if factors.is_empty() {
            factors.push("Base cancellation rate".to_string());
        }

        CancellationPrediction {
            probability,
            factors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_events() -> Vec<OrderEvent> {
        let mut events = Vec::new();
        let mut timestamp = Utc::now();

        for i in 0..50 {
            events.push(OrderEvent {
                timestamp,
                direction: if i % 3 == 0 {
                    OrderDirection::Buy
                } else {
                    OrderDirection::Sell
                },
                size: dec!(100) + Decimal::from(i * 10),
                price: dec!(1000) + Decimal::from(i),
                cancelled: i % 10 == 0,
            });
            timestamp += Duration::seconds(30 + i % 10);
        }

        events
    }

    #[test]
    fn test_order_flow_predictor() {
        let events = create_test_events();
        let mut predictor = OrderFlowPredictor::new(100, 20);

        for event in events {
            predictor.add_event(event);
        }

        let prediction = predictor.predict_next().unwrap();
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.size_range.0 <= prediction.size_range.1);
        assert!(prediction.time_until_next.num_seconds() > 0);
    }

    #[test]
    fn test_order_size_analyzer() {
        let events = create_test_events();
        let distribution = OrderSizeAnalyzer::analyze(&events).unwrap();

        assert!(distribution.mean > Decimal::ZERO);
        assert!(distribution.median > Decimal::ZERO);
        assert!(distribution.std_dev > 0.0);
        assert_eq!(distribution.sample_size, events.len());
        assert!(distribution.percentiles.0 <= distribution.percentiles.1);
        assert!(distribution.percentiles.2 <= distribution.percentiles.3);
    }

    #[test]
    fn test_inter_arrival_time_analyzer() {
        let events = create_test_events();
        let model = InterArrivalTimeAnalyzer::build_model(&events).unwrap();

        assert!(model.mean_seconds > 0.0);
        assert!(model.lambda > 0.0);
        assert!(model.recent_avg_seconds > 0.0);

        let prob_30s = InterArrivalTimeAnalyzer::predict_probability(&model, 30.0);
        let prob_60s = InterArrivalTimeAnalyzer::predict_probability(&model, 60.0);

        assert!((0.0..=1.0).contains(&prob_30s));
        assert!(prob_60s >= prob_30s); // More time = higher probability
    }

    #[test]
    fn test_cancellation_predictor() {
        let events = create_test_events();
        let predictor = CancellationPredictor::new(&events);

        let prediction_small = predictor.predict(dec!(100), 300);
        let prediction_large = predictor.predict(dec!(10000), 7200);

        assert!(prediction_small.probability >= 0.0 && prediction_small.probability <= 1.0);
        assert!(prediction_large.probability >= 0.0 && prediction_large.probability <= 1.0);
        assert!(prediction_large.probability >= prediction_small.probability);
    }
}
