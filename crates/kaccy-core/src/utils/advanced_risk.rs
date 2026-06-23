//! Advanced Risk Metrics
//!
//! This module provides sophisticated risk measurement techniques including
//! Expected Shortfall (ES/CVaR), Stress VaR, and Incremental Risk Charge.

use crate::error::{CoreError, Result};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, Normal};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default number of paths drawn by the Monte Carlo Expected Shortfall engine.
///
/// 100_000 samples keeps the standard error of the tail estimate small enough
/// that the empirical ES converges to the analytic value within a few basis
/// points for a normal return distribution.
const DEFAULT_MONTE_CARLO_SAMPLES: usize = 100_000;

/// Fixed seed used so Monte Carlo Expected Shortfall is reproducible.
///
/// Risk numbers feed audit trails and downstream limits, so a deterministic
/// stream is preferable to a fresh entropy draw on every call. A different
/// stream can still be requested through [`ExpectedShortfallCalculator::calculate_monte_carlo_with`].
const DEFAULT_MONTE_CARLO_SEED: u64 = 0x5249_534B_5F4D_4321; // "RISK_M!" packed

/// Expected Shortfall (ES) calculator, also known as Conditional Value at Risk (CVaR)
///
/// ES measures the expected loss in the worst α% of cases, providing a more
/// comprehensive tail risk measure than VaR.
#[derive(Debug, Clone)]
pub struct ExpectedShortfallCalculator {
    /// Confidence level (e.g. 0.95 for 95%)
    confidence_level: Decimal,
    /// Method used for the ES calculation
    method: ESMethod,
}

/// Method used to calculate Expected Shortfall
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ESMethod {
    /// Historical simulation using actual return distribution
    Historical,
    /// Parametric (assumes normal distribution)
    Parametric,
    /// Monte Carlo simulation
    MonteCarlo,
}

impl ExpectedShortfallCalculator {
    /// Create a new ES calculator
    ///
    /// # Arguments
    /// * `confidence_level` - Confidence level (e.g., 0.95 for 95%)
    /// * `method` - Calculation method
    pub fn new(confidence_level: Decimal, method: ESMethod) -> Result<Self> {
        if confidence_level <= Decimal::ZERO || confidence_level >= Decimal::ONE {
            return Err(CoreError::Validation(
                "Confidence level must be between 0 and 1".to_string(),
            ));
        }

        Ok(Self {
            confidence_level,
            method,
        })
    }

    /// Calculate Expected Shortfall from return data
    pub fn calculate(&self, returns: &[Decimal]) -> Result<ESResult> {
        if returns.is_empty() {
            return Err(CoreError::Validation(
                "Returns data cannot be empty".to_string(),
            ));
        }

        match self.method {
            ESMethod::Historical => self.calculate_historical(returns),
            ESMethod::Parametric => self.calculate_parametric(returns),
            ESMethod::MonteCarlo => self.calculate_monte_carlo(returns),
        }
    }

    fn calculate_historical(&self, returns: &[Decimal]) -> Result<ESResult> {
        let mut sorted_returns = returns.to_vec();
        sorted_returns.sort();

        // Find VaR threshold
        let alpha = Decimal::ONE - self.confidence_level;
        let var_index = (alpha * Decimal::from(sorted_returns.len()))
            .to_usize()
            .unwrap_or(0);

        if var_index >= sorted_returns.len() {
            return Err(CoreError::Validation(
                "Insufficient data for ES calculation".to_string(),
            ));
        }

        let var = sorted_returns[var_index];

        // Calculate mean of returns beyond VaR
        let tail_returns: Vec<_> = sorted_returns.iter().take(var_index + 1).collect();
        let es = if tail_returns.is_empty() {
            var
        } else {
            tail_returns.iter().copied().sum::<Decimal>() / Decimal::from(tail_returns.len())
        };

        Ok(ESResult {
            expected_shortfall: es.abs(),
            var: var.abs(),
            confidence_level: self.confidence_level,
            method: self.method,
            tail_observations: tail_returns.len(),
        })
    }

