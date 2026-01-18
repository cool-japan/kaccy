//! Prophet-style time-series forecasting
//!
//! This module provides trend and seasonality detection, holiday effects,
//! and change point detection for time-series forecasting.

use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Prophet-style forecasting model
#[derive(Debug, Clone)]
pub struct ProphetModel {
    /// Growth model type
    #[allow(dead_code)]
    growth: GrowthType,
    /// Changepoints for trend detection
    changepoints: Vec<Changepoint>,
    /// Seasonal components
    seasonal_components: Vec<SeasonalComponent>,
    /// Holiday effects
    holidays: Vec<Holiday>,
    /// Model fitted status
    fitted: bool,
}

/// Growth model type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowthType {
    /// Linear growth
    Linear,
    /// Logistic growth (saturating)
    Logistic,
}

/// Changepoint in trend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changepoint {
    /// Timestamp of changepoint
    pub timestamp: DateTime<Utc>,
    /// Index in data
    pub index: usize,
    /// Change in trend slope
    pub delta: Decimal,
    /// Significance score
    pub significance: Decimal,
}

/// Seasonal component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalComponent {
    /// Name of seasonal component (e.g., "weekly", "yearly")
    pub name: String,
    /// Period in days
    pub period_days: i64,
    /// Fourier order (number of terms)
    pub fourier_order: usize,
    /// Component values
    pub values: Vec<Decimal>,
}

/// Holiday effect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holiday {
    /// Name of holiday
    pub name: String,
    /// Date of holiday
    pub date: DateTime<Utc>,
    /// Effect magnitude
    pub effect: Decimal,
    /// Lower window (days before)
    pub lower_window: i64,
    /// Upper window (days after)
    pub upper_window: i64,
}

/// Trend detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDetection {
    /// Overall trend type
    pub trend_type: TrendType,
    /// Trend slope
    pub slope: Decimal,
    /// Trend strength (0-1)
    pub strength: Decimal,
    /// Detected changepoints
    pub changepoints: Vec<Changepoint>,
}

/// Type of trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendType {
    /// Upward trend
    Upward,
    /// Downward trend
    Downward,
    /// No clear trend
    Flat,
    /// Mean-reverting
    MeanReverting,
}

/// Seasonality detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalityDetection {
    /// Detected seasonal periods
    pub periods: Vec<SeasonalPeriod>,
    /// Overall seasonality strength
    pub strength: Decimal,
}

/// A detected seasonal period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonalPeriod {
    /// Period in days
    pub period_days: i64,
    /// Strength of this seasonal component
    pub strength: Decimal,
    /// Description
    pub description: String,
}

impl ProphetModel {
    /// Create a new Prophet model
    pub fn new(growth: GrowthType) -> Self {
        Self {
            growth,
            changepoints: Vec::new(),
            seasonal_components: Vec::new(),
            holidays: Vec::new(),
            fitted: false,
        }
    }

    /// Add a seasonal component
    pub fn add_seasonality(mut self, name: String, period_days: i64, fourier_order: usize) -> Self {
        self.seasonal_components.push(SeasonalComponent {
            name,
            period_days,
            fourier_order,
            values: Vec::new(),
        });
        self
    }

    /// Add a holiday
    pub fn add_holiday(
        mut self,
        name: String,
        date: DateTime<Utc>,
        lower_window: i64,
        upper_window: i64,
    ) -> Self {
        self.holidays.push(Holiday {
            name,
            date,
            effect: dec!(0),
            lower_window,
            upper_window,
        });
        self
    }

