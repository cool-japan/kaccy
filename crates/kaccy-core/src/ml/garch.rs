//! GARCH (Generalized Autoregressive Conditional Heteroskedasticity) models
//!
//! This module provides volatility modeling with volatility clustering detection,
//! asymmetric GARCH (GJR-GARCH), and multivariate GARCH capabilities.

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// GARCH model parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GarchParams {
    /// Constant term (omega)
    pub omega: Decimal,
    /// ARCH coefficient (alpha)
    pub alpha: Decimal,
    /// GARCH coefficient (beta)
    pub beta: Decimal,
}

impl GarchParams {
    /// Create new GARCH parameters
    pub fn new(omega: Decimal, alpha: Decimal, beta: Decimal) -> Self {
        Self { omega, alpha, beta }
    }

    /// Validate parameters
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.omega <= dec!(0) {
            return Err(CoreError::Validation("Omega must be positive".to_string()));
        }
        if self.alpha < dec!(0) || self.beta < dec!(0) {
            return Err(CoreError::Validation(
                "Alpha and beta must be non-negative".to_string(),
            ));
        }
        if self.alpha + self.beta >= dec!(1) {
            return Err(CoreError::Validation(
                "Alpha + beta must be less than 1 for stationarity".to_string(),
            ));
        }
        Ok(())
    }

    /// Create default parameters
    pub fn default_params() -> Self {
        Self {
            omega: dec!(0.00001),
            alpha: dec!(0.1),
            beta: dec!(0.85),
        }
    }
}

/// GJR-GARCH parameters (asymmetric GARCH)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GjrGarchParams {
    /// Base GARCH parameters
    pub garch: GarchParams,
    /// Leverage effect coefficient (gamma)
    pub gamma: Decimal,
}

impl GjrGarchParams {
    /// Create new GJR-GARCH parameters
    pub fn new(omega: Decimal, alpha: Decimal, beta: Decimal, gamma: Decimal) -> Self {
        Self {
            garch: GarchParams::new(omega, alpha, beta),
            gamma,
        }
    }

    /// Validate parameters
    pub fn validate(&self) -> Result<(), CoreError> {
        self.garch.validate()?;
        if self.gamma < dec!(0) {
            return Err(CoreError::Validation(
                "Gamma must be non-negative".to_string(),
            ));
        }
        Ok(())
    }
}

/// Volatility clustering result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityClustering {
    /// Detected clusters of high volatility periods
    pub clusters: Vec<VolatilityCluster>,
    /// Average cluster duration
    pub avg_cluster_duration: usize,
    /// Clustering coefficient
    pub clustering_coefficient: Decimal,
}

/// A single volatility cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityCluster {
    /// Start index of cluster
    pub start_idx: usize,
    /// End index of cluster
    pub end_idx: usize,
    /// Average volatility in cluster
    pub avg_volatility: Decimal,
    /// Peak volatility in cluster
    pub peak_volatility: Decimal,
}

impl VolatilityCluster {
    /// Get cluster duration
    pub fn duration(&self) -> usize {
        self.end_idx.saturating_sub(self.start_idx) + 1
    }
}

/// GARCH(1,1) model
#[derive(Debug, Clone)]
pub struct GarchModel {
    params: GarchParams,
    fitted: bool,
    conditional_variance: Vec<Decimal>,
}

impl GarchModel {
    /// Create a new GARCH model
    pub fn new(params: GarchParams) -> Result<Self, CoreError> {
        params.validate()?;
        Ok(Self {
            params,
            fitted: false,
            conditional_variance: Vec::new(),
        })
    }

    /// Fit the GARCH model to returns data
    pub fn fit(&mut self, returns: &[Decimal]) -> Result<(), CoreError> {
        if returns.len() < 10 {
            return Err(CoreError::Validation(
                "Insufficient data for GARCH fitting".to_string(),
            ));
        }

        // Initialize conditional variance
        let mut variance = Vec::with_capacity(returns.len());

        // Initial variance (unconditional variance estimate)
        let mean: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        let initial_var: Decimal = returns
            .iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        variance.push(initial_var);

        // Compute conditional variance using GARCH(1,1) recursion
        // h_t = omega + alpha * epsilon_{t-1}^2 + beta * h_{t-1}
        for t in 1..returns.len() {
            let prev_shock = returns[t - 1] * returns[t - 1];
            let prev_variance = variance[t - 1];

            let h_t = self.params.omega
                + self.params.alpha * prev_shock
                + self.params.beta * prev_variance;

            variance.push(h_t.max(dec!(0.000001))); // Ensure positive variance
        }

        self.conditional_variance = variance;
        self.fitted = true;

        Ok(())
    }