    fn calculate_parametric(&self, returns: &[Decimal]) -> Result<ESResult> {
        // Calculate mean and standard deviation
        let n = Decimal::from(returns.len());
        let mean: Decimal = returns.iter().sum::<Decimal>() / n;
        let variance: Decimal = returns.iter().map(|r| (r - mean).powi(2)).sum::<Decimal>() / n;
        let std_dev = variance.sqrt().ok_or_else(|| {
            CoreError::Calculation("Failed to calculate standard deviation".to_string())
        })?;

        // For normal distribution:
        // VaR = μ + σ * z_α
        // ES = μ + σ * φ(z_α) / α
        // where φ is the standard normal PDF and z_α is the quantile

        let alpha = Decimal::ONE - self.confidence_level;

        // Approximate z-score for common confidence levels
        let z_alpha = self.get_z_score(alpha)?;

        let var = (mean + std_dev * z_alpha).abs();

        // Standard normal PDF at z_alpha
        let phi_z = self.normal_pdf(z_alpha);

        // ES calculation for normal distribution
        let es = (mean + std_dev * phi_z / alpha).abs();

        Ok(ESResult {
            expected_shortfall: es,
            var,
            confidence_level: self.confidence_level,
            method: self.method,
            tail_observations: 0, // Not applicable for parametric
        })
    }

    fn calculate_monte_carlo(&self, returns: &[Decimal]) -> Result<ESResult> {
        self.calculate_monte_carlo_with(returns, DEFAULT_MONTE_CARLO_SAMPLES, DEFAULT_MONTE_CARLO_SEED)
    }

    /// Calculate Expected Shortfall via Monte Carlo simulation with explicit
    /// control over the number of simulated paths and the RNG seed.
    ///
    /// The return distribution is modelled as a Gaussian whose mean and
    /// (sample) standard deviation are estimated from `returns`. `num_samples`
    /// paths are drawn from that distribution, converted into losses
    /// (`loss = -return`), and the empirical VaR/ES are read off the simulated
    /// loss distribution at the configured tail probability `alpha = 1 - confidence`.
    ///
    /// Expected Shortfall is the average loss conditional on the loss being at
    /// least as severe as the VaR threshold.
    ///
    /// # Arguments
    /// * `returns` - Historical returns used to estimate the distribution.
    /// * `num_samples` - Number of Monte Carlo paths (must be non-zero).
    /// * `seed` - RNG seed for reproducible results.
    pub fn calculate_monte_carlo_with(
        &self,
        returns: &[Decimal],
        num_samples: usize,
        seed: u64,
    ) -> Result<ESResult> {
        if returns.is_empty() {
            return Err(CoreError::Validation(
                "Returns data cannot be empty".to_string(),
            ));
        }
        if num_samples == 0 {
            return Err(CoreError::Validation(
                "Monte Carlo sample count must be greater than zero".to_string(),
            ));
        }

        // Estimate the return distribution parameters in f64 for the simulation.
        let n = returns.len() as f64;
        let mean: f64 = returns
            .iter()
            .map(|r| r.to_f64().unwrap_or(0.0))
            .sum::<f64>()
            / n;

        // Sample variance (unbiased, Bessel-corrected) when we have more than
        // one observation; fall back to the population formula for a single point.
        let denom = if returns.len() > 1 { n - 1.0 } else { n };
        let variance: f64 = returns
            .iter()
            .map(|r| {
                let x = r.to_f64().unwrap_or(0.0) - mean;
                x * x
            })
            .sum::<f64>()
            / denom;
        let std_dev = variance.sqrt();

        if !mean.is_finite() || !std_dev.is_finite() {
            return Err(CoreError::Calculation(
                "Failed to estimate return distribution parameters".to_string(),
            ));
        }

        // Draw the simulated loss distribution. `Normal::new` rejects a
        // non-finite std dev; a zero std dev is valid and yields a degenerate
        // (constant) distribution, which is the correct behaviour for a
        // riskless series.
        let normal = Normal::new(mean, std_dev).map_err(|e| {
            CoreError::Calculation(format!("Invalid Monte Carlo distribution: {e}"))
        })?;
        let mut rng = StdRng::seed_from_u64(seed);

        let mut losses: Vec<f64> = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            let simulated_return = normal.sample(&mut rng);
            // Loss is the negative of the return: large losses are large positive numbers.
            losses.push(-simulated_return);
        }

