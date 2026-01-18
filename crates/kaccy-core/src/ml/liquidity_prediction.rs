//! Liquidity Prediction Module
//!
//! This module provides predictive models for market liquidity including:
//! - Intraday liquidity forecasting
//! - Market maker behavior modeling
//! - Spread prediction
//! - Depth prediction

use crate::CoreError;
use crate::ml::features::PricePoint;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Liquidity snapshot at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquiditySnapshot {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Bid-ask spread
    pub spread: Decimal,
    /// Total bid depth (volume)
    pub bid_depth: Decimal,
    /// Total ask depth (volume)
    pub ask_depth: Decimal,
    /// Mid price
    pub mid_price: Decimal,
}

/// Intraday liquidity forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityForecast {
    /// Hour of day (0-23)
    pub hour: u32,
    /// Predicted spread (as decimal, e.g., 0.01 = 1%)
    pub predicted_spread: Decimal,
    /// Predicted total depth
    pub predicted_depth: Decimal,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
}

/// Intraday liquidity forecaster
#[derive(Debug, Clone)]
pub struct IntradayLiquidityForecaster {
    /// Historical hourly patterns
    hourly_patterns: HashMap<u32, HourlyLiquidityStats>,
}

#[derive(Debug, Clone)]
struct HourlyLiquidityStats {
    avg_spread: f64,
    avg_depth: f64,
    sample_count: usize,
}

impl IntradayLiquidityForecaster {
    /// Create a new intraday liquidity forecaster
    pub fn new() -> Self {
        Self {
            hourly_patterns: HashMap::new(),
        }
    }

    /// Train the forecaster with historical liquidity snapshots
    pub fn train(&mut self, snapshots: &[LiquiditySnapshot]) -> anyhow::Result<()> {
        if snapshots.is_empty() {
            return Err(CoreError::Validation("No snapshots provided".to_string()).into());
        }

        // Group by hour of day
        let mut hourly_data: HashMap<u32, Vec<(f64, f64)>> = HashMap::new();

        for snapshot in snapshots {
            let hour = snapshot.timestamp.hour();
            let spread = snapshot.spread.to_f64().unwrap_or(0.0);
            let depth = (snapshot.bid_depth + snapshot.ask_depth)
                .to_f64()
                .unwrap_or(0.0);

            hourly_data.entry(hour).or_default().push((spread, depth));
        }

        // Calculate stats for each hour
        for (hour, data) in hourly_data {
            let avg_spread = data.iter().map(|(s, _)| s).sum::<f64>() / data.len() as f64;
            let avg_depth = data.iter().map(|(_, d)| d).sum::<f64>() / data.len() as f64;

            self.hourly_patterns.insert(
                hour,
                HourlyLiquidityStats {
                    avg_spread,
                    avg_depth,
                    sample_count: data.len(),
                },
            );
        }

        Ok(())
    }

    /// Forecast liquidity for a specific hour
    pub fn forecast(&self, hour: u32) -> anyhow::Result<LiquidityForecast> {
        if hour > 23 {
            return Err(CoreError::Validation("Invalid hour".to_string()).into());
        }

        // Get stats for this hour, or interpolate from nearby hours
        let stats = if let Some(stats) = self.hourly_patterns.get(&hour) {
            stats.clone()
        } else {
            // Interpolate from nearby hours
            let prev_hour = if hour == 0 { 23 } else { hour - 1 };
            let next_hour = if hour == 23 { 0 } else { hour + 1 };

            let prev_stats = self.hourly_patterns.get(&prev_hour);
            let next_stats = self.hourly_patterns.get(&next_hour);

            match (prev_stats, next_stats) {
                (Some(p), Some(n)) => HourlyLiquidityStats {
                    avg_spread: (p.avg_spread + n.avg_spread) / 2.0,
                    avg_depth: (p.avg_depth + n.avg_depth) / 2.0,
                    sample_count: (p.sample_count + n.sample_count) / 2,
                },
                (Some(p), None) => p.clone(),
                (None, Some(n)) => n.clone(),
                (None, None) => {
                    return Err(
                        CoreError::Validation("Insufficient training data".to_string()).into(),
                    );
                }
            }
        };

        // Confidence based on sample count
        let confidence = (stats.sample_count as f64 / 100.0).clamp(0.3, 1.0);

        Ok(LiquidityForecast {
            hour,
            predicted_spread: Decimal::from_f64_retain(stats.avg_spread).unwrap_or(dec!(0.01)),
            predicted_depth: Decimal::from_f64_retain(stats.avg_depth).unwrap_or(dec!(1000)),
            confidence,
        })
    }
}

