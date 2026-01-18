//! Portfolio optimization using Modern Portfolio Theory (MPT)
//!
//! Implements mean-variance optimization, efficient frontier calculation,
//! and risk-return optimization strategies.

use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Portfolio asset allocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAllocation {
    /// Token ID
    pub token_id: Uuid,
    /// Weight in portfolio (0-100%)
    pub weight: Decimal,
    /// Expected return (annual %)
    pub expected_return: Decimal,
    /// Volatility (standard deviation, annual %)
    pub volatility: Decimal,
}

/// Portfolio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    /// Portfolio ID
    pub id: Uuid,
    /// Asset allocations
    pub allocations: Vec<AssetAllocation>,
    /// Correlation matrix (token_id pairs -> correlation coefficient)
    pub correlations: HashMap<(Uuid, Uuid), Decimal>,
}

/// Portfolio metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    /// Expected return (annual %)
    pub expected_return: Decimal,
    /// Portfolio volatility (standard deviation, annual %)
    pub volatility: Decimal,
    /// Sharpe ratio (risk-adjusted return)
    pub sharpe_ratio: Decimal,
    /// Portfolio variance
    pub variance: Decimal,
}

/// Optimization constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationConstraint {
    /// Minimum weight for any asset
    pub min_weight: Decimal,
    /// Maximum weight for any asset
    pub max_weight: Decimal,
    /// Maximum turnover allowed (for rebalancing)
    pub max_turnover: Option<Decimal>,
    /// Target return (if specified)
    pub target_return: Option<Decimal>,
    /// Maximum volatility (if specified)
    pub max_volatility: Option<Decimal>,
}

/// Point on the efficient frontier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficientFrontierPoint {
    /// Expected return
    pub expected_return: Decimal,
    /// Volatility
    pub volatility: Decimal,
    /// Sharpe ratio
    pub sharpe_ratio: Decimal,
    /// Optimal weights
    pub weights: HashMap<Uuid, Decimal>,
}

impl Portfolio {
    /// Create a new portfolio
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            allocations: Vec::new(),
            correlations: HashMap::new(),
        }
    }

    /// Add an asset to the portfolio
    pub fn add_asset(
        &mut self,
        token_id: Uuid,
        weight: Decimal,
        expected_return: Decimal,
        volatility: Decimal,
    ) {
        self.allocations.push(AssetAllocation {
            token_id,
            weight,
            expected_return,
            volatility,
        });
    }

    /// Set correlation between two assets
    pub fn set_correlation(&mut self, token1: Uuid, token2: Uuid, correlation: Decimal) {
        self.correlations.insert((token1, token2), correlation);
        self.correlations.insert((token2, token1), correlation);
    }

    /// Get correlation between two assets (default 0 if not set)
    pub fn get_correlation(&self, token1: Uuid, token2: Uuid) -> Decimal {
        if token1 == token2 {
            return dec!(1.0);
        }
        self.correlations
            .get(&(token1, token2))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Calculate portfolio expected return
    pub fn calculate_expected_return(&self) -> Decimal {
        self.allocations
            .iter()
            .map(|a| (a.weight / dec!(100)) * a.expected_return)
            .sum()
    }

    /// Calculate portfolio variance
    pub fn calculate_variance(&self) -> Decimal {
        let mut variance = Decimal::ZERO;

        for i in 0..self.allocations.len() {
            for j in 0..self.allocations.len() {
                let asset_i = &self.allocations[i];
                let asset_j = &self.allocations[j];

                let weight_i = asset_i.weight / dec!(100);
                let weight_j = asset_j.weight / dec!(100);
                let correlation = self.get_correlation(asset_i.token_id, asset_j.token_id);

                variance +=
                    weight_i * weight_j * asset_i.volatility * asset_j.volatility * correlation;
            }
        }

        variance
    }

    /// Calculate portfolio volatility (standard deviation)
    pub fn calculate_volatility(&self) -> Decimal {
        let variance = self.calculate_variance();
        // Approximation of sqrt for Decimal
        self.sqrt_decimal(variance)
    }

    /// Calculate Sharpe ratio (assuming risk-free rate)
    pub fn calculate_sharpe_ratio(&self, risk_free_rate: Decimal) -> Decimal {
        let expected_return = self.calculate_expected_return();
        let volatility = self.calculate_volatility();

        if volatility == Decimal::ZERO {
            return Decimal::ZERO;
        }

        (expected_return - risk_free_rate) / volatility
    }

    /// Get portfolio metrics
    pub fn get_metrics(&self, risk_free_rate: Decimal) -> PortfolioMetrics {
        let expected_return = self.calculate_expected_return();
        let variance = self.calculate_variance();
        let volatility = self.sqrt_decimal(variance);
        let sharpe_ratio = if volatility > Decimal::ZERO {
            (expected_return - risk_free_rate) / volatility
        } else {
            Decimal::ZERO
        };

        PortfolioMetrics {
            expected_return,
            volatility,
            sharpe_ratio,
            variance,
        }
    }

    /// Normalize weights to sum to 100%
    pub fn normalize_weights(&mut self) {
        let total_weight: Decimal = self.allocations.iter().map(|a| a.weight).sum();

        if total_weight > Decimal::ZERO {
            for allocation in &mut self.allocations {
                allocation.weight = (allocation.weight / total_weight) * dec!(100);
            }
        }
    }

    /// Square root approximation for Decimal (Newton's method)
    fn sqrt_decimal(&self, x: Decimal) -> Decimal {
        if x <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let mut guess = x / dec!(2);
        for _ in 0..20 {
            // 20 iterations for convergence
            let next_guess = (guess + x / guess) / dec!(2);
            if (next_guess - guess).abs() < dec!(0.0001) {
                break;
            }
            guess = next_guess;
        }
        guess
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new()
    }
}