        // Sort ascending so the worst losses sit at the tail (largest values).
        losses.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let alpha = (Decimal::ONE - self.confidence_level)
            .to_f64()
            .ok_or_else(|| CoreError::Calculation("Failed to convert alpha".to_string()))?;

        // The VaR threshold sits at the (1 - alpha) quantile of the loss
        // distribution. We index from the worst-loss end so that the tail beyond
        // VaR contains the worst `alpha` fraction of the simulated paths.
        let tail_count = ((num_samples as f64) * alpha).ceil() as usize;
        let tail_count = tail_count.clamp(1, num_samples);
        let var_index = num_samples - tail_count;
        let var_loss = losses[var_index];

        // Expected Shortfall: mean loss over the tail (losses at or beyond VaR).
        let tail_slice = &losses[var_index..];
        let es_loss = tail_slice.iter().sum::<f64>() / (tail_slice.len() as f64);

        let var = Decimal::from_f64(var_loss)
            .ok_or_else(|| CoreError::Calculation("Failed to convert VaR".to_string()))?;
        let expected_shortfall = Decimal::from_f64(es_loss)
            .ok_or_else(|| CoreError::Calculation("Failed to convert ES".to_string()))?;

        Ok(ESResult {
            expected_shortfall: expected_shortfall.max(Decimal::ZERO),
            var: var.max(Decimal::ZERO),
            confidence_level: self.confidence_level,
            method: ESMethod::MonteCarlo,
            tail_observations: tail_slice.len(),
        })
    }

    fn get_z_score(&self, alpha: Decimal) -> Result<Decimal> {
        // Real inverse standard-normal CDF (quantile). `alpha` is the lower-tail
        // probability, so the result is the (negative) z-score for the VaR/ES
        // tail at the configured confidence level.
        let alpha_f64 = alpha
            .to_f64()
            .ok_or_else(|| CoreError::Calculation("Failed to convert alpha".to_string()))?;

        let z = inverse_normal_cdf(alpha_f64)?;

        Decimal::from_f64(z)
            .ok_or_else(|| CoreError::Calculation("Failed to convert z-score".to_string()))
    }

    fn normal_pdf(&self, z: Decimal) -> Decimal {
        let z_f64 = z.to_f64().unwrap_or(0.0);
        let pdf = (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-0.5 * z_f64 * z_f64).exp();
        Decimal::from_f64(pdf).unwrap_or(Decimal::ZERO)
    }
}

/// Error function `erf(x)` via its Maclaurin series.
///
/// `erf(x) = (2/√π) · Σ_{n≥0} (-1)^n · x^(2n+1) / (n!·(2n+1))`.
///
/// The series converges to f64 machine precision rapidly for moderate `|x|`
/// (it is used only for `|x| < 2`, where well under 30 terms suffice); the tail
/// is handled by [`erfc_continued_fraction`].
fn erf_series(x: f64) -> f64 {
    let x_squared = x * x;
    let mut term = x; // n = 0 term: x / (0! · 1).
    let mut sum = term;
    let mut n = 1.0_f64;
    loop {
        // term_n / term_{n-1} = -x² · (2n-1) / (n · (2n+1)).
        term *= -x_squared * (2.0 * n - 1.0) / (n * (2.0 * n + 1.0));
        sum += term;
        if term.abs() <= f64::EPSILON * sum.abs() {
            break;
        }
        n += 1.0;
        if n > 200.0 {
            break;
        }
    }
    std::f64::consts::FRAC_2_SQRT_PI * sum
}

