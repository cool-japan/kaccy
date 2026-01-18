//! Trade Cost Analysis (TCA)
//!
//! Provides comprehensive analysis of trade execution costs including implementation
//! shortfall, market impact, and opportunity cost.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trade execution record for TCA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub trade_id: Uuid,
    pub token_id: Uuid,
    pub side: ExecutionSide,
    pub decision_time: DateTime<Utc>,
    pub decision_price: Decimal,
    pub execution_time: DateTime<Utc>,
    pub execution_price: Decimal,
    pub quantity: Decimal,
    pub benchmark_price: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionSide {
    Buy,
    Sell,
}

/// Implementation shortfall analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationShortfall {
    /// Total implementation shortfall (basis points)
    pub total_shortfall_bps: f64,
    /// Delay cost (decision to submission)
    pub delay_cost_bps: f64,
    /// Market impact cost
    pub market_impact_bps: f64,
    /// Opportunity cost (unfilled orders)
    pub opportunity_cost_bps: f64,
    /// Total cost in absolute terms
    pub total_cost: Decimal,
}

/// Market impact decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketImpact {
    /// Temporary impact (mean reversion)
    pub temporary_impact_bps: f64,
    /// Permanent impact
    pub permanent_impact_bps: f64,
    /// Total impact
    pub total_impact_bps: f64,
}

/// Trade cost analyzer
#[derive(Debug)]
pub struct TradeCostAnalyzer {
    /// Historical executions
    executions: Vec<ExecutionRecord>,
}

impl TradeCostAnalyzer {
    pub fn new() -> Self {
        Self {
            executions: Vec::new(),
        }
    }

    /// Add execution record
    pub fn add_execution(&mut self, execution: ExecutionRecord) {
        self.executions.push(execution);
    }

    /// Calculate implementation shortfall
    pub fn calculate_implementation_shortfall(
        &self,
        execution: &ExecutionRecord,
        post_trade_price: Option<Decimal>,
    ) -> ImplementationShortfall {
        // Arrival price (decision price)
        let arrival_price = execution.decision_price;

        // Execution price
        let exec_price = execution.execution_price;

        // Calculate slippage
        let slippage = match execution.side {
            ExecutionSide::Buy => exec_price - arrival_price,
            ExecutionSide::Sell => arrival_price - exec_price,
        };

        let slippage_bps = self.to_basis_points(slippage, arrival_price);

        // Delay cost (if decision and execution times differ)
        let delay_cost = self.calculate_delay_cost(execution);

        // Market impact
        let market_impact = self.calculate_market_impact_simple(execution, post_trade_price);

        // Opportunity cost (assumed 0 for filled orders)
        let opportunity_cost_bps = 0.0;

        // Total cost
        let total_cost = slippage * execution.quantity;

        ImplementationShortfall {
            total_shortfall_bps: slippage_bps,
            delay_cost_bps: delay_cost,
            market_impact_bps: market_impact,
            opportunity_cost_bps,
            total_cost,
        }
    }

    /// Calculate delay cost
    fn calculate_delay_cost(&self, execution: &ExecutionRecord) -> f64 {
        let price_change = match execution.side {
            ExecutionSide::Buy => execution.execution_price - execution.decision_price,
            ExecutionSide::Sell => execution.decision_price - execution.execution_price,
        };

        self.to_basis_points(price_change, execution.decision_price)
    }

    /// Calculate market impact (simplified)
    fn calculate_market_impact_simple(
        &self,
        execution: &ExecutionRecord,
        post_trade_price: Option<Decimal>,
    ) -> f64 {
        if let Some(post_price) = post_trade_price {
            let impact = match execution.side {
                ExecutionSide::Buy => execution.execution_price - post_price,
                ExecutionSide::Sell => post_price - execution.execution_price,
            };

            self.to_basis_points(impact, execution.execution_price)
        } else {
            0.0
        }
    }

