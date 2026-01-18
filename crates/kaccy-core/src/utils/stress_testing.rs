//! Portfolio Stress Testing
//!
//! This module provides comprehensive stress testing capabilities for portfolios,
//! including scenario analysis, historical stress scenarios, Monte Carlo simulation,
//! and extreme value analysis.

use crate::error::Result;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Token ID
    pub token_id: i64,
    /// Quantity held
    pub quantity: Decimal,
    /// Current price
    pub current_price: Decimal,
    /// Historical volatility (annualized)
    pub volatility: Decimal,
}

impl Position {
    /// Calculate the current value of the position
    pub fn value(&self) -> Decimal {
        self.quantity * self.current_price
    }
}

/// Represents a portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    /// Portfolio positions
    pub positions: Vec<Position>,
    /// Cash balance
    pub cash: Decimal,
}

impl Portfolio {
    /// Create a new portfolio
    pub fn new(positions: Vec<Position>, cash: Decimal) -> Self {
        Self { positions, cash }
    }

    /// Calculate total portfolio value
    pub fn total_value(&self) -> Decimal {
        let positions_value: Decimal = self.positions.iter().map(|p| p.value()).sum();
        positions_value + self.cash
    }

    /// Apply a shock scenario to the portfolio
    pub fn apply_shock(&self, shocks: &HashMap<i64, Decimal>) -> Portfolio {
        let shocked_positions: Vec<Position> = self
            .positions
            .iter()
            .map(|p| {
                let shock = shocks.get(&p.token_id).copied().unwrap_or(dec!(0));
                Position {
                    token_id: p.token_id,
                    quantity: p.quantity,
                    current_price: p.current_price * (dec!(1) + shock),
                    volatility: p.volatility,
                }
            })
            .collect();

        Portfolio {
            positions: shocked_positions,
            cash: self.cash,
        }
    }
}

/// Stress test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    /// Scenario name
    pub name: String,
    /// Description
    pub description: String,
    /// Price shocks per token (token_id -> percentage change)
    pub shocks: HashMap<i64, Decimal>,
}

impl StressScenario {
    /// Create a new stress scenario
    pub fn new(name: String, description: String, shocks: HashMap<i64, Decimal>) -> Self {
        Self {
            name,
            description,
            shocks,
        }
    }

    /// Market crash scenario (-30% across all assets)
    pub fn market_crash(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            shocks.insert(token_id, dec!(-0.30));
        }
        Self {
            name: "Market Crash".to_string(),
            description: "30% decline across all assets".to_string(),
            shocks,
        }
    }

    /// Flash crash scenario (-50% across all assets)
    pub fn flash_crash(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            shocks.insert(token_id, dec!(-0.50));
        }
        Self {
            name: "Flash Crash".to_string(),
            description: "50% decline across all assets".to_string(),
            shocks,
        }
    }

    /// Moderate correction scenario (-15% across all assets)
    pub fn moderate_correction(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            shocks.insert(token_id, dec!(-0.15));
        }
        Self {
            name: "Moderate Correction".to_string(),
            description: "15% decline across all assets".to_string(),
            shocks,
        }
    }

    /// Bull market scenario (+50% across all assets)
    pub fn bull_market(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            shocks.insert(token_id, dec!(0.50));
        }
        Self {
            name: "Bull Market".to_string(),
            description: "50% increase across all assets".to_string(),
            shocks,
        }
    }
}

/// Result of a stress test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    /// Scenario name
    pub scenario_name: String,
    /// Initial portfolio value
    pub initial_value: Decimal,
    /// Portfolio value after shock
    pub stressed_value: Decimal,
    /// Absolute loss/gain
    pub pnl: Decimal,
    /// Percentage change
    pub pnl_percentage: Decimal,
}

impl StressTestResult {
    /// Create a new stress test result
    pub fn new(scenario_name: String, initial_value: Decimal, stressed_value: Decimal) -> Self {
        let pnl = stressed_value - initial_value;
        let pnl_percentage = if initial_value > dec!(0) {
            (pnl / initial_value) * dec!(100)
        } else {
            dec!(0)
        };

        Self {
            scenario_name,
            initial_value,
            stressed_value,
            pnl,
            pnl_percentage,
        }
    }
}

/// Stress tester
pub struct StressTester {
    /// Portfolio to test
    portfolio: Portfolio,
}

impl StressTester {
    /// Create a new stress tester
    pub fn new(portfolio: Portfolio) -> Self {
        Self { portfolio }
    }