    /// Forecast volatility for next n periods
    pub fn forecast(&self, steps: usize) -> Result<Vec<Decimal>, CoreError> {
        if !self.fitted {
            return Err(CoreError::Validation("Model not fitted".to_string()));
        }

        if self.conditional_variance.is_empty() {
            return Err(CoreError::Validation("No variance history".to_string()));
        }

        let mut forecasts = Vec::with_capacity(steps);
        let last_variance = self.conditional_variance[self.conditional_variance.len() - 1];

        // Multi-step ahead volatility forecast
        // h_{t+k} = omega + (alpha + beta) * h_{t+k-1}
        let persistence = self.params.alpha + self.params.beta;
        let long_run_var = self.params.omega / (dec!(1) - persistence).max(dec!(0.001));

        let mut h = last_variance;
        for _ in 0..steps {
            h = self.params.omega + persistence * h;
            // Converges to long-run variance
            h = h * dec!(0.95) + long_run_var * dec!(0.05);
            forecasts.push(h);
        }

        Ok(forecasts)
    }

    /// Get conditional variance (volatility squared)
    pub fn conditional_variance(&self) -> &[Decimal] {
        &self.conditional_variance
    }

    /// Get conditional volatility (standard deviation)
    pub fn conditional_volatility(&self) -> Vec<Decimal> {
        self.conditional_variance
            .iter()
            .map(|v| v.sqrt().unwrap_or(dec!(0)))
            .collect()
    }

    /// Detect volatility clustering
    pub fn detect_clustering(
        &self,
        threshold_multiplier: Decimal,
    ) -> Result<VolatilityClustering, CoreError> {
        if !self.fitted {
            return Err(CoreError::Validation("Model not fitted".to_string()));
        }

        let volatility = self.conditional_volatility();
        let mean_vol: Decimal =
            volatility.iter().sum::<Decimal>() / Decimal::from(volatility.len());
        let threshold = mean_vol * threshold_multiplier;

        let mut clusters = Vec::new();
        let mut in_cluster = false;
        let mut cluster_start = 0;
        let mut cluster_sum = dec!(0);
        let mut cluster_peak = dec!(0);

        for (i, vol) in volatility.iter().enumerate() {
            if *vol > threshold {
                if !in_cluster {
                    // Start new cluster
                    in_cluster = true;
                    cluster_start = i;
                    cluster_sum = *vol;
                    cluster_peak = *vol;
                } else {
                    // Continue cluster
                    cluster_sum += *vol;
                    cluster_peak = cluster_peak.max(*vol);
                }
            } else if in_cluster {
                // End cluster
                in_cluster = false;
                let duration = i - cluster_start;
                clusters.push(VolatilityCluster {
                    start_idx: cluster_start,
                    end_idx: i - 1,
                    avg_volatility: cluster_sum / Decimal::from(duration),
                    peak_volatility: cluster_peak,
                });
            }
        }

        // Handle cluster at end
        if in_cluster {
            let duration = volatility.len() - cluster_start;
            clusters.push(VolatilityCluster {
                start_idx: cluster_start,
                end_idx: volatility.len() - 1,
                avg_volatility: cluster_sum / Decimal::from(duration),
                peak_volatility: cluster_peak,
            });
        }

        let avg_duration = if !clusters.is_empty() {
            clusters.iter().map(|c| c.duration()).sum::<usize>() / clusters.len()
        } else {
            0
        };

        // Clustering coefficient: ratio of clustered periods to total periods
        let clustered_periods: usize = clusters.iter().map(|c| c.duration()).sum();
        let clustering_coefficient =
            Decimal::from(clustered_periods) / Decimal::from(volatility.len()).max(dec!(1));

        Ok(VolatilityClustering {
            clusters,
            avg_cluster_duration: avg_duration,
            clustering_coefficient,
        })
    }
}