/// Portfolio optimizer
pub struct PortfolioOptimizer {
    /// Risk-free rate for Sharpe ratio calculation
    pub risk_free_rate: Decimal,
}

impl PortfolioOptimizer {
    /// Create a new optimizer
    pub fn new(risk_free_rate: Decimal) -> Self {
        Self { risk_free_rate }
    }

    /// Find maximum Sharpe ratio portfolio
    pub fn maximize_sharpe_ratio(
        &self,
        assets: &[AssetAllocation],
        correlations: &HashMap<(Uuid, Uuid), Decimal>,
        constraints: &OptimizationConstraint,
    ) -> Result<Portfolio> {
        if assets.is_empty() {
            return Err(CoreError::Validation("No assets provided".to_string()));
        }

        // Simplified optimization using grid search
        // In production, use proper quadratic programming
        let mut best_portfolio: Option<Portfolio> = None;
        let mut best_sharpe = Decimal::MIN;

        let n_assets = assets.len();
        let _step = dec!(0.05); // 5% steps

        // Generate random portfolios and find best Sharpe ratio
        for _ in 0..1000 {
            let mut portfolio = Portfolio::new();
            portfolio.correlations = correlations.clone();

            // Generate random weights within constraints
            let mut weights = Vec::new();
            let mut total = Decimal::ZERO;

            for _ in 0..n_assets {
                let w = constraints.min_weight
                    + (constraints.max_weight - constraints.min_weight) * self.random_decimal();
                weights.push(w);
                total += w;
            }

            // Skip if total is zero or too small to avoid division by zero
            if total <= Decimal::new(1, 6) {
                // total <= 0.000001
                continue;
            }

            // Normalize weights
            for (i, asset) in assets.iter().enumerate() {
                let normalized_weight = (weights[i] / total) * dec!(100);
                portfolio.add_asset(
                    asset.token_id,
                    normalized_weight,
                    asset.expected_return,
                    asset.volatility,
                );
            }

            let sharpe = portfolio.calculate_sharpe_ratio(self.risk_free_rate);

            if sharpe > best_sharpe {
                best_sharpe = sharpe;
                best_portfolio = Some(portfolio);
            }
        }

        best_portfolio.ok_or(CoreError::Validation(
            "Failed to find optimal portfolio".to_string(),
        ))
    }

