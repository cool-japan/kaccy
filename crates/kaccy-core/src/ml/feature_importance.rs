//! Feature Importance Analysis
//!
//! Provides methods to analyze and rank feature importance for model interpretability.

use serde::{Deserialize, Serialize};

/// Feature importance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureImportance {
    /// Name of the feature.
    pub feature_name: String,
    /// Numeric importance score (higher is more important).
    pub importance_score: f64,
    /// Rank of the feature by importance (1 = most important).
    pub rank: usize,
}

/// Feature importance analyzer
#[derive(Debug)]
pub struct FeatureImportanceAnalyzer {
    /// Method to use for importance calculation
    method: ImportanceMethod,
}

/// Method used to compute feature importance scores
#[derive(Debug, Clone, Copy)]
pub enum ImportanceMethod {
    /// Permutation importance
    Permutation,
    /// Correlation-based importance
    Correlation,
    /// Variance-based importance
    Variance,
    /// SHAP-like values (simplified)
    ShapLike,
}

impl FeatureImportanceAnalyzer {
    /// Creates a new analyzer using the specified importance method.
    pub fn new(method: ImportanceMethod) -> Self {
        Self { method }
    }

    /// Calculate feature importance scores
    pub fn analyze(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
        feature_names: Option<&[String]>,
    ) -> anyhow::Result<Vec<FeatureImportance>> {
        if features.is_empty() || targets.is_empty() {
            anyhow::bail!("Empty data");
        }

        let num_features = features[0].len();

        let names: Vec<String> = if let Some(names) = feature_names {
            if names.len() != num_features {
                anyhow::bail!("Feature names length mismatch");
            }
            names.to_vec()
        } else {
            (0..num_features)
                .map(|i| format!("feature_{}", i))
                .collect()
        };

        let scores = match self.method {
            ImportanceMethod::Permutation => self.permutation_importance(features, targets)?,
            ImportanceMethod::Correlation => self.correlation_importance(features, targets)?,
            ImportanceMethod::Variance => self.variance_importance(features)?,
            ImportanceMethod::ShapLike => self.shap_like_importance(features, targets)?,
        };

        // Create importance objects with ranks
        let mut importances: Vec<_> = names
            .into_iter()
            .zip(scores)
            .map(|(name, score)| FeatureImportance {
                feature_name: name,
                importance_score: score,
                rank: 0,
            })
            .collect();

        // Sort by importance (descending) and assign ranks
        importances.sort_by(|a, b| b.importance_score.partial_cmp(&a.importance_score).unwrap());

        for (rank, importance) in importances.iter_mut().enumerate() {
            importance.rank = rank + 1;
        }

        Ok(importances)
    }

    /// Permutation importance: measure drop in performance when feature is shuffled
    fn permutation_importance(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let num_features = features[0].len();
        let baseline_score = self.calculate_score(features, targets)?;

        let mut importances = Vec::new();

        for feat_idx in 0..num_features {
            // Create permuted features
            let mut permuted = features.to_vec();
            self.permute_feature(&mut permuted, feat_idx);

            // Calculate score with permuted feature
            let permuted_score = self.calculate_score(&permuted, targets)?;

            // Importance is the drop in performance
            let importance = (baseline_score - permuted_score).abs();
            importances.push(importance);
        }

        Ok(importances)
    }

    /// Correlation-based importance
    fn correlation_importance(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let num_features = features[0].len();
        let mut importances = Vec::new();

        for feat_idx in 0..num_features {
            let feature_values: Vec<f64> = features.iter().map(|f| f[feat_idx]).collect();
            let correlation = self.correlation(&feature_values, targets).abs();
            importances.push(correlation);
        }

        Ok(importances)
    }

    /// Variance-based importance
    fn variance_importance(&self, features: &[Vec<f64>]) -> anyhow::Result<Vec<f64>> {
        let num_features = features[0].len();
        let mut importances = Vec::new();

        for feat_idx in 0..num_features {
            let feature_values: Vec<f64> = features.iter().map(|f| f[feat_idx]).collect();
            let variance = self.variance(&feature_values);
            importances.push(variance);
        }

        Ok(importances)
    }

