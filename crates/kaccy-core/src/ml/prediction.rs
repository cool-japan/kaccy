//! Price prediction models for forecasting future prices
//!
//! This module provides various machine learning and statistical models
//! for predicting future token prices with confidence intervals.

use super::features::PricePoint;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Prediction result with confidence interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePrediction {
    /// Timestamp for which this prediction applies
    pub timestamp: DateTime<Utc>,
    /// Predicted price at the given timestamp
    pub predicted_price: Decimal,
    /// Lower bound of the prediction confidence interval
    pub lower_bound: Decimal,
    /// Upper bound of the prediction confidence interval
    pub upper_bound: Decimal,
    /// Confidence level (e.g. 0.95 for 95%)
    pub confidence_level: f64,
}

impl PricePrediction {
    /// Create a new price prediction with confidence interval
    pub fn new(
        timestamp: DateTime<Utc>,
        predicted_price: Decimal,
        lower_bound: Decimal,
        upper_bound: Decimal,
        confidence_level: f64,
    ) -> Self {
        Self {
            timestamp,
            predicted_price,
            lower_bound,
            upper_bound,
            confidence_level,
        }
    }

    /// Get the prediction interval width
    pub fn interval_width(&self) -> Decimal {
        self.upper_bound - self.lower_bound
    }

    /// Check if actual price falls within prediction interval
    pub fn is_accurate(&self, actual_price: Decimal) -> bool {
        actual_price >= self.lower_bound && actual_price <= self.upper_bound
    }
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformance {
    /// Mean Absolute Error
    pub mae: f64,
    /// Root Mean Square Error
    pub rmse: f64,
    /// Mean Absolute Percentage Error
    pub mape: f64,
    /// R-squared coefficient of determination
    pub r_squared: f64,
    /// Number of predictions evaluated
    pub predictions: usize,
}

impl ModelPerformance {
    /// Calculate performance metrics from predicted and actual values
    pub fn calculate(predictions: &[f64], actuals: &[f64]) -> anyhow::Result<Self> {
        if predictions.len() != actuals.len() || predictions.is_empty() {
            anyhow::bail!("Predictions and actuals must have the same non-zero length");
        }

        let n = predictions.len() as f64;

        // MAE
        let mae = predictions
            .iter()
            .zip(actuals.iter())
            .map(|(p, a)| (p - a).abs())
            .sum::<f64>()
            / n;

        // RMSE
        let mse = predictions
            .iter()
            .zip(actuals.iter())
            .map(|(p, a)| (p - a).powi(2))
            .sum::<f64>()
            / n;
        let rmse = mse.sqrt();

        // MAPE
        let mape = predictions
            .iter()
            .zip(actuals.iter())
            .filter(|&(_, a)| *a != 0.0)
            .map(|(p, a)| ((p - a) / a).abs())
            .sum::<f64>()
            / n
            * 100.0;

        // R-squared
        let mean_actual = actuals.iter().sum::<f64>() / n;
        let ss_tot = actuals
            .iter()
            .map(|&a| (a - mean_actual).powi(2))
            .sum::<f64>();
        let ss_res = predictions
            .iter()
            .zip(actuals.iter())
            .map(|(p, a)| (a - p).powi(2))
            .sum::<f64>();
        let r_squared = 1.0 - (ss_res / ss_tot.max(1e-10));

        Ok(Self {
            mae,
            rmse,
            mape,
            r_squared,
            predictions: predictions.len(),
        })
    }
}

/// Price prediction model trait
pub trait PredictionModel: Send + Sync {
    /// Train the model on historical data
    fn train(&mut self, data: &[PricePoint]) -> anyhow::Result<()>;

    /// Predict future prices
    fn predict(&self, horizon: usize) -> anyhow::Result<Vec<PricePrediction>>;

    /// Get model performance metrics
    fn performance(&self) -> Option<ModelPerformance>;

    /// Model name
    fn name(&self) -> &str;
}

/// Simple moving average prediction model
#[derive(Debug, Clone)]
pub struct MovingAveragePredictionModel {
    /// Number of periods in the moving average window
    period: usize,
    /// Historical price data used for training and prediction
    historical_prices: Vec<PricePoint>,
    /// Cached performance metrics from training
    performance: Option<ModelPerformance>,
}

impl MovingAveragePredictionModel {
    /// Create a new moving average prediction model with the given window period
    pub fn new(period: usize) -> Self {
        Self {
            period,
            historical_prices: Vec::new(),
            performance: None,
        }
    }