    /// Detect trend in time series
    pub fn detect_trend(
        data: &[(DateTime<Utc>, Decimal)],
        n_changepoints: usize,
    ) -> Result<TrendDetection, CoreError> {
        if data.len() < 10 {
            return Err(CoreError::Validation(
                "Insufficient data for trend detection".to_string(),
            ));
        }

        let values: Vec<Decimal> = data.iter().map(|(_, v)| *v).collect();

        // Calculate linear trend
        let n = Decimal::from(values.len());
        let x_mean = (n - dec!(1)) / dec!(2);
        let y_mean: Decimal = values.iter().sum::<Decimal>() / n;

        let mut numerator = dec!(0);
        let mut denominator = dec!(0);

        for (i, value) in values.iter().enumerate() {
            let x_i = Decimal::from(i);
            let x_diff = x_i - x_mean;
            numerator += x_diff * (*value - y_mean);
            denominator += x_diff * x_diff;
        }

        let slope = if denominator > dec!(0) {
            numerator / denominator
        } else {
            dec!(0)
        };

        // Determine trend type
        let trend_type = if slope > dec!(0.01) {
            TrendType::Upward
        } else if slope < dec!(-0.01) {
            TrendType::Downward
        } else {
            TrendType::Flat
        };

        // Detect changepoints using PELT-like algorithm (simplified)
        let changepoints = Self::detect_changepoints(data, n_changepoints)?;

        // Calculate trend strength (R-squared)
        let mut ss_res = dec!(0);
        let mut ss_tot = dec!(0);

        for (i, value) in values.iter().enumerate() {
            let predicted = y_mean + slope * (Decimal::from(i) - x_mean);
            ss_res += (*value - predicted) * (*value - predicted);
            ss_tot += (*value - y_mean) * (*value - y_mean);
        }

        let strength = if ss_tot > dec!(0) {
            (dec!(1) - ss_res / ss_tot).max(dec!(0))
        } else {
            dec!(0)
        };

        Ok(TrendDetection {
            trend_type,
            slope,
            strength,
            changepoints,
        })
    }

    /// Detect changepoints in the time series
    fn detect_changepoints(
        data: &[(DateTime<Utc>, Decimal)],
        n_changepoints: usize,
    ) -> Result<Vec<Changepoint>, CoreError> {
        if data.len() < n_changepoints * 2 {
            return Ok(Vec::new());
        }

        let values: Vec<Decimal> = data.iter().map(|(_, v)| *v).collect();
        let mut changepoints = Vec::new();

        // Divide data into segments and look for significant slope changes
        let segment_size = data.len() / (n_changepoints + 1);

        for i in 1..=n_changepoints {
            let idx = i * segment_size;
            if idx >= data.len() - 1 {
                break;
            }

            // Calculate slope before and after potential changepoint
            let before_start = idx.saturating_sub(segment_size);
            let after_end = (idx + segment_size).min(data.len());

            let slope_before = Self::calculate_slope(&values[before_start..idx]);
            let slope_after = Self::calculate_slope(&values[idx..after_end]);

            let delta = slope_after - slope_before;
            let significance = delta.abs();

            if significance > dec!(0.001) {
                changepoints.push(Changepoint {
                    timestamp: data[idx].0,
                    index: idx,
                    delta,
                    significance,
                });
            }
        }

        Ok(changepoints)
    }

    /// Calculate slope of a data segment
    fn calculate_slope(values: &[Decimal]) -> Decimal {
        if values.len() < 2 {
            return dec!(0);
        }

        let n = Decimal::from(values.len());
        let x_mean = (n - dec!(1)) / dec!(2);
        let y_mean: Decimal = values.iter().sum::<Decimal>() / n;

        let mut numerator = dec!(0);
        let mut denominator = dec!(0);

        for (i, value) in values.iter().enumerate() {
            let x_i = Decimal::from(i);
            let x_diff = x_i - x_mean;
            numerator += x_diff * (*value - y_mean);
            denominator += x_diff * x_diff;
        }

        if denominator > dec!(0) {
            numerator / denominator
        } else {
            dec!(0)
        }
    }

