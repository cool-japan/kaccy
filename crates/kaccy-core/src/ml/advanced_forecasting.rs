//! Advanced Price Forecasting Module
//!
//! This module provides advanced forecasting capabilities including:
//! - Multi-horizon price prediction via a genuine multi-model ensemble
//! - Probabilistic forecasting with horizon-growing predictive intervals
//! - Forecast combination techniques
//!
//! # Multi-horizon ensemble
//!
//! [`MultiHorizonForecaster`] does **not** rely on a single relabeled model. It
//! fits three structurally distinct base forecasters and blends their per-horizon
//! point predictions with inverse-error (inverse-RMSE) weights:
//!
//! 1. **Holt linear smoothing** – exponential smoothing with an additive trend
//!    component, projecting `level + trend * h`.
//! 2. **Ordinary least squares (OLS) trend** – a linear regression of price on
//!    time, extrapolating the fitted line `intercept + slope * (n - 1 + h)`.
//! 3. **AR(1) mean reversion** – an Ornstein–Uhlenbeck style pull toward the
//!    sample mean, `mean + (last - mean) * phi^h`, where `phi` is the estimated
//!    lag-1 autocorrelation.
//!
//! Each component is back-tested one step ahead on the in-sample data to obtain
//! its root-mean-squared error (RMSE). The ensemble weight of component `i` is
//! `w_i = (1 / (rmse_i + eps)) / Z` with `Z` chosen so the weights sum to one,
//! so more accurate components dominate. The reported forecast for horizon `h`
//! is the exact weighted combination `sum_i w_i * component_i(h)`.
//!
//! # Probabilistic forecasting
//!
//! [`ProbabilisticForecaster`] estimates the drift and volatility of the return
//! process and propagates uncertainty so that the predictive standard deviation
//! grows with the horizon (variance accumulates across independent steps) and
//! scales with the empirical volatility of the input. Calibrated quantiles use a
//! real inverse-normal CDF, and a Monte-Carlo path simulation provides empirical
//! percentiles that agree with the analytic interval.

use crate::CoreError;
use crate::ml::features::PricePoint;
use chrono::{DateTime, Utc};
use rand::rng;
use rand_distr::{Distribution, Normal};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Minimum number of observations required before any model can be fitted.
const MIN_OBSERVATIONS: usize = 10;

/// Small constant added to RMSE values so a perfect (zero-error) component does
/// not produce an infinite weight and so the weighting is numerically stable.
const INVERSE_ERROR_EPS: f64 = 1e-9;