    fn calculate_ma(&self, prices: &[f64]) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }
        prices.iter().sum::<f64>() / prices.len() as f64
    }

    fn calculate_std(&self, prices: &[f64], mean: f64) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }
        let variance =
            prices.iter().map(|&p| (p - mean).powi(2)).sum::<f64>() / (prices.len() - 1) as f64;
        variance.sqrt()
    }
}

impl PredictionModel for MovingAveragePredictionModel {
    fn train(&mut self, data: &[PricePoint]) -> anyhow::Result<()> {
        if data.len() < self.period {
            anyhow::bail!(
                "Insufficient data for training (need at least {} points)",
                self.period
            );
        }

        self.historical_prices = data.to_vec();

        // Calculate performance on training data
        let prices: Vec<f64> = data
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let mut predictions = Vec::new();
        let mut actuals = Vec::new();

        for i in self.period..prices.len() {
            let window = &prices[i - self.period..i];
            let pred = self.calculate_ma(window);
            predictions.push(pred);
            actuals.push(prices[i]);
        }

        if !predictions.is_empty() {
            self.performance = Some(ModelPerformance::calculate(&predictions, &actuals)?);
        }

        Ok(())
    }

    fn predict(&self, horizon: usize) -> anyhow::Result<Vec<PricePrediction>> {
        if self.historical_prices.is_empty() {
            anyhow::bail!("Model not trained");
        }

        let prices: Vec<f64> = self
            .historical_prices
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let last_timestamp = self.historical_prices.last().expect("historical_prices is non-empty per len check at function start").timestamp;
        let window_size = self.period.min(prices.len());
        let window = &prices[prices.len() - window_size..];

        let ma = self.calculate_ma(window);
        let std = self.calculate_std(window, ma);

        let mut predictions = Vec::new();
        for i in 1..=horizon {
            let timestamp = last_timestamp + Duration::days(i as i64);

            // Confidence interval widens with horizon
            let confidence = 0.95;
            let z_score = 1.96; // 95% confidence
            let horizon_factor = (i as f64).sqrt();
            let interval = z_score * std * horizon_factor;

            let predicted = Decimal::from_f64_retain(ma).unwrap_or(Decimal::ZERO);
            let lower = Decimal::from_f64_retain((ma - interval).max(0.0)).unwrap_or(Decimal::ZERO);
            let upper = Decimal::from_f64_retain(ma + interval).unwrap_or(Decimal::ZERO);

            predictions.push(PricePrediction::new(
                timestamp, predicted, lower, upper, confidence,
            ));
        }

        Ok(predictions)
    }

    fn performance(&self) -> Option<ModelPerformance> {
        self.performance.clone()
    }

    fn name(&self) -> &str {
        "Moving Average"
    }
}

/// Linear regression prediction model
#[derive(Debug, Clone)]
pub struct LinearRegressionModel {
    /// Fitted slope of the regression line
    slope: f64,
    /// Fitted intercept of the regression line
    intercept: f64,
    /// Historical price data used for training and prediction
    historical_prices: Vec<PricePoint>,
    /// Cached performance metrics from training
    performance: Option<ModelPerformance>,
}

impl LinearRegressionModel {
    /// Create a new linear regression prediction model
    pub fn new() -> Self {
        Self {
            slope: 0.0,
            intercept: 0.0,
            historical_prices: Vec::new(),
            performance: None,
        }
    }

    fn fit_linear_regression(&mut self, prices: &[f64]) -> anyhow::Result<()> {
        if prices.len() < 2 {
            anyhow::bail!("Need at least 2 data points for linear regression");
        }

        let n = prices.len() as f64;
        let x: Vec<f64> = (0..prices.len()).map(|i| i as f64).collect();

        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = prices.iter().sum();
        let sum_xy: f64 = x.iter().zip(prices.iter()).map(|(a, b)| a * b).sum();
        let sum_x2: f64 = x.iter().map(|a| a * a).sum();

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            anyhow::bail!("Cannot fit linear regression - singular matrix");
        }

        self.slope = (n * sum_xy - sum_x * sum_y) / denominator;
        self.intercept = (sum_y - self.slope * sum_x) / n;