/// Complementary error function `erfc(x) = 1 - erf(x)` for `x > 0` via the
/// classic continued-fraction expansion evaluated with the modified Lentz
/// algorithm. Accurate to f64 machine precision and numerically stable in the
/// tail where `1 - erf(x)` would otherwise suffer catastrophic cancellation.
fn erfc_continued_fraction(x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / x;
    let mut h = d;
    let mut i = 1.0_f64;
    loop {
        let a = i / 2.0;
        d = a * d + x;
        if d.abs() < TINY {
            d = TINY;
        }
        c = x + a / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = c * d;
        h *= delta;
        if (delta - 1.0).abs() < f64::EPSILON {
            break;
        }
        i += 1.0;
        if i > 400.0 {
            break;
        }
    }
    (-x * x).exp() / std::f64::consts::PI.sqrt() * h
}

/// Complementary error function `erfc(x) = 1 - erf(x)` for all finite `x`,
/// accurate to f64 machine precision.
fn erfc(x: f64) -> f64 {
    let ax = x.abs();
    let result = if ax < 2.0 {
        1.0 - erf_series(ax)
    } else {
        erfc_continued_fraction(ax)
    };
    if x < 0.0 { 2.0 - result } else { result }
}

/// Standard normal cumulative distribution function `Φ(x) = ½·erfc(-x/√2)`.
///
/// Uses the machine-precision [`erfc`] implementation so the Halley refinement
/// step inside [`inverse_normal_cdf`] genuinely converges to near machine
/// precision.
fn standard_normal_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// Inverse of the standard normal cumulative distribution function (the
/// probit / quantile function): returns the value `z` such that `Φ(z) = p`.
///
/// Uses Peter Acklam's rational approximation for the bulk of the distribution,
/// followed by a single Halley refinement step that lifts the accuracy to near
/// machine precision (relative error < 1e-15 for `p` away from the extremes).
///
/// # Arguments
/// * `p` - A probability in the open interval `(0, 1)`.
///
/// # Errors
/// Returns [`CoreError::Validation`] if `p` is not strictly between 0 and 1.
fn inverse_normal_cdf(p: f64) -> Result<f64> {
    if !(p > 0.0 && p < 1.0) {
        return Err(CoreError::Validation(
            "Probability for inverse normal CDF must be in the open interval (0, 1)".to_string(),
        ));
    }

    // Coefficients for Acklam's rational approximation.
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239e0,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838e0,
        -2.549_732_539_343_734e0,
        4.374_664_141_464_968e0,
        2.938_163_982_698_783e0,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996e0,
        3.754_408_661_907_416e0,
    ];

    // Break-points delimiting the lower tail, central, and upper tail regions.
    const P_LOW: f64 = 0.024_25;
    const P_HIGH: f64 = 1.0 - P_LOW;

    let mut x = if p < P_LOW {
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
        // Upper tail (mirror of the lower tail).
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };

    // One Halley iteration to polish the approximation to ~machine precision.
    let e = standard_normal_cdf(x) - p;
    let pdf = (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-0.5 * x * x).exp();
    if pdf > 0.0 {
        let u = e / pdf;
        x -= u / (1.0 + 0.5 * x * u);
    }

    Ok(x)
}

/// Expected Shortfall calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ESResult {
    /// Expected Shortfall value
    pub expected_shortfall: Decimal,
    /// Value at Risk at the same confidence level
    pub var: Decimal,
    /// Confidence level used
    pub confidence_level: Decimal,
    /// Calculation method used
    pub method: ESMethod,
    /// Number of tail observations (for historical method)
    pub tail_observations: usize,
}

impl ESResult {
    /// Get the ES/VaR ratio
    pub fn es_var_ratio(&self) -> Decimal {
        if self.var > Decimal::ZERO {
            self.expected_shortfall / self.var
        } else {
            Decimal::ONE
        }
    }

    /// Check if ES is significantly worse than VaR
    pub fn has_fat_tails(&self) -> bool {
        // ES should generally be 10-50% higher than VaR
        // If it's much higher, indicates fat tails
        self.es_var_ratio() > Decimal::new(15, 1) // 1.5
    }
}