/// GJR-GARCH model (asymmetric GARCH)
#[derive(Debug, Clone)]
pub struct GjrGarchModel {
    params: GjrGarchParams,
    fitted: bool,
    conditional_variance: Vec<Decimal>,
}

impl GjrGarchModel {
    /// Create a new GJR-GARCH model
    pub fn new(params: GjrGarchParams) -> Result<Self, CoreError> {
        params.validate()?;
        Ok(Self {
            params,
            fitted: false,
            conditional_variance: Vec::new(),
        })
    }

    /// Fit the GJR-GARCH model to returns data
    pub fn fit(&mut self, returns: &[Decimal]) -> Result<(), CoreError> {
        if returns.len() < 10 {
            return Err(CoreError::Validation(
                "Insufficient data for GJR-GARCH fitting".to_string(),
            ));
        }

        let mut variance = Vec::with_capacity(returns.len());

        // Initial variance
        let mean: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        let initial_var: Decimal = returns
            .iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        variance.push(initial_var);

        // GJR-GARCH recursion
        // h_t = omega + (alpha + gamma * I_{t-1}) * epsilon_{t-1}^2 + beta * h_{t-1}
        // where I_{t-1} = 1 if epsilon_{t-1} < 0, else 0 (leverage effect)
        for t in 1..returns.len() {
            let prev_return = returns[t - 1];
            let prev_shock = prev_return * prev_return;
            let prev_variance = variance[t - 1];

            // Leverage indicator: 1 if negative return (bad news)
            let leverage = if prev_return < dec!(0) {
                dec!(1)
            } else {
                dec!(0)
            };

            let h_t = self.params.garch.omega
                + (self.params.garch.alpha + self.params.gamma * leverage) * prev_shock
                + self.params.garch.beta * prev_variance;

            variance.push(h_t.max(dec!(0.000001)));
        }

        self.conditional_variance = variance;
        self.fitted = true;

        Ok(())
    }

    /// Forecast volatility
    pub fn forecast(&self, steps: usize) -> Result<Vec<Decimal>, CoreError> {
        if !self.fitted {
            return Err(CoreError::Validation("Model not fitted".to_string()));
        }

        let mut forecasts = Vec::with_capacity(steps);
        let last_variance = self.conditional_variance[self.conditional_variance.len() - 1];

        // Simplified multi-step forecast (assumes average leverage effect)
        let avg_leverage = dec!(0.5); // Assume 50% chance of negative return
        let persistence =
            self.params.garch.alpha + self.params.gamma * avg_leverage + self.params.garch.beta;

        let long_run_var = self.params.garch.omega / (dec!(1) - persistence).max(dec!(0.001));

        let mut h = last_variance;
        for _ in 0..steps {
            h = self.params.garch.omega + persistence * h;
            h = h * dec!(0.95) + long_run_var * dec!(0.05);
            forecasts.push(h);
        }

        Ok(forecasts)
    }

    /// Get conditional volatility
    pub fn conditional_volatility(&self) -> Vec<Decimal> {
        self.conditional_variance
            .iter()
            .map(|v| v.sqrt().unwrap_or(dec!(0)))
            .collect()
    }
}

/// Simplified multivariate GARCH (diagonal BEKK model)
#[derive(Debug, Clone)]
pub struct MultivariateGarch {
    /// Number of assets
    n_assets: usize,
    /// Individual GARCH models for each asset
    models: Vec<GarchModel>,
    /// Correlation matrix (simplified)
    #[allow(dead_code)]
    correlation: Vec<Vec<Decimal>>,
}

impl MultivariateGarch {
    /// Create a new multivariate GARCH model
    pub fn new(n_assets: usize) -> Self {
        let models = (0..n_assets)
            .map(|_| {
                GarchModel::new(GarchParams::default_params())
                    .expect("default GARCH parameters are valid")
            })
            .collect();

        let correlation = vec![vec![dec!(0); n_assets]; n_assets];

        Self {
            n_assets,
            models,
            correlation,
        }
    }

