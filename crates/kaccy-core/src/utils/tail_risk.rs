//! Tail Risk Hedging
//!
//! This module provides tools for tail risk hedging, including black swan protection strategies,
//! tail risk parity, and crisis alpha strategies.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Tail risk hedging strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TailHedgingStrategy {
    /// Put option protection
    PutOptions {
        /// Strike price relative to current (e.g., 0.8 = 20% below)
        strike_ratio: Decimal,
        /// Percentage of portfolio to protect
        coverage: Decimal,
    },
    /// Volatility instruments (VIX, volatility swaps)
    Volatility {
        /// Target volatility exposure
        target_exposure: Decimal,
    },
    /// Trend following
    TrendFollowing {
        /// Lookback period in days
        lookback_days: usize,
        /// Allocation to trend strategy
        allocation: Decimal,
    },
    /// Diversification to safe havens (gold, treasuries)
    SafeHaven {
        /// Allocation to safe haven assets
        allocation: Decimal,
    },
    /// Tail risk parity
    TailRiskParity {
        /// Target tail risk contribution per asset
        target_tail_contribution: Decimal,
    },
}

/// Black swan scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackSwanScenario {
    /// Scenario name
    pub name: String,
    /// Probability (very low)
    pub probability: Decimal,
    /// Market crash percentage
    pub market_crash_pct: Decimal,
    /// Volatility spike multiplier
    pub volatility_spike: Decimal,
    /// Correlation spike (correlations go to 1)
    pub correlation_spike: Decimal,
}

impl BlackSwanScenario {
    /// Create a financial crisis scenario (2008-style)
    pub fn financial_crisis() -> Self {
        Self {
            name: "Financial Crisis".to_string(),
            probability: dec!(0.01),      // 1% annual probability
            market_crash_pct: dec!(0.40), // 40% market crash
            volatility_spike: dec!(3.0),  // 3x volatility
            correlation_spike: dec!(0.9), // Correlations approach 1
        }
    }

    /// Create a crypto winter scenario
    pub fn crypto_winter() -> Self {
        Self {
            name: "Crypto Winter".to_string(),
            probability: dec!(0.05),       // 5% annual probability
            market_crash_pct: dec!(0.70),  // 70% crash
            volatility_spike: dec!(4.0),   // 4x volatility
            correlation_spike: dec!(0.95), // Very high correlation
        }
    }

    /// Create a flash crash scenario
    pub fn flash_crash() -> Self {
        Self {
            name: "Flash Crash".to_string(),
            probability: dec!(0.10),      // 10% annual probability
            market_crash_pct: dec!(0.20), // 20% crash
            volatility_spike: dec!(5.0),  // 5x volatility spike
            correlation_spike: dec!(0.80),
        }
    }

    /// Create a black swan event (unknown unknown)
    pub fn black_swan() -> Self {
        Self {
            name: "Black Swan Event".to_string(),
            probability: dec!(0.001),     // 0.1% annual probability
            market_crash_pct: dec!(0.50), // 50%+ crash
            volatility_spike: dec!(10.0), // 10x volatility
            correlation_spike: dec!(1.0), // Perfect correlation
        }
    }
}

/// Tail risk metrics for a portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailRiskMetrics {
    /// Expected Shortfall at 1% level
    pub expected_shortfall_1pct: Decimal,
    /// Expected Shortfall at 5% level
    pub expected_shortfall_5pct: Decimal,
    /// Maximum drawdown
    pub max_drawdown: Decimal,
    /// Tail ratio (gain in top 5% / loss in bottom 5%)
    pub tail_ratio: Decimal,
    /// Skewness (negative = left tail risk)
    pub skewness: Decimal,
    /// Excess kurtosis (positive = fat tails)
    pub kurtosis: Decimal,
}

/// Tail risk hedger
pub struct TailRiskHedger {
    /// Current portfolio value
    portfolio_value: Decimal,
    /// Hedging strategies
    strategies: Vec<TailHedgingStrategy>,
    /// Black swan scenarios to protect against
    scenarios: Vec<BlackSwanScenario>,
}

impl TailRiskHedger {
    /// Create a new tail risk hedger
    pub fn new(portfolio_value: Decimal) -> Self {
        Self {
            portfolio_value,
            strategies: Vec::new(),
            scenarios: vec![
                BlackSwanScenario::financial_crisis(),
                BlackSwanScenario::crypto_winter(),
                BlackSwanScenario::flash_crash(),
            ],
        }
    }