        Ok(())
    }

    fn predict_at(&self, x: f64) -> f64 {
        self.slope * x + self.intercept
    }

    fn calculate_std_error(&self, prices: &[f64]) -> f64 {
        if prices.len() < 3 {
            return 0.0;
        }

        let n = prices.len() as f64;
        let residuals_sq: f64 = prices
            .iter()
            .enumerate()
            .map(|(i, &actual)| {
                let predicted = self.predict_at(i as f64);
                (actual - predicted).powi(2)
            })
            .sum();

        (residuals_sq / (n - 2.0)).sqrt()
    }
}

impl Default for LinearRegressionModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictionModel for LinearRegressionModel {
    fn train(&mut self, data: &[PricePoint]) -> anyhow::Result<()> {
        if data.len() < 2 {
            anyhow::bail!("Insufficient data for training");
        }

        self.historical_prices = data.to_vec();

        let prices: Vec<f64> = data
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        self.fit_linear_regression(&prices)?;

        // Calculate performance
        let predictions: Vec<f64> = (0..prices.len())
            .map(|i| self.predict_at(i as f64))
            .collect();

        self.performance = Some(ModelPerformance::calculate(&predictions, &prices)?);

        Ok(())
    }

    fn predict(&self, horizon: usize) -> anyhow::Result<Vec<PricePrediction>> {
        if self.historical_prices.is_empty() {
            anyhow::bail!("Model not trained");
        }

        let prices: Vec<f64> = self
            .historical_prices
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let last_timestamp = self.historical_prices.last().expect("historical_prices is non-empty per len check at function start").timestamp;
        let std_error = self.calculate_std_error(&prices);

        let mut predictions = Vec::new();
        for i in 1..=horizon {
            let x = (prices.len() + i - 1) as f64;
            let timestamp = last_timestamp + Duration::days(i as i64);

            let predicted_val = self.predict_at(x);

            // Confidence interval widens with horizon
            let confidence = 0.95;
            let z_score = 1.96; // 95% confidence
            let horizon_factor = (i as f64).sqrt();
            let interval = z_score * std_error * horizon_factor;

            let predicted = Decimal::from_f64_retain(predicted_val).unwrap_or(Decimal::ZERO);
            let lower = Decimal::from_f64_retain((predicted_val - interval).max(0.0))
                .unwrap_or(Decimal::ZERO);
            let upper = Decimal::from_f64_retain(predicted_val + interval).unwrap_or(Decimal::ZERO);

            predictions.push(PricePrediction::new(
                timestamp, predicted, lower, upper, confidence,
            ));
        }

        Ok(predictions)
    }

    fn performance(&self) -> Option<ModelPerformance> {
        self.performance.clone()
    }

    fn name(&self) -> &str {
        "Linear Regression"
    }
}

/// Ensemble model that combines multiple prediction models
pub struct EnsembleModel {
    /// Component prediction models
    models: Vec<Box<dyn PredictionModel>>,
    /// Per-model blending weights (parallel to `models`)
    weights: Vec<f64>,
}

impl EnsembleModel {
    /// Create a new empty ensemble model
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            weights: Vec::new(),
        }
    }

    /// Add a model to the ensemble with a given weight
    pub fn add_model(mut self, model: Box<dyn PredictionModel>, weight: f64) -> Self {
        self.models.push(model);
        self.weights.push(weight);
        self
    }

    /// Assign equal weights to all models currently in the ensemble
    pub fn with_equal_weights(mut self) -> Self {
        let n = self.models.len();
        if n > 0 {
            let weight = 1.0 / n as f64;
            self.weights = vec![weight; n];
        }
        self
    }
}

impl Default for EnsembleModel {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictionModel for EnsembleModel {
    fn train(&mut self, data: &[PricePoint]) -> anyhow::Result<()> {
        for model in &mut self.models {
            model.train(data)?;
        }
        Ok(())
    }