    /// Analyze market impact with decomposition
    pub fn analyze_market_impact(
        &self,
        execution: &ExecutionRecord,
        prices_after: &[(DateTime<Utc>, Decimal)],
    ) -> MarketImpact {
        let exec_price = execution.execution_price;

        // Find price shortly after (temporary)
        let temp_price = prices_after
            .iter()
            .take(5) // First 5 observations
            .map(|(_, p)| *p)
            .sum::<Decimal>()
            / Decimal::from(5.min(prices_after.len()));

        // Find price after longer period (permanent)
        let perm_price = prices_after
            .iter()
            .rev()
            .take(10)
            .map(|(_, p)| *p)
            .sum::<Decimal>()
            / Decimal::from(10.min(prices_after.len()));

        let temporary_impact = match execution.side {
            ExecutionSide::Buy => exec_price - temp_price,
            ExecutionSide::Sell => temp_price - exec_price,
        };

        let permanent_impact = match execution.side {
            ExecutionSide::Buy => exec_price - perm_price,
            ExecutionSide::Sell => perm_price - exec_price,
        };

        let temporary_impact_bps = self.to_basis_points(temporary_impact, exec_price);
        let permanent_impact_bps = self.to_basis_points(permanent_impact, exec_price);

        MarketImpact {
            temporary_impact_bps,
            permanent_impact_bps,
            total_impact_bps: temporary_impact_bps + permanent_impact_bps,
        }
    }

    /// Calculate effective spread
    pub fn effective_spread(&self, execution: &ExecutionRecord) -> f64 {
        let spread = match execution.side {
            ExecutionSide::Buy => execution.execution_price - execution.benchmark_price,
            ExecutionSide::Sell => execution.benchmark_price - execution.execution_price,
        };

        self.to_basis_points(spread * Decimal::from(2), execution.benchmark_price)
    }

    /// Calculate realized spread
    pub fn realized_spread(&self, execution: &ExecutionRecord, midpoint_after: Decimal) -> f64 {
        let spread = match execution.side {
            ExecutionSide::Buy => midpoint_after - execution.execution_price,
            ExecutionSide::Sell => execution.execution_price - midpoint_after,
        };

        self.to_basis_points(spread * Decimal::from(2), execution.benchmark_price)
    }

    /// Calculate price improvement
    pub fn price_improvement(&self, execution: &ExecutionRecord, reference_price: Decimal) -> f64 {
        let improvement = match execution.side {
            ExecutionSide::Buy => reference_price - execution.execution_price,
            ExecutionSide::Sell => execution.execution_price - reference_price,
        };

        self.to_basis_points(improvement, reference_price)
    }

    /// Convert to basis points
    fn to_basis_points(&self, value: Decimal, base: Decimal) -> f64 {
        if base == Decimal::ZERO {
            return 0.0;
        }
        ((value / base) * Decimal::from(10000))
            .to_string()
            .parse()
            .unwrap_or(0.0)
    }

    /// Get execution statistics
    pub fn get_statistics(&self) -> ExecutionStatistics {
        if self.executions.is_empty() {
            return ExecutionStatistics::default();
        }

        let total_executions = self.executions.len();

        let avg_slippage: f64 = self
            .executions
            .iter()
            .map(|e| {
                let slippage = match e.side {
                    ExecutionSide::Buy => e.execution_price - e.decision_price,
                    ExecutionSide::Sell => e.decision_price - e.execution_price,
                };
                self.to_basis_points(slippage, e.decision_price)
            })
            .sum::<f64>()
            / total_executions as f64;

        let total_volume: Decimal = self.executions.iter().map(|e| e.quantity).sum();

        let avg_execution_time: f64 = self
            .executions
            .iter()
            .map(|e| (e.execution_time - e.decision_time).num_milliseconds() as f64)
            .sum::<f64>()
            / total_executions as f64;

        ExecutionStatistics {
            total_executions,
            total_volume,
            average_slippage_bps: avg_slippage,
            average_execution_time_ms: avg_execution_time,
        }
    }
}

impl Default for TradeCostAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    pub total_executions: usize,
    pub total_volume: Decimal,
    pub average_slippage_bps: f64,
    pub average_execution_time_ms: f64,
}

impl Default for ExecutionStatistics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            total_volume: Decimal::ZERO,
            average_slippage_bps: 0.0,
            average_execution_time_ms: 0.0,
        }
    }
}

/// VWAP performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapPerformance {
    /// Execution VWAP
    pub execution_vwap: Decimal,
    /// Benchmark VWAP
    pub benchmark_vwap: Decimal,
    /// Deviation from benchmark (bps)
    pub deviation_bps: f64,
    /// Participation rate
    pub participation_rate: f64,
}

/// VWAP analyzer
#[derive(Debug)]
pub struct VwapAnalyzer;