    /// SHAP-like importance (simplified additive feature attribution)
    fn shap_like_importance(
        &self,
        features: &[Vec<f64>],
        targets: &[f64],
    ) -> anyhow::Result<Vec<f64>> {
        let num_features = features[0].len();
        let mut importances = vec![0.0; num_features];

        // For each sample, estimate marginal contribution of each feature
        for (sample_idx, sample) in features.iter().enumerate() {
            let target = targets[sample_idx];

            // Global mean as baseline
            let baseline = targets.iter().sum::<f64>() / targets.len() as f64;

            for feat_idx in 0..num_features {
                // Simplified: contribution is proportional to feature value weighted by correlation
                let feature_values: Vec<f64> = features.iter().map(|f| f[feat_idx]).collect();
                let correlation = self.correlation(&feature_values, targets);

                let contribution = (sample[feat_idx] - self.mean(&feature_values))
                    * correlation
                    * (target - baseline).signum();

                importances[feat_idx] += contribution.abs();
            }
        }

        // Normalize by number of samples
        for imp in &mut importances {
            *imp /= features.len() as f64;
        }

        Ok(importances)
    }

    /// Simple R-squared score
    fn calculate_score(&self, _features: &[Vec<f64>], targets: &[f64]) -> anyhow::Result<f64> {
        // Simple baseline: mean of targets
        let mean_target = targets.iter().sum::<f64>() / targets.len() as f64;

        // Total sum of squares
        let ss_tot: f64 = targets.iter().map(|&y| (y - mean_target).powi(2)).sum();

        // Residual sum of squares (using simple mean prediction as baseline)
        let ss_res: f64 = targets.iter().map(|&y| (y - mean_target).powi(2)).sum();

        // R-squared
        let r_squared = 1.0 - (ss_res / ss_tot);

        Ok(r_squared)
    }

    /// Permute a single feature
    fn permute_feature(&self, features: &mut [Vec<f64>], feature_idx: usize) {
        use std::collections::hash_map::RandomState;
        use std::hash::BuildHasher;

        let n = features.len();
        let hasher = RandomState::new();

        for i in 0..n {
            let j = (hasher.hash_one(i) as usize) % n;

            let temp = features[i][feature_idx];
            features[i][feature_idx] = features[j][feature_idx];
            features[j][feature_idx] = temp;
        }
    }

    /// Calculate correlation between two vectors
    fn correlation(&self, x: &[f64], y: &[f64]) -> f64 {
        if x.len() != y.len() || x.is_empty() {
            return 0.0;
        }

        let mean_x = self.mean(x);
        let mean_y = self.mean(y);

        let mut cov = 0.0;
        let mut var_x = 0.0;
        let mut var_y = 0.0;

        for (xi, yi) in x.iter().zip(y) {
            let dx = xi - mean_x;
            let dy = yi - mean_y;
            cov += dx * dy;
            var_x += dx * dx;
            var_y += dy * dy;
        }

        if var_x == 0.0 || var_y == 0.0 {
            return 0.0;
        }

        cov / (var_x * var_y).sqrt()
    }

    /// Calculate mean
    fn mean(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// Calculate variance
    fn variance(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let mean = self.mean(values);
        values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64
    }
}

/// Feature selection based on importance
#[derive(Debug)]
pub struct FeatureSelector {
    /// Minimum importance threshold
    threshold: f64,
    /// Maximum number of features to select
    max_features: Option<usize>,
}

impl FeatureSelector {
    /// Creates a new `FeatureSelector` with the given minimum importance threshold.
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            max_features: None,
        }
    }

    /// Sets an upper bound on the number of features selected.
    pub fn with_max_features(mut self, max_features: usize) -> Self {
        self.max_features = Some(max_features);
        self
    }

    /// Select features based on importance scores
    pub fn select(&self, importances: &[FeatureImportance]) -> Vec<usize> {
        let mut selected: Vec<_> = importances
            .iter()
            .enumerate()
            .filter(|(_, imp)| imp.importance_score >= self.threshold)
            .map(|(idx, _)| idx)
            .collect();

        // Limit to max_features if specified
        if let Some(max) = self.max_features {
            selected.truncate(max);
        }

        selected
    }

    /// Get feature indices to keep
    pub fn get_feature_mask(&self, importances: &[FeatureImportance]) -> Vec<bool> {
        let selected = self.select(importances);
        let mut mask = vec![false; importances.len()];
        for idx in selected {
            mask[idx] = true;
        }
        mask
    }
}