    /// Find minimum variance portfolio
    pub fn minimize_variance(
        &self,
        assets: &[AssetAllocation],
        correlations: &HashMap<(Uuid, Uuid), Decimal>,
        constraints: &OptimizationConstraint,
    ) -> Result<Portfolio> {
        if assets.is_empty() {
            return Err(CoreError::Validation("No assets provided".to_string()));
        }

        let mut best_portfolio: Option<Portfolio> = None;
        let mut best_variance = Decimal::MAX;

        let n_assets = assets.len();

        for _ in 0..1000 {
            let mut portfolio = Portfolio::new();
            portfolio.correlations = correlations.clone();

            let mut weights = Vec::new();
            let mut total = Decimal::ZERO;

            for _ in 0..n_assets {
                let w = constraints.min_weight
                    + (constraints.max_weight - constraints.min_weight) * self.random_decimal();
                weights.push(w);
                total += w;
            }

            // Skip if total is zero or too small to avoid division by zero
            if total <= Decimal::new(1, 6) {
                // total <= 0.000001
                continue;
            }

            for (i, asset) in assets.iter().enumerate() {
                let normalized_weight = (weights[i] / total) * dec!(100);
                portfolio.add_asset(
                    asset.token_id,
                    normalized_weight,
                    asset.expected_return,
                    asset.volatility,
                );
            }

            let variance = portfolio.calculate_variance();

            if variance < best_variance {
                best_variance = variance;
                best_portfolio = Some(portfolio);
            }
        }

        best_portfolio.ok_or(CoreError::Validation(
            "Failed to find optimal portfolio".to_string(),
        ))
    }

    /// Calculate efficient frontier
    pub fn calculate_efficient_frontier(
        &self,
        assets: &[AssetAllocation],
        correlations: &HashMap<(Uuid, Uuid), Decimal>,
        constraints: &OptimizationConstraint,
        num_points: usize,
    ) -> Result<Vec<EfficientFrontierPoint>> {
        let mut frontier = Vec::new();

        // Find min and max return portfolios
        let min_return = assets
            .iter()
            .map(|a| a.expected_return)
            .fold(Decimal::MAX, Decimal::min);
        let max_return = assets
            .iter()
            .map(|a| a.expected_return)
            .fold(Decimal::MIN, Decimal::max);

        let step = (max_return - min_return) / Decimal::from(num_points.max(2) - 1);

        for i in 0..num_points {
            let target_return = min_return + step * Decimal::from(i);

            // Find portfolio with minimum variance for this target return
            let mut best_portfolio: Option<Portfolio> = None;
            let mut best_variance = Decimal::MAX;

            for _ in 0..500 {
                let mut portfolio = Portfolio::new();
                portfolio.correlations = correlations.clone();

                let mut weights = Vec::new();
                let mut total = Decimal::ZERO;

                for _ in 0..assets.len() {
                    let w = constraints.min_weight
                        + (constraints.max_weight - constraints.min_weight) * self.random_decimal();
                    weights.push(w);
                    total += w;
                }

                // Skip if total is zero or too small to avoid division by zero
                if total <= Decimal::new(1, 6) {
                    // total <= 0.000001
                    continue;
                }

                for (j, asset) in assets.iter().enumerate() {
                    let normalized_weight = (weights[j] / total) * dec!(100);
                    portfolio.add_asset(
                        asset.token_id,
                        normalized_weight,
                        asset.expected_return,
                        asset.volatility,
                    );
                }

                let actual_return = portfolio.calculate_expected_return();
                if (actual_return - target_return).abs() < dec!(0.5) {
                    let variance = portfolio.calculate_variance();
                    if variance < best_variance {
                        best_variance = variance;
                        best_portfolio = Some(portfolio);
                    }
                }
            }

            if let Some(portfolio) = best_portfolio {
                let metrics = portfolio.get_metrics(self.risk_free_rate);
                let weights: HashMap<Uuid, Decimal> = portfolio
                    .allocations
                    .iter()
                    .map(|a| (a.token_id, a.weight))
                    .collect();

                frontier.push(EfficientFrontierPoint {
                    expected_return: metrics.expected_return,
                    volatility: metrics.volatility,
                    sharpe_ratio: metrics.sharpe_ratio,
                    weights,
                });
            }
        }

        Ok(frontier)
    }