    /// Run a single stress scenario
    pub fn run_scenario(&self, scenario: &StressScenario) -> StressTestResult {
        let initial_value = self.portfolio.total_value();
        let stressed_portfolio = self.portfolio.apply_shock(&scenario.shocks);
        let stressed_value = stressed_portfolio.total_value();

        StressTestResult::new(scenario.name.clone(), initial_value, stressed_value)
    }

    /// Run multiple stress scenarios
    pub fn run_scenarios(&self, scenarios: &[StressScenario]) -> Vec<StressTestResult> {
        scenarios.iter().map(|s| self.run_scenario(s)).collect()
    }

    /// Run default stress scenarios
    pub fn run_default_scenarios(&self) -> Vec<StressTestResult> {
        let token_ids: Vec<i64> = self
            .portfolio
            .positions
            .iter()
            .map(|p| p.token_id)
            .collect();

        let scenarios = vec![
            StressScenario::market_crash(&token_ids),
            StressScenario::flash_crash(&token_ids),
            StressScenario::moderate_correction(&token_ids),
            StressScenario::bull_market(&token_ids),
        ];

        self.run_scenarios(&scenarios)
    }
}

/// Monte Carlo simulation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloParams {
    /// Number of simulations
    pub num_simulations: usize,
    /// Time horizon in days
    pub time_horizon_days: usize,
    /// Confidence level for VaR/CVaR (e.g., 0.95 for 95%)
    pub confidence_level: Decimal,
}

impl Default for MonteCarloParams {
    fn default() -> Self {
        Self {
            num_simulations: 10000,
            time_horizon_days: 30,
            confidence_level: dec!(0.95),
        }
    }
}

/// Monte Carlo simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    /// Simulated portfolio values
    pub simulated_values: Vec<Decimal>,
    /// Mean portfolio value
    pub mean_value: Decimal,
    /// Median portfolio value
    pub median_value: Decimal,
    /// 5th percentile (worst 5%)
    pub percentile_5: Decimal,
    /// 95th percentile (best 5%)
    pub percentile_95: Decimal,
    /// Value at Risk (VaR) at confidence level
    pub var: Decimal,
    /// Conditional Value at Risk (CVaR/Expected Shortfall)
    pub cvar: Decimal,
    /// Probability of loss
    pub prob_of_loss: Decimal,
}

/// Monte Carlo simulator
pub struct MonteCarloSimulator {
    /// Portfolio to simulate
    portfolio: Portfolio,
    /// Simulation parameters
    params: MonteCarloParams,
}

impl MonteCarloSimulator {
    /// Create a new Monte Carlo simulator
    pub fn new(portfolio: Portfolio, params: MonteCarloParams) -> Self {
        Self { portfolio, params }
    }

    /// Run Monte Carlo simulation
    pub fn run(&self) -> Result<MonteCarloResult> {
        use rand::Rng;

        let initial_value = self.portfolio.total_value();
        let mut simulated_values = Vec::with_capacity(self.params.num_simulations);
        let mut rng = rand::rng();

        let time_factor = Decimal::from(self.params.time_horizon_days) / dec!(365);
        let time_sqrt = time_factor.sqrt().unwrap_or(dec!(1));

        // Run simulations
        for _ in 0..self.params.num_simulations {
            let mut simulated_portfolio_value = self.portfolio.cash;

            for position in &self.portfolio.positions {
                // Convert volatility to f64 for normal distribution
                let vol_f64: f64 = position.volatility.to_string().parse().unwrap_or(0.2);
                let time_sqrt_f64: f64 = time_sqrt.to_string().parse().unwrap_or(1.0);
                let std_dev = vol_f64 * time_sqrt_f64;

                // Box-Muller transform to generate normal random variable
                let u1: f64 = rng.random();
                let u2: f64 = rng.random();
                let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                let shock_f64 = z * std_dev;
                let shock = Decimal::from_f64_retain(shock_f64).unwrap_or(dec!(0));

                let shocked_price = position.current_price * (dec!(1) + shock);
                simulated_portfolio_value += position.quantity * shocked_price;
            }

            simulated_values.push(simulated_portfolio_value);
        }

        // Sort for percentile calculations
        simulated_values.sort();

        // Calculate statistics
        let mean_value =
            simulated_values.iter().sum::<Decimal>() / Decimal::from(simulated_values.len());

        let median_idx = simulated_values.len() / 2;
        let median_value = simulated_values[median_idx];

        let percentile_5_idx = (simulated_values.len() as f64 * 0.05) as usize;
        let percentile_5 = simulated_values[percentile_5_idx];

        let percentile_95_idx = (simulated_values.len() as f64 * 0.95) as usize;
        let percentile_95 = simulated_values[percentile_95_idx];

        // VaR: loss at confidence level
        let var_idx = (simulated_values.len() as f64
            * (dec!(1) - self.params.confidence_level)
                .to_string()
                .parse::<f64>()
                .unwrap()) as usize;
        let var_value = simulated_values[var_idx];
        let var = initial_value - var_value;

        // CVaR: average of losses beyond VaR
        let tail_values: Vec<Decimal> =
            simulated_values.iter().take(var_idx + 1).copied().collect();
        let cvar_value = if !tail_values.is_empty() {
            tail_values.iter().sum::<Decimal>() / Decimal::from(tail_values.len())
        } else {
            var_value
        };
        let cvar = initial_value - cvar_value;

        // Probability of loss
        let losses = simulated_values
            .iter()
            .filter(|&&v| v < initial_value)
            .count();
        let prob_of_loss = Decimal::from(losses) / Decimal::from(simulated_values.len());

        Ok(MonteCarloResult {
            simulated_values,
            mean_value,
            median_value,
            percentile_5,
            percentile_95,
            var,
            cvar,
            prob_of_loss,
        })
    }
}

