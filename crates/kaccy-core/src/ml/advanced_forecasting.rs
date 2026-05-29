//! Advanced Price Forecasting Module
//!
//! This module provides advanced forecasting capabilities including:
//! - Multi-horizon price prediction
//! - Ensemble forecasting models
//! - Probabilistic forecasting
//! - Forecast combination techniques

use crate::CoreError;
use crate::ml::features::PricePoint;
use chrono::{DateTime, Utc};
use rand::rng;
use rand_distr::{Distribution, Normal};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Multi-horizon forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHorizonForecast {
    /// Horizons (in periods)
    pub horizons: Vec<usize>,
    /// Predicted prices for each horizon
    pub predictions: Vec<Decimal>,
    /// Confidence intervals (lower, upper) for each horizon
    pub confidence_intervals: Vec<(Decimal, Decimal)>,
    /// Forecast timestamp
    pub timestamp: DateTime<Utc>,
}

/// Multi-horizon forecaster
#[derive(Debug, Clone)]
pub struct MultiHorizonForecaster {
    /// Historical data buffer
    data: VecDeque<PricePoint>,
    /// Maximum buffer size
    max_size: usize,
    /// Confidence level (e.g., 0.95 for 95%)
    confidence_level: f64,
}

impl MultiHorizonForecaster {
    /// Create a new multi-horizon forecaster
    pub fn new(max_size: usize, confidence_level: f64) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
            confidence_level,
        }
    }

    /// Add a data point
    pub fn add_data(&mut self, point: PricePoint) {
        self.data.push_back(point);
        if self.data.len() > self.max_size {
            self.data.pop_front();
        }
    }

    /// Forecast multiple horizons
    pub fn forecast(&self, horizons: &[usize]) -> anyhow::Result<MultiHorizonForecast> {
        if self.data.len() < 10 {
            return Err(CoreError::Validation("Insufficient data".to_string()).into());
        }

        let prices: Vec<f64> = self
            .data
            .iter()
            .map(|p| p.close.to_f64().unwrap_or(0.0))
            .collect();

        // Simple exponential smoothing with trend
        let alpha = 0.3;
        let beta = 0.1;

        let mut level = prices[0];
        let mut trend = 0.0;

        for &price in &prices[1..] {
            let prev_level = level;
            level = alpha * price + (1.0 - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (1.0 - beta) * trend;
        }

        // Calculate residuals for confidence intervals
        let mut residuals = Vec::new();
        let mut simulated_level = prices[0];
        let mut simulated_trend = 0.0;

        for &price in &prices[1..] {
            let forecast = simulated_level + simulated_trend;
            residuals.push(price - forecast);

            let prev_level = simulated_level;
            simulated_level = alpha * price + (1.0 - alpha) * (simulated_level + simulated_trend);
            simulated_trend =
                beta * (simulated_level - prev_level) + (1.0 - beta) * simulated_trend;
        }

        let residual_std = if residuals.len() > 1 {
            let mean_residual = residuals.iter().sum::<f64>() / residuals.len() as f64;
            let variance = residuals
                .iter()
                .map(|r| (r - mean_residual).powi(2))
                .sum::<f64>()
                / residuals.len() as f64;
            variance.sqrt()
        } else {
            0.01
        };

        // Z-score for confidence level
        let z_score = match self.confidence_level {
            x if x >= 0.99 => 2.576,
            x if x >= 0.95 => 1.96,
            x if x >= 0.90 => 1.645,
            _ => 1.96,
        };

        // Generate forecasts for each horizon
        let mut predictions = Vec::new();
        let mut confidence_intervals = Vec::new();

        for &horizon in horizons {
            let forecast = level + trend * horizon as f64;
            let prediction = Decimal::from_f64_retain(forecast).unwrap_or(Decimal::ZERO);

            // Wider intervals for longer horizons
            let interval_width = residual_std * z_score * (1.0 + horizon as f64 * 0.1);
            let lower =
                Decimal::from_f64_retain(forecast - interval_width).unwrap_or(Decimal::ZERO);
            let upper =
                Decimal::from_f64_retain(forecast + interval_width).unwrap_or(Decimal::ZERO);

            predictions.push(prediction);
            confidence_intervals.push((lower.max(Decimal::ZERO), upper));
        }

        Ok(MultiHorizonForecast {
            horizons: horizons.to_vec(),
            predictions,
            confidence_intervals,
            timestamp: Utc::now(),
        })
    }
}