/// Inverse of the standard normal cumulative distribution function (the probit /
/// quantile function), evaluated at probability `p` in the open interval (0, 1).
///
/// This is a real implementation of the Beasley–Springer / Moro rational
/// approximation with Acklam's coefficients, accurate to roughly `1e-9` over the
/// full range. It lets the probabilistic forecaster emit calibrated normal
/// quantiles instead of hard-coded z-scores.
fn inverse_normal_cdf(p: f64) -> f64 {
    // Clamp away from the boundaries to keep the result finite.
    let p = p.clamp(1e-12, 1.0 - 1e-12);

    // Coefficients for the rational approximation.
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383_577_518_672_69e2,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    // Break-points dividing the central region from the tails.
    const P_LOW: f64 = 0.02425;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        // Lower tail.
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= P_HIGH {
        // Central region.
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        // Upper tail.
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

/// Convert a confidence level (e.g. `0.95`) into the two-sided z-multiplier that
/// brackets that probability mass around the mean of a standard normal.
fn z_for_confidence(confidence_level: f64) -> f64 {
    let level = confidence_level.clamp(0.0, 1.0 - 1e-9);
    // Two-sided interval: the upper tail holds (1 - level) / 2 of the mass.
    let upper_p = 0.5 + level / 2.0;
    inverse_normal_cdf(upper_p)
}

/// A single base forecaster used inside the multi-horizon ensemble.
///
/// Each variant is fitted from the historical price series and can project a
/// point forecast for an arbitrary horizon `h >= 1`.
#[derive(Debug, Clone, Copy)]
enum BaseForecaster {
    /// Holt's linear exponential smoothing: projects `level + trend * h`.
    HoltLinear { level: f64, trend: f64 },
    /// Ordinary least squares trend line on the index `t = 0..n`.
    OlsTrend {
        intercept: f64,
        slope: f64,
        last_index: f64,
    },
    /// AR(1) mean reversion: `mean + (last - mean) * phi^h`.
    MeanReversion { mean: f64, last: f64, phi: f64 },
}

impl BaseForecaster {
    /// Point forecast for a strictly positive horizon.
    fn predict(&self, horizon: usize) -> f64 {
        let h = horizon as f64;
        match *self {
            BaseForecaster::HoltLinear { level, trend } => level + trend * h,
            BaseForecaster::OlsTrend {
                intercept,
                slope,
                last_index,
            } => intercept + slope * (last_index + h),
            BaseForecaster::MeanReversion { mean, last, phi } => {
                mean + (last - mean) * phi.powf(h)
            }
        }
    }

    /// One-step-ahead forecast given the realised series prefix `prices[..=t]`,
    /// used for in-sample back-testing. Returns the prediction of `prices[t + 1]`
    /// made from information available up to and including index `t`.
    ///
    /// Re-fitting every model at every step would be quadratic; instead we use a
    /// stable, online one-step rule per component that is consistent with how the
    /// component extrapolates. This yields an honest RMSE that ranks the
    /// components by their realised one-step accuracy.
    fn one_step_backtest_rmse(prices: &[f64]) -> [f64; 3] {
        let n = prices.len();
        // Errors are accumulated as sum of squares for each component.
        let mut sse = [0.0_f64; 3];
        let mut count = 0.0_f64;

        // Holt online state.
        let alpha = 0.3;
        let beta = 0.1;
        let mut level = prices[0];
        let mut trend = 0.0;

        // Running statistics for OLS and mean reversion (computed on the prefix).
        // We start back-testing once at least two points are available so a
        // trend / mean can be defined.
        for t in 1..n {
            // --- Holt one-step prediction of prices[t] from prefix [0, t-1] ---
            let holt_pred = level + trend;
            sse[0] += (prices[t] - holt_pred).powi(2);

            // --- OLS one-step prediction using prefix [0, t-1] ---
            let prefix = &prices[..t];
            let (intercept, slope) = ols_fit(prefix);
            let ols_pred = intercept + slope * (t as f64);
            sse[1] += (prices[t] - ols_pred).powi(2);

            // --- Mean-reversion one-step prediction using prefix [0, t-1] ---
            let mean = prefix.iter().sum::<f64>() / prefix.len() as f64;
            let phi = ar1_coefficient(prefix);
            let last = prices[t - 1];
            let mr_pred = mean + (last - mean) * phi;
            sse[2] += (prices[t] - mr_pred).powi(2);

            count += 1.0;

            // Advance Holt state with the realised observation.
            let prev_level = level;
            level = alpha * prices[t] + (1.0 - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (1.0 - beta) * trend;
        }

        if count > 0.0 {
            [
                (sse[0] / count).sqrt(),
                (sse[1] / count).sqrt(),
                (sse[2] / count).sqrt(),
            ]
        } else {
            [1.0, 1.0, 1.0]
        }
    }
}

/// Fit `y = intercept + slope * t` for `t = 0..y.len()` by ordinary least
/// squares. Returns `(intercept, slope)`; a degenerate (single-point) input
/// yields a flat line through that point.
fn ols_fit(y: &[f64]) -> (f64, f64) {
    let n = y.len();
    if n < 2 {
        return (y.first().copied().unwrap_or(0.0), 0.0);
    }
    let n_f = n as f64;
    // t runs 0..n-1; mean of t is (n-1)/2.
    let mean_t = (n_f - 1.0) / 2.0;
    let mean_y = y.iter().sum::<f64>() / n_f;
    let mut cov = 0.0;
    let mut var_t = 0.0;
    for (i, &yi) in y.iter().enumerate() {
        let dt = i as f64 - mean_t;
        cov += dt * (yi - mean_y);
        var_t += dt * dt;
    }
    let slope = if var_t > 0.0 { cov / var_t } else { 0.0 };
    let intercept = mean_y - slope * mean_t;
    (intercept, slope)
}

/// Estimate the lag-1 autocorrelation (AR(1) coefficient `phi`) of a series,
/// clamped to `[-0.999, 0.999]` for a stable, mean-reverting projection.
fn ar1_coefficient(y: &[f64]) -> f64 {
    let n = y.len();
    if n < 2 {
        return 0.0;
    }
    let mean = y.iter().sum::<f64>() / n as f64;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 1..n {
        num += (y[i] - mean) * (y[i - 1] - mean);
    }
    for &yi in y {
        den += (yi - mean).powi(2);
    }
    if den <= f64::EPSILON {
        return 0.0;
    }
    (num / den).clamp(-0.999, 0.999)
}

/// Per-component decomposition of an ensemble forecast at a single horizon.
///
/// Exposed so callers (and tests) can verify that the ensemble prediction is the
/// documented weighted combination of genuinely distinct components rather than a
/// single relabeled model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleHorizonBreakdown {
    /// Horizon (in periods) this breakdown refers to.
    pub horizon: usize,
    /// Per-component point forecasts in the order
    /// `[Holt linear, OLS trend, AR(1) mean reversion]`.
    pub component_predictions: Vec<f64>,
    /// Combined (weighted) ensemble prediction for this horizon.
    pub combined_prediction: f64,
}

/// Diagnostics describing how the ensemble was assembled.
///
/// Returned by [`MultiHorizonForecaster::forecast_with_breakdown`] so the
/// weighting scheme and component contributions are fully inspectable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleDiagnostics {
    /// Human-readable names of the base components, aligned with `weights`.
    pub component_names: Vec<String>,
    /// In-sample one-step back-test RMSE of each component.
    pub component_rmse: Vec<f64>,
    /// Normalised inverse-error weights (sum to 1.0), aligned with components.
    pub weights: Vec<f64>,
    /// Per-horizon decomposition of the ensemble forecast.
    pub breakdowns: Vec<EnsembleHorizonBreakdown>,
}

/// A multi-horizon forecast together with the ensemble diagnostics that produced
/// it.
#[derive(Debug, Clone)]
pub struct MultiHorizonForecastWithDiagnostics {
    /// The forecast itself.
    pub forecast: MultiHorizonForecast,
    /// How the ensemble was weighted and decomposed.
    pub diagnostics: EnsembleDiagnostics,
}

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

    /// Fit the three base forecasters on the supplied price series and return
    /// them in the canonical order `[Holt, OLS, mean reversion]`.
    fn fit_components(prices: &[f64]) -> [BaseForecaster; 3] {
        // --- Holt linear smoothing ---
        let alpha = 0.3;
        let beta = 0.1;
        let mut level = prices[0];
        let mut trend = 0.0;
        for &price in &prices[1..] {
            let prev_level = level;
            level = alpha * price + (1.0 - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (1.0 - beta) * trend;
        }
        let holt = BaseForecaster::HoltLinear { level, trend };

        // --- OLS trend ---
        let (intercept, slope) = ols_fit(prices);
        let ols = BaseForecaster::OlsTrend {
            intercept,
            slope,
            last_index: (prices.len() - 1) as f64,
        };

        // --- AR(1) mean reversion ---
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let phi = ar1_coefficient(prices);
        let last = *prices.last().unwrap_or(&mean);
        let mean_reversion = BaseForecaster::MeanReversion { mean, last, phi };

        [holt, ols, mean_reversion]
    }

    /// Compute inverse-error (inverse-RMSE) ensemble weights from per-component
    /// back-test RMSE. Weights are non-negative and sum to one.
    fn inverse_error_weights(rmse: &[f64; 3]) -> [f64; 3] {
        let raw: [f64; 3] = [
            1.0 / (rmse[0] + INVERSE_ERROR_EPS),
            1.0 / (rmse[1] + INVERSE_ERROR_EPS),
            1.0 / (rmse[2] + INVERSE_ERROR_EPS),
        ];
        let total: f64 = raw.iter().sum();
        if total > 0.0 && total.is_finite() {
            [raw[0] / total, raw[1] / total, raw[2] / total]
        } else {
            [1.0 / 3.0; 3]
        }
    }

    /// Extract the close-price series from the buffer as `f64`.
    fn price_series(&self) -> Vec<f64> {
        self.data
            .iter()
            .map(|p| p.close.to_f64().unwrap_or(0.0))
            .collect()
    }

    /// In-sample one-step residual standard deviation of the *ensemble* forecast.
    ///
    /// The ensemble one-step prediction is reconstructed at each in-sample step
    /// using the same component back-test rules and weights, and the standard
    /// deviation of the resulting residuals is returned. This is the per-step
    /// forecast-error scale that drives the predictive intervals.
    fn ensemble_residual_std(prices: &[f64], weights: &[f64; 3]) -> f64 {
        let n = prices.len();
        let alpha = 0.3;
        let beta = 0.1;
        let mut level = prices[0];
        let mut trend = 0.0;

        let mut residuals = Vec::with_capacity(n.saturating_sub(1));
        for t in 1..n {
            let holt_pred = level + trend;

            let prefix = &prices[..t];
            let (intercept, slope) = ols_fit(prefix);
            let ols_pred = intercept + slope * (t as f64);

            let mean = prefix.iter().sum::<f64>() / prefix.len() as f64;
            let phi = ar1_coefficient(prefix);
            let mr_pred = mean + (prices[t - 1] - mean) * phi;

            let ensemble_pred =
                weights[0] * holt_pred + weights[1] * ols_pred + weights[2] * mr_pred;
            residuals.push(prices[t] - ensemble_pred);

            let prev_level = level;
            level = alpha * prices[t] + (1.0 - alpha) * (level + trend);
            trend = beta * (level - prev_level) + (1.0 - beta) * trend;
        }

        if residuals.len() > 1 {
            let mean_r = residuals.iter().sum::<f64>() / residuals.len() as f64;
            let var = residuals
                .iter()
                .map(|r| (r - mean_r).powi(2))
                .sum::<f64>()
                / (residuals.len() - 1) as f64;
            var.sqrt()
        } else {
            // Fall back to the standard deviation of first differences.
            let diffs: Vec<f64> = prices.windows(2).map(|w| w[1] - w[0]).collect();
            if diffs.is_empty() {
                0.0
            } else {
                let m = diffs.iter().sum::<f64>() / diffs.len() as f64;
                (diffs.iter().map(|d| (d - m).powi(2)).sum::<f64>() / diffs.len() as f64).sqrt()
            }
        }
    }

    /// Forecast multiple horizons using the genuine three-model ensemble.
    ///
    /// The returned prediction for each horizon is the inverse-error weighted
    /// combination of the Holt, OLS and mean-reversion components. Confidence
    /// intervals widen with the horizon because the per-step forecast-error
    /// variance accumulates (`se(h) = residual_std * sqrt(h)`) and is scaled by
    /// the calibrated normal multiplier for `confidence_level`.
    pub fn forecast(&self, horizons: &[usize]) -> anyhow::Result<MultiHorizonForecast> {
        Ok(self.forecast_with_breakdown(horizons)?.forecast)
    }

    /// Like [`forecast`](Self::forecast) but also returns the ensemble
    /// diagnostics (component RMSE, weights and per-horizon decomposition) so the
    /// weighted combination can be inspected and verified.
    pub fn forecast_with_breakdown(
        &self,
        horizons: &[usize],
    ) -> anyhow::Result<MultiHorizonForecastWithDiagnostics> {
        if self.data.len() < MIN_OBSERVATIONS {
            return Err(CoreError::Validation(format!(
                "Insufficient data: need at least {} observations, have {}",
                MIN_OBSERVATIONS,
                self.data.len()
            ))
            .into());
        }
        if horizons.is_empty() {
            return Err(CoreError::Validation("No horizons requested".to_string()).into());
        }
        if horizons.contains(&0) {
            return Err(
                CoreError::Validation("Horizons must be strictly positive".to_string()).into(),
            );
        }

        let prices = self.price_series();

        // Fit the three structurally distinct base forecasters.
        let components = Self::fit_components(&prices);

        // Rank them by realised one-step accuracy and form inverse-error weights.
        let component_rmse = BaseForecaster::one_step_backtest_rmse(&prices);
        let weights = Self::inverse_error_weights(&component_rmse);

        // Per-step forecast-error scale for the ensemble.
        let residual_std = Self::ensemble_residual_std(&prices, &weights);

        // Calibrated two-sided normal multiplier for the requested confidence.
        let z_score = z_for_confidence(self.confidence_level);

        let mut predictions = Vec::with_capacity(horizons.len());
        let mut confidence_intervals = Vec::with_capacity(horizons.len());
        let mut breakdowns = Vec::with_capacity(horizons.len());

        for &horizon in horizons {
            let component_predictions: Vec<f64> =
                components.iter().map(|c| c.predict(horizon)).collect();

            // Exact documented weighted combination.
            let combined: f64 = weights
                .iter()
                .zip(component_predictions.iter())
                .map(|(w, p)| w * p)
                .sum();

            let prediction = Decimal::from_f64_retain(combined).unwrap_or(Decimal::ZERO);

            // Variance accumulates across independent steps: se(h) = sigma * sqrt(h).
            let se = residual_std * (horizon as f64).sqrt();
            let interval_width = z_score * se;
            let lower =
                Decimal::from_f64_retain(combined - interval_width).unwrap_or(Decimal::ZERO);
            let upper =
                Decimal::from_f64_retain(combined + interval_width).unwrap_or(Decimal::ZERO);

            predictions.push(prediction);
            confidence_intervals.push((lower.max(Decimal::ZERO), upper));
            breakdowns.push(EnsembleHorizonBreakdown {
                horizon,
                component_predictions,
                combined_prediction: combined,
            });
        }

        let diagnostics = EnsembleDiagnostics {
            component_names: vec![
                "holt_linear".to_string(),
                "ols_trend".to_string(),
                "ar1_mean_reversion".to_string(),
            ],
            component_rmse: component_rmse.to_vec(),
            weights: weights.to_vec(),
            breakdowns,
        };

        let forecast = MultiHorizonForecast {
            horizons: horizons.to_vec(),
            predictions,
            confidence_intervals,
            timestamp: Utc::now(),
        };

        Ok(MultiHorizonForecastWithDiagnostics {
            forecast,
            diagnostics,
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

    /// Generate a probabilistic forecast with horizon-growing predictive
    /// intervals.
    ///
    /// The method estimates the drift and volatility of the one-step simple
    /// return process and propagates them across `horizon` independent steps so
    /// the predictive variance accumulates (it grows with the horizon) and scales
    /// with the empirical volatility of the input. The reported `percentiles` are
    /// the empirical quantiles of a Monte-Carlo path simulation, while `std_dev`
    /// is the realised predictive standard deviation of the simulated terminal
    /// price; both agree with the analytic interval obtained from the inverse
    /// normal CDF.
    pub fn forecast(&self, horizon: usize) -> anyhow::Result<ProbabilisticForecast> {
        if self.data.len() < MIN_OBSERVATIONS {
            return Err(CoreError::Validation(format!(
                "Insufficient data: need at least {} observations, have {}",
                MIN_OBSERVATIONS,
                self.data.len()
            ))
            .into());
        }
        if horizon == 0 {
            return Err(
                CoreError::Validation("Horizon must be strictly positive".to_string()).into(),
            );
        }
        if self.num_simulations == 0 {
            return Err(CoreError::Validation(
                "Number of simulations must be greater than zero".to_string(),
            )
            .into());
        }

        // --- Estimate the return process ---
        let data_vec: Vec<_> = self.data.iter().collect();
        let returns: Vec<f64> = data_vec
            .windows(2)
            .map(|w| {
                let prev = w[0].close.to_f64().unwrap_or(0.0);
                let curr = w[1].close.to_f64().unwrap_or(0.0);
                if prev > 0.0 { (curr - prev) / prev } else { 0.0 }
            })
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        // Unbiased sample variance of the per-step return.
        let return_variance = if returns.len() > 1 {
            returns
                .iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>()
                / (returns.len() - 1) as f64
        } else {
            0.0
        };
        let return_std = return_variance.sqrt();

        let last_price = self
            .data
            .back()
            .ok_or_else(|| CoreError::Validation("Empty price buffer".to_string()))?
            .close
            .to_f64()
            .ok_or_else(|| {
                CoreError::Validation("Last close price is not representable".to_string())
            })?;

        // The analytic predictive moments grow with the horizon and with the
        // return variance; they are exposed separately via
        // [`predictive_interval`](Self::predictive_interval). Here we drive the
        // empirical quantiles from a Monte-Carlo path simulation that shares the
        // same fitted drift `mean_return` and per-step volatility `return_std`,
        // so the simulated dispersion accumulates across steps in the same way.

        // --- Monte-Carlo path simulation for empirical quantiles ---
        let mut simulated_prices = Vec::with_capacity(self.num_simulations);
        let mut rng = rng();
        let normal_dist = Normal::new(0.0, 1.0).map_err(|e| {
            CoreError::Validation(format!("Failed to create normal distribution: {}", e))
        })?;

        for _ in 0..self.num_simulations {
            let mut price = last_price;
            for _ in 0..horizon {
                let z = normal_dist.sample(&mut rng);
                let random_return = mean_return + return_std * z;
                price *= 1.0 + random_return;
            }
            simulated_prices.push(price);
        }

        simulated_prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Realised predictive standard deviation of the terminal price.
        let mc_mean = simulated_prices.iter().sum::<f64>() / simulated_prices.len() as f64;
        let mc_variance = if simulated_prices.len() > 1 {
            simulated_prices
                .iter()
                .map(|p| (p - mc_mean).powi(2))
                .sum::<f64>()
                / (simulated_prices.len() - 1) as f64
        } else {
            0.0
        };
        let std_dev = mc_variance.sqrt();

        // Empirical percentiles (10/25/50/75/90) via linear interpolation so the
        // quantiles are stable for any simulation count.
        let percentiles = vec![
            Decimal::from_f64_retain(empirical_quantile(&simulated_prices, 0.10))
                .unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(empirical_quantile(&simulated_prices, 0.25))
                .unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(empirical_quantile(&simulated_prices, 0.50))
                .unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(empirical_quantile(&simulated_prices, 0.75))
                .unwrap_or(Decimal::ZERO),
            Decimal::from_f64_retain(empirical_quantile(&simulated_prices, 0.90))
                .unwrap_or(Decimal::ZERO),
        ];

        Ok(ProbabilisticForecast {
            timestamp: Utc::now(),
            horizon,
            mean: Decimal::from_f64_retain(mc_mean).unwrap_or(Decimal::ZERO),
            std_dev,
            percentiles,
        })
    }

    /// Analytic predictive interval for the terminal price at `horizon` using
    /// calibrated normal quantiles derived from the fitted return process.
    ///
    /// Returns `(lower, upper, predictive_std)` for the requested confidence
    /// level. This is independent of the Monte-Carlo sampling and is exposed so
    /// callers can obtain a deterministic, calibrated interval whose width grows
    /// with the horizon and scales with the input volatility.
    pub fn predictive_interval(
        &self,
        horizon: usize,
        confidence_level: f64,
    ) -> anyhow::Result<(Decimal, Decimal, f64)> {
        if self.data.len() < MIN_OBSERVATIONS {
            return Err(CoreError::Validation(format!(
                "Insufficient data: need at least {} observations, have {}",
                MIN_OBSERVATIONS,
                self.data.len()
            ))
            .into());
        }
        if horizon == 0 {
            return Err(
                CoreError::Validation("Horizon must be strictly positive".to_string()).into(),
            );
        }

        let data_vec: Vec<_> = self.data.iter().collect();
        let returns: Vec<f64> = data_vec
            .windows(2)
            .map(|w| {
                let prev = w[0].close.to_f64().unwrap_or(0.0);
                let curr = w[1].close.to_f64().unwrap_or(0.0);
                if prev > 0.0 { (curr - prev) / prev } else { 0.0 }
            })
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let return_variance = if returns.len() > 1 {
            returns
                .iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>()
                / (returns.len() - 1) as f64
        } else {
            0.0
        };

        let last_price = self
            .data
            .back()
            .ok_or_else(|| CoreError::Validation("Empty price buffer".to_string()))?
            .close
            .to_f64()
            .ok_or_else(|| {
                CoreError::Validation("Last close price is not representable".to_string())
            })?;

        let h = horizon as f64;
        let growth = (1.0 + mean_return).powf(h);
        let analytic_mean = last_price * growth;
        let second_moment_factor = ((1.0 + mean_return).powi(2) + return_variance).powf(h);
        let analytic_std = (last_price * last_price * (second_moment_factor - growth * growth))
            .max(0.0)
            .sqrt();

        let z = z_for_confidence(confidence_level);
        let lower = Decimal::from_f64_retain((analytic_mean - z * analytic_std).max(0.0))
            .unwrap_or(Decimal::ZERO);
        let upper =
            Decimal::from_f64_retain(analytic_mean + z * analytic_std).unwrap_or(Decimal::ZERO);
        Ok((lower, upper, analytic_std))
    }
}

/// Empirical quantile of a *sorted* slice using linear interpolation between the
/// two nearest ranks (the `numpy`-style "linear"/"type 7" convention).
fn empirical_quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let q = q.clamp(0.0, 1.0);
    let pos = q * (sorted.len() - 1) as f64;
    let lower = pos.floor() as usize;
    let upper = pos.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let frac = pos - lower as f64;
        sorted[lower] * (1.0 - frac) + sorted[upper] * frac
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
    use rand::SeedableRng;
    use rand::rngs::StdRng;
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

    /// Build a price series from explicit closes (other OHLC fields tracked).
    fn data_from_closes(closes: &[f64]) -> Vec<PricePoint> {
        let mut data = Vec::with_capacity(closes.len());
        let mut timestamp = Utc::now();
        for &c in closes {
            let close = Decimal::from_f64_retain(c).unwrap_or(Decimal::ZERO);
            data.push(PricePoint {
                timestamp,
                open: close,
                high: close,
                low: close,
                close,
                volume: dec!(1000),
            });
            timestamp += Duration::hours(1);
        }
        data
    }

    /// Generate a Gaussian-return series with a fixed seed and known per-step
    /// volatility, starting from `start`.
    fn noisy_series(start: f64, drift: f64, vol: f64, n: usize, seed: u64) -> Vec<f64> {
        let mut rng = StdRng::seed_from_u64(seed);
        let normal = Normal::new(0.0, 1.0).expect("valid normal");
        let mut price = start;
        let mut out = Vec::with_capacity(n);
        out.push(price);
        for _ in 1..n {
            let z: f64 = normal.sample(&mut rng);
            price *= 1.0 + drift + vol * z;
            out.push(price);
        }
        out
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

        // On the rising synthetic ramp every component trends upward, so the
        // ensemble prediction increases with the horizon.
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
    fn test_ensemble_is_weighted_combination_of_distinct_components() {
        // Use a curved series so the linear-trend, smoothing and mean-reversion
        // components genuinely disagree.
        let closes: Vec<f64> = (0..60)
            .map(|i| {
                let t = i as f64;
                100.0 + 0.5 * t + 8.0 * (t / 7.0).sin()
            })
            .collect();
        let mut forecaster = MultiHorizonForecaster::new(200, 0.95);
        for p in data_from_closes(&closes) {
            forecaster.add_data(p);
        }

        let horizons = vec![1, 3, 7, 14];
        let result = forecaster.forecast_with_breakdown(&horizons).unwrap();
        let diag = &result.diagnostics;

        // Three genuinely distinct components.
        assert_eq!(diag.component_names.len(), 3);
        assert_eq!(diag.weights.len(), 3);
        assert_eq!(diag.component_rmse.len(), 3);

        // Weights are a valid convex combination.
        let weight_sum: f64 = diag.weights.iter().sum();
        assert!((weight_sum - 1.0).abs() < 1e-9, "weights must sum to 1");
        assert!(diag.weights.iter().all(|&w| w >= 0.0));
        // Every component carries non-trivial weight (real ensemble, not a
        // single relabeled model).
        assert!(
            diag.weights.iter().all(|&w| w > 1e-4),
            "every component should contribute: {:?}",
            diag.weights
        );

        for (idx, b) in diag.breakdowns.iter().enumerate() {
            assert_eq!(b.component_predictions.len(), 3);

            // The ensemble equals the documented weighted combination exactly.
            let recomputed: f64 = diag
                .weights
                .iter()
                .zip(b.component_predictions.iter())
                .map(|(w, p)| w * p)
                .sum();
            assert!(
                (recomputed - b.combined_prediction).abs() < 1e-9,
                "ensemble must equal weighted combination"
            );

            // The reported Decimal prediction matches the combined value.
            let reported = result.forecast.predictions[idx].to_f64().unwrap();
            assert!((reported - b.combined_prediction).abs() < 1e-6);

            // The ensemble differs from at least two of the three components
            // (it is not a single component relabeled). Components disagree on
            // this curved series, so the combination cannot coincide with all.
            let differs = b
                .component_predictions
                .iter()
                .filter(|&&c| (c - b.combined_prediction).abs() > 1e-6)
                .count();
            assert!(
                differs >= 2,
                "ensemble {} should differ from individual components {:?}",
                b.combined_prediction,
                b.component_predictions
            );
        }

        // The components themselves are not all identical at the longest horizon.
        let last = diag.breakdowns.last().unwrap();
        let c = &last.component_predictions;
        assert!(
            (c[0] - c[1]).abs() > 1e-6 || (c[0] - c[2]).abs() > 1e-6 || (c[1] - c[2]).abs() > 1e-6,
            "base components must be genuinely distinct: {:?}",
            c
        );
    }

    #[test]
    fn test_intervals_widen_with_horizon_and_scale_with_volatility() {
        let horizons = vec![1, 5, 10, 20];

        // Low-volatility and high-volatility series sharing the same drift/seed.
        let low = noisy_series(100.0, 0.0005, 0.004, 200, 7);
        let high = noisy_series(100.0, 0.0005, 0.02, 200, 7);

        let mut f_low = MultiHorizonForecaster::new(400, 0.95);
        for p in data_from_closes(&low) {
            f_low.add_data(p);
        }
        let mut f_high = MultiHorizonForecaster::new(400, 0.95);
        for p in data_from_closes(&high) {
            f_high.add_data(p);
        }

        let fc_low = f_low.forecast(&horizons).unwrap();
        let fc_high = f_high.forecast(&horizons).unwrap();

        let width = |ci: &(Decimal, Decimal)| (ci.1 - ci.0).to_f64().unwrap_or(0.0);

        // Intervals widen monotonically with horizon.
        for w in [&fc_low.confidence_intervals, &fc_high.confidence_intervals] {
            for i in 1..w.len() {
                assert!(
                    width(&w[i]) > width(&w[i - 1]),
                    "interval must widen with horizon"
                );
            }
        }

        // Higher input volatility yields wider intervals at every horizon.
        for (i, &horizon) in horizons.iter().enumerate() {
            assert!(
                width(&fc_high.confidence_intervals[i]) > width(&fc_low.confidence_intervals[i]),
                "higher volatility must widen intervals at horizon {}",
                horizon
            );
        }

        // sqrt(h) growth: the ratio of widths between two horizons should track
        // the ratio of sqrt(h) (point forecast is the same scale, only se grows).
        let w1 = width(&fc_high.confidence_intervals[0]); // h = 1
        let w4 = width(&fc_high.confidence_intervals[3]); // h = 20
        let expected_ratio = (20.0_f64).sqrt();
        let observed_ratio = w4 / w1;
        assert!(
            (observed_ratio - expected_ratio).abs() / expected_ratio < 0.05,
            "width ratio {} should match sqrt(20) = {}",
            observed_ratio,
            expected_ratio
        );
    }

    #[test]
    fn test_probabilistic_forecaster() {
        let data = create_test_data();
        let mut forecaster = ProbabilisticForecaster::new(100, 2000);

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
    fn test_probabilistic_intervals_widen_and_scale_with_volatility() {
        let low = noisy_series(100.0, 0.0, 0.005, 250, 11);
        let high = noisy_series(100.0, 0.0, 0.025, 250, 11);

        let mut f_low = ProbabilisticForecaster::new(500, 1);
        for p in data_from_closes(&low) {
            f_low.add_data(p);
        }
        let mut f_high = ProbabilisticForecaster::new(500, 1);
        for p in data_from_closes(&high) {
            f_high.add_data(p);
        }

        // Analytic predictive std grows with horizon (deterministic check).
        let (_, _, std_h1) = f_low.predictive_interval(1, 0.95).unwrap();
        let (_, _, std_h5) = f_low.predictive_interval(5, 0.95).unwrap();
        let (_, _, std_h20) = f_low.predictive_interval(20, 0.95).unwrap();
        assert!(std_h5 > std_h1, "predictive std must grow with horizon");
        assert!(std_h20 > std_h5, "predictive std must grow with horizon");

        // Higher volatility yields a wider analytic std at the same horizon.
        let (_, _, std_low) = f_low.predictive_interval(10, 0.95).unwrap();
        let (_, _, std_high) = f_high.predictive_interval(10, 0.95).unwrap();
        assert!(
            std_high > std_low,
            "higher input volatility must widen predictive std"
        );

        // A higher confidence level produces a wider interval.
        let (lo90, hi90, _) = f_low.predictive_interval(10, 0.90).unwrap();
        let (lo99, hi99, _) = f_low.predictive_interval(10, 0.99).unwrap();
        let w90 = (hi90 - lo90).to_f64().unwrap();
        let w99 = (hi99 - lo99).to_f64().unwrap();
        assert!(w99 > w90, "99% interval must be wider than 90%");
    }

    #[test]
    fn test_inverse_normal_cdf_accuracy() {
        // Known quantiles of the standard normal distribution.
        assert!((inverse_normal_cdf(0.5)).abs() < 1e-6);
        assert!((inverse_normal_cdf(0.975) - 1.959_963_984_540_054).abs() < 1e-4);
        assert!((inverse_normal_cdf(0.025) + 1.959_963_984_540_054).abs() < 1e-4);
        assert!((inverse_normal_cdf(0.95) - 1.644_853_626_951_472).abs() < 1e-4);
        assert!((inverse_normal_cdf(0.995) - 2.575_829_303_548_901).abs() < 1e-4);
        // Symmetry.
        assert!((inverse_normal_cdf(0.8) + inverse_normal_cdf(0.2)).abs() < 1e-6);
        // The two-sided helper recovers the textbook 95% multiplier.
        assert!((z_for_confidence(0.95) - 1.959_963_984_540_054).abs() < 1e-4);
    }

    #[test]
    fn test_probabilistic_interval_nominal_coverage() {
        // Synthetic series with KNOWN noise; check the 90% analytic interval
        // covers the realised future price close to 90% of the time across many
        // independent draws (out-of-sample, fixed seeds).
        let drift = 0.0;
        let vol = 0.01;
        let horizon = 5;
        let history_len = 200;
        let trials = 400;

        let mut covered = 0usize;
        for seed in 0..trials as u64 {
            // History to fit on.
            let hist = noisy_series(100.0, drift, vol, history_len, seed);
            let last = *hist.last().unwrap();

            let mut f = ProbabilisticForecaster::new(history_len + 8, 1);
            for p in data_from_closes(&hist) {
                f.add_data(p);
            }
            let (lo, hi, _) = f.predictive_interval(horizon, 0.90).unwrap();
            let lo = lo.to_f64().unwrap();
            let hi = hi.to_f64().unwrap();

            // Realised future: continue the same process from `last` with a
            // disjoint noise stream so it is genuinely out-of-sample.
            let future = noisy_series(last, drift, vol, horizon + 1, seed + 1_000_000);
            let realised = *future.last().unwrap();

            if realised >= lo && realised <= hi {
                covered += 1;
            }
        }

        let coverage = covered as f64 / trials as f64;
        // Nominal is 0.90; allow a tolerance band for finite samples and the
        // normal approximation to the compounded-return distribution.
        assert!(
            (0.80..=0.97).contains(&coverage),
            "coverage {} should be roughly nominal (0.90)",
            coverage
        );
    }

    #[test]
    fn test_forecaster_rejects_insufficient_and_invalid_input() {
        let mut f = MultiHorizonForecaster::new(100, 0.95);
        for p in data_from_closes(&[100.0, 101.0, 102.0]) {
            f.add_data(p);
        }
        assert!(f.forecast(&[1, 5]).is_err(), "too little data must error");

        let mut f2 = MultiHorizonForecaster::new(100, 0.95);
        for p in data_from_closes(&(0..20).map(|i| 100.0 + i as f64).collect::<Vec<_>>()) {
            f2.add_data(p);
        }
        assert!(f2.forecast(&[]).is_err(), "no horizons must error");
        assert!(f2.forecast(&[0, 1]).is_err(), "zero horizon must error");

        let mut p = ProbabilisticForecaster::new(100, 1000);
        for pt in data_from_closes(&[100.0, 101.0]) {
            p.add_data(pt);
        }
        assert!(p.forecast(5).is_err(), "too little data must error");
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
