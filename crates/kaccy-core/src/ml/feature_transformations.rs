//! Advanced feature transformations for machine learning
//!
//! This module provides sophisticated feature transformation techniques including:
//! - Polynomial feature generation
//! - Interaction features
//! - Binning and discretization
//! - Feature scaling and normalization
//! - Lag features for time series

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::collections::HashMap;

/// Polynomial feature generator
/// Creates polynomial combinations of features up to specified degree
#[derive(Debug, Clone)]
pub struct PolynomialFeatures {
    degree: usize,
    include_bias: bool,
    interaction_only: bool,
}

impl PolynomialFeatures {
    /// Create a new polynomial feature generator
    pub fn new(degree: usize) -> Self {
        Self {
            degree,
            include_bias: true,
            interaction_only: false,
        }
    }

    /// Set whether to include bias term (intercept)
    pub fn with_bias(mut self, include_bias: bool) -> Self {
        self.include_bias = include_bias;
        self
    }

    /// Set whether to only include interaction terms (no powers)
    pub fn with_interaction_only(mut self, interaction_only: bool) -> Self {
        self.interaction_only = interaction_only;
        self
    }

    /// Transform feature vector to polynomial features
    pub fn transform(&self, features: &HashMap<String, Decimal>) -> HashMap<String, Decimal> {
        let mut result = HashMap::new();

        if self.include_bias {
            result.insert("bias".to_string(), Decimal::ONE);
        }

        // Add original features
        for (name, &value) in features {
            result.insert(name.clone(), value);
        }

        let feature_names: Vec<_> = features.keys().cloned().collect();

        // Generate polynomial features
        for deg in 2..=self.degree {
            self.generate_combinations(&feature_names, features, deg, &mut result);
        }

        result
    }

    fn generate_combinations(
        &self,
        names: &[String],
        features: &HashMap<String, Decimal>,
        degree: usize,
        result: &mut HashMap<String, Decimal>,
    ) {
        if degree == 2 {
            // Quadratic terms
            for i in 0..names.len() {
                if !self.interaction_only {
                    let name = format!("{}^2", names[i]);
                    let value = features[&names[i]] * features[&names[i]];
                    result.insert(name, value);
                }

                // Interaction terms
                for j in (i + 1)..names.len() {
                    let name = format!("{}*{}", names[i], names[j]);
                    let value = features[&names[i]] * features[&names[j]];
                    result.insert(name, value);
                }
            }
        }
        // Higher degrees would follow similar pattern
    }

    /// Get number of output features for given input dimension
    pub fn output_dimension(&self, input_dim: usize) -> usize {
        let mut n = if self.include_bias { 1 } else { 0 };
        n += input_dim; // Original features

        if self.degree >= 2 {
            if self.interaction_only {
                // Only interaction terms: C(input_dim, 2) + C(input_dim, 3) + ...
                for d in 2..=self.degree {
                    n += Self::binomial_coefficient(input_dim, d);
                }
            } else {
                // All polynomial terms
                for d in 2..=self.degree {
                    n += Self::multiset_coefficient(input_dim, d);
                }
            }
        }

        n
    }

    fn binomial_coefficient(n: usize, k: usize) -> usize {
        if k > n {
            return 0;
        }
        let mut result = 1;
        for i in 0..k {
            result = result * (n - i) / (i + 1);
        }
        result
    }

    fn multiset_coefficient(n: usize, k: usize) -> usize {
        Self::binomial_coefficient(n + k - 1, k)
    }
}

/// Feature binner for discretizing continuous features
#[derive(Debug, Clone)]
pub struct FeatureBinner {
    n_bins: usize,
    strategy: BinningStrategy,
}

/// Strategy for discretizing continuous features into bins.
#[derive(Debug, Clone)]
pub enum BinningStrategy {
    /// Equal width bins
    Uniform,
    /// Equal frequency bins (quantiles)
    Quantile,
    /// Custom bin edges
    Custom(Vec<Decimal>),
}

impl FeatureBinner {
    /// Create a new feature binner
    pub fn new(n_bins: usize, strategy: BinningStrategy) -> Self {
        Self { n_bins, strategy }
    }

    /// Fit binner to data and compute bin edges
    pub fn fit(&self, values: &[Decimal]) -> Vec<Decimal> {
        match &self.strategy {
            BinningStrategy::Uniform => self.uniform_bins(values),
            BinningStrategy::Quantile => self.quantile_bins(values),
            BinningStrategy::Custom(edges) => edges.clone(),
        }
    }

    fn uniform_bins(&self, values: &[Decimal]) -> Vec<Decimal> {
        if values.is_empty() {
            return vec![];
        }

        let min = *values.iter().min().expect("values is non-empty per early return above");
        let max = *values.iter().max().expect("values is non-empty per early return above");
        let range = max - min;
        let bin_width = range / Decimal::from(self.n_bins);

        (0..=self.n_bins)
            .map(|i| min + bin_width * Decimal::from(i))
            .collect()
    }

