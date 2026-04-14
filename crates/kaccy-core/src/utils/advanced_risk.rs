//! Advanced Risk Metrics
//!
//! This module provides sophisticated risk measurement techniques including
//! Expected Shortfall (ES/CVaR), Stress VaR, and Incremental Risk Charge.

use crate::error::{CoreError, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        // For now, use historical method with bootstrap
        // In a full implementation, would use Monte Carlo simulation
        self.calculate_historical(returns)
    }

    fn get_z_score(&self, alpha: Decimal) -> Result<Decimal> {
        // Approximate z-scores for common alpha values
        // This is a simplified approximation; a full implementation would use
        // proper inverse normal CDF
        let alpha_f64 = alpha.to_f64().unwrap_or(0.05);

        let z = match () {
            _ if (alpha_f64 - 0.01).abs() < 0.001 => -2.326, // 99% confidence
            _ if (alpha_f64 - 0.025).abs() < 0.001 => -1.960, // 97.5% confidence
            _ if (alpha_f64 - 0.05).abs() < 0.001 => -1.645, // 95% confidence
            _ if (alpha_f64 - 0.10).abs() < 0.001 => -1.282, // 90% confidence
            _ => {
                // Rough approximation for other values
                -((2.0 * std::f64::consts::PI).ln().sqrt() * alpha_f64.ln())
            }
        };

        Decimal::from_f64(z)
            .ok_or_else(|| CoreError::Calculation("Failed to convert z-score".to_string()))
    }

    fn normal_pdf(&self, z: Decimal) -> Decimal {
        let z_f64 = z.to_f64().unwrap_or(0.0);
        let pdf = (1.0 / (2.0 * std::f64::consts::PI).sqrt()) * (-0.5 * z_f64 * z_f64).exp();
        Decimal::from_f64(pdf).unwrap_or(Decimal::ZERO)
    }
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
}