/// Stress VaR calculator for crisis scenarios
#[derive(Debug, Clone)]
pub struct StressVarCalculator {
    /// Named stress scenarios available for calculation
    scenarios: HashMap<String, StressScenario>,
}

/// A named stress scenario with per-asset market shocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    /// Name of the scenario
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Per-asset shock magnitudes (negative = loss)
    pub market_shocks: HashMap<String, Decimal>,
}

impl StressScenario {
    /// Create a new stress scenario with the given name and description
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            market_shocks: HashMap::new(),
        }
    }

    /// Register a shock magnitude for the given asset
    pub fn add_shock(&mut self, asset: impl Into<String>, shock: Decimal) {
        self.market_shocks.insert(asset.into(), shock);
    }
}

impl StressVarCalculator {
    /// Create a new stress VaR calculator
    pub fn new() -> Self {
        Self {
            scenarios: HashMap::new(),
        }
    }

    /// Add a predefined stress scenario
    pub fn add_scenario(&mut self, scenario: StressScenario) {
        self.scenarios.insert(scenario.name.clone(), scenario);
    }

    /// Calculate stress VaR for a portfolio
    pub fn calculate(
        &self,
        portfolio: &HashMap<String, Decimal>,
        scenario_name: &str,
    ) -> Result<StressVarResult> {
        let scenario = self.scenarios.get(scenario_name).ok_or_else(|| {
            CoreError::NotFound(format!("Scenario '{}' not found", scenario_name))
        })?;

        let mut total_loss = Decimal::ZERO;
        let mut stressed_positions = Vec::new();

        for (asset, position) in portfolio {
            if let Some(shock) = scenario.market_shocks.get(asset) {
                let loss = position * shock;
                total_loss += loss;
                stressed_positions.push((asset.clone(), loss));
            }
        }

        Ok(StressVarResult {
            scenario_name: scenario_name.to_string(),
            total_loss: total_loss.abs(),
            stressed_positions,
        })
    }

    /// Add common crisis scenarios
    pub fn add_common_scenarios(&mut self) {
        // 2008 Financial Crisis
        let mut crisis_2008 =
            StressScenario::new("2008_financial_crisis", "Lehman Brothers collapse scenario");
        crisis_2008.add_shock("equities", Decimal::new(-40, 2)); // -40%
        crisis_2008.add_shock("credit", Decimal::new(-30, 2)); // -30%
        crisis_2008.add_shock("commodities", Decimal::new(-25, 2)); // -25%
        self.add_scenario(crisis_2008);

        // COVID-19 Market Crash
        let mut covid_crash =
            StressScenario::new("covid_2020_crash", "COVID-19 pandemic market crash");
        covid_crash.add_shock("equities", Decimal::new(-35, 2)); // -35%
        covid_crash.add_shock("oil", Decimal::new(-60, 2)); // -60%
        covid_crash.add_shock("volatility", Decimal::new(300, 2)); // +300%
        self.add_scenario(covid_crash);

        // Crypto Winter
        let mut crypto_winter =
            StressScenario::new("crypto_winter_2022", "Cryptocurrency bear market");
        crypto_winter.add_shock("btc", Decimal::new(-75, 2)); // -75%
        crypto_winter.add_shock("eth", Decimal::new(-80, 2)); // -80%
        crypto_winter.add_shock("altcoins", Decimal::new(-90, 2)); // -90%
        self.add_scenario(crypto_winter);
    }
}

impl Default for StressVarCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a stress VaR calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressVarResult {
    /// Name of the applied stress scenario
    pub scenario_name: String,
    /// Total portfolio loss under the scenario
    pub total_loss: Decimal,
    /// Per-position stressed P&L
    pub stressed_positions: Vec<(String, Decimal)>,
}

/// Incremental Risk Charge (IRC) calculator
#[derive(Debug, Clone)]
pub struct IncrementalRiskCalculator {
    /// Holding horizon in trading days
    horizon_days: u32,
    /// Confidence level for the IRC calculation
    confidence_level: Decimal,
}