impl Default for IntradayLiquidityForecaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Market maker behavior pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerBehavior {
    /// Average quote update frequency (seconds)
    pub avg_update_frequency: f64,
    /// Typical spread (as decimal)
    pub typical_spread: Decimal,
    /// Quote size consistency (0.0 to 1.0, higher = more consistent)
    pub size_consistency: f64,
    /// Inventory management aggressiveness (0.0 to 1.0)
    pub inventory_aggressiveness: f64,
}

/// Market maker behavior modeler
#[derive(Debug, Clone)]
pub struct MarketMakerModeler {
    /// Window size for analysis
    window_size: usize,
}

impl Default for MarketMakerModeler {
    fn default() -> Self {
        Self { window_size: 100 }
    }
}

impl MarketMakerModeler {
    /// Create a new market maker modeler
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }

    /// Model market maker behavior from quote updates
    pub fn model_behavior(
        &self,
        snapshots: &[LiquiditySnapshot],
    ) -> anyhow::Result<MarketMakerBehavior> {
        if snapshots.len() < 2 {
            return Err(CoreError::Validation("Need at least 2 snapshots".to_string()).into());
        }

        let window = if snapshots.len() > self.window_size {
            &snapshots[snapshots.len() - self.window_size..]
        } else {
            snapshots
        };

        // Calculate average update frequency
        let mut intervals = Vec::new();
        for i in 1..window.len() {
            let interval = (window[i].timestamp - window[i - 1].timestamp).num_seconds() as f64;
            if interval > 0.0 {
                intervals.push(interval);
            }
        }
        let avg_update_frequency = if !intervals.is_empty() {
            intervals.iter().sum::<f64>() / intervals.len() as f64
        } else {
            60.0
        };

        // Calculate typical spread
        let spreads: Vec<f64> = window
            .iter()
            .map(|s| s.spread.to_f64().unwrap_or(0.0))
            .collect();
        let typical_spread_f64 = spreads.iter().sum::<f64>() / spreads.len() as f64;
        let typical_spread = Decimal::from_f64_retain(typical_spread_f64).unwrap_or(dec!(0.01));

        // Calculate size consistency (inverse of coefficient of variation)
        let depths: Vec<f64> = window
            .iter()
            .map(|s| (s.bid_depth + s.ask_depth).to_f64().unwrap_or(0.0))
            .collect();
        let avg_depth = depths.iter().sum::<f64>() / depths.len() as f64;
        let depth_variance =
            depths.iter().map(|d| (d - avg_depth).powi(2)).sum::<f64>() / depths.len() as f64;
        let depth_std = depth_variance.sqrt();
        let cv = if avg_depth > 0.0 {
            depth_std / avg_depth
        } else {
            1.0
        };
        let size_consistency = (1.0 - cv.min(1.0)).max(0.0);

        // Calculate inventory aggressiveness (depth imbalance)
        let imbalances: Vec<f64> = window
            .iter()
            .map(|s| {
                let bid = s.bid_depth.to_f64().unwrap_or(0.0);
                let ask = s.ask_depth.to_f64().unwrap_or(0.0);
                let total = bid + ask;
                if total > 0.0 {
                    ((bid - ask) / total).abs()
                } else {
                    0.0
                }
            })
            .collect();
        let inventory_aggressiveness = imbalances.iter().sum::<f64>() / imbalances.len() as f64;

        Ok(MarketMakerBehavior {
            avg_update_frequency,
            typical_spread,
            size_consistency,
            inventory_aggressiveness,
        })
    }
}

/// Spread prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadPrediction {
    /// Predicted spread (as decimal)
    pub predicted_spread: Decimal,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Contributing factors
    pub factors: Vec<String>,
}