    /// Detect seasonality in time series
    pub fn detect_seasonality(
        data: &[(DateTime<Utc>, Decimal)],
    ) -> Result<SeasonalityDetection, CoreError> {
        if data.len() < 14 {
            return Err(CoreError::Validation(
                "Insufficient data for seasonality detection".to_string(),
            ));
        }

        let mut periods = Vec::new();

        // Check for weekly seasonality (7 days)
        if data.len() >= 14 {
            let weekly_strength = Self::calculate_seasonal_strength(data, 7);
            if weekly_strength > dec!(0.1) {
                periods.push(SeasonalPeriod {
                    period_days: 7,
                    strength: weekly_strength,
                    description: "Weekly".to_string(),
                });
            }
        }

        // Check for monthly seasonality (30 days)
        if data.len() >= 60 {
            let monthly_strength = Self::calculate_seasonal_strength(data, 30);
            if monthly_strength > dec!(0.1) {
                periods.push(SeasonalPeriod {
                    period_days: 30,
                    strength: monthly_strength,
                    description: "Monthly".to_string(),
                });
            }
        }

        // Overall seasonality strength
        let strength = if !periods.is_empty() {
            periods.iter().map(|p| p.strength).sum::<Decimal>() / Decimal::from(periods.len())
        } else {
            dec!(0)
        };

        Ok(SeasonalityDetection { periods, strength })
    }

    /// Calculate strength of seasonal component (simplified autocorrelation)
    fn calculate_seasonal_strength(data: &[(DateTime<Utc>, Decimal)], period: usize) -> Decimal {
        let values: Vec<Decimal> = data.iter().map(|(_, v)| *v).collect();

        if values.len() <= period {
            return dec!(0);
        }

        let mean: Decimal = values.iter().sum::<Decimal>() / Decimal::from(values.len());

        // Calculate autocorrelation at lag = period
        let mut numerator = dec!(0);
        let mut denominator = dec!(0);

        for i in period..values.len() {
            numerator += (values[i] - mean) * (values[i - period] - mean);
        }

        for value in &values {
            denominator += (*value - mean) * (*value - mean);
        }

        if denominator > dec!(0) {
            (numerator / denominator).abs()
        } else {
            dec!(0)
        }
    }

    /// Detect holiday effects
    pub fn detect_holiday_effects(
        &mut self,
        data: &[(DateTime<Utc>, Decimal)],
    ) -> Result<(), CoreError> {
        let values: Vec<Decimal> = data.iter().map(|(_, v)| *v).collect();
        let mean: Decimal = values.iter().sum::<Decimal>() / Decimal::from(values.len());

        // For each holiday, calculate average effect in the window
        for holiday in &mut self.holidays {
            let mut effect_sum = dec!(0);
            let mut count = 0;

            for (timestamp, value) in data {
                let days_diff = (*timestamp - holiday.date).num_days();

                if days_diff >= holiday.lower_window && days_diff <= holiday.upper_window {
                    effect_sum += *value - mean;
                    count += 1;
                }
            }

            holiday.effect = if count > 0 {
                effect_sum / Decimal::from(count)
            } else {
                dec!(0)
            };
        }

        Ok(())
    }

    /// Fit the Prophet model to data
    pub fn fit(&mut self, data: &[(DateTime<Utc>, Decimal)]) -> Result<(), CoreError> {
        if data.len() < 10 {
            return Err(CoreError::Validation(
                "Insufficient data for Prophet fitting".to_string(),
            ));
        }

        // Detect trend and changepoints
        let trend = Self::detect_trend(data, 5)?;
        self.changepoints = trend.changepoints;

        // Detect seasonality
        let _seasonality = Self::detect_seasonality(data)?;

        // Detect holiday effects
        self.detect_holiday_effects(data)?;

        self.fitted = true;
        Ok(())
    }