impl IncrementalRiskCalculator {
    /// Create a new IRC calculator with the given horizon and confidence level
    pub fn new(horizon_days: u32, confidence_level: Decimal) -> Result<Self> {
        if confidence_level <= Decimal::ZERO || confidence_level >= Decimal::ONE {
            return Err(CoreError::Validation(
                "Confidence level must be between 0 and 1".to_string(),
            ));
        }

        Ok(Self {
            horizon_days,
            confidence_level,
        })
    }

    /// Calculate IRC considering default and migration risk
    pub fn calculate(
        &self,
        positions: &[(String, Decimal, Decimal)], // (name, exposure, default_prob)
    ) -> Result<IRCResult> {
        let mut total_default_risk = Decimal::ZERO;
        let mut total_migration_risk = Decimal::ZERO;

        for (_name, exposure, default_prob) in positions {
            // Default risk: expected loss from default
            let default_risk = exposure * default_prob;
            total_default_risk += default_risk;

            // Migration risk: simplified as a fraction of default risk
            // In a full implementation, would model rating transitions
            let migration_risk = default_risk * Decimal::new(3, 1); // 30% of default risk
            total_migration_risk += migration_risk;
        }

        let total_irc = total_default_risk + total_migration_risk;

        Ok(IRCResult {
            total_irc,
            default_risk: total_default_risk,
            migration_risk: total_migration_risk,
            horizon_days: self.horizon_days,
            confidence_level: self.confidence_level,
        })
    }
}

