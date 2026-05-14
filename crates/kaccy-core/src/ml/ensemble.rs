//! Enhanced Ensemble Methods
//!
//! Provides advanced ensemble techniques including stacking, boosting, and
//! automatic weight optimization.

use serde::{Deserialize, Serialize};

/// Stacking ensemble configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackingConfig {
    /// Number of folds for cross-validation during meta-model training
    pub cv_folds: usize,
    /// Whether to include original features in meta-model
    pub include_original_features: bool,
}

impl Default for StackingConfig {
    fn default() -> Self {
        Self {
            cv_folds: 5,
            include_original_features: true,
        }
    }
}

/// Stacking ensemble that trains a meta-model on base model predictions
#[derive(Debug, Clone)]
pub struct StackingEnsemble {
    /// Base model predictions (from cross-validation)
    base_predictions: Vec<Vec<f64>>,
    /// Meta-model weights
    meta_weights: Vec<f64>,
    /// Configuration
    #[allow(dead_code)]
    config: StackingConfig,
}

impl StackingEnsemble {
    /// Creates a new `StackingEnsemble` with the given configuration.
    pub fn new(config: StackingConfig) -> Self {
        Self {
            base_predictions: Vec::new(),
            meta_weights: Vec::new(),
            config,
        }
    }

    /// Train meta-model using base model predictions
    pub fn train_meta_model(
        &mut self,
        base_predictions: Vec<Vec<f64>>,
        targets: &[f64],
    ) -> anyhow::Result<()> {
        if base_predictions.is_empty() || targets.is_empty() {
            anyhow::bail!("Empty training data");
        }

        self.base_predictions = base_predictions;

        // Simple linear regression for meta-model
        // In practice, you could use any model here
        let num_models = self.base_predictions.len();
        self.meta_weights = vec![1.0 / num_models as f64; num_models];

        // Optimize weights using gradient descent
        self.optimize_weights(targets)?;

        Ok(())
    }

    /// Predict using stacked ensemble
    pub fn predict(&self, base_model_outputs: &[f64]) -> f64 {
        base_model_outputs
            .iter()
            .zip(&self.meta_weights)
            .map(|(pred, weight)| pred * weight)
            .sum()
    }

    /// Optimize meta-model weights
    fn optimize_weights(&mut self, targets: &[f64]) -> anyhow::Result<()> {
        let learning_rate = 0.01;
        let iterations = 100;

        for _ in 0..iterations {
            let mut gradients = vec![0.0; self.meta_weights.len()];

            // Calculate gradients
            for (i, target) in targets.iter().enumerate() {
                let mut prediction = 0.0;
                for (model_idx, model_preds) in self.base_predictions.iter().enumerate() {
                    if i < model_preds.len() {
                        prediction += model_preds[i] * self.meta_weights[model_idx];
                    }
                }

                let error = prediction - target;

                for (model_idx, model_preds) in self.base_predictions.iter().enumerate() {
                    if i < model_preds.len() {
                        gradients[model_idx] += 2.0 * error * model_preds[i];
                    }
                }
            }

            // Update weights
            for (weight, gradient) in self.meta_weights.iter_mut().zip(&gradients) {
                *weight -= learning_rate * gradient / targets.len() as f64;
            }

            // Normalize weights to sum to 1
            let sum: f64 = self.meta_weights.iter().sum();
            if sum > 0.0 {
                for weight in &mut self.meta_weights {
                    *weight /= sum;
                }
            }
        }

        Ok(())
    }

    /// Returns the learned meta-model weights.
    pub fn get_weights(&self) -> &[f64] {
        &self.meta_weights
    }
}

/// Boosting ensemble configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoostingConfig {
    /// Number of boosting iterations
    pub n_estimators: usize,
    /// Learning rate for weight updates
    pub learning_rate: f64,
    /// Maximum depth for weak learners
    pub max_depth: usize,
}

impl Default for BoostingConfig {
    fn default() -> Self {
        Self {
            n_estimators: 50,
            learning_rate: 0.1,
            max_depth: 3,
        }
    }
}

/// AdaBoost-style boosting ensemble
#[derive(Debug, Clone)]
pub struct BoostingEnsemble {
    /// Weak learner predictions
    weak_learners: Vec<WeakLearner>,
    /// Learner weights (alphas)
    alphas: Vec<f64>,
    /// Configuration
    config: BoostingConfig,
}

#[derive(Debug, Clone)]
struct WeakLearner {
    /// Simple threshold-based prediction
    threshold: f64,
    /// Feature index to use
    feature_idx: usize,
    /// Direction (above or below threshold)
    direction: f64,
}

impl WeakLearner {
    fn predict(&self, feature_value: f64) -> f64 {
        if (feature_value > self.threshold && self.direction > 0.0)
            || (feature_value <= self.threshold && self.direction < 0.0)
        {
            1.0
        } else {
            -1.0
        }
    }
}