/// Spread predictor
#[derive(Debug, Clone)]
pub struct SpreadPredictor {
    /// Historical average spread
    avg_spread: f64,
    /// Spread volatility
    #[allow(dead_code)]
    spread_volatility: f64,
}

impl SpreadPredictor {
    /// Create and train a spread predictor
    pub fn train(snapshots: &[LiquiditySnapshot]) -> anyhow::Result<Self> {
        if snapshots.is_empty() {
            return Err(CoreError::Validation("No snapshots provided".to_string()).into());
        }

        let spreads: Vec<f64> = snapshots
            .iter()
            .map(|s| s.spread.to_f64().unwrap_or(0.0))
            .collect();

        let avg_spread = spreads.iter().sum::<f64>() / spreads.len() as f64;

        let variance = spreads
            .iter()
            .map(|s| (s - avg_spread).powi(2))
            .sum::<f64>()
            / spreads.len() as f64;
        let spread_volatility = variance.sqrt();

        Ok(Self {
            avg_spread,
            spread_volatility,
        })
    }

    /// Predict spread based on market conditions
    pub fn predict(&self, recent_volatility: f64, recent_volume: Decimal) -> SpreadPrediction {
        let mut predicted_spread = self.avg_spread;
        let mut factors = Vec::new();
        let mut confidence = 0.7_f64;

        // High volatility → wider spreads
        if recent_volatility > 0.05 {
            predicted_spread *= 1.5;
            factors.push("High volatility".to_string());
            confidence -= 0.1;
        } else if recent_volatility > 0.02 {
            predicted_spread *= 1.2;
            factors.push("Moderate volatility".to_string());
        }

        // Low volume → wider spreads
        let volume_f = recent_volume.to_f64().unwrap_or(0.0);
        if volume_f < 100.0 {
            predicted_spread *= 1.3;
            factors.push("Low volume".to_string());
            confidence -= 0.1;
        }

        if factors.is_empty() {
            factors.push("Normal market conditions".to_string());
        }

        SpreadPrediction {
            predicted_spread: Decimal::from_f64_retain(predicted_spread).unwrap_or(dec!(0.01)),
            confidence: confidence.max(0.3),
            factors,
        }
    }
}

/// Depth prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthPrediction {
    /// Predicted bid depth
    pub predicted_bid_depth: Decimal,
    /// Predicted ask depth
    pub predicted_ask_depth: Decimal,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
}

/// Depth predictor
#[derive(Debug, Clone)]
pub struct DepthPredictor {
    /// Average bid depth
    avg_bid_depth: f64,
    /// Average ask depth
    avg_ask_depth: f64,
    /// Depth volatility
    #[allow(dead_code)]
    depth_volatility: f64,
}

impl DepthPredictor {
    /// Create and train a depth predictor
    pub fn train(snapshots: &[LiquiditySnapshot]) -> anyhow::Result<Self> {
        if snapshots.is_empty() {
            return Err(CoreError::Validation("No snapshots provided".to_string()).into());
        }

        let bid_depths: Vec<f64> = snapshots
            .iter()
            .map(|s| s.bid_depth.to_f64().unwrap_or(0.0))
            .collect();
        let ask_depths: Vec<f64> = snapshots
            .iter()
            .map(|s| s.ask_depth.to_f64().unwrap_or(0.0))
            .collect();

        let avg_bid_depth = bid_depths.iter().sum::<f64>() / bid_depths.len() as f64;
        let avg_ask_depth = ask_depths.iter().sum::<f64>() / ask_depths.len() as f64;

        let total_depths: Vec<f64> = bid_depths
            .iter()
            .zip(ask_depths.iter())
            .map(|(b, a)| b + a)
            .collect();
        let avg_total = total_depths.iter().sum::<f64>() / total_depths.len() as f64;

        let variance = total_depths
            .iter()
            .map(|d| (d - avg_total).powi(2))
            .sum::<f64>()
            / total_depths.len() as f64;
        let depth_volatility = variance.sqrt();

        Ok(Self {
            avg_bid_depth,
            avg_ask_depth,
            depth_volatility,
        })
    }