    /// Generate a random decimal between 0 and 1
    fn random_decimal(&self) -> Decimal {
        let mut rng = rand::rng();
        let random_value = rng.random_range(0u64..10000);
        Decimal::from(random_value) / dec!(10000)
    }
}

impl Default for PortfolioOptimizer {
    fn default() -> Self {
        Self::new(dec!(2.0)) // 2% risk-free rate
    }
}

/// Investor view for Black-Litterman model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorView {
    /// Token ID(s) for the view
    pub tokens: Vec<Uuid>,
    /// Expected return (annual %)
    pub expected_return: Decimal,
    /// Confidence level (0-1, where 1 is highest confidence)
    pub confidence: Decimal,
    /// View type (absolute or relative)
    pub view_type: ViewType,
}

/// Type of investor view
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViewType {
    /// Absolute view: "Token A will return X%"
    Absolute,
    /// Relative view: "Token A will outperform Token B by X%"
    Relative,
}

/// Black-Litterman model for portfolio optimization
/// Combines market equilibrium returns with investor views
#[derive(Debug)]
pub struct BlackLittermanModel {
    /// Risk aversion coefficient
    pub risk_aversion: Decimal,
    /// Market risk-free rate
    pub risk_free_rate: Decimal,
    /// Tau parameter (uncertainty in prior)
    pub tau: Decimal,
}

impl BlackLittermanModel {
    /// Create a new Black-Litterman model
    pub fn new(risk_aversion: Decimal, risk_free_rate: Decimal, tau: Decimal) -> Self {
        Self {
            risk_aversion,
            risk_free_rate,
            tau,
        }
    }

    /// Calculate market equilibrium returns from market cap weights
    /// Uses reverse optimization: Pi = lambda * Sigma * w_mkt
    pub fn calculate_equilibrium_returns(
        &self,
        market_weights: &HashMap<Uuid, Decimal>,
        covariance_matrix: &HashMap<(Uuid, Uuid), Decimal>,
    ) -> Result<HashMap<Uuid, Decimal>> {
        let mut equilibrium_returns = HashMap::new();

        for (token_id, &_weight) in market_weights {
            let mut return_value = Decimal::ZERO;

            // Pi_i = lambda * sum_j(Sigma_ij * w_j)
            for (other_token, &other_weight) in market_weights {
                let covariance = covariance_matrix
                    .get(&(*token_id, *other_token))
                    .copied()
                    .unwrap_or(Decimal::ZERO);

                return_value += covariance * other_weight;
            }

            return_value *= self.risk_aversion;
            equilibrium_returns.insert(*token_id, return_value);
        }

        Ok(equilibrium_returns)
    }

    /// Calculate posterior returns combining equilibrium and views
    /// Formula: E[R] = [(tau*Sigma)^-1 + P'*Omega^-1*P]^-1 * [(tau*Sigma)^-1*Pi + P'*Omega^-1*Q]
    /// Simplified version for practical use
    pub fn calculate_posterior_returns(
        &self,
        equilibrium_returns: &HashMap<Uuid, Decimal>,
        views: &[InvestorView],
        _covariance_matrix: &HashMap<(Uuid, Uuid), Decimal>,
    ) -> Result<HashMap<Uuid, Decimal>> {
        let mut posterior_returns = equilibrium_returns.clone();

        // For each view, adjust the posterior returns
        for view in views {
            let confidence_weight = view.confidence;

            match view.view_type {
                ViewType::Absolute => {
                    // Absolute view: blend equilibrium with view
                    if let Some(token_id) = view.tokens.first() {
                        if let Some(equilibrium) = equilibrium_returns.get(token_id) {
                            let blended = equilibrium * (Decimal::ONE - confidence_weight)
                                + view.expected_return * confidence_weight;
                            posterior_returns.insert(*token_id, blended);
                        }
                    }
                }
                ViewType::Relative => {
                    // Relative view: adjust both tokens
                    if view.tokens.len() >= 2 {
                        let token_a = view.tokens[0];
                        let token_b = view.tokens[1];

                        if let (Some(&eq_a), Some(&eq_b)) = (
                            equilibrium_returns.get(&token_a),
                            equilibrium_returns.get(&token_b),
                        ) {
                            // Expected outperformance
                            let adjustment = view.expected_return * confidence_weight;

                            // Adjust both tokens to maintain relative difference
                            let adj_a = eq_a + adjustment / dec!(2);
                            let adj_b = eq_b - adjustment / dec!(2);

                            posterior_returns.insert(token_a, adj_a);
                            posterior_returns.insert(token_b, adj_b);
                        }
                    }
                }
            }
        }

        Ok(posterior_returns)
    }