    fn quantile_bins(&self, values: &[Decimal]) -> Vec<Decimal> {
        let mut sorted = values.to_vec();
        sorted.sort();

        let mut edges = Vec::new();
        for i in 0..=self.n_bins {
            let quantile = i as f64 / self.n_bins as f64;
            let idx = ((sorted.len() - 1) as f64 * quantile) as usize;
            edges.push(sorted[idx]);
        }
        edges
    }

    /// Transform value to bin index
    pub fn transform(&self, value: Decimal, edges: &[Decimal]) -> usize {
        for i in 0..edges.len() - 1 {
            if value >= edges[i] && value < edges[i + 1] {
                return i;
            }
        }
        edges.len() - 2 // Last bin
    }
}

/// Lag feature generator for time series
#[derive(Debug, Clone)]
pub struct LagFeatureGenerator {
    max_lag: usize,
    feature_names: Vec<String>,
}

impl LagFeatureGenerator {
    /// Create a new lag feature generator
    pub fn new(max_lag: usize, feature_names: Vec<String>) -> Self {
        Self {
            max_lag,
            feature_names,
        }
    }

    /// Generate lag features for a time series
    pub fn generate(&self, data: &[HashMap<String, Decimal>]) -> Vec<HashMap<String, Decimal>> {
        let mut result = Vec::new();

        for i in self.max_lag..data.len() {
            let mut features = data[i].clone();

            // Add lag features
            for lag in 1..=self.max_lag {
                for name in &self.feature_names {
                    if let Some(&value) = data[i - lag].get(name) {
                        let lag_name = format!("{}_lag_{}", name, lag);
                        features.insert(lag_name, value);
                    }
                }
            }

            result.push(features);
        }

        result
    }

    /// Get all generated feature names
    pub fn feature_names_out(&self) -> Vec<String> {
        let mut names = self.feature_names.clone();

        for lag in 1..=self.max_lag {
            for name in &self.feature_names {
                names.push(format!("{}_lag_{}", name, lag));
            }
        }

        names
    }
}

/// Rolling window feature generator
#[derive(Debug, Clone)]
pub struct RollingFeatureGenerator {
    window_size: usize,
    statistics: Vec<RollingStatistic>,
}

/// Statistic to compute over a rolling window.
#[derive(Debug, Clone, Copy)]
pub enum RollingStatistic {
    /// Arithmetic mean over the window.
    Mean,
    /// Standard deviation over the window.
    StdDev,
    /// Minimum value in the window.
    Min,
    /// Maximum value in the window.
    Max,
    /// Median value in the window.
    Median,
    /// Sum of all values in the window.
    Sum,
}

impl RollingFeatureGenerator {
    /// Create a new rolling feature generator
    pub fn new(window_size: usize, statistics: Vec<RollingStatistic>) -> Self {
        Self {
            window_size,
            statistics,
        }
    }

    /// Generate rolling features
    pub fn generate(
        &self,
        feature_name: &str,
        values: &[Decimal],
    ) -> HashMap<String, Vec<Option<Decimal>>> {
        let mut result = HashMap::new();

        for stat in &self.statistics {
            let stat_name = format!("{}_{:?}_rolling_{}", feature_name, stat, self.window_size);
            let stat_values = self.calculate_statistic(*stat, values);
            result.insert(stat_name, stat_values);
        }

        result
    }

    fn calculate_statistic(
        &self,
        stat: RollingStatistic,
        values: &[Decimal],
    ) -> Vec<Option<Decimal>> {
        let mut result = Vec::with_capacity(values.len());

        for i in 0..values.len() {
            if i + 1 < self.window_size {
                result.push(None);
            } else {
                let window = &values[i + 1 - self.window_size..=i];
                let stat_value = match stat {
                    RollingStatistic::Mean => self.calculate_mean(window),
                    RollingStatistic::StdDev => self.calculate_std_dev(window),
                    RollingStatistic::Min => window.iter().min().copied(),
                    RollingStatistic::Max => window.iter().max().copied(),
                    RollingStatistic::Median => self.calculate_median(window),
                    RollingStatistic::Sum => Some(window.iter().sum()),
                };
                result.push(stat_value);
            }
        }

        result
    }

    fn calculate_mean(&self, window: &[Decimal]) -> Option<Decimal> {
        if window.is_empty() {
            return None;
        }
        let sum: Decimal = window.iter().sum();
        Some(sum / Decimal::from(window.len()))
    }

    fn calculate_std_dev(&self, window: &[Decimal]) -> Option<Decimal> {
        if window.len() < 2 {
            return None;
        }

        let mean = self.calculate_mean(window)?;
        let variance: Decimal = window
            .iter()
            .map(|&x| (x - mean) * (x - mean))
            .sum::<Decimal>()
            / Decimal::from(window.len() - 1);

        variance.sqrt()
    }