    /// Add a hedging strategy
    pub fn add_strategy(&mut self, strategy: TailHedgingStrategy) {
        self.strategies.push(strategy);
    }

    /// Calculate cost of hedging
    pub fn calculate_hedging_cost(&self) -> Decimal {
        let mut total_cost = dec!(0);

        for strategy in &self.strategies {
            let cost = match strategy {
                TailHedgingStrategy::PutOptions {
                    strike_ratio,
                    coverage,
                } => {
                    // Cost ≈ 2-5% of notional per year for OTM puts
                    let notional = self.portfolio_value * coverage;
                    let otm_factor = dec!(1) - strike_ratio; // How far out of the money
                    let base_cost = dec!(0.03); // 3% base cost
                    notional * base_cost * (dec!(1) - otm_factor)
                }
                TailHedgingStrategy::Volatility { target_exposure } => {
                    // Volatility instruments have negative carry in normal markets
                    target_exposure * dec!(0.05) // ~5% annual cost
                }
                TailHedgingStrategy::TrendFollowing { allocation, .. } => {
                    // Transaction costs and slippage
                    self.portfolio_value * allocation * dec!(0.01) // 1% cost
                }
                TailHedgingStrategy::SafeHaven { allocation } => {
                    // Opportunity cost (lower expected returns)
                    self.portfolio_value * allocation * dec!(0.02) // 2% opportunity cost
                }
                TailHedgingStrategy::TailRiskParity { .. } => {
                    // Rebalancing costs
                    self.portfolio_value * dec!(0.005) // 0.5% rebalancing cost
                }
            };

            total_cost += cost;
        }

        total_cost
    }

    /// Calculate expected protection in a scenario
    pub fn calculate_protection(&self, scenario: &BlackSwanScenario) -> Decimal {
        let unhedged_loss = self.portfolio_value * scenario.market_crash_pct;
        let mut total_protection = dec!(0);

        for strategy in &self.strategies {
            let protection = match strategy {
                TailHedgingStrategy::PutOptions {
                    strike_ratio,
                    coverage,
                } => {
                    // Put pays off when market drops below strike
                    let strike_level = dec!(1) - (dec!(1) - strike_ratio);
                    let crash_level = dec!(1) - scenario.market_crash_pct;

                    if crash_level < strike_level {
                        // Put is in the money
                        let payoff_pct = strike_level - crash_level;
                        self.portfolio_value * coverage * payoff_pct
                    } else {
                        dec!(0)
                    }
                }
                TailHedgingStrategy::Volatility { target_exposure } => {
                    // Volatility spikes in crises
                    target_exposure * scenario.volatility_spike * dec!(0.5)
                }
                TailHedgingStrategy::TrendFollowing { allocation, .. } => {
                    // Trend following profits in sustained moves
                    if scenario.market_crash_pct > dec!(0.30) {
                        self.portfolio_value * allocation * dec!(0.20) // 20% profit
                    } else {
                        dec!(0)
                    }
                }
                TailHedgingStrategy::SafeHaven { allocation } => {
                    // Safe havens tend to rise or hold value
                    self.portfolio_value * allocation * dec!(0.10) // 10% gain in crisis
                }
                TailHedgingStrategy::TailRiskParity { .. } => {
                    // Better balanced tail risk
                    unhedged_loss * dec!(0.30) // 30% reduction in tail loss
                }
            };

            total_protection += protection;
        }

        total_protection
    }

    /// Calculate hedge effectiveness ratio
    pub fn hedge_effectiveness(&self, scenario: &BlackSwanScenario) -> Decimal {
        let unhedged_loss = self.portfolio_value * scenario.market_crash_pct;
        let protection = self.calculate_protection(scenario);

        if unhedged_loss > dec!(0) {
            protection / unhedged_loss
        } else {
            dec!(0)
        }
    }

    /// Calculate crisis alpha (return in crisis scenario)
    pub fn crisis_alpha(&self, scenario: &BlackSwanScenario) -> Decimal {
        let unhedged_loss = self.portfolio_value * scenario.market_crash_pct;
        let protection = self.calculate_protection(scenario);
        let hedging_cost = self.calculate_hedging_cost();

        let net_loss = unhedged_loss - protection + hedging_cost;

        // Crisis alpha = (protected loss - unhedged loss) / portfolio value
        ((unhedged_loss - net_loss) / self.portfolio_value) * dec!(100)
    }