    /// Calculate posterior covariance matrix
    /// Formula: Sigma_posterior = [(tau*Sigma)^-1 + P'*Omega^-1*P]^-1
    /// Simplified: adjust covariance based on view confidence
    pub fn calculate_posterior_covariance(
        &self,
        prior_covariance: &HashMap<(Uuid, Uuid), Decimal>,
        views: &[InvestorView],
    ) -> Result<HashMap<(Uuid, Uuid), Decimal>> {
        let mut posterior_covariance = prior_covariance.clone();

        // Adjust uncertainty based on views
        for view in views {
            let uncertainty_reduction = view.confidence * self.tau;

            for token in &view.tokens {
                // Reduce variance for tokens with views
                if let Some(variance) = prior_covariance.get(&(*token, *token)) {
                    let adjusted_variance = variance * (Decimal::ONE - uncertainty_reduction);
                    posterior_covariance.insert((*token, *token), adjusted_variance);
                }
            }
        }

        Ok(posterior_covariance)
    }

    /// Generate optimal portfolio using Black-Litterman returns
    pub fn optimize_portfolio(
        &self,
        market_weights: &HashMap<Uuid, Decimal>,
        views: &[InvestorView],
        covariance_matrix: &HashMap<(Uuid, Uuid), Decimal>,
        constraints: &OptimizationConstraint,
    ) -> Result<HashMap<Uuid, Decimal>> {
        // Step 1: Calculate equilibrium returns
        let equilibrium_returns =
            self.calculate_equilibrium_returns(market_weights, covariance_matrix)?;

        // Step 2: Calculate posterior returns
        let posterior_returns =
            self.calculate_posterior_returns(&equilibrium_returns, views, covariance_matrix)?;

        // Step 3: Calculate posterior covariance
        let posterior_covariance = self.calculate_posterior_covariance(covariance_matrix, views)?;

        // Step 4: Optimize using posterior estimates
        // Simplified mean-variance optimization: w = (1/lambda) * Sigma^-1 * (R - Rf)
        let mut optimal_weights = HashMap::new();
        let mut total_weight = Decimal::ZERO;

        for (token_id, &expected_return) in &posterior_returns {
            // Simple allocation proportional to excess return / variance
            let variance = posterior_covariance
                .get(&(*token_id, *token_id))
                .copied()
                .unwrap_or(dec!(1));

            let excess_return = expected_return - self.risk_free_rate;
            let weight = if variance > Decimal::ZERO {
                (excess_return / variance).max(Decimal::ZERO)
            } else {
                Decimal::ZERO
            };

            optimal_weights.insert(*token_id, weight);
            total_weight += weight;
        }

        // Normalize weights to sum to 100%
        if total_weight > Decimal::ZERO {
            for weight in optimal_weights.values_mut() {
                *weight = (*weight / total_weight) * dec!(100);

                // Apply constraints
                *weight = (*weight)
                    .max(constraints.min_weight)
                    .min(constraints.max_weight);
            }
        }

        // Re-normalize after applying constraints
        let total: Decimal = optimal_weights.values().sum();
        if total > Decimal::ZERO {
            for weight in optimal_weights.values_mut() {
                *weight = (*weight / total) * dec!(100);
            }
        }

        Ok(optimal_weights)
    }
}