/// Probabilistic forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilisticForecast {
    /// Forecast timestamp
    pub timestamp: DateTime<Utc>,
    /// Horizon (periods ahead)
    pub horizon: usize,
    /// Mean prediction
    pub mean: Decimal,
    /// Standard deviation
    pub std_dev: f64,
    /// Percentile predictions (10th, 25th, 50th, 75th, 90th)
    pub percentiles: Vec<Decimal>,
}

/// Probabilistic forecaster
#[derive(Debug, Clone)]
pub struct ProbabilisticForecaster {
    /// Historical data
    data: VecDeque<PricePoint>,
    /// Maximum buffer size
    max_size: usize,
    /// Number of simulation paths
    num_simulations: usize,
}

impl ProbabilisticForecaster {
    /// Create a new probabilistic forecaster
    pub fn new(max_size: usize, num_simulations: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
            num_simulations,
        }
    }

    /// Add a data point
    pub fn add_data(&mut self, point: PricePoint) {
        self.data.push_back(point);
        if self.data.len() > self.max_size {
            self.data.pop_front();
        }
    }

    /// Generate probabilistic forecast
    pub fn forecast(&self, horizon: usize) -> anyhow::Result<ProbabilisticForecast> {
        if self.data.len() < 10 {
            return Err(CoreError::Validation("Insufficient data".to_string()).into());
        }

        // Calculate returns
        let data_vec: Vec<_> = self.data.iter().collect();
        let returns: Vec<f64> = data_vec
            .windows(2)
            .map(|w| {
                let prev = w[0].close.to_f64().unwrap_or(0.0);
                let curr = w[1].close.to_f64().unwrap_or(0.0);
                if prev > 0.0 {
                    (curr - prev) / prev
                } else {
                    0.0
                }
            })
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let return_variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;
        let return_std = return_variance.sqrt();

        // Last price
        let last_price = self
            .data
            .back()
            .expect("data has elements (returns is non-empty so data has elements)")
            .close
            .to_f64()
            .unwrap_or(100.0);

        // Monte Carlo simulation
        let mut simulated_prices = Vec::new();
        let mut rng = rng();
        let normal_dist = Normal::new(0.0, 1.0).map_err(|e| {
            CoreError::Validation(format!("Failed to create normal distribution: {}", e))
        })?;

        for _ in 0..self.num_simulations {
            let mut price = last_price;
            for _ in 0..horizon {
                // Simulate return using normal distribution
                let z = normal_dist.sample(&mut rng);
                let random_return = mean_return + return_std * z;
                price *= 1.0 + random_return;
            }
            simulated_prices.push(price);
        }

        simulated_prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate statistics
        let mean = simulated_prices.iter().sum::<f64>() / simulated_prices.len() as f64;
        let variance = simulated_prices
            .iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>()
            / simulated_prices.len() as f64;
        let std_dev = variance.sqrt();

        // Extract percentiles
        let p10_idx = (simulated_prices.len() as f64 * 0.10) as usize;
        let p25_idx = (simulated_prices.len() as f64 * 0.25) as usize;
        let p50_idx = (simulated_prices.len() as f64 * 0.50) as usize;
        let p75_idx = (simulated_prices.len() as f64 * 0.75) as usize;
        let p90_idx = (simulated_prices.len() as f64 * 0.90) as usize;

        let percentiles = vec![
            Decimal::from_f64_retain(simulated_prices[p10_idx]).unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(simulated_prices[p25_idx]).unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(simulated_prices[p50_idx]).unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(simulated_prices[p75_idx]).unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(simulated_prices[p90_idx]).unwrap_or(Decimal::ZERO),
        ];

        Ok(ProbabilisticForecast {
            timestamp: Utc::now(),
            horizon,
            mean: Decimal::from_f64_retain(mean).unwrap_or(Decimal::ZERO),
            std_dev,
            percentiles,
        })
    }
}

/// Forecast combination weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinationWeights {
    /// Model names
    pub models: Vec<String>,
    /// Weights (sum to 1.0)
    pub weights: Vec<f64>,
}

