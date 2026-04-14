//! Online Learning Capabilities
//!
//! Provides incremental learning algorithms that can update models with new data
//! without requiring full retraining.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Online learning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineLearningConfig {
    /// Learning rate
    pub learning_rate: f64,
    /// Whether to use adaptive learning rate
    pub adaptive_learning_rate: bool,
    /// Decay factor for adaptive learning rate
    pub learning_rate_decay: f64,
    /// Window size for drift detection
    pub drift_window_size: usize,
    /// Drift detection threshold
    pub drift_threshold: f64,
}

impl Default for OnlineLearningConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            adaptive_learning_rate: true,
            learning_rate_decay: 0.99,
            drift_window_size: 100,
            drift_threshold: 0.1,
        }
    }
}

/// Online learner trait for incremental model updates
pub trait OnlineLearner {
    /// Update model with a single new data point
    fn update(&mut self, features: &[f64], target: f64) -> anyhow::Result<()>;

    /// Batch update with multiple data points
    fn batch_update(&mut self, batch: &[(Vec<f64>, f64)]) -> anyhow::Result<()> {
        for (features, target) in batch {
            self.update(features, *target)?;
        }
        Ok(())
    }

    /// Get current learning rate
    fn learning_rate(&self) -> f64;

    /// Set learning rate
    fn set_learning_rate(&mut self, rate: f64);
}

/// Online linear regression model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineLinearRegression {
    /// Model weights
    pub weights: Vec<f64>,
    /// Bias term
    pub bias: f64,
    /// Learning rate
    pub learning_rate: f64,
    /// Number of updates performed
    pub update_count: usize,
    /// Configuration
    pub config: OnlineLearningConfig,
}

impl OnlineLinearRegression {
    /// Create a new online linear regression model
    pub fn new(num_features: usize, config: OnlineLearningConfig) -> Self {
        Self {
            weights: vec![0.0; num_features],
            bias: 0.0,
            learning_rate: config.learning_rate,
            update_count: 0,
            config,
        }
    }

    /// Predict the output for the given feature vector
    pub fn predict(&self, features: &[f64]) -> anyhow::Result<f64> {
        if features.len() != self.weights.len() {
            anyhow::bail!("Feature dimension mismatch");
        }

        let prediction: f64 = features
            .iter()
            .zip(&self.weights)
            .map(|(x, w)| x * w)
            .sum::<f64>()
            + self.bias;

        Ok(prediction)
    }

    fn apply_adaptive_learning_rate(&mut self) {
        if self.config.adaptive_learning_rate {
            self.learning_rate *= self.config.learning_rate_decay;
        }
    }
}

impl OnlineLearner for OnlineLinearRegression {
    fn update(&mut self, features: &[f64], target: f64) -> anyhow::Result<()> {
        if features.len() != self.weights.len() {
            anyhow::bail!("Feature dimension mismatch");
        }

        // Make prediction
        let prediction = self.predict(features)?;

        // Calculate error
        let error = target - prediction;

        // Update weights using gradient descent
        for (i, &feature) in features.iter().enumerate() {
            self.weights[i] += self.learning_rate * error * feature;
        }

        // Update bias
        self.bias += self.learning_rate * error;

        // Update count and learning rate
        self.update_count += 1;
        self.apply_adaptive_learning_rate();

        Ok(())
    }

    fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, rate: f64) {
        self.learning_rate = rate;
    }
}

/// Concept drift detector
#[derive(Debug, Clone)]
pub struct DriftDetector {
    /// Recent prediction errors
    error_window: VecDeque<f64>,
    /// Window size
    window_size: usize,
    /// Drift threshold
    threshold: f64,
    /// Baseline error mean
    baseline_mean: f64,
    /// Baseline error std dev
    baseline_std: f64,
    /// Number of drift events detected
    pub drift_count: usize,
}

impl DriftDetector {
    /// Create a new drift detector with the given window size and threshold
    pub fn new(window_size: usize, threshold: f64) -> Self {
        Self {
            error_window: VecDeque::with_capacity(window_size),
            window_size,
            threshold,
            baseline_mean: 0.0,
            baseline_std: 0.0,
            drift_count: 0,
        }
    }