    /// Calculate expected utility with hedging
    pub fn expected_utility(&self, risk_aversion: Decimal) -> Decimal {
        let hedging_cost = self.calculate_hedging_cost();
        let mut expected_value = -hedging_cost; // Start with cost

        for scenario in &self.scenarios {
            let unhedged_loss = self.portfolio_value * scenario.market_crash_pct;
            let protection = self.calculate_protection(scenario);
            let net_loss = unhedged_loss - protection;

            // Expected utility contribution = probability × (protection - risk_penalty)
            let risk_penalty = net_loss * risk_aversion;
            expected_value += scenario.probability * (protection - risk_penalty);
        }

        expected_value
    }

    /// Calculate optimal hedge ratio using mean-variance approach
    pub fn optimal_hedge_ratio(&self, _expected_return: Decimal, volatility: Decimal) -> Decimal {
        // Optimal hedge = (Expected Loss - Hedging Cost) / Variance of Loss
        let expected_loss: Decimal = self
            .scenarios
            .iter()
            .map(|s| s.probability * self.portfolio_value * s.market_crash_pct)
            .sum();

        let hedging_cost = self.calculate_hedging_cost();

        if volatility > dec!(0) {
            ((expected_loss - hedging_cost) / (volatility * volatility)).max(dec!(0))
        } else {
            dec!(0)
        }
    }
}

/// Tail Risk Parity calculator
pub struct TailRiskParity;

impl TailRiskParity {
    /// Calculate tail risk contribution for each asset
    pub fn calculate_tail_contributions(
        weights: &[Decimal],
        tail_covariance: &[Vec<Decimal>],
    ) -> Vec<Decimal> {
        let n = weights.len();
        let mut contributions = vec![dec!(0); n];

        for i in 0..n {
            let mut tail_var_contribution = dec!(0);

            for (j, weight) in weights.iter().enumerate().take(n) {
                tail_var_contribution += weight * tail_covariance[i][j];
            }

            contributions[i] = weights[i] * tail_var_contribution;
        }

        contributions
    }

    /// Calculate optimal weights for equal tail risk contribution
    pub fn optimize_weights(
        initial_weights: &[Decimal],
        tail_covariance: &[Vec<Decimal>],
        max_iterations: usize,
    ) -> Vec<Decimal> {
        let n = initial_weights.len();
        let mut weights = initial_weights.to_vec();

        for _ in 0..max_iterations {
            let contributions = Self::calculate_tail_contributions(&weights, tail_covariance);
            let avg_contribution: Decimal =
                contributions.iter().sum::<Decimal>() / Decimal::from(n as i64);

            // Adjust weights to equalize tail risk contributions
            for i in 0..n {
                if contributions[i] > dec!(0) {
                    let adjustment = avg_contribution / contributions[i];
                    weights[i] *= dec!(1) + (adjustment - dec!(1)) * dec!(0.1); // Small step
                }
            }

            // Normalize weights to sum to 1
            let total: Decimal = weights.iter().sum();
            if total > dec!(0) {
                for weight in &mut weights {
                    *weight /= total;
                }
            }
        }

        weights
    }

    /// Calculate tail risk ratio (measure of imbalance)
    pub fn tail_risk_ratio(weights: &[Decimal], tail_covariance: &[Vec<Decimal>]) -> Decimal {
        let contributions = Self::calculate_tail_contributions(weights, tail_covariance);

        if contributions.is_empty() {
            return dec!(0);
        }

        let max_contrib = contributions.iter().max().copied().unwrap_or(dec!(0));
        let min_contrib = contributions
            .iter()
            .filter(|&&c| c > dec!(0))
            .min()
            .copied()
            .unwrap_or(dec!(0));

        if min_contrib > dec!(0) {
            max_contrib / min_contrib
        } else {
            dec!(0)
        }
    }
}

/// Crisis alpha strategy
pub struct CrisisAlphaStrategy {
    /// Strategy name
    pub name: String,
    /// Normal market expected return (annual)
    pub normal_return: Decimal,
    /// Crisis market expected return (annual)
    pub crisis_return: Decimal,
    /// Volatility in normal markets
    pub normal_volatility: Decimal,
    /// Volatility in crisis markets
    pub crisis_volatility: Decimal,
}