/// Combined forecast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedForecast {
    /// Combined prediction
    pub prediction: Decimal,
    /// Individual model predictions
    pub individual_predictions: Vec<(String, Decimal)>,
    /// Combination method used
    pub method: String,
}

/// Forecast combiner
#[derive(Debug, Clone)]
pub struct ForecastCombiner;

impl ForecastCombiner {
    /// Simple average combination
    pub fn simple_average(forecasts: &[(String, Decimal)]) -> anyhow::Result<CombinedForecast> {
        if forecasts.is_empty() {
            return Err(CoreError::Validation("No forecasts provided".to_string()).into());
        }

        let sum: f64 = forecasts
            .iter()
            .map(|(_, pred)| pred.to_f64().unwrap_or(0.0))
            .sum();
        let avg = sum / forecasts.len() as f64;

        Ok(CombinedForecast {
            prediction: Decimal::from_f64_retain(avg).unwrap_or(Decimal::ZERO),
            individual_predictions: forecasts.to_vec(),
            method: "Simple Average".to_string(),
        })
    }

    /// Weighted average combination
    pub fn weighted_average(
        forecasts: &[(String, Decimal)],
        weights: &CombinationWeights,
    ) -> anyhow::Result<CombinedForecast> {
        if forecasts.len() != weights.weights.len() {
            return Err(CoreError::Validation("Mismatched weights".to_string()).into());
        }

        let weighted_sum: f64 = forecasts
            .iter()
            .zip(weights.weights.iter())
            .map(|((_, pred), &weight)| pred.to_f64().unwrap_or(0.0) * weight)
            .sum();

        Ok(CombinedForecast {
            prediction: Decimal::from_f64_retain(weighted_sum).unwrap_or(Decimal::ZERO),
            individual_predictions: forecasts.to_vec(),
            method: "Weighted Average".to_string(),
        })
    }