/// Extreme value analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtremeValueResult {
    /// Extreme event threshold
    pub threshold: Decimal,
    /// Probability of extreme event
    pub probability: Decimal,
    /// Expected loss given extreme event
    pub expected_loss: Decimal,
    /// Maximum observed loss
    pub max_loss: Decimal,
}

/// Historical stress scenario based on historical events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalStressScenario {
    /// Event name (e.g., "2008 Financial Crisis")
    pub event_name: String,
    /// Date of event
    pub event_date: String,
    /// Historical shocks observed
    pub shocks: HashMap<i64, Decimal>,
}

impl HistoricalStressScenario {
    /// Create a new historical stress scenario
    pub fn new(event_name: String, event_date: String, shocks: HashMap<i64, Decimal>) -> Self {
        Self {
            event_name,
            event_date,
            shocks,
        }
    }

    /// 2008 Financial Crisis scenario
    pub fn financial_crisis_2008(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            // Simulate -37% decline (S&P 500 2008)
            shocks.insert(token_id, dec!(-0.37));
        }
        Self {
            event_name: "2008 Financial Crisis".to_string(),
            event_date: "2008-09-15".to_string(),
            shocks,
        }
    }

    /// 2020 COVID-19 Crash scenario
    pub fn covid_crash_2020(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            // Simulate -34% decline (S&P 500 Feb-Mar 2020)
            shocks.insert(token_id, dec!(-0.34));
        }
        Self {
            event_name: "2020 COVID-19 Crash".to_string(),
            event_date: "2020-03-12".to_string(),
            shocks,
        }
    }

    /// 2022 Crypto Winter scenario
    pub fn crypto_winter_2022(token_ids: &[i64]) -> Self {
        let mut shocks = HashMap::new();
        for &token_id in token_ids {
            // Simulate -70% decline (typical for crypto bear market)
            shocks.insert(token_id, dec!(-0.70));
        }
        Self {
            event_name: "2022 Crypto Winter".to_string(),
            event_date: "2022-05-10".to_string(),
            shocks,
        }
    }

    /// Convert to StressScenario
    pub fn to_stress_scenario(&self) -> StressScenario {
        StressScenario {
            name: self.event_name.clone(),
            description: format!("Historical scenario from {}", self.event_date),
            shocks: self.shocks.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_portfolio() -> Portfolio {
        let positions = vec![
            Position {
                token_id: 1,
                quantity: dec!(100),
                current_price: dec!(10),
                volatility: dec!(0.3),
            },
            Position {
                token_id: 2,
                quantity: dec!(50),
                current_price: dec!(20),
                volatility: dec!(0.4),
            },
        ];
        Portfolio::new(positions, dec!(1000))
    }

    #[test]
    fn test_portfolio_total_value() {
        let portfolio = create_test_portfolio();
        // 100 * 10 + 50 * 20 + 1000 = 3000
        assert_eq!(portfolio.total_value(), dec!(3000));
    }

    #[test]
    fn test_position_value() {
        let position = Position {
            token_id: 1,
            quantity: dec!(100),
            current_price: dec!(10),
            volatility: dec!(0.3),
        };
        assert_eq!(position.value(), dec!(1000));
    }

    #[test]
    fn test_market_crash_scenario() {
        let portfolio = create_test_portfolio();
        let tester = StressTester::new(portfolio.clone());
        let scenario = StressScenario::market_crash(&[1, 2]);

        let result = tester.run_scenario(&scenario);
        assert_eq!(result.scenario_name, "Market Crash");
        assert_eq!(result.initial_value, dec!(3000));
        // After -30%: 100*7 + 50*14 + 1000 = 2400
        assert_eq!(result.stressed_value, dec!(2400));
        assert_eq!(result.pnl, dec!(-600));
    }

    #[test]
    fn test_flash_crash_scenario() {
        let portfolio = create_test_portfolio();
        let tester = StressTester::new(portfolio);
        let scenario = StressScenario::flash_crash(&[1, 2]);

        let result = tester.run_scenario(&scenario);
        assert_eq!(result.scenario_name, "Flash Crash");
        // After -50%: 100*5 + 50*10 + 1000 = 2000
        assert_eq!(result.stressed_value, dec!(2000));
        assert_eq!(result.pnl, dec!(-1000));
    }

    #[test]
    fn test_bull_market_scenario() {
        let portfolio = create_test_portfolio();
        let tester = StressTester::new(portfolio);
        let scenario = StressScenario::bull_market(&[1, 2]);

        let result = tester.run_scenario(&scenario);
        assert_eq!(result.scenario_name, "Bull Market");
        // After +50%: 100*15 + 50*30 + 1000 = 4000
        assert_eq!(result.stressed_value, dec!(4000));
        assert_eq!(result.pnl, dec!(1000));
    }

    #[test]
    fn test_default_scenarios() {
        let portfolio = create_test_portfolio();
        let tester = StressTester::new(portfolio);
        let results = tester.run_default_scenarios();

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].scenario_name, "Market Crash");
        assert_eq!(results[1].scenario_name, "Flash Crash");
        assert_eq!(results[2].scenario_name, "Moderate Correction");
        assert_eq!(results[3].scenario_name, "Bull Market");
    }

    #[test]
    fn test_monte_carlo_simulation() {
        let portfolio = create_test_portfolio();
        let params = MonteCarloParams {
            num_simulations: 1000,
            time_horizon_days: 30,
            confidence_level: dec!(0.95),
        };
        let simulator = MonteCarloSimulator::new(portfolio.clone(), params);

        let result = simulator.run().unwrap();
        assert_eq!(result.simulated_values.len(), 1000);
        assert!(result.mean_value > dec!(0));
        assert!(result.var >= dec!(0));
        assert!(result.cvar >= result.var);
        assert!(result.prob_of_loss >= dec!(0) && result.prob_of_loss <= dec!(1));
    }

    #[test]
    fn test_historical_scenarios() {
        let token_ids = vec![1, 2];
        let crisis_2008 = HistoricalStressScenario::financial_crisis_2008(&token_ids);
        let covid_2020 = HistoricalStressScenario::covid_crash_2020(&token_ids);
        let crypto_2022 = HistoricalStressScenario::crypto_winter_2022(&token_ids);

        assert_eq!(crisis_2008.event_name, "2008 Financial Crisis");
        assert_eq!(covid_2020.event_name, "2020 COVID-19 Crash");
        assert_eq!(crypto_2022.event_name, "2022 Crypto Winter");

        let portfolio = create_test_portfolio();
        let tester = StressTester::new(portfolio);

        let result = tester.run_scenario(&crisis_2008.to_stress_scenario());
        assert!(result.pnl < dec!(0));
    }

    #[test]
    fn test_apply_shock() {
        let portfolio = create_test_portfolio();
        let mut shocks = HashMap::new();
        shocks.insert(1, dec!(-0.5)); // -50% on token 1
        shocks.insert(2, dec!(0.5)); // +50% on token 2

        let shocked = portfolio.apply_shock(&shocks);
        // Token 1: 100 * 5 = 500
        // Token 2: 50 * 30 = 1500
        // Cash: 1000
        // Total: 3000
        assert_eq!(shocked.total_value(), dec!(3000));
    }

    #[test]
    fn test_stress_test_result_pnl_percentage() {
        let result = StressTestResult::new("Test".to_string(), dec!(1000), dec!(800));
        assert_eq!(result.pnl, dec!(-200));
        assert_eq!(result.pnl_percentage, dec!(-20));
    }
}