/// Result of an Incremental Risk Charge calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRCResult {
    /// Total incremental risk charge
    pub total_irc: Decimal,
    /// Component due to default risk
    pub default_risk: Decimal,
    /// Component due to rating migration risk
    pub migration_risk: Decimal,
    /// Holding horizon in trading days
    pub horizon_days: u32,
    /// Confidence level used for the calculation
    pub confidence_level: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_expected_shortfall_historical() {
        let returns = vec![
            dec!(-0.05),
            dec!(-0.03),
            dec!(-0.02),
            dec!(-0.01),
            dec!(0.01),
            dec!(0.02),
            dec!(0.03),
            dec!(0.04),
        ];

        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::Historical).unwrap();
        let result = calculator.calculate(&returns).unwrap();

        assert!(result.expected_shortfall > Decimal::ZERO);
        assert!(result.expected_shortfall >= result.var);
    }

    #[test]
    fn test_expected_shortfall_parametric() {
        let returns = vec![dec!(-0.05), dec!(-0.03), dec!(0.01), dec!(0.02), dec!(0.03)];

        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::Parametric).unwrap();
        let result = calculator.calculate(&returns).unwrap();

        assert!(result.expected_shortfall > Decimal::ZERO);
        assert!(result.expected_shortfall >= result.var);
    }

    #[test]
    fn test_es_var_ratio() {
        let result = ESResult {
            expected_shortfall: dec!(0.06),
            var: dec!(0.05),
            confidence_level: dec!(0.95),
            method: ESMethod::Historical,
            tail_observations: 5,
        };

        assert_eq!(result.es_var_ratio(), dec!(1.2));
        assert!(!result.has_fat_tails());
    }

    #[test]
    fn test_stress_var() {
        let mut calculator = StressVarCalculator::new();
        calculator.add_common_scenarios();

        let mut portfolio = HashMap::new();
        portfolio.insert("btc".to_string(), dec!(10000));
        portfolio.insert("eth".to_string(), dec!(5000));

        let result = calculator
            .calculate(&portfolio, "crypto_winter_2022")
            .unwrap();

        assert!(result.total_loss > Decimal::ZERO);
        assert_eq!(result.scenario_name, "crypto_winter_2022");
    }

    #[test]
    fn test_incremental_risk() {
        let positions = vec![
            ("Bond A".to_string(), dec!(100000), dec!(0.01)),
            ("Bond B".to_string(), dec!(50000), dec!(0.02)),
        ];

        let calculator = IncrementalRiskCalculator::new(252, dec!(0.99)).unwrap();
        let result = calculator.calculate(&positions).unwrap();

        assert!(result.total_irc > Decimal::ZERO);
        assert!(result.default_risk > Decimal::ZERO);
        assert!(result.migration_risk > Decimal::ZERO);
    }

    #[test]
    fn test_stress_scenario_creation() {
        let mut scenario = StressScenario::new("test", "Test scenario");
        scenario.add_shock("asset1", dec!(-0.3));
        scenario.add_shock("asset2", dec!(-0.5));

        assert_eq!(scenario.name, "test");
        assert_eq!(scenario.market_shocks.len(), 2);
    }

    /// Standard normal PDF helper for analytic comparisons in tests.
    fn phi(z: f64) -> f64 {
        (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-0.5 * z * z).exp()
    }

    #[test]
    fn test_inverse_normal_known_quantiles() {
        // Median.
        assert!(inverse_normal_cdf(0.5).unwrap().abs() < 1e-12);
        // z_{0.975} ≈ 1.95996, z_{0.95} ≈ 1.64485.
        assert!((inverse_normal_cdf(0.975).unwrap() - 1.959_963_984_540_054).abs() < 1e-9);
        assert!((inverse_normal_cdf(0.95).unwrap() - 1.644_853_626_951_472).abs() < 1e-9);
        assert!((inverse_normal_cdf(0.99).unwrap() - 2.326_347_874_040_841).abs() < 1e-9);
        // Symmetry: lower-tail quantiles are the negatives of the upper-tail ones.
        assert!((inverse_normal_cdf(0.05).unwrap() + 1.644_853_626_951_472).abs() < 1e-9);
        assert!((inverse_normal_cdf(0.025).unwrap() + 1.959_963_984_540_054).abs() < 1e-9);
    }

    #[test]
    fn test_inverse_normal_round_trip() {
        // Φ(Φ⁻¹(p)) ≈ p across a spread of probabilities, including the tails.
        for &p in &[0.001, 0.01, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99, 0.999] {
            let z = inverse_normal_cdf(p).unwrap();
            assert!(
                (standard_normal_cdf(z) - p).abs() < 1e-6,
                "round-trip failed for p={p}"
            );
        }
    }

    #[test]
    fn test_inverse_normal_rejects_out_of_range() {
        assert!(inverse_normal_cdf(0.0).is_err());
        assert!(inverse_normal_cdf(1.0).is_err());
        assert!(inverse_normal_cdf(-0.1).is_err());
        assert!(inverse_normal_cdf(1.5).is_err());
    }

    #[test]
    fn test_parametric_es_uses_real_quantile() {
        // The parametric path must feed the REAL inverse-normal quantile into the
        // closed-form normal ES. We reconstruct the expected value with an
        // independently-computed `z_alpha` and `phi(z_alpha)`; a hardcoded
        // z-table would shift `phi(z_alpha)` enough to break the tight
        // tolerance, so this also guards against regressing to the old table.
        let returns = vec![dec!(-0.05), dec!(-0.03), dec!(0.01), dec!(0.02), dec!(0.03)];
        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::Parametric).unwrap();
        let result = calculator.calculate(&returns).unwrap();

        let n = returns.len() as f64;
        let mu: f64 = returns.iter().map(|r| r.to_f64().unwrap()).sum::<f64>() / n;
        // The implementation uses the population standard deviation (÷n).
        let var_pop: f64 = returns
            .iter()
            .map(|r| {
                let x = r.to_f64().unwrap() - mu;
                x * x
            })
            .sum::<f64>()
            / n;
        let sigma = var_pop.sqrt();
        let alpha = 0.05;
        // Real probit at the tail probability (negative); phi() of it drives ES.
        let z_alpha = inverse_normal_cdf(alpha).unwrap();
        assert!((z_alpha + 1.644_853_626_951_472).abs() < 1e-9);
        let analytic_es = (mu + sigma * phi(z_alpha) / alpha).abs();

        let es = result.expected_shortfall.to_f64().unwrap();
        assert!(
            (es - analytic_es).abs() < 1e-9,
            "parametric ES {es} vs analytic {analytic_es}"
        );
        assert!(result.expected_shortfall >= result.var);
    }

    #[test]
    fn test_monte_carlo_matches_analytic_normal_es() {
        // Build a large, near-perfectly-normal sample so the estimated mu/sigma
        // are well-defined, then check the simulated ES against the closed-form
        // normal ES: ES = mu - sigma * phi(z_alpha) / alpha (in loss terms,
        // reported as a positive magnitude).
        let mu = 0.001_f64;
        let sigma = 0.02_f64;
        let n = 20_000_usize;

        // Inverse-transform sampling on an even grid yields a deterministic,
        // tightly-normal empirical sample (no RNG dependence for the inputs).
        let mut returns: Vec<Decimal> = Vec::with_capacity(n);
        for i in 0..n {
            let u = (i as f64 + 0.5) / (n as f64);
            let z = inverse_normal_cdf(u).unwrap();
            returns.push(Decimal::from_f64(mu + sigma * z).unwrap());
        }

        let confidence = dec!(0.975);
        let alpha = 0.025_f64;
        let calculator =
            ExpectedShortfallCalculator::new(confidence, ESMethod::MonteCarlo).unwrap();
        let result = calculator
            .calculate_monte_carlo_with(&returns, 200_000, 0xDEAD_BEEF)
            .unwrap();

        // Analytic normal ES (loss magnitude): -mu + sigma * phi(z_alpha)/alpha
        // where z_alpha = Φ⁻¹(alpha) < 0.
        let z_alpha = inverse_normal_cdf(alpha).unwrap();
        let analytic_es_loss = -mu + sigma * phi(z_alpha) / alpha;
        let analytic_var_loss = -(mu + sigma * z_alpha);

        let es = result.expected_shortfall.to_f64().unwrap();
        let var = result.var.to_f64().unwrap();

        assert!(
            (es - analytic_es_loss).abs() < 1e-3,
            "MC ES {es} vs analytic {analytic_es_loss}"
        );
        assert!(
            (var - analytic_var_loss).abs() < 1e-3,
            "MC VaR {var} vs analytic {analytic_var_loss}"
        );
        // Fundamental coherence property: ES is never less severe than VaR.
        assert!(result.expected_shortfall >= result.var);
        assert!(result.tail_observations > 0);
        assert_eq!(result.method, ESMethod::MonteCarlo);
    }

    #[test]
    fn test_monte_carlo_is_reproducible() {
        let returns = vec![
            dec!(-0.05),
            dec!(-0.03),
            dec!(-0.02),
            dec!(-0.01),
            dec!(0.01),
            dec!(0.02),
            dec!(0.03),
            dec!(0.04),
        ];
        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::MonteCarlo).unwrap();
        let a = calculator
            .calculate_monte_carlo_with(&returns, 50_000, 42)
            .unwrap();
        let b = calculator
            .calculate_monte_carlo_with(&returns, 50_000, 42)
            .unwrap();
        assert_eq!(a.expected_shortfall, b.expected_shortfall);
        assert_eq!(a.var, b.var);
    }

    #[test]
    fn test_monte_carlo_via_calculate_dispatch() {
        // The public `calculate` entry point must route MonteCarlo to the real
        // simulation and produce a coherent ES >= VaR result.
        let returns = vec![dec!(-0.04), dec!(-0.02), dec!(0.0), dec!(0.02), dec!(0.04)];
        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::MonteCarlo).unwrap();
        let result = calculator.calculate(&returns).unwrap();
        assert_eq!(result.method, ESMethod::MonteCarlo);
        assert!(result.expected_shortfall >= result.var);
        assert!(result.expected_shortfall >= Decimal::ZERO);
    }

    #[test]
    fn test_monte_carlo_rejects_invalid_inputs() {
        let calculator =
            ExpectedShortfallCalculator::new(dec!(0.95), ESMethod::MonteCarlo).unwrap();
        assert!(calculator.calculate_monte_carlo_with(&[], 1000, 1).is_err());
        assert!(
            calculator
                .calculate_monte_carlo_with(&[dec!(0.01)], 0, 1)
                .is_err()
        );
    }
}
