//! ARIMA and SARIMA time-series forecasting models
//!
//! This module provides Auto-ARIMA parameter selection, seasonal decomposition,
//! and forecast confidence intervals for time-series data.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// ARIMA model parameters (p, d, q)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ArimaParams {
    /// Autoregressive order (p)
    pub p: usize,
    /// Differencing order (d)
    pub d: usize,
    /// Moving average order (q)
    pub q: usize,
}

impl ArimaParams {
    /// Create new ARIMA parameters
    pub fn new(p: usize, d: usize, q: usize) -> Self {
        Self { p, d, q }
    }

    /// Validate parameters
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.p > 10 || self.d > 2 || self.q > 10 {
            return Err(CoreError::Validation(
                "ARIMA parameters out of reasonable range (p,q <= 10, d <= 2)".to_string(),
            ));
        }
        Ok(())
    }
}

/// SARIMA model parameters (p, d, q) x (P, D, Q, s)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SarimaParams {
    /// Non-seasonal parameters
    pub arima: ArimaParams,
    /// Seasonal autoregressive order (P)
    #[allow(dead_code)]
    pub seasonal_p: usize,
    /// Seasonal differencing order (D)
    #[allow(dead_code)]
    pub seasonal_d: usize,
    /// Seasonal moving average order (Q)
    #[allow(dead_code)]
    pub seasonal_q: usize,
    /// Seasonal period (s)
    pub seasonal_period: usize,
}

