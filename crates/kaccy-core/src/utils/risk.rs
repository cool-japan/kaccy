//! Risk management utilities
//!
//! This module provides utilities for managing trading risk including position sizing,
//! risk metrics, and portfolio analysis.

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Position sizing calculator
#[derive(Debug, Clone)]
pub struct PositionSizer {
    /// Total portfolio value
    portfolio_value: Decimal,
    /// Maximum risk per trade as percentage (e.g., 1% = 0.01)
    max_risk_per_trade: Decimal,
    /// Maximum position size as percentage of portfolio
    max_position_size: Decimal,
}

impl PositionSizer {
    /// Create a new position sizer
    pub fn new(portfolio_value: Decimal) -> Self {
        Self {
            portfolio_value,
            max_risk_per_trade: dec!(0.01), // 1%
            max_position_size: dec!(0.10),  // 10%
        }
    }

    /// Set maximum risk per trade
    pub fn with_max_risk_per_trade(mut self, risk_pct: Decimal) -> Self {
        self.max_risk_per_trade = risk_pct;
        self
    }

    /// Set maximum position size
    pub fn with_max_position_size(mut self, size_pct: Decimal) -> Self {
        self.max_position_size = size_pct;
        self
    }

    /// Calculate position size based on risk
    pub fn calculate_position_size(
        &self,
        entry_price: Decimal,
        stop_loss_price: Decimal,
    ) -> Result<Decimal> {
        if entry_price <= dec!(0) || stop_loss_price <= dec!(0) {
            return Err(CoreError::InvalidPrice(
                "Price must be positive".to_string(),
            ));
        }

        let risk_per_unit = (entry_price - stop_loss_price).abs();
        if risk_per_unit.is_zero() {
            return Err(CoreError::InvalidPrice(
                "Entry and stop loss prices cannot be equal".to_string(),
            ));
        }

        let max_risk_amount = self.portfolio_value * self.max_risk_per_trade;
        let position_size = max_risk_amount / risk_per_unit;

        // Cap position size at maximum percentage of portfolio
        let max_position_value = self.portfolio_value * self.max_position_size;
        let max_position_size = max_position_value / entry_price;

        Ok(position_size.min(max_position_size))
    }

    /// Get maximum dollar risk per trade
    pub fn max_risk_amount(&self) -> Decimal {
        self.portfolio_value * self.max_risk_per_trade
    }

    /// Get maximum position value
    pub fn max_position_value(&self) -> Decimal {
        self.portfolio_value * self.max_position_size
    }
}

/// Risk metrics for a position or portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    /// Value at Risk (VaR) - maximum expected loss
    pub value_at_risk: Decimal,
    /// Expected shortfall (conditional VaR)
    pub expected_shortfall: Decimal,
    /// Sharpe ratio (risk-adjusted return)
    pub sharpe_ratio: Option<Decimal>,
    /// Maximum drawdown percentage
    pub max_drawdown: Decimal,
    /// Current leverage ratio
    pub leverage_ratio: Decimal,
    /// Exposure as percentage of portfolio
    pub exposure_pct: Decimal,
}

impl RiskMetrics {
    /// Create new risk metrics
    pub fn new() -> Self {
        Self {
            value_at_risk: dec!(0),
            expected_shortfall: dec!(0),
            sharpe_ratio: None,
            max_drawdown: dec!(0),
            leverage_ratio: dec!(1),
            exposure_pct: dec!(0),
        }
    }

    /// Check if metrics are within acceptable thresholds
    pub fn is_acceptable(&self, max_leverage: Decimal, max_exposure_pct: Decimal) -> bool {
        self.leverage_ratio <= max_leverage && self.exposure_pct <= max_exposure_pct
    }

    /// Get risk level (low, medium, high, critical)
    pub fn risk_level(&self) -> RiskLevel {
        if self.leverage_ratio > dec!(5) || self.exposure_pct > dec!(80) {
            RiskLevel::Critical
        } else if self.leverage_ratio > dec!(3) || self.exposure_pct > dec!(60) {
            RiskLevel::High
        } else if self.leverage_ratio > dec!(1.5) || self.exposure_pct > dec!(40) {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}

impl Default for RiskMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Risk level classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Minimal risk; no action required.
    Low,
    /// Moderate risk; monitoring recommended.
    Medium,
    /// Elevated risk; action may be required.
    High,
    /// Severe risk; immediate action required.
    Critical,
}

/// Portfolio risk analyzer
#[derive(Debug, Clone)]
pub struct PortfolioRiskAnalyzer {
    /// Total portfolio value
    total_value: Decimal,
    /// Individual position values
    positions: Vec<Decimal>,
    /// Historical returns (for VaR calculation)
    returns: Vec<Decimal>,
}

impl PortfolioRiskAnalyzer {
    /// Create a new portfolio analyzer
    pub fn new(total_value: Decimal) -> Self {
        Self {
            total_value,
            positions: Vec::new(),
            returns: Vec::new(),
        }
    }