/// Feature importance visualization helper
#[derive(Debug)]
pub struct ImportanceVisualizer;

impl ImportanceVisualizer {
    /// Generate a text-based bar chart of feature importances
    #[allow(dead_code)]
    pub fn text_chart(importances: &[FeatureImportance], max_width: usize) -> String {
        let max_score = importances
            .iter()
            .map(|i| i.importance_score)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(1.0);

        let mut chart = String::new();

        for imp in importances {
            let bar_width = ((imp.importance_score / max_score) * max_width as f64) as usize;
            let bar = "█".repeat(bar_width);

            chart.push_str(&format!(
                "{:20} | {} {:.4}\n",
                imp.feature_name, bar, imp.importance_score
            ));
        }

        chart
    }

    /// Get top-N most important features
    pub fn top_n(importances: &[FeatureImportance], n: usize) -> Vec<FeatureImportance> {
        let mut sorted = importances.to_vec();
        sorted.sort_by(|a, b| b.importance_score.partial_cmp(&a.importance_score).unwrap());
        sorted.truncate(n);
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> (Vec<Vec<f64>>, Vec<f64>) {
        // Create synthetic data where feature 0 is highly correlated with target
        let features = vec![
            vec![1.0, 10.0, 5.0],
            vec![2.0, 11.0, 6.0],
            vec![3.0, 9.0, 4.0],
            vec![4.0, 12.0, 7.0],
            vec![5.0, 8.0, 3.0],
        ];
        let targets = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // Strongly correlated with feature 0

        (features, targets)
    }

    #[test]
    fn test_correlation_importance() {
        let (features, targets) = create_test_data();

        let analyzer = FeatureImportanceAnalyzer::new(ImportanceMethod::Correlation);
        let importances = analyzer.analyze(&features, &targets, None).unwrap();

        assert_eq!(importances.len(), 3);

        // Feature 0 should have highest importance
        assert_eq!(importances[0].feature_name, "feature_0");
        assert!(importances[0].importance_score > importances[1].importance_score);
    }

    #[test]
    fn test_variance_importance() {
        let (features, targets) = create_test_data();

        let analyzer = FeatureImportanceAnalyzer::new(ImportanceMethod::Variance);
        let importances = analyzer.analyze(&features, &targets, None).unwrap();

        assert_eq!(importances.len(), 3);

        // All features should have some variance
        for imp in &importances {
            assert!(imp.importance_score > 0.0);
        }
    }

    #[test]
    fn test_feature_selector() {
        let importances = vec![
            FeatureImportance {
                feature_name: "f1".to_string(),
                importance_score: 0.8,
                rank: 1,
            },
            FeatureImportance {
                feature_name: "f2".to_string(),
                importance_score: 0.3,
                rank: 2,
            },
            FeatureImportance {
                feature_name: "f3".to_string(),
                importance_score: 0.1,
                rank: 3,
            },
        ];

        let selector = FeatureSelector::new(0.5);
        let selected = selector.select(&importances);

        assert_eq!(selected.len(), 1); // Only f1 exceeds threshold
        assert_eq!(selected[0], 0);
    }

    #[test]
    fn test_feature_selector_max_features() {
        let importances = vec![
            FeatureImportance {
                feature_name: "f1".to_string(),
                importance_score: 0.8,
                rank: 1,
            },
            FeatureImportance {
                feature_name: "f2".to_string(),
                importance_score: 0.6,
                rank: 2,
            },
            FeatureImportance {
                feature_name: "f3".to_string(),
                importance_score: 0.4,
                rank: 3,
            },
        ];

        let selector = FeatureSelector::new(0.0).with_max_features(2);
        let selected = selector.select(&importances);

        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn test_top_n() {
        let importances = vec![
            FeatureImportance {
                feature_name: "f1".to_string(),
                importance_score: 0.5,
                rank: 2,
            },
            FeatureImportance {
                feature_name: "f2".to_string(),
                importance_score: 0.8,
                rank: 1,
            },
            FeatureImportance {
                feature_name: "f3".to_string(),
                importance_score: 0.1,
                rank: 3,
            },
        ];

        let top = ImportanceVisualizer::top_n(&importances, 2);

        assert_eq!(top.len(), 2);
        assert_eq!(top[0].feature_name, "f2");
        assert_eq!(top[1].feature_name, "f1");
    }
}