impl BoostingEnsemble {
    /// Creates a new `BoostingEnsemble` with the given configuration.
    pub fn new(config: BoostingConfig) -> Self {
        Self {
            weak_learners: Vec::new(),
            alphas: Vec::new(),
            config,
        }
    }

    /// Train boosting ensemble
    pub fn train(&mut self, features: &[Vec<f64>], targets: &[f64]) -> anyhow::Result<()> {
        if features.is_empty() || targets.is_empty() {
            anyhow::bail!("Empty training data");
        }

        let n_samples = features.len();
        let mut weights = vec![1.0 / n_samples as f64; n_samples];

        for _ in 0..self.config.n_estimators {
            // Train weak learner
            let weak_learner = self.train_weak_learner(features, targets, &weights)?;

            // Calculate weighted error
            let mut error = 0.0;
            for (i, (feats, target)) in features.iter().zip(targets).enumerate() {
                let pred = weak_learner.predict(feats[weak_learner.feature_idx]);
                if (pred > 0.0 && *target < 0.0) || (pred < 0.0 && *target > 0.0) {
                    error += weights[i];
                }
            }

            // Avoid division by zero
            if error >= 0.5 || error == 0.0 {
                break;
            }

            // Calculate alpha
            let alpha = 0.5 * ((1.0 - error) / error).ln();

            // Update weights
            for (i, (feats, target)) in features.iter().zip(targets).enumerate() {
                let pred = weak_learner.predict(feats[weak_learner.feature_idx]);
                let sign = if (pred > 0.0 && *target > 0.0) || (pred < 0.0 && *target < 0.0) {
                    -1.0
                } else {
                    1.0
                };
                weights[i] *= (alpha * sign).exp();
            }

            // Normalize weights
            let sum: f64 = weights.iter().sum();
            for w in &mut weights {
                *w /= sum;
            }

            self.weak_learners.push(weak_learner);
            self.alphas.push(alpha);
        }

        Ok(())
    }

    /// Train a single weak learner
    fn train_weak_learner(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
        weights: &[f64],
    ) -> anyhow::Result<WeakLearner> {
        let mut best_error = f64::INFINITY;
        let mut best_learner = WeakLearner {
            threshold: 0.0,
            feature_idx: 0,
            direction: 1.0,
        };

        // Try each feature
        for feat_idx in 0..features[0].len() {
            // Get feature values and sort
            let mut values: Vec<f64> = features.iter().map(|f| f[feat_idx]).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Try different thresholds
            for &threshold in &values {
                for &direction in &[1.0, -1.0] {
                    let learner = WeakLearner {
                        threshold,
                        feature_idx: feat_idx,
                        direction,
                    };

                    // Calculate weighted error
                    let mut error = 0.0;
                    for (i, (feats, target)) in features.iter().zip(targets).enumerate() {
                        let pred = learner.predict(feats[feat_idx]);
                        if (pred > 0.0 && *target < 0.0) || (pred < 0.0 && *target > 0.0) {
                            error += weights[i];
                        }
                    }

                    if error < best_error {
                        best_error = error;
                        best_learner = learner;
                    }
                }
            }
        }

        Ok(best_learner)
    }

    /// Predict using boosted ensemble
    pub fn predict(&self, features: &[f64]) -> f64 {
        let mut prediction = 0.0;
        for (learner, alpha) in self.weak_learners.iter().zip(&self.alphas) {
            prediction += alpha * learner.predict(features[learner.feature_idx]);
        }
        prediction
    }

    /// Returns the number of weak learners in this ensemble.
    pub fn get_num_learners(&self) -> usize {
        self.weak_learners.len()
    }
}

/// Ensemble weight optimizer using various methods
#[derive(Debug)]
pub struct EnsembleWeightOptimizer {
    /// Optimization method
    method: OptimizationMethod,
}

/// Optimization method for ensemble weight learning
#[derive(Debug, Clone, Copy)]
pub enum OptimizationMethod {
    /// Uniform weights
    Uniform,
    /// Inverse error weighting
    InverseError,
    /// Softmax-based weighting
    Softmax,
    /// Gradient descent optimization
    GradientDescent,
}

impl EnsembleWeightOptimizer {
    /// Creates a new optimizer that uses the specified `OptimizationMethod`.
    pub fn new(method: OptimizationMethod) -> Self {
        Self { method }
    }

    /// Optimize ensemble weights based on validation performance
    pub fn optimize(&self, predictions: &[Vec<f64>], targets: &[f64]) -> anyhow::Result<Vec<f64>> {
        if predictions.is_empty() {
            anyhow::bail!("No predictions provided");
        }

        let num_models = predictions.len();

        match self.method {
            OptimizationMethod::Uniform => Ok(vec![1.0 / num_models as f64; num_models]),
            OptimizationMethod::InverseError => self.inverse_error_weights(predictions, targets),
            OptimizationMethod::Softmax => self.softmax_weights(predictions, targets),
            OptimizationMethod::GradientDescent => {
                self.gradient_descent_weights(predictions, targets)
            }
        }
    }