impl CrisisAlphaStrategy {
    /// Create a long volatility strategy
    pub fn long_volatility() -> Self {
        Self {
            name: "Long Volatility".to_string(),
            normal_return: dec!(-0.10), // -10% carry cost
            crisis_return: dec!(0.50),  // +50% in crisis
            normal_volatility: dec!(0.30),
            crisis_volatility: dec!(0.80),
        }
    }

    /// Create a trend following strategy
    pub fn trend_following() -> Self {
        Self {
            name: "Trend Following".to_string(),
            normal_return: dec!(0.05), // +5% in normal markets
            crisis_return: dec!(0.25), // +25% in crisis
            normal_volatility: dec!(0.15),
            crisis_volatility: dec!(0.25),
        }
    }

    /// Create a tail risk hedge fund strategy
    pub fn tail_hedge_fund() -> Self {
        Self {
            name: "Tail Risk Hedge Fund".to_string(),
            normal_return: dec!(-0.05), // -5% carry
            crisis_return: dec!(1.00),  // +100% in crisis
            normal_volatility: dec!(0.40),
            crisis_volatility: dec!(1.50),
        }
    }

    /// Calculate expected return given crisis probability
    pub fn expected_return(&self, crisis_probability: Decimal) -> Decimal {
        crisis_probability * self.crisis_return
            + (dec!(1) - crisis_probability) * self.normal_return
    }

    /// Calculate Sharpe ratio in normal markets
    pub fn normal_sharpe(&self, risk_free_rate: Decimal) -> Decimal {
        if self.normal_volatility > dec!(0) {
            (self.normal_return - risk_free_rate) / self.normal_volatility
        } else {
            dec!(0)
        }
    }