    /// Predict depth based on price data
    pub fn predict(&self, recent_price_data: &[PricePoint]) -> DepthPrediction {
        let mut predicted_bid_depth = self.avg_bid_depth;
        let mut predicted_ask_depth = self.avg_ask_depth;
        let mut confidence = 0.7_f64;

        if !recent_price_data.is_empty() {
            // High recent volume → higher depth
            let recent_volume: f64 = recent_price_data
                .iter()
                .map(|p| p.volume.to_f64().unwrap_or(0.0))
                .sum::<f64>()
                / recent_price_data.len() as f64;

            if recent_volume > 10000.0 {
                predicted_bid_depth *= 1.3;
                predicted_ask_depth *= 1.3;
            } else if recent_volume < 100.0 {
                predicted_bid_depth *= 0.7;
                predicted_ask_depth *= 0.7;
                confidence -= 0.2;
            }
        }

        DepthPrediction {
            predicted_bid_depth: Decimal::from_f64_retain(predicted_bid_depth)
                .unwrap_or(dec!(1000)),
            predicted_ask_depth: Decimal::from_f64_retain(predicted_ask_depth)
                .unwrap_or(dec!(1000)),
            confidence: confidence.max(0.3),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshots() -> Vec<LiquiditySnapshot> {
        let mut snapshots = Vec::new();
        let base_time = Utc::now();

        for i in 0..100 {
            let hour = i % 24;
            let timestamp = base_time + chrono::Duration::hours(i);

            // Simulate higher spreads and lower depth during off-hours
            let (spread, bid_depth, ask_depth) = if (9..=16).contains(&hour) {
                (dec!(0.01), dec!(5000), dec!(5000))
            } else {
                (dec!(0.02), dec!(2000), dec!(2000))
            };

            snapshots.push(LiquiditySnapshot {
                timestamp,
                spread,
                bid_depth,
                ask_depth,
                mid_price: dec!(1000) + Decimal::from(i),
            });
        }

        snapshots
    }

    #[test]
    fn test_intraday_liquidity_forecaster() {
        let snapshots = create_test_snapshots();
        let mut forecaster = IntradayLiquidityForecaster::new();

        forecaster.train(&snapshots).unwrap();

        let forecast_10am = forecaster.forecast(10).unwrap();
        let forecast_midnight = forecaster.forecast(0).unwrap();

        assert!(forecast_10am.confidence >= 0.0 && forecast_10am.confidence <= 1.0);
        assert!(forecast_midnight.predicted_spread >= dec!(0));
        assert!(forecast_10am.predicted_spread >= dec!(0));
        // Both forecasts should have positive depth
        assert!(forecast_10am.predicted_depth > dec!(0));
        assert!(forecast_midnight.predicted_depth > dec!(0));
    }

    #[test]
    fn test_market_maker_modeler() {
        let snapshots = create_test_snapshots();
        let modeler = MarketMakerModeler::default();

        let behavior = modeler.model_behavior(&snapshots).unwrap();

        assert!(behavior.avg_update_frequency > 0.0);
        assert!(behavior.typical_spread >= dec!(0));
        assert!(behavior.size_consistency >= 0.0 && behavior.size_consistency <= 1.0);
        assert!(
            behavior.inventory_aggressiveness >= 0.0 && behavior.inventory_aggressiveness <= 1.0
        );
    }

    #[test]
    fn test_spread_predictor() {
        let snapshots = create_test_snapshots();
        let predictor = SpreadPredictor::train(&snapshots).unwrap();

        let prediction_low_vol = predictor.predict(0.01, dec!(10000));
        let prediction_high_vol = predictor.predict(0.10, dec!(100));

        assert!(prediction_low_vol.predicted_spread > dec!(0));
        assert!(prediction_high_vol.predicted_spread > prediction_low_vol.predicted_spread);
        assert!(!prediction_high_vol.factors.is_empty());
    }

    #[test]
    fn test_depth_predictor() {
        let snapshots = create_test_snapshots();
        let predictor = DepthPredictor::train(&snapshots).unwrap();

        let prediction = predictor.predict(&[]);

        assert!(prediction.predicted_bid_depth > dec!(0));
        assert!(prediction.predicted_ask_depth > dec!(0));
        assert!(prediction.confidence >= 0.0_f64 && prediction.confidence <= 1.0_f64);
    }
}