    /// Add new error and check for drift
    pub fn add_error(&mut self, error: f64) -> bool {
        // Add to window
        self.error_window.push_back(error.abs());
        if self.error_window.len() > self.window_size {
            self.error_window.pop_front();
        }

        // Initialize baseline on first full window
        if self.error_window.len() == self.window_size && self.baseline_mean == 0.0 {
            self.update_baseline();
            return false;
        }

        // Check for drift if we have baseline
        if self.baseline_mean > 0.0 && self.error_window.len() == self.window_size {
            let current_mean = self.calculate_mean();
            let drift_detected =
                (current_mean - self.baseline_mean).abs() / self.baseline_std > self.threshold;

            if drift_detected {
                self.drift_count += 1;
                // Update baseline to new distribution
                self.update_baseline();
            }

            return drift_detected;
        }

        false
    }

    fn calculate_mean(&self) -> f64 {
        self.error_window.iter().sum::<f64>() / self.error_window.len() as f64
    }

    fn calculate_std(&self) -> f64 {
        let mean = self.calculate_mean();
        let variance = self
            .error_window
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>()
            / self.error_window.len() as f64;
        variance.sqrt()
    }

    fn update_baseline(&mut self) {
        self.baseline_mean = self.calculate_mean();
        self.baseline_std = self.calculate_std().max(1e-6); // Avoid division by zero
    }

    /// Reset the detector, clearing the error history and baseline
    pub fn reset(&mut self) {
        self.error_window.clear();
        self.baseline_mean = 0.0;
        self.baseline_std = 0.0;
    }
}

/// Online model with drift detection
#[derive(Debug)]
pub struct AdaptiveOnlineModel<L: OnlineLearner> {
    /// Base learner
    pub learner: L,
    /// Drift detector
    pub drift_detector: DriftDetector,
    /// Retrain on drift flag
    pub retrain_on_drift: bool,
    /// Training data buffer for retraining
    data_buffer: VecDeque<(Vec<f64>, f64)>,
    /// Maximum buffer size
    max_buffer_size: usize,
}

impl<L: OnlineLearner> AdaptiveOnlineModel<L> {
    /// Create a new adaptive model wrapping the given learner with drift detection
    pub fn new(learner: L, drift_detector: DriftDetector, max_buffer_size: usize) -> Self {
        Self {
            learner,
            drift_detector,
            retrain_on_drift: true,
            data_buffer: VecDeque::with_capacity(max_buffer_size),
            max_buffer_size,
        }
    }

    /// Update model and check for drift
    pub fn update(&mut self, features: &[f64], target: f64) -> anyhow::Result<DriftStatus> {
        // Store in buffer
        if self.data_buffer.len() >= self.max_buffer_size {
            self.data_buffer.pop_front();
        }
        self.data_buffer.push_back((features.to_vec(), target));

        // For drift detection, we use the target as a placeholder prediction
        // In a real implementation, this would call a predict method on the learner
        let prediction = target;

        // Update model
        self.learner.update(features, target)?;

        // Check for drift
        let error = (target - prediction).abs();
        let drift_detected = self.drift_detector.add_error(error);

        if drift_detected && self.retrain_on_drift {
            // Retrain with buffered data
            for (feats, tgt) in &self.data_buffer {
                self.learner.update(feats, *tgt)?;
            }
            Ok(DriftStatus::DriftDetectedAndRetrained)
        } else if drift_detected {
            Ok(DriftStatus::DriftDetected)
        } else {
            Ok(DriftStatus::NoDrift)
        }
    }
}

/// Drift status result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftStatus {
    /// No concept drift was detected
    NoDrift,
    /// Concept drift was detected but retraining was not triggered
    DriftDetected,
    /// Concept drift was detected and the model was retrained
    DriftDetectedAndRetrained,
}

/// Incremental learning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalStats {
    /// Total number of model updates processed
    pub total_updates: usize,
    /// Number of concept drift events detected
    pub drift_events: usize,
    /// Current learning rate of the underlying model
    pub current_learning_rate: f64,
    /// Timestamp of the most recent update
    pub last_update: DateTime<Utc>,
    /// Exponential moving average of prediction error
    pub average_error: f64,
}