    /// Calculate crisis alpha (excess return in crisis)
    pub fn crisis_alpha(&self, market_crash_pct: Decimal) -> Decimal {
        // Crisis alpha = Strategy return - Market return
        self.crisis_return - (-market_crash_pct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_black_swan_scenarios() {
        let crisis = BlackSwanScenario::financial_crisis();
        assert_eq!(crisis.market_crash_pct, dec!(0.40));
        assert!(crisis.probability > dec!(0));

        let crypto = BlackSwanScenario::crypto_winter();
        assert_eq!(crypto.market_crash_pct, dec!(0.70));

        let flash = BlackSwanScenario::flash_crash();
        assert_eq!(flash.market_crash_pct, dec!(0.20));
    }

    #[test]
    fn test_tail_risk_hedger_creation() {
        let hedger = TailRiskHedger::new(dec!(100000));
        assert_eq!(hedger.portfolio_value, dec!(100000));
        assert_eq!(hedger.strategies.len(), 0);
    }

    #[test]
    fn test_put_option_strategy_cost() {
        let mut hedger = TailRiskHedger::new(dec!(100000));
        hedger.add_strategy(TailHedgingStrategy::PutOptions {
            strike_ratio: dec!(0.90), // 10% OTM
            coverage: dec!(0.50),     // 50% coverage
        });

        let cost = hedger.calculate_hedging_cost();
        assert!(cost > dec!(0));
        assert!(cost < dec!(5000)); // Should be reasonable
    }

    #[test]
    fn test_put_option_protection() {
        let mut hedger = TailRiskHedger::new(dec!(100000));
        hedger.add_strategy(TailHedgingStrategy::PutOptions {
            strike_ratio: dec!(0.80), // 20% OTM
            coverage: dec!(1.0),      // 100% coverage
        });

        let scenario = BlackSwanScenario::financial_crisis(); // 40% crash
        let protection = hedger.calculate_protection(&scenario);

        // Put should pay off: (80% - 60%) * 100000 = 20000
        assert!(protection > dec!(15000));
        assert!(protection < dec!(25000));
    }

    #[test]
    fn test_hedge_effectiveness() {
        let mut hedger = TailRiskHedger::new(dec!(100000));
        hedger.add_strategy(TailHedgingStrategy::PutOptions {
            strike_ratio: dec!(0.80),
            coverage: dec!(1.0),
        });

        let scenario = BlackSwanScenario::financial_crisis();
        let effectiveness = hedger.hedge_effectiveness(&scenario);

        // Should protect against part of the loss
        assert!(effectiveness > dec!(0));
        assert!(effectiveness <= dec!(1));
    }

    #[test]
    fn test_crisis_alpha() {
        let mut hedger = TailRiskHedger::new(dec!(100000));
        hedger.add_strategy(TailHedgingStrategy::Volatility {
            target_exposure: dec!(10000),
        });

        let scenario = BlackSwanScenario::flash_crash();
        let alpha = hedger.crisis_alpha(&scenario);

        // Crisis alpha should be positive if hedging works
        assert!(alpha != dec!(0));
    }

    #[test]
    fn test_tail_risk_parity_contributions() {
        let weights = vec![dec!(0.5), dec!(0.3), dec!(0.2)];
        let tail_cov = vec![
            vec![dec!(0.04), dec!(0.02), dec!(0.01)],
            vec![dec!(0.02), dec!(0.09), dec!(0.03)],
            vec![dec!(0.01), dec!(0.03), dec!(0.16)],
        ];

        let contributions = TailRiskParity::calculate_tail_contributions(&weights, &tail_cov);

        assert_eq!(contributions.len(), 3);
        // All contributions should be positive
        assert!(contributions.iter().all(|&c| c >= dec!(0)));
    }

    #[test]
    fn test_tail_risk_parity_optimization() {
        let initial = vec![dec!(0.5), dec!(0.3), dec!(0.2)];
        let tail_cov = vec![
            vec![dec!(0.04), dec!(0.01), dec!(0.01)],
            vec![dec!(0.01), dec!(0.04), dec!(0.01)],
            vec![dec!(0.01), dec!(0.01), dec!(0.04)],
        ];

        let optimized = TailRiskParity::optimize_weights(&initial, &tail_cov, 10);

        // Weights should sum to approximately 1
        let sum: Decimal = optimized.iter().sum();
        assert!(sum > dec!(0.95) && sum < dec!(1.05));

        // All weights should be positive
        assert!(optimized.iter().all(|&w| w >= dec!(0)));
    }

    #[test]
    fn test_crisis_alpha_strategy() {
        let strategy = CrisisAlphaStrategy::long_volatility();

        assert_eq!(strategy.name, "Long Volatility");
        assert!(strategy.normal_return < dec!(0)); // Negative carry
        assert!(strategy.crisis_return > dec!(0)); // Positive in crisis
    }

    #[test]
    fn test_strategy_expected_return() {
        let strategy = CrisisAlphaStrategy::trend_following();
        let crisis_prob = dec!(0.10); // 10% probability

        let exp_return = strategy.expected_return(crisis_prob);

        // Should be weighted average
        assert!(exp_return > strategy.normal_return);
        assert!(exp_return < strategy.crisis_return);
    }

    #[test]
    fn test_strategy_crisis_alpha() {
        let strategy = CrisisAlphaStrategy::tail_hedge_fund();
        let market_crash = dec!(0.40); // 40% crash

        let alpha = strategy.crisis_alpha(market_crash);

        // Alpha = 100% - (-40%) = 140%
        assert!(alpha > dec!(1.0));
    }

    #[test]
    fn test_multiple_strategies() {
        let mut hedger = TailRiskHedger::new(dec!(1000000));

        hedger.add_strategy(TailHedgingStrategy::PutOptions {
            strike_ratio: dec!(0.85),
            coverage: dec!(0.50),
        });

        hedger.add_strategy(TailHedgingStrategy::SafeHaven {
            allocation: dec!(0.10),
        });

        let cost = hedger.calculate_hedging_cost();
        assert!(cost > dec!(0));

        let scenario = BlackSwanScenario::crypto_winter();
        let protection = hedger.calculate_protection(&scenario);
        assert!(protection > dec!(0));
    }

    #[test]
    fn test_optimal_hedge_ratio() {
        let mut hedger = TailRiskHedger::new(dec!(100000));
        hedger.add_strategy(TailHedgingStrategy::PutOptions {
            strike_ratio: dec!(0.80),
            coverage: dec!(1.0),
        });

        let expected_return = dec!(0.10);
        let volatility = dec!(0.20);

        let ratio = hedger.optimal_hedge_ratio(expected_return, volatility);
        assert!(ratio >= dec!(0));
    }
}