impl SarimaParams {
    /// Create new SARIMA parameters
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        p: usize,
        d: usize,
        q: usize,
        seasonal_p: usize,
        seasonal_d: usize,
        seasonal_q: usize,
        seasonal_period: usize,
    ) -> Self {
        Self {
            arima: ArimaParams::new(p, d, q),
            seasonal_p,
            seasonal_d,
            seasonal_q,
            seasonal_period,
        }
    }

    /// Validate parameters
    pub fn validate(&self) -> Result<(), CoreError> {
        self.arima.validate()?;
        if self.seasonal_period == 0 {
            return Err(CoreError::Validation(
                "Seasonal period must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Time-series observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesObservation {
    /// Timestamp of this observation.
    pub timestamp: DateTime<Utc>,
    /// Observed value at this timestamp.
    pub value: Decimal,
}

/// Seasonal decomposition components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalDecomposition {
    /// Original time series
    pub original: Vec<Decimal>,
    /// Trend component
    pub trend: Vec<Decimal>,
    /// Seasonal component
    pub seasonal: Vec<Decimal>,
    /// Residual component
    pub residual: Vec<Decimal>,
}

impl SeasonalDecomposition {
    /// Perform additive seasonal decomposition
    /// Model: Y = T + S + R (trend + seasonal + residual)
    pub fn decompose_additive(data: &[Decimal], period: usize) -> Result<Self, CoreError> {
        if data.len() < period * 2 {
            return Err(CoreError::Validation(
                "Insufficient data for seasonal decomposition".to_string(),
            ));
        }

        let n = data.len();

        // Calculate trend using moving average
        let mut trend = vec![dec!(0); n];
        let window_size = period;

        for (i, trend_val) in trend.iter_mut().enumerate() {
            let start = i.saturating_sub(window_size / 2);
            let end = (i + window_size / 2 + 1).min(n);
            let count = end - start;

            let sum: Decimal = data[start..end].iter().sum();
            *trend_val = sum / Decimal::from(count);
        }

        // Calculate seasonal component
        let mut seasonal = vec![dec!(0); n];
        let mut seasonal_averages = vec![dec!(0); period];

        // Calculate average for each seasonal period
        for (s, avg) in seasonal_averages.iter_mut().enumerate() {
            let mut sum = dec!(0);
            let mut count = 0;

            for i in (s..n).step_by(period) {
                sum += data[i] - trend[i];
                count += 1;
            }

            *avg = if count > 0 {
                sum / Decimal::from(count)
            } else {
                dec!(0)
            };
        }

        // Normalize seasonal component to sum to zero
        let seasonal_sum: Decimal = seasonal_averages.iter().sum();
        let seasonal_adj = seasonal_sum / Decimal::from(period);
        for s in seasonal_averages.iter_mut() {
            *s -= seasonal_adj;
        }

        // Assign seasonal components
        for i in 0..n {
            seasonal[i] = seasonal_averages[i % period];
        }

        // Calculate residuals
        let mut residual = vec![dec!(0); n];
        for i in 0..n {
            residual[i] = data[i] - trend[i] - seasonal[i];
        }

        Ok(Self {
            original: data.to_vec(),
            trend,
            seasonal,
            residual,
        })
    }

    /// Reconstruct the time series from components
    pub fn reconstruct(&self, index: usize) -> Decimal {
        if index >= self.original.len() {
            return dec!(0);
        }
        self.trend[index] + self.seasonal[index] + self.residual[index]
    }
}

/// Forecast result with confidence intervals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    /// Forecasted values
    pub values: Vec<Decimal>,
    /// Lower bound of confidence interval
    pub lower_bound: Vec<Decimal>,
    /// Upper bound of confidence interval
    pub upper_bound: Vec<Decimal>,
    /// Confidence level (e.g., 0.95 for 95%)
    pub confidence_level: Decimal,
}

impl Forecast {
    /// Create a new forecast
    pub fn new(values: Vec<Decimal>, confidence_level: Decimal) -> Self {
        let n = values.len();
        Self {
            values: values.clone(),
            lower_bound: vec![dec!(0); n],
            upper_bound: vec![dec!(0); n],
            confidence_level,
        }
    }

    /// Set confidence intervals based on standard error
    pub fn with_confidence_intervals(mut self, std_errors: Vec<Decimal>, z_score: Decimal) -> Self {
        for (i, (&value, &std_err)) in self.values.iter().zip(std_errors.iter()).enumerate() {
            let margin = std_err * z_score;
            self.lower_bound[i] = value - margin;
            self.upper_bound[i] = value + margin;
        }
        self
    }
}

/// Auto-ARIMA model selector
#[derive(Debug, Clone)]
pub struct AutoArima {
    /// Maximum p order to test
    max_p: usize,
    /// Maximum d order to test
    max_d: usize,
    /// Maximum q order to test
    max_q: usize,
    /// Use stepwise search (faster but may not find global optimum)
    stepwise: bool,
}

impl AutoArima {
    /// Create a new Auto-ARIMA selector
    pub fn new() -> Self {
        Self {
            max_p: 5,
            max_d: 2,
            max_q: 5,
            stepwise: true,
        }
    }

    /// Set maximum orders
    pub fn with_max_orders(mut self, max_p: usize, max_d: usize, max_q: usize) -> Self {
        self.max_p = max_p;
        self.max_d = max_d;
        self.max_q = max_q;
        self
    }

    /// Select best ARIMA parameters using AIC (Akaike Information Criterion)
    pub fn select_parameters(&self, data: &[Decimal]) -> Result<ArimaParams, CoreError> {
        if data.len() < 10 {
            return Err(CoreError::Validation(
                "Insufficient data for auto-ARIMA".to_string(),
            ));
        }

        let mut best_aic = Decimal::MAX;
        let mut best_params = ArimaParams::new(1, 0, 1);

        if self.stepwise {
            // Stepwise search: start with common models
            let candidate_models = vec![(0, 0, 0), (1, 0, 0), (0, 0, 1), (1, 0, 1), (2, 1, 2)];

            for (p, d, q) in candidate_models {
                if p <= self.max_p && d <= self.max_d && q <= self.max_q {
                    let params = ArimaParams::new(p, d, q);
                    if let Ok(aic) = self.calculate_aic(data, &params) {
                        if aic < best_aic {
                            best_aic = aic;
                            best_params = params;
                        }
                    }
                }
            }
        } else {
            // Grid search: try all combinations
            for p in 0..=self.max_p {
                for d in 0..=self.max_d {
                    for q in 0..=self.max_q {
                        let params = ArimaParams::new(p, d, q);
                        if let Ok(aic) = self.calculate_aic(data, &params) {
                            if aic < best_aic {
                                best_aic = aic;
                                best_params = params;
                            }
                        }
                    }
                }
            }
        }

        Ok(best_params)
    }

    /// Calculate AIC for given parameters (simplified)
    fn calculate_aic(&self, data: &[Decimal], params: &ArimaParams) -> Result<Decimal, CoreError> {
        let n = data.len();
        let k = params.p + params.q + 1; // Number of parameters

        // Apply differencing
        let mut diff_data = data.to_vec();
        for _ in 0..params.d {
            diff_data = Self::difference(&diff_data);
        }

        if diff_data.is_empty() {
            return Err(CoreError::Validation(
                "Empty data after differencing".to_string(),
            ));
        }

        // Calculate residual sum of squares (simplified)
        let mean: Decimal = diff_data.iter().sum::<Decimal>() / Decimal::from(diff_data.len());
        let rss: Decimal = diff_data.iter().map(|x| (*x - mean) * (*x - mean)).sum();

        let variance = rss / Decimal::from(n);

        // AIC = 2k + n * ln(variance)
        // Using simplified approximation since we don't have ln function
        let aic = Decimal::from(2 * k) + variance * Decimal::from(n);

        Ok(aic)
    }

    /// Apply differencing to data
    fn difference(data: &[Decimal]) -> Vec<Decimal> {
        if data.len() <= 1 {
            return Vec::new();
        }
        data.windows(2).map(|w| w[1] - w[0]).collect()
    }
}

impl Default for AutoArima {
    fn default() -> Self {
        Self::new()
    }
}

/// ARIMA model
#[derive(Debug, Clone)]
pub struct ArimaModel {
    params: ArimaParams,
    fitted: bool,
}

impl ArimaModel {
    /// Create a new ARIMA model
    pub fn new(params: ArimaParams) -> Result<Self, CoreError> {
        params.validate()?;
        Ok(Self {
            params,
            fitted: false,
        })
    }

    /// Fit the model to data
    pub fn fit(&mut self, _data: &[Decimal]) -> Result<(), CoreError> {
        // In a full implementation, this would estimate AR and MA coefficients
        // For now, we just mark as fitted
        self.fitted = true;
        Ok(())
    }

    /// Generate forecast
    pub fn forecast(&self, data: &[Decimal], steps: usize) -> Result<Forecast, CoreError> {
        if !self.fitted {
            return Err(CoreError::Validation("Model not fitted".to_string()));
        }

        if data.is_empty() {
            return Err(CoreError::Validation("No data provided".to_string()));
        }

        // Simplified forecasting: use naive persistence with trend
        let mut forecasts = Vec::with_capacity(steps);
        let n = data.len();

        // Estimate simple trend
        let trend = if n >= 2 {
            (data[n - 1] - data[n.saturating_sub(10)]) / Decimal::from(10.min(n - 1))
        } else {
            dec!(0)
        };

        let last_value = data[n - 1];
        for i in 1..=steps {
            forecasts.push(last_value + trend * Decimal::from(i));
        }

        // Estimate standard error (simplified)
        let mean: Decimal = data.iter().sum::<Decimal>() / Decimal::from(n);
        let variance: Decimal = data
            .iter()
            .map(|x| (*x - mean) * (*x - mean))
            .sum::<Decimal>()
            / Decimal::from(n);
        let std_error = variance.sqrt().unwrap_or(dec!(1));

        let std_errors = vec![std_error * Decimal::from(steps).sqrt().unwrap_or(dec!(1)); steps];

        // 95% confidence interval (z-score = 1.96)
        let forecast =
            Forecast::new(forecasts, dec!(0.95)).with_confidence_intervals(std_errors, dec!(1.96));

        Ok(forecast)
    }

    /// Get model parameters
    pub fn params(&self) -> &ArimaParams {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arima_params_validation() {
        let params = ArimaParams::new(1, 1, 1);
        assert!(params.validate().is_ok());

        let invalid = ArimaParams::new(20, 5, 20);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_sarima_params_validation() {
        let params = SarimaParams::new(1, 1, 1, 1, 1, 1, 12);
        assert!(params.validate().is_ok());

        let invalid = SarimaParams::new(1, 1, 1, 1, 1, 1, 0);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_seasonal_decomposition() {
        // Simple periodic data
        let data: Vec<Decimal> = (0..24)
            .map(|i| {
                dec!(100) + Decimal::from(i % 4) * dec!(10) // Period of 4
            })
            .collect();

        let decomp = SeasonalDecomposition::decompose_additive(&data, 4).unwrap();
        assert_eq!(decomp.original.len(), data.len());
        assert_eq!(decomp.trend.len(), data.len());
        assert_eq!(decomp.seasonal.len(), data.len());
        assert_eq!(decomp.residual.len(), data.len());
    }

    #[test]
    fn test_auto_arima_selection() {
        let data: Vec<Decimal> = (0..50).map(|i| Decimal::from(i) + dec!(100)).collect();

        let auto = AutoArima::new();
        let params = auto.select_parameters(&data).unwrap();

        assert!(params.p <= 5);
        assert!(params.d <= 2);
        assert!(params.q <= 5);
    }

    #[test]
    fn test_arima_model() {
        let data: Vec<Decimal> = (0..30).map(|i| Decimal::from(i) + dec!(100)).collect();

        let params = ArimaParams::new(1, 1, 1);
        let mut model = ArimaModel::new(params).unwrap();

        assert!(model.fit(&data).is_ok());

        let forecast = model.forecast(&data, 5).unwrap();
        assert_eq!(forecast.values.len(), 5);
        assert_eq!(forecast.lower_bound.len(), 5);
        assert_eq!(forecast.upper_bound.len(), 5);
    }

    #[test]
    fn test_forecast_confidence_intervals() {
        let values = vec![dec!(100), dec!(105), dec!(110)];
        let std_errors = vec![dec!(2), dec!(3), dec!(4)];

        let forecast = Forecast::new(values.clone(), dec!(0.95))
            .with_confidence_intervals(std_errors, dec!(1.96));

        assert_eq!(forecast.values.len(), 3);
        assert!(forecast.lower_bound[0] < forecast.values[0]);
        assert!(forecast.upper_bound[0] > forecast.values[0]);
    }

    #[test]
    fn test_differencing() {
        let data = vec![dec!(100), dec!(105), dec!(110), dec!(115)];
        let diff = AutoArima::difference(&data);

        assert_eq!(diff.len(), 3);
        assert_eq!(diff[0], dec!(5));
        assert_eq!(diff[1], dec!(5));
        assert_eq!(diff[2], dec!(5));
    }

    #[test]
    fn test_insufficient_data_error() {
        let data: Vec<Decimal> = vec![dec!(100)];

        let auto = AutoArima::new();
        assert!(auto.select_parameters(&data).is_err());
    }
}