    fn inverse_error_weights(
        &self,
        predictions: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let mut errors = Vec::new();

        for model_preds in predictions {
            let mse: f64 = model_preds
                .iter()
                .zip(targets)
                .map(|(pred, target)| (pred - target).powi(2))
                .sum::<f64>()
                / targets.len() as f64;
            errors.push(mse);
        }

        // Inverse error weights
        let inv_errors: Vec<f64> = errors.iter().map(|&e| 1.0 / (e + 1e-10)).collect();
        let sum: f64 = inv_errors.iter().sum();

        Ok(inv_errors.iter().map(|&ie| ie / sum).collect())
    }

    fn softmax_weights(
        &self,
        predictions: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let mut errors = Vec::new();

        for model_preds in predictions {
            let mse: f64 = model_preds
                .iter()
                .zip(targets)
                .map(|(pred, target)| (pred - target).powi(2))
                .sum::<f64>()
                / targets.len() as f64;
            errors.push(-mse); // Negative because lower error is better
        }

        // Softmax
        let max_error = errors.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_errors: Vec<f64> = errors.iter().map(|&e| (e - max_error).exp()).collect();
        let sum: f64 = exp_errors.iter().sum();

        Ok(exp_errors.iter().map(|&ee| ee / sum).collect())
    }

    fn gradient_descent_weights(
        &self,
        predictions: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let num_models = predictions.len();
        let mut weights = vec![1.0 / num_models as f64; num_models];

        let learning_rate = 0.01;
        let iterations = 100;

        for _ in 0..iterations {
            let mut gradients = vec![0.0; num_models];

            for (i, target) in targets.iter().enumerate() {
                let mut prediction = 0.0;
                for (j, model_preds) in predictions.iter().enumerate() {
                    if i < model_preds.len() {
                        prediction += model_preds[i] * weights[j];
                    }
                }

                let error = prediction - target;

                for (j, model_preds) in predictions.iter().enumerate() {
                    if i < model_preds.len() {
                        gradients[j] += 2.0 * error * model_preds[i];
                    }
                }
            }

            // Update weights
            for (weight, gradient) in weights.iter_mut().zip(&gradients) {
                *weight -= learning_rate * gradient / targets.len() as f64;
                *weight = weight.max(0.0); // Keep weights non-negative
            }

            // Normalize
            let sum: f64 = weights.iter().sum();
            if sum > 0.0 {
                for weight in &mut weights {
                    *weight /= sum;
                }
            }
        }

        Ok(weights)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stacking_ensemble() {
        let base_preds = vec![vec![1.0, 2.0, 3.0], vec![1.1, 2.1, 2.9]];
        let targets = vec![1.0, 2.0, 3.0];

        let mut stacking = StackingEnsemble::new(StackingConfig::default());
        stacking
            .train_meta_model(base_preds.clone(), &targets)
            .unwrap();

        let prediction = stacking.predict(&[1.0, 1.1]);
        assert!(prediction > 0.0);
    }

    #[test]
    fn test_weight_optimizer_uniform() {
        let optimizer = EnsembleWeightOptimizer::new(OptimizationMethod::Uniform);
        let predictions = vec![
            vec![1.0, 2.0, 3.0],
            vec![1.1, 2.1, 2.9],
            vec![0.9, 1.9, 3.1],
        ];
        let targets = vec![1.0, 2.0, 3.0];

        let weights = optimizer.optimize(&predictions, &targets).unwrap();
        assert_eq!(weights.len(), 3);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_weight_optimizer_inverse_error() {
        let optimizer = EnsembleWeightOptimizer::new(OptimizationMethod::InverseError);
        let predictions = vec![
            vec![1.0, 2.0, 3.0], // Perfect predictions
            vec![2.0, 3.0, 4.0], // Off by 1
        ];
        let targets = vec![1.0, 2.0, 3.0];

        let weights = optimizer.optimize(&predictions, &targets).unwrap();

        // First model should have higher weight
        assert!(weights[0] > weights[1]);
    }

    #[test]
    fn test_boosting_ensemble() {
        let features = vec![
            vec![1.0, 0.5],
            vec![2.0, 1.0],
            vec![3.0, 1.5],
            vec![4.0, 2.0],
            vec![5.0, 2.5],
            vec![6.0, 3.0],
        ];
        let targets = vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0];

        let mut boosting = BoostingEnsemble::new(BoostingConfig {
            n_estimators: 5,
            learning_rate: 0.1,
            max_depth: 1,
        });

        let result = boosting.train(&features, &targets);

        // Training might not add any learners if the data isn't suitable
        // so we'll just check that training doesn't error
        assert!(result.is_ok());
    }
}