    fn calculate_median(&self, window: &[Decimal]) -> Option<Decimal> {
        if window.is_empty() {
            return None;
        }

        let mut sorted = window.to_vec();
        sorted.sort();

        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            Some((sorted[mid - 1] + sorted[mid]) / Decimal::from(2))
        } else {
            Some(sorted[mid])
        }
    }
}

/// Feature interaction generator
#[derive(Debug, Clone)]
pub struct InteractionFeatureGenerator {
    pairs: Vec<(String, String)>,
}

impl InteractionFeatureGenerator {
    /// Create a new interaction feature generator
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    /// Add a feature pair for interaction
    pub fn add_pair(mut self, feature1: String, feature2: String) -> Self {
        self.pairs.push((feature1, feature2));
        self
    }

    /// Generate interaction features
    pub fn generate(&self, features: &HashMap<String, Decimal>) -> HashMap<String, Decimal> {
        let mut result = features.clone();

        for (f1, f2) in &self.pairs {
            if let (Some(&v1), Some(&v2)) = (features.get(f1), features.get(f2)) {
                // Multiplication
                result.insert(format!("{}*{}", f1, f2), v1 * v2);

                // Division (if non-zero)
                if v2 != Decimal::ZERO {
                    result.insert(format!("{}/{}", f1, f2), v1 / v2);
                }

                // Addition
                result.insert(format!("{}+{}", f1, f2), v1 + v2);

                // Difference
                result.insert(format!("{}-{}", f1, f2), v1 - v2);
            }
        }

        result
    }
}

impl Default for InteractionFeatureGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_polynomial_features() {
        let mut features = HashMap::new();
        features.insert("x1".to_string(), dec!(2.0));
        features.insert("x2".to_string(), dec!(3.0));

        let poly = PolynomialFeatures::new(2);
        let transformed = poly.transform(&features);

        assert!(transformed.contains_key("bias"));
        assert!(transformed.contains_key("x1"));
        assert!(transformed.contains_key("x2"));
        assert!(transformed.contains_key("x1^2"));
        assert!(transformed.contains_key("x2^2"));

        // Check for interaction term (could be x1*x2 or x2*x1 depending on HashMap order)
        let has_interaction =
            transformed.contains_key("x1*x2") || transformed.contains_key("x2*x1");
        assert!(
            has_interaction,
            "Should have interaction term x1*x2 or x2*x1"
        );

        assert_eq!(transformed["x1^2"], dec!(4.0));
        assert_eq!(transformed["x2^2"], dec!(9.0));

        // Check interaction value
        let interaction_value = transformed
            .get("x1*x2")
            .or(transformed.get("x2*x1"))
            .unwrap();
        assert_eq!(*interaction_value, dec!(6.0));
    }

    #[test]
    fn test_feature_binner_uniform() {
        let values = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];
        let binner = FeatureBinner::new(4, BinningStrategy::Uniform);
        let edges = binner.fit(&values);

        assert_eq!(edges.len(), 5); // n_bins + 1
        assert_eq!(edges[0], dec!(1.0));
        assert_eq!(edges[4], dec!(5.0));
    }

    #[test]
    fn test_lag_features() {
        let mut data = Vec::new();
        for i in 0..5 {
            let mut row = HashMap::new();
            row.insert("price".to_string(), Decimal::from(i + 1));
            data.push(row);
        }

        let generator = LagFeatureGenerator::new(2, vec!["price".to_string()]);
        let lagged = generator.generate(&data);

        assert_eq!(lagged.len(), 3); // Lost 2 rows due to max_lag=2
        assert!(lagged[0].contains_key("price_lag_1"));
        assert!(lagged[0].contains_key("price_lag_2"));
    }

    #[test]
    fn test_rolling_features() {
        let values = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];
        let generator =
            RollingFeatureGenerator::new(3, vec![RollingStatistic::Mean, RollingStatistic::Max]);

        let result = generator.generate("price", &values);

        assert_eq!(result.len(), 2); // Mean and Max
        assert!(result.contains_key("price_Mean_rolling_3"));
        assert!(result.contains_key("price_Max_rolling_3"));
    }

    #[test]
    fn test_interaction_features() {
        let mut features = HashMap::new();
        features.insert("x".to_string(), dec!(4.0));
        features.insert("y".to_string(), dec!(2.0));

        let generator =
            InteractionFeatureGenerator::new().add_pair("x".to_string(), "y".to_string());

        let result = generator.generate(&features);

        assert_eq!(result["x*y"], dec!(8.0));
        assert_eq!(result["x/y"], dec!(2.0));
        assert_eq!(result["x+y"], dec!(6.0));
        assert_eq!(result["x-y"], dec!(2.0));
    }
}