    /// Fit to multivariate returns data
    pub fn fit(&mut self, returns: &[Vec<Decimal>]) -> Result<(), CoreError> {
        if returns.is_empty() {
            return Err(CoreError::Validation("No data provided".to_string()));
        }

        if returns[0].len() != self.n_assets {
            return Err(CoreError::Validation("Data dimension mismatch".to_string()));
        }

        // Fit individual GARCH models
        for i in 0..self.n_assets {
            let asset_returns: Vec<Decimal> = returns.iter().map(|r| r[i]).collect();
            self.models[i].fit(&asset_returns)?;
        }

        Ok(())
    }

    /// Forecast covariance matrix
    pub fn forecast_covariance(&self, steps: usize) -> Result<Vec<Vec<Decimal>>, CoreError> {
        // Simplified: forecast variances, use constant correlations
        let mut forecasts = Vec::new();

        for model in &self.models {
            let var_forecast = model.forecast(steps)?;
            forecasts.push(var_forecast);
        }

        // Return variance matrix at step 0 (simplified)
        let mut cov_matrix = vec![vec![dec!(0); self.n_assets]; self.n_assets];
        for i in 0..self.n_assets {
            cov_matrix[i][i] = forecasts[i][0];
        }

        Ok(cov_matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_garch_params_validation() {
        let params = GarchParams::new(dec!(0.00001), dec!(0.1), dec!(0.85));
        assert!(params.validate().is_ok());

        let invalid = GarchParams::new(dec!(0.00001), dec!(0.5), dec!(0.6));
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_gjr_garch_params() {
        let params = GjrGarchParams::new(dec!(0.00001), dec!(0.1), dec!(0.85), dec!(0.05));
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_garch_fit_and_forecast() {
        let returns: Vec<Decimal> = (0..100)
            .map(|i| if i % 10 < 5 { dec!(0.01) } else { dec!(-0.01) })
            .collect();

        let params = GarchParams::default_params();
        let mut model = GarchModel::new(params).unwrap();

        assert!(model.fit(&returns).is_ok());
        assert!(model.fitted);

        let forecast = model.forecast(5).unwrap();
        assert_eq!(forecast.len(), 5);
        assert!(forecast[0] > dec!(0));
    }

    #[test]
    fn test_volatility_clustering_detection() {
        let returns: Vec<Decimal> = (0..100)
            .map(|i| {
                if (20..40).contains(&i) {
                    dec!(0.05) // High volatility period
                } else {
                    dec!(0.001)
                }
            })
            .collect();

        let params = GarchParams::default_params();
        let mut model = GarchModel::new(params).unwrap();
        model.fit(&returns).unwrap();

        let clustering = model.detect_clustering(dec!(1.5)).unwrap();
        assert!(!clustering.clusters.is_empty());
    }

    #[test]
    fn test_gjr_garch_fit() {
        let returns: Vec<Decimal> = (0..50)
            .map(|i| if i % 2 == 0 { dec!(0.01) } else { dec!(-0.015) })
            .collect();

        let params = GjrGarchParams::new(dec!(0.00001), dec!(0.08), dec!(0.85), dec!(0.05));
        let mut model = GjrGarchModel::new(params).unwrap();

        assert!(model.fit(&returns).is_ok());

        let volatility = model.conditional_volatility();
        assert_eq!(volatility.len(), returns.len());
    }

    #[test]
    fn test_multivariate_garch() {
        let returns: Vec<Vec<Decimal>> = (0..30)
            .map(|i| {
                vec![
                    Decimal::from(i % 5) * dec!(0.001),
                    Decimal::from((i + 1) % 5) * dec!(0.001),
                ]
            })
            .collect();

        let mut mv_garch = MultivariateGarch::new(2);
        assert!(mv_garch.fit(&returns).is_ok());

        let cov = mv_garch.forecast_covariance(1).unwrap();
        assert_eq!(cov.len(), 2);
        assert_eq!(cov[0].len(), 2);
    }

    #[test]
    fn test_insufficient_data() {
        let returns = vec![dec!(0.01)];
        let params = GarchParams::default_params();
        let mut model = GarchModel::new(params).unwrap();

        assert!(model.fit(&returns).is_err());
    }

    #[test]
    fn test_volatility_cluster_duration() {
        let cluster = VolatilityCluster {
            start_idx: 10,
            end_idx: 20,
            avg_volatility: dec!(0.05),
            peak_volatility: dec!(0.08),
        };

        assert_eq!(cluster.duration(), 11);
    }
}