    /// Add a position
    pub fn add_position(&mut self, value: Decimal) {
        self.positions.push(value);
    }

    /// Add historical return
    pub fn add_return(&mut self, return_pct: Decimal) {
        self.returns.push(return_pct);
    }

    /// Calculate total exposure
    pub fn total_exposure(&self) -> Decimal {
        self.positions.iter().sum()
    }

    /// Calculate exposure percentage
    pub fn exposure_percentage(&self) -> Decimal {
        if self.total_value.is_zero() {
            return dec!(0);
        }
        (self.total_exposure() / self.total_value) * dec!(100)
    }

    /// Calculate concentration risk (Herfindahl index)
    pub fn concentration_risk(&self) -> Decimal {
        if self.positions.is_empty() {
            return dec!(0);
        }

        let total = self.total_exposure();
        if total.is_zero() {
            return dec!(0);
        }

        self.positions
            .iter()
            .map(|&pos| {
                let weight = pos / total;
                weight * weight
            })
            .sum()
    }

    /// Calculate diversification score (inverse of concentration)
    /// Returns 1.0 for fully diversified, approaches 0 for concentrated
    pub fn diversification_score(&self) -> Decimal {
        let n = Decimal::from(self.positions.len());
        if n.is_zero() {
            return dec!(0);
        }

        let herfindahl = self.concentration_risk();
        (dec!(1) / n - herfindahl) / (dec!(1) / n - dec!(1) / n)
    }

    /// Calculate Value at Risk (VaR) at given confidence level
    /// confidence_level: 0.95 for 95% confidence (5% probability of exceeding VaR)
    pub fn calculate_var(&self, confidence_level: Decimal) -> Decimal {
        if self.returns.is_empty() {
            return dec!(0);
        }

        let mut sorted_returns = self.returns.clone();
        sorted_returns.sort();

        let index = ((dec!(1) - confidence_level) * Decimal::from(sorted_returns.len()))
            .to_usize()
            .unwrap_or(0)
            .min(sorted_returns.len() - 1);

        // VaR is negative return at the percentile
        -sorted_returns[index] * self.total_value
    }

    /// Calculate expected shortfall (average loss beyond VaR)
    pub fn calculate_expected_shortfall(&self, confidence_level: Decimal) -> Decimal {
        if self.returns.is_empty() {
            return dec!(0);
        }

        let mut sorted_returns = self.returns.clone();
        sorted_returns.sort();

        let cutoff_index = ((dec!(1) - confidence_level) * Decimal::from(sorted_returns.len()))
            .to_usize()
            .unwrap_or(0);

        if cutoff_index == 0 {
            return dec!(0);
        }

        let worst_returns: Decimal = sorted_returns[..cutoff_index].iter().sum();
        let avg_worst = worst_returns / Decimal::from(cutoff_index);

        -avg_worst * self.total_value
    }

    /// Calculate Sharpe ratio (risk-adjusted return)
    /// risk_free_rate: annual risk-free rate (e.g., 0.02 for 2%)
    pub fn calculate_sharpe_ratio(&self, risk_free_rate: Decimal) -> Option<Decimal> {
        if self.returns.is_empty() {
            return None;
        }

        let mean_return: Decimal =
            self.returns.iter().sum::<Decimal>() / Decimal::from(self.returns.len());

        let variance: Decimal = self
            .returns
            .iter()
            .map(|&r| (r - mean_return) * (r - mean_return))
            .sum::<Decimal>()
            / Decimal::from(self.returns.len());

        let std_dev = variance.sqrt()?;

        if std_dev.is_zero() {
            return None;
        }

        Some((mean_return - risk_free_rate) / std_dev)
    }

    /// Calculate maximum drawdown
    pub fn calculate_max_drawdown(&self) -> Decimal {
        if self.returns.is_empty() {
            return dec!(0);
        }

        let mut cumulative_value = dec!(1);
        let mut peak_value = dec!(1);
        let mut max_drawdown = dec!(0);

        for &return_pct in &self.returns {
            cumulative_value *= dec!(1) + return_pct;
            peak_value = peak_value.max(cumulative_value);

            let drawdown = (peak_value - cumulative_value) / peak_value;
            max_drawdown = max_drawdown.max(drawdown);
        }

        max_drawdown * dec!(100) // Convert to percentage
    }