    fn predict(&self, horizon: usize) -> anyhow::Result<Vec<PricePrediction>> {
        if self.models.is_empty() {
            anyhow::bail!("No models in ensemble");
        }

        // Get predictions from all models
        let mut all_predictions = Vec::new();
        for model in &self.models {
            all_predictions.push(model.predict(horizon)?);
        }

        // Combine predictions using weighted average
        let mut combined = Vec::new();
        for h in 0..horizon {
            let timestamp = all_predictions[0][h].timestamp;

            let mut weighted_price = 0.0;
            let mut weighted_lower = 0.0;
            let mut weighted_upper = 0.0;
            let mut total_weight = 0.0;

            for (i, preds) in all_predictions.iter().enumerate() {
                let weight = if i < self.weights.len() {
                    self.weights[i]
                } else {
                    1.0 / self.models.len() as f64
                };

                weighted_price += preds[h]
                    .predicted_price
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    * weight;
                weighted_lower += preds[h]
                    .lower_bound
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    * weight;
                weighted_upper += preds[h]
                    .upper_bound
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    * weight;
                total_weight += weight;
            }

            let predicted =
                Decimal::from_f64_retain(weighted_price / total_weight).unwrap_or(Decimal::ZERO);
            let lower =
                Decimal::from_f64_retain(weighted_lower / total_weight).unwrap_or(Decimal::ZERO);
            let upper =
                Decimal::from_f64_retain(weighted_upper / total_weight).unwrap_or(Decimal::ZERO);

            combined.push(PricePrediction::new(
                timestamp, predicted, lower, upper, 0.95,
            ));
        }

        Ok(combined)
    }

    fn performance(&self) -> Option<ModelPerformance> {
        // Average performance of all models
        let perfs: Vec<_> = self.models.iter().filter_map(|m| m.performance()).collect();

        if perfs.is_empty() {
            return None;
        }

        let n = perfs.len() as f64;
        Some(ModelPerformance {
            mae: perfs.iter().map(|p| p.mae).sum::<f64>() / n,
            rmse: perfs.iter().map(|p| p.rmse).sum::<f64>() / n,
            mape: perfs.iter().map(|p| p.mape).sum::<f64>() / n,
            r_squared: perfs.iter().map(|p| p.r_squared).sum::<f64>() / n,
            predictions: perfs[0].predictions,
        })
    }

    fn name(&self) -> &str {
        "Ensemble"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_data() -> Vec<PricePoint> {
        let now = Utc::now();
        (0..100)
            .map(|i| PricePoint {
                timestamp: now - chrono::Duration::days(100 - i),
                open: dec!(100) + Decimal::from(i),
                high: dec!(105) + Decimal::from(i),
                low: dec!(95) + Decimal::from(i),
                close: dec!(100) + Decimal::from(i),
                volume: dec!(1000),
            })
            .collect()
    }

    #[test]
    fn test_moving_average_model() {
        let data = create_test_data();
        let mut model = MovingAveragePredictionModel::new(20);

        model.train(&data).unwrap();
        let predictions = model.predict(5).unwrap();

        assert_eq!(predictions.len(), 5);
        assert!(predictions[0].predicted_price > Decimal::ZERO);
        assert!(predictions[0].lower_bound < predictions[0].predicted_price);
        assert!(predictions[0].upper_bound > predictions[0].predicted_price);
    }

    #[test]
    fn test_linear_regression_model() {
        let data = create_test_data();
        let mut model = LinearRegressionModel::new();

        model.train(&data).unwrap();
        let predictions = model.predict(5).unwrap();

        assert_eq!(predictions.len(), 5);
        assert!(predictions[0].predicted_price > Decimal::ZERO);
    }

    #[test]
    fn test_ensemble_model() {
        let data = create_test_data();
        let mut model = EnsembleModel::new()
            .add_model(Box::new(MovingAveragePredictionModel::new(20)), 0.5)
            .add_model(Box::new(LinearRegressionModel::new()), 0.5);

        model.train(&data).unwrap();
        let predictions = model.predict(5).unwrap();

        assert_eq!(predictions.len(), 5);
        assert!(predictions[0].predicted_price > Decimal::ZERO);
    }

    #[test]
    fn test_model_performance() {
        let predictions = vec![100.0, 105.0, 110.0, 115.0];
        let actuals = vec![102.0, 104.0, 112.0, 116.0];

        let perf = ModelPerformance::calculate(&predictions, &actuals).unwrap();
        assert!(perf.mae > 0.0);
        assert!(perf.rmse > 0.0);
        assert!(perf.mape > 0.0);
    }

    #[test]
    fn test_prediction_accuracy() {
        let now = Utc::now();
        let pred = PricePrediction::new(now, dec!(100), dec!(95), dec!(105), 0.95);

        assert!(pred.is_accurate(dec!(100)));
        assert!(pred.is_accurate(dec!(95)));
        assert!(pred.is_accurate(dec!(105)));
        assert!(!pred.is_accurate(dec!(90)));
        assert!(!pred.is_accurate(dec!(110)));
    }
}