impl Default for BlackLittermanModel {
    fn default() -> Self {
        Self::new(
            dec!(2.5),  // Risk aversion coefficient
            dec!(2.0),  // 2% risk-free rate
            dec!(0.05), // 5% uncertainty in prior (tau)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_creation() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        portfolio.add_asset(token1, dec!(60), dec!(10), dec!(15));
        portfolio.add_asset(token2, dec!(40), dec!(8), dec!(10));

        assert_eq!(portfolio.allocations.len(), 2);
    }

    #[test]
    fn test_expected_return_calculation() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        portfolio.add_asset(token1, dec!(60), dec!(10), dec!(15));
        portfolio.add_asset(token2, dec!(40), dec!(8), dec!(10));

        let expected_return = portfolio.calculate_expected_return();
        assert_eq!(expected_return, dec!(9.2)); // 0.6*10 + 0.4*8 = 9.2
    }

    #[test]
    fn test_variance_calculation_uncorrelated() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        portfolio.add_asset(token1, dec!(50), dec!(10), dec!(20));
        portfolio.add_asset(token2, dec!(50), dec!(8), dec!(15));

        // Set correlation to 0
        portfolio.set_correlation(token1, token2, dec!(0));

        let variance = portfolio.calculate_variance();
        // Variance = 0.5^2 * 20^2 + 0.5^2 * 15^2 = 100 + 56.25 = 156.25
        assert!((variance - dec!(156.25)).abs() < dec!(0.01));
    }

    #[test]
    fn test_sharpe_ratio() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();

        portfolio.add_asset(token1, dec!(100), dec!(10), dec!(20));

        let sharpe = portfolio.calculate_sharpe_ratio(dec!(2)); // 2% risk-free rate
        // Sharpe = (10 - 2) / 20 = 0.4
        assert!((sharpe - dec!(0.4)).abs() < dec!(0.01));
    }

    #[test]
    fn test_weight_normalization() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        portfolio.add_asset(token1, dec!(30), dec!(10), dec!(15));
        portfolio.add_asset(token2, dec!(20), dec!(8), dec!(10));

        portfolio.normalize_weights();

        let total: Decimal = portfolio.allocations.iter().map(|a| a.weight).sum();
        assert_eq!(total, dec!(100));
    }

    #[test]
    fn test_correlation_symmetry() {
        let mut portfolio = Portfolio::new();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        portfolio.set_correlation(token1, token2, dec!(0.5));

        assert_eq!(
            portfolio.get_correlation(token1, token2),
            portfolio.get_correlation(token2, token1)
        );
    }

    #[test]
    fn test_optimizer_maximize_sharpe() {
        let optimizer = PortfolioOptimizer::new(dec!(2));
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let assets = vec![
            AssetAllocation {
                token_id: token1,
                weight: Decimal::ZERO,
                expected_return: dec!(10),
                volatility: dec!(20),
            },
            AssetAllocation {
                token_id: token2,
                weight: Decimal::ZERO,
                expected_return: dec!(8),
                volatility: dec!(15),
            },
        ];

        let mut correlations = HashMap::new();
        correlations.insert((token1, token2), dec!(0.3));
        correlations.insert((token2, token1), dec!(0.3));

        let constraints = OptimizationConstraint {
            min_weight: dec!(0),
            max_weight: dec!(100),
            max_turnover: None,
            target_return: None,
            max_volatility: None,
        };

        let result = optimizer.maximize_sharpe_ratio(&assets, &correlations, &constraints);
        assert!(result.is_ok());

        let portfolio = result.unwrap();
        assert_eq!(portfolio.allocations.len(), 2);

        let total: Decimal = portfolio.allocations.iter().map(|a| a.weight).sum();
        assert!((total - dec!(100)).abs() < dec!(0.01));
    }

    #[test]
    fn test_black_litterman_equilibrium_returns() {
        let bl_model = BlackLittermanModel::default();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let mut market_weights = HashMap::new();
        market_weights.insert(token1, dec!(0.6));
        market_weights.insert(token2, dec!(0.4));

        let mut covariance = HashMap::new();
        covariance.insert((token1, token1), dec!(0.04)); // 20% vol -> 0.04 variance
        covariance.insert((token1, token2), dec!(0.006));
        covariance.insert((token2, token1), dec!(0.006));
        covariance.insert((token2, token2), dec!(0.0225)); // 15% vol -> 0.0225 variance

        let result = bl_model.calculate_equilibrium_returns(&market_weights, &covariance);
        assert!(result.is_ok());

        let eq_returns = result.unwrap();
        assert_eq!(eq_returns.len(), 2);
        assert!(eq_returns.contains_key(&token1));
        assert!(eq_returns.contains_key(&token2));
    }

    #[test]
    fn test_black_litterman_absolute_view() {
        let bl_model = BlackLittermanModel::default();
        let token1 = Uuid::new_v4();

        let mut equilibrium_returns = HashMap::new();
        equilibrium_returns.insert(token1, dec!(8));

        let views = vec![InvestorView {
            tokens: vec![token1],
            expected_return: dec!(12),
            confidence: dec!(0.5),
            view_type: ViewType::Absolute,
        }];

        let covariance = HashMap::new();

        let result =
            bl_model.calculate_posterior_returns(&equilibrium_returns, &views, &covariance);
        assert!(result.is_ok());

        let posterior = result.unwrap();
        let posterior_return = posterior.get(&token1).unwrap();

        // Should be blend: 8 * 0.5 + 12 * 0.5 = 10
        assert_eq!(*posterior_return, dec!(10));
    }

    #[test]
    fn test_black_litterman_relative_view() {
        let bl_model = BlackLittermanModel::default();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let mut equilibrium_returns = HashMap::new();
        equilibrium_returns.insert(token1, dec!(8));
        equilibrium_returns.insert(token2, dec!(6));

        let views = vec![InvestorView {
            tokens: vec![token1, token2],
            expected_return: dec!(4), // Token1 outperforms Token2 by 4%
            confidence: dec!(1.0),
            view_type: ViewType::Relative,
        }];

        let covariance = HashMap::new();

        let result =
            bl_model.calculate_posterior_returns(&equilibrium_returns, &views, &covariance);
        assert!(result.is_ok());

        let posterior = result.unwrap();
        let return1 = *posterior.get(&token1).unwrap();
        let return2 = *posterior.get(&token2).unwrap();

        // Difference should reflect the view
        assert!(return1 > return2);
    }

    #[test]
    fn test_black_litterman_optimize_portfolio() {
        let bl_model = BlackLittermanModel::default();
        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let mut market_weights = HashMap::new();
        market_weights.insert(token1, dec!(0.6));
        market_weights.insert(token2, dec!(0.4));

        let mut covariance = HashMap::new();
        covariance.insert((token1, token1), dec!(0.04));
        covariance.insert((token1, token2), dec!(0.006));
        covariance.insert((token2, token1), dec!(0.006));
        covariance.insert((token2, token2), dec!(0.0225));

        let views = vec![InvestorView {
            tokens: vec![token1],
            expected_return: dec!(15),
            confidence: dec!(0.8),
            view_type: ViewType::Absolute,
        }];

        let constraints = OptimizationConstraint {
            min_weight: dec!(0),
            max_weight: dec!(100),
            max_turnover: None,
            target_return: None,
            max_volatility: None,
        };

        let result =
            bl_model.optimize_portfolio(&market_weights, &views, &covariance, &constraints);
        assert!(result.is_ok());

        let weights = result.unwrap();
        assert_eq!(weights.len(), 2);

        let total: Decimal = weights.values().sum();
        assert!((total - dec!(100)).abs() < dec!(0.01));
    }

    #[test]
    fn test_black_litterman_posterior_covariance() {
        let bl_model = BlackLittermanModel::default();
        let token1 = Uuid::new_v4();

        let mut prior_covariance = HashMap::new();
        prior_covariance.insert((token1, token1), dec!(0.04));

        let views = vec![InvestorView {
            tokens: vec![token1],
            expected_return: dec!(12),
            confidence: dec!(0.5),
            view_type: ViewType::Absolute,
        }];

        let result = bl_model.calculate_posterior_covariance(&prior_covariance, &views);
        assert!(result.is_ok());

        let posterior = result.unwrap();
        let posterior_var = *posterior.get(&(token1, token1)).unwrap();

        // Posterior variance should be less than prior (more certainty)
        assert!(posterior_var < dec!(0.04));
    }
}