    /// Generate comprehensive risk metrics
    pub fn generate_metrics(&self, confidence_level: Decimal) -> RiskMetrics {
        RiskMetrics {
            value_at_risk: self.calculate_var(confidence_level),
            expected_shortfall: self.calculate_expected_shortfall(confidence_level),
            sharpe_ratio: self.calculate_sharpe_ratio(dec!(0.02)), // 2% risk-free rate
            max_drawdown: self.calculate_max_drawdown(),
            leverage_ratio: self.total_exposure() / self.total_value,
            exposure_pct: self.exposure_percentage(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_sizer_basic() {
        let sizer = PositionSizer::new(dec!(10000));
        assert_eq!(sizer.max_risk_amount(), dec!(100)); // 1% of 10000
        assert_eq!(sizer.max_position_value(), dec!(1000)); // 10% of 10000
    }

    #[test]
    fn test_position_sizer_calculation() {
        let sizer = PositionSizer::new(dec!(10000))
            .with_max_risk_per_trade(dec!(0.02)) // 2%
            .with_max_position_size(dec!(0.20)); // 20%

        let position_size = sizer.calculate_position_size(dec!(100), dec!(95)).unwrap();

        // Risk: $200 (2% of $10k), Risk per unit: $5, Position: 40 units
        // But capped at 20% of portfolio: $2000 / $100 = 20 units
        assert_eq!(position_size, dec!(20));
    }

    #[test]
    fn test_position_sizer_invalid_prices() {
        let sizer = PositionSizer::new(dec!(10000));

        let result = sizer.calculate_position_size(dec!(0), dec!(95));
        assert!(result.is_err());

        let result = sizer.calculate_position_size(dec!(100), dec!(100));
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_metrics_creation() {
        let metrics = RiskMetrics::new();
        assert_eq!(metrics.value_at_risk, dec!(0));
        assert_eq!(metrics.leverage_ratio, dec!(1));
    }

    #[test]
    fn test_risk_metrics_acceptability() {
        let mut metrics = RiskMetrics::new();
        metrics.leverage_ratio = dec!(2);
        metrics.exposure_pct = dec!(50);

        assert!(metrics.is_acceptable(dec!(3), dec!(60)));
        assert!(!metrics.is_acceptable(dec!(1.5), dec!(60)));
        assert!(!metrics.is_acceptable(dec!(3), dec!(40)));
    }

    #[test]
    fn test_risk_level() {
        let mut metrics = RiskMetrics::new();

        metrics.leverage_ratio = dec!(1);
        assert_eq!(metrics.risk_level(), RiskLevel::Low);

        metrics.leverage_ratio = dec!(2);
        assert_eq!(metrics.risk_level(), RiskLevel::Medium);

        metrics.leverage_ratio = dec!(4);
        assert_eq!(metrics.risk_level(), RiskLevel::High);

        metrics.leverage_ratio = dec!(6);
        assert_eq!(metrics.risk_level(), RiskLevel::Critical);
    }

    #[test]
    fn test_portfolio_exposure() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));
        analyzer.add_position(dec!(2000));
        analyzer.add_position(dec!(3000));

        assert_eq!(analyzer.total_exposure(), dec!(5000));
        assert_eq!(analyzer.exposure_percentage(), dec!(50));
    }

    #[test]
    fn test_concentration_risk() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));

        // Equal positions - low concentration
        analyzer.add_position(dec!(2500));
        analyzer.add_position(dec!(2500));
        analyzer.add_position(dec!(2500));
        analyzer.add_position(dec!(2500));

        let concentration = analyzer.concentration_risk();
        assert_eq!(concentration, dec!(0.25)); // 1/4 for equal weights
    }

    #[test]
    fn test_var_calculation() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));

        // Add some returns
        analyzer.add_return(dec!(0.02)); // +2%
        analyzer.add_return(dec!(0.01)); // +1%
        analyzer.add_return(dec!(-0.01)); // -1%
        analyzer.add_return(dec!(-0.02)); // -2%
        analyzer.add_return(dec!(-0.03)); // -3%

        let var_95 = analyzer.calculate_var(dec!(0.95));
        assert!(var_95 > dec!(0)); // Should be positive (loss amount)
    }

    #[test]
    fn test_max_drawdown() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));

        // Simulate a drawdown scenario
        analyzer.add_return(dec!(0.10)); // +10%
        analyzer.add_return(dec!(0.05)); // +5%
        analyzer.add_return(dec!(-0.15)); // -15%
        analyzer.add_return(dec!(-0.10)); // -10%
        analyzer.add_return(dec!(0.05)); // +5%

        let drawdown = analyzer.calculate_max_drawdown();
        assert!(drawdown > dec!(0));
    }

    #[test]
    fn test_sharpe_ratio() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));

        analyzer.add_return(dec!(0.05));
        analyzer.add_return(dec!(0.03));
        analyzer.add_return(dec!(0.04));
        analyzer.add_return(dec!(0.06));

        let sharpe = analyzer.calculate_sharpe_ratio(dec!(0.02));
        assert!(sharpe.is_some());
    }

    #[test]
    fn test_generate_metrics() {
        let mut analyzer = PortfolioRiskAnalyzer::new(dec!(10000));
        analyzer.add_position(dec!(5000));
        analyzer.add_return(dec!(0.01));
        analyzer.add_return(dec!(-0.01));

        let metrics = analyzer.generate_metrics(dec!(0.95));
        assert_eq!(metrics.exposure_pct, dec!(50));
        assert!(metrics.value_at_risk >= dec!(0));
    }
}