impl VwapAnalyzer {
    /// Calculate VWAP performance
    pub fn analyze_vwap(
        executions: &[ExecutionRecord],
        benchmark_trades: &[(Decimal, Decimal)], // (price, volume)
    ) -> VwapPerformance {
        // Calculate execution VWAP
        let exec_vwap = Self::calculate_vwap(
            &executions
                .iter()
                .map(|e| (e.execution_price, e.quantity))
                .collect::<Vec<_>>(),
        );

        // Calculate benchmark VWAP
        let benchmark_vwap = Self::calculate_vwap(benchmark_trades);

        // Calculate deviation
        let deviation = exec_vwap - benchmark_vwap;
        let deviation_bps = if benchmark_vwap > Decimal::ZERO {
            ((deviation / benchmark_vwap) * Decimal::from(10000))
                .to_string()
                .parse()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Calculate participation rate
        let total_exec_volume: Decimal = executions.iter().map(|e| e.quantity).sum();
        let total_market_volume: Decimal = benchmark_trades.iter().map(|(_, v)| *v).sum();
        let participation_rate = if total_market_volume > Decimal::ZERO {
            (total_exec_volume / total_market_volume)
                .to_string()
                .parse()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        VwapPerformance {
            execution_vwap: exec_vwap,
            benchmark_vwap,
            deviation_bps,
            participation_rate,
        }
    }

    /// Calculate VWAP from price-volume pairs
    fn calculate_vwap(trades: &[(Decimal, Decimal)]) -> Decimal {
        let total_value: Decimal = trades.iter().map(|(p, v)| *p * *v).sum();
        let total_volume: Decimal = trades.iter().map(|(_, v)| *v).sum();

        if total_volume > Decimal::ZERO {
            total_value / total_volume
        } else {
            Decimal::ZERO
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_execution(side: ExecutionSide) -> ExecutionRecord {
        let now = Utc::now();
        ExecutionRecord {
            trade_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            side,
            decision_time: now,
            decision_price: dec!(100),
            execution_time: now,
            execution_price: dec!(101),
            quantity: dec!(1000),
            benchmark_price: dec!(100.5),
        }
    }

    #[test]
    fn test_implementation_shortfall() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = create_test_execution(ExecutionSide::Buy);

        let shortfall = analyzer.calculate_implementation_shortfall(&execution, Some(dec!(100.5)));

        // Buy at 101 when decision was at 100 = 1 unit slippage = 100 bps
        assert!((shortfall.total_shortfall_bps - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_effective_spread() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = create_test_execution(ExecutionSide::Buy);

        let spread = analyzer.effective_spread(&execution);

        // (101 - 100.5) * 2 / 100.5 * 10000 ≈ 99.5 bps
        assert!((spread - 99.5).abs() < 1.0);
    }

    #[test]
    fn test_price_improvement() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = create_test_execution(ExecutionSide::Buy);

        // Reference price worse than execution
        let improvement = analyzer.price_improvement(&execution, dec!(102));

        // Saved 1 on reference of 102 = 98 bps
        assert!(improvement > 90.0);
    }

    #[test]
    fn test_vwap_analysis() {
        let executions = vec![
            ExecutionRecord {
                trade_id: Uuid::new_v4(),
                token_id: Uuid::new_v4(),
                side: ExecutionSide::Buy,
                decision_time: Utc::now(),
                decision_price: dec!(100),
                execution_time: Utc::now(),
                execution_price: dec!(100),
                quantity: dec!(100),
                benchmark_price: dec!(100),
            },
            ExecutionRecord {
                trade_id: Uuid::new_v4(),
                token_id: Uuid::new_v4(),
                side: ExecutionSide::Buy,
                decision_time: Utc::now(),
                decision_price: dec!(102),
                execution_time: Utc::now(),
                execution_price: dec!(102),
                quantity: dec!(100),
                benchmark_price: dec!(102),
            },
        ];

        let benchmark = vec![(dec!(100), dec!(100)), (dec!(102), dec!(100))];

        let performance = VwapAnalyzer::analyze_vwap(&executions, &benchmark);

        assert_eq!(performance.execution_vwap, dec!(101));
        assert_eq!(performance.benchmark_vwap, dec!(101));
        assert!((performance.deviation_bps).abs() < 0.01);
    }

    #[test]
    fn test_execution_statistics() {
        let mut analyzer = TradeCostAnalyzer::new();

        analyzer.add_execution(create_test_execution(ExecutionSide::Buy));
        analyzer.add_execution(create_test_execution(ExecutionSide::Sell));

        let stats = analyzer.get_statistics();

        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.total_volume, dec!(2000));
    }
}