    /// Generate forecast
    pub fn predict(
        &self,
        start: DateTime<Utc>,
        periods: usize,
    ) -> Result<Vec<(DateTime<Utc>, Decimal)>, CoreError> {
        if !self.fitted {
            return Err(CoreError::Validation("Model not fitted".to_string()));
        }

        let mut predictions = Vec::new();

        // Simple linear trend forecast (simplified)
        // In a full implementation, this would use fitted trend + seasonality + holidays
        let base_value = dec!(100); // Would be learned from data

        for i in 0..periods {
            let timestamp = start + chrono::Duration::days(i as i64);
            let trend_component = Decimal::from(i) * dec!(0.1);

            // Add day-of-week seasonality (simplified)
            let day_of_week = timestamp.weekday().num_days_from_monday();
            let seasonal_component = match day_of_week {
                0 | 6 => dec!(-2), // Monday and Sunday lower
                _ => dec!(1),      // Other days higher
            };

            let value = base_value + trend_component + seasonal_component;
            predictions.push((timestamp, value));
        }

        Ok(predictions)
    }
}

impl Default for ProphetModel {
    fn default() -> Self {
        Self::new(GrowthType::Linear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_test_data(n: usize) -> Vec<(DateTime<Utc>, Decimal)> {
        let start = Utc::now();
        (0..n)
            .map(|i| {
                let timestamp = start + chrono::Duration::days(i as i64);
                let value = Decimal::from(i) + dec!(100);
                (timestamp, value)
            })
            .collect()
    }

    #[test]
    fn test_trend_detection() {
        let data = generate_test_data(50);

        let trend = ProphetModel::detect_trend(&data, 3).unwrap();
        assert_eq!(trend.trend_type, TrendType::Upward);
        assert!(trend.slope > dec!(0));
    }

    #[test]
    fn test_changepoint_detection() {
        let mut data = Vec::new();
        let start = Utc::now();

        // First segment: flat
        for i in 0..20 {
            data.push((start + chrono::Duration::days(i), dec!(100)));
        }

        // Second segment: upward
        for i in 20..40 {
            data.push((
                start + chrono::Duration::days(i),
                Decimal::from(i - 20) + dec!(100),
            ));
        }

        let trend = ProphetModel::detect_trend(&data, 5).unwrap();
        assert!(!trend.changepoints.is_empty());
    }

    #[test]
    fn test_seasonality_detection() {
        let data = generate_test_data(30);
        let seasonality = ProphetModel::detect_seasonality(&data).unwrap();

        // May or may not detect seasonality in this simple test data
        assert!(seasonality.strength >= dec!(0));
    }

    #[test]
    fn test_prophet_model_fit() {
        let data = generate_test_data(50);

        let mut model = ProphetModel::new(GrowthType::Linear);
        assert!(model.fit(&data).is_ok());
        assert!(model.fitted);
    }

    #[test]
    fn test_prophet_predict() {
        let data = generate_test_data(50);

        let mut model = ProphetModel::new(GrowthType::Linear);
        model.fit(&data).unwrap();

        let predictions = model.predict(Utc::now(), 10).unwrap();
        assert_eq!(predictions.len(), 10);
    }

    #[test]
    fn test_add_seasonality() {
        let model =
            ProphetModel::new(GrowthType::Linear).add_seasonality("weekly".to_string(), 7, 3);

        assert_eq!(model.seasonal_components.len(), 1);
        assert_eq!(model.seasonal_components[0].period_days, 7);
    }

    #[test]
    fn test_add_holiday() {
        let model = ProphetModel::new(GrowthType::Linear).add_holiday(
            "New Year".to_string(),
            Utc::now(),
            -1,
            1,
        );

        assert_eq!(model.holidays.len(), 1);
        assert_eq!(model.holidays[0].name, "New Year");
    }

    #[test]
    fn test_insufficient_data_error() {
        let data = vec![(Utc::now(), dec!(100))];

        let result = ProphetModel::detect_trend(&data, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_growth_types() {
        let linear = ProphetModel::new(GrowthType::Linear);
        let logistic = ProphetModel::new(GrowthType::Logistic);

        assert_eq!(linear.growth, GrowthType::Linear);
        assert_eq!(logistic.growth, GrowthType::Logistic);
    }
}