impl IncrementalStats {
    /// Create a new zeroed incremental stats tracker
    pub fn new() -> Self {
        Self {
            total_updates: 0,
            drift_events: 0,
            current_learning_rate: 0.01,
            last_update: Utc::now(),
            average_error: 0.0,
        }
    }

    /// Record a model update step
    pub fn update(&mut self, learning_rate: f64, error: f64, drift_detected: bool) {
        self.total_updates += 1;
        if drift_detected {
            self.drift_events += 1;
        }
        self.current_learning_rate = learning_rate;
        self.last_update = Utc::now();

        // Update average error with exponential moving average
        let alpha = 0.1;
        self.average_error = alpha * error + (1.0 - alpha) * self.average_error;
    }
}

impl Default for IncrementalStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Online moving average for streaming data
#[derive(Debug, Clone)]
pub struct OnlineMovingAverage {
    /// Circular buffer holding the current window of values
    window: VecDeque<Decimal>,
    /// Maximum number of values to keep in the window
    window_size: usize,
    /// Running sum of all values in the window
    sum: Decimal,
}

impl OnlineMovingAverage {
    /// Create a new online moving average with the given window size
    pub fn new(window_size: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            sum: Decimal::ZERO,
        }
    }

    /// Add a new value to the window and return the updated moving average
    pub fn add(&mut self, value: Decimal) -> Decimal {
        if self.window.len() == self.window_size {
            if let Some(old) = self.window.pop_front() {
                self.sum -= old;
            }
        }

        self.window.push_back(value);
        self.sum += value;

        self.average()
    }

    /// Return the current moving average
    pub fn average(&self) -> Decimal {
        if self.window.is_empty() {
            Decimal::ZERO
        } else {
            self.sum / Decimal::from(self.window.len())
        }
    }

    /// Reset the moving average, clearing all accumulated values
    pub fn reset(&mut self) {
        self.window.clear();
        self.sum = Decimal::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_online_linear_regression() {
        let config = OnlineLearningConfig::default();
        let mut model = OnlineLinearRegression::new(2, config);

        // Train with simple linear relationship: y = 2*x1 + 3*x2
        for _ in 0..100 {
            let x1 = 1.0;
            let x2 = 2.0;
            let y = 2.0 * x1 + 3.0 * x2;
            model.update(&[x1, x2], y).unwrap();
        }

        // Test prediction
        let prediction = model.predict(&[1.0, 2.0]).unwrap();
        assert!((prediction - 8.0).abs() < 1.0); // Should be close to 8.0
    }

    #[test]
    fn test_drift_detector() {
        let mut detector = DriftDetector::new(10, 2.0);

        // Add stable errors
        for _ in 0..20 {
            detector.add_error(0.1);
            // No drift in stable phase
        }

        // Add sudden spike in errors
        for _ in 0..10 {
            detector.add_error(1.0);
            // Should detect drift
        }

        assert!(detector.drift_count > 0);
    }

    #[test]
    fn test_online_moving_average() {
        let mut oma = OnlineMovingAverage::new(3);

        assert_eq!(oma.add(Decimal::from(10)), Decimal::from(10));
        assert_eq!(oma.add(Decimal::from(20)), Decimal::from(15));
        assert_eq!(oma.add(Decimal::from(30)), Decimal::from(20));
        assert_eq!(oma.add(Decimal::from(40)), Decimal::from(30)); // (20+30+40)/3
    }

    #[test]
    fn test_adaptive_learning_rate() {
        let config = OnlineLearningConfig {
            learning_rate: 0.01,
            adaptive_learning_rate: true,
            learning_rate_decay: 0.9,
            drift_window_size: 100,
            drift_threshold: 0.1,
        };

        let mut model = OnlineLinearRegression::new(2, config);
        let initial_lr = model.learning_rate;

        model.update(&[1.0, 2.0], 5.0).unwrap();

        assert!(model.learning_rate < initial_lr);
    }

    #[test]
    fn test_incremental_stats() {
        let mut stats = IncrementalStats::new();

        stats.update(0.01, 0.5, false);
        assert_eq!(stats.total_updates, 1);
        assert_eq!(stats.drift_events, 0);

        stats.update(0.01, 0.3, true);
        assert_eq!(stats.total_updates, 2);
        assert_eq!(stats.drift_events, 1);
    }
}