    /// Median combination (robust to outliers)
    pub fn median(forecasts: &[(String, Decimal)]) -> anyhow::Result<CombinedForecast> {
        if forecasts.is_empty() {
            return Err(CoreError::Validation("No forecasts provided".to_string()).into());
        }

        let mut values: Vec<f64> = forecasts
            .iter()
            .map(|(_, pred)| pred.to_f64().unwrap_or(0.0))
            .collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if values.len() % 2 == 0 {
            (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.0
        } else {
            values[values.len() / 2]
        };

        Ok(CombinedForecast {
            prediction: Decimal::from_f64_retain(median).unwrap_or(Decimal::ZERO),
            individual_predictions: forecasts.to_vec(),
            method: "Median".to_string(),
        })
    }

    /// Trimmed mean combination (removes outliers)
    pub fn trimmed_mean(
        forecasts: &[(String, Decimal)],
        trim_percent: f64,
    ) -> anyhow::Result<CombinedForecast> {
        if forecasts.is_empty() {
            return Err(CoreError::Validation("No forecasts provided".to_string()).into());
        }

        let mut indexed_values: Vec<(usize, f64)> = forecasts
            .iter()
            .enumerate()
            .map(|(i, (_, pred))| (i, pred.to_f64().unwrap_or(0.0)))
            .collect();
        indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let trim_count = (forecasts.len() as f64 * trim_percent).round() as usize;
        let trimmed = if trim_count * 2 < indexed_values.len() {
            &indexed_values[trim_count..indexed_values.len() - trim_count]
        } else {
            &indexed_values
        };

        let sum: f64 = trimmed.iter().map(|(_, val)| val).sum();
        let avg = sum / trimmed.len() as f64;

        Ok(CombinedForecast {
            prediction: Decimal::from_f64_retain(avg).unwrap_or(Decimal::ZERO),
            individual_predictions: forecasts.to_vec(),
            method: format!("Trimmed Mean ({}%)", trim_percent * 100.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    fn create_test_data() -> Vec<PricePoint> {
        let mut data = Vec::new();
        let mut timestamp = Utc::now();

        for i in 0..50 {
            data.push(PricePoint {
                timestamp,
                open: dec!(100) + Decimal::from(i),
                high: dec!(105) + Decimal::from(i),
                low: dec!(95) + Decimal::from(i),
                close: dec!(100) + Decimal::from(i),
                volume: dec!(1000),
            });
            timestamp += Duration::hours(1);
        }

        data
    }

    #[test]
    fn test_multi_horizon_forecaster() {
        let data = create_test_data();
        let mut forecaster = MultiHorizonForecaster::new(100, 0.95);

        for point in data {
            forecaster.add_data(point);
        }

        let horizons = vec![1, 5, 10];
        let forecast = forecaster.forecast(&horizons).unwrap();

        assert_eq!(forecast.horizons, horizons);
        assert_eq!(forecast.predictions.len(), horizons.len());
        assert_eq!(forecast.confidence_intervals.len(), horizons.len());

        // Check that predictions increase for longer horizons (given our test data)
        assert!(forecast.predictions[1] >= forecast.predictions[0]);

        // Check that confidence intervals widen for longer horizons
        let width_1 = (forecast.confidence_intervals[0].1 - forecast.confidence_intervals[0].0)
            .to_f64()
            .unwrap_or(0.0);
        let width_10 = (forecast.confidence_intervals[2].1 - forecast.confidence_intervals[2].0)
            .to_f64()
            .unwrap_or(0.0);
        assert!(width_10 > width_1);
    }

    #[test]
    fn test_probabilistic_forecaster() {
        let data = create_test_data();
        let mut forecaster = ProbabilisticForecaster::new(100, 1000);

        for point in data {
            forecaster.add_data(point);
        }

        let forecast = forecaster.forecast(5).unwrap();

        assert_eq!(forecast.horizon, 5);
        assert!(forecast.mean > Decimal::ZERO);
        assert!(forecast.std_dev > 0.0);
        assert_eq!(forecast.percentiles.len(), 5);

        // Check percentiles are ordered
        for i in 1..forecast.percentiles.len() {
            assert!(forecast.percentiles[i] >= forecast.percentiles[i - 1]);
        }
    }

    #[test]
    fn test_forecast_combiner_simple_average() {
        let forecasts = vec![
            ("Model1".to_string(), dec!(100)),
            ("Model2".to_string(), dec!(110)),
            ("Model3".to_string(), dec!(105)),
        ];

        let combined = ForecastCombiner::simple_average(&forecasts).unwrap();
        assert_eq!(combined.prediction, dec!(105));
        assert_eq!(combined.method, "Simple Average");
    }

    #[test]
    fn test_forecast_combiner_weighted_average() {
        let forecasts = vec![
            ("Model1".to_string(), dec!(100)),
            ("Model2".to_string(), dec!(110)),
            ("Model3".to_string(), dec!(105)),
        ];

        let weights = CombinationWeights {
            models: vec![
                "Model1".to_string(),
                "Model2".to_string(),
                "Model3".to_string(),
            ],
            weights: vec![0.5, 0.3, 0.2],
        };

        let combined = ForecastCombiner::weighted_average(&forecasts, &weights).unwrap();
        // 100*0.5 + 110*0.3 + 105*0.2 = 50 + 33 + 21 = 104
        assert_eq!(combined.prediction, dec!(104));
    }

    #[test]
    fn test_forecast_combiner_median() {
        let forecasts = vec![
            ("Model1".to_string(), dec!(100)),
            ("Model2".to_string(), dec!(110)),
            ("Model3".to_string(), dec!(105)),
            ("Model4".to_string(), dec!(200)), // Outlier
        ];

        let combined = ForecastCombiner::median(&forecasts).unwrap();
        // Median of [100, 105, 110, 200] = (105 + 110) / 2 = 107.5
        assert_eq!(combined.prediction, dec!(107.5));
        assert_eq!(combined.method, "Median");
    }

    #[test]
    fn test_forecast_combiner_trimmed_mean() {
        let forecasts = vec![
            ("Model1".to_string(), dec!(100)),
            ("Model2".to_string(), dec!(105)),
            ("Model3".to_string(), dec!(110)),
            ("Model4".to_string(), dec!(200)), // Outlier
        ];

        let combined = ForecastCombiner::trimmed_mean(&forecasts, 0.25).unwrap();
        // Trim 1 from each end: [105, 110], average = 107.5
        assert_eq!(combined.prediction, dec!(107.5));
    }
}
