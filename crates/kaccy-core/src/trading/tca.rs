//! Trade Cost Analysis (TCA)
//!
//! Provides comprehensive analysis of trade execution costs including implementation
//! shortfall, market impact, and opportunity cost.
//!
//! # Market Impact Model
//!
//! When post-trade price data is unavailable, this module estimates market impact
//! using an Almgren-style square-root model:
//!
//! ```text
//! impact_bps = coefficient * volatility * sqrt(order_quantity / average_daily_volume)
//! ```
//!
//! **Parameters:**
//! - `coefficient` — dimensionless scaling factor calibrated to empirical data.
//!   The literature value for the "universal" square-root law (Bouchaud et al., 2011;
//!   Kyle & Obizhaeva, 2016) lies in the range [0.5, 1.5] when volatility is
//!   expressed as daily return std-dev and ADV is in the same unit as `quantity`.
//!   The default used here is `0.5` (conservative, appropriate for a crypto-native
//!   market-maker context where realized spreads are already wide).
//! - `volatility` — daily return standard deviation (dimensionless, e.g. 0.05 = 5 %/day).
//!   Must be stored on [`ExecutionRecord::volatility_daily`].
//! - `order_quantity / average_daily_volume` — participation rate; both quantities
//!   must be in the same unit (token count or USD notional — consistent within a record).
//!
//! **Sign convention:** impact is always returned as a positive basis-point value
//! representing the cost incurred by the trader (adverse price move relative to
//! mid-market). The caller is responsible for applying the correct sign based on
//! trade side.
//!
//! **References:**
//! - Almgren R. et al. (2005), "Direct Estimation of Equity Market Impact", *Risk*.
//! - Bouchaud J.-P. et al. (2011), "The inelastic market hypothesis", *arXiv*.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Default Almgren-style square-root impact coefficient (dimensionless).
///
/// Calibrated conservatively at 0.5.  Callers may supply a domain-specific
/// value via [`TradeCostAnalyzerConfig`] when constructing [`TradeCostAnalyzer`].
pub const DEFAULT_IMPACT_COEFFICIENT: f64 = 0.5;

/// Errors that can arise during market-impact estimation.
#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq)]
pub enum MarketImpactError {
    /// Neither post-trade price nor the volatility required by the model is
    /// available on the [`ExecutionRecord`].  Without at least one source of
    /// information the impact cannot be estimated; returning 0.0 would be
    /// silently wrong.
    #[error(
        "market impact cannot be estimated: post_trade_price is absent and \
         ExecutionRecord::volatility_daily is None — at least one is required"
    )]
    InsufficientData,

    /// Average daily volume (ADV) is absent on the record, making the
    /// participation-rate denominator undefined.
    #[error(
        "market impact cannot be estimated: ExecutionRecord::average_daily_volume is None — \
         the square-root model requires ADV to compute the participation rate"
    )]
    MissingAverageDailyVolume,

    /// ADV was present but zero, which would cause division-by-zero in the
    /// participation-rate calculation.
    #[error(
        "market impact cannot be estimated: average_daily_volume is zero — \
         participation rate is undefined"
    )]
    ZeroAverageDailyVolume,

    /// The computed impact was non-finite (NaN or ±∞), which indicates a
    /// degenerate input combination (e.g. extreme volatility).
    #[error("market impact calculation produced a non-finite result: {detail}")]
    NonFiniteResult { detail: String },
}

/// Configuration for [`TradeCostAnalyzer`].
#[derive(Debug, Clone)]
pub struct TradeCostAnalyzerConfig {
    /// Almgren-style square-root impact coefficient (η in the literature).
    ///
    /// Dimensionless.  Default: [`DEFAULT_IMPACT_COEFFICIENT`] (0.5).
    pub impact_coefficient: f64,
}

impl Default for TradeCostAnalyzerConfig {
    fn default() -> Self {
        Self {
            impact_coefficient: DEFAULT_IMPACT_COEFFICIENT,
        }
    }
}

/// Trade execution record for TCA.
///
/// Two optional fields — [`volatility_daily`] and [`average_daily_volume`] —
/// are required by the Almgren square-root impact model when no post-trade
/// price is available.  Both carry `#[serde(default)]` so that existing
/// serialised records that omit them remain deserializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique identifier for this trade.
    pub trade_id: Uuid,
    /// Token that was traded.
    pub token_id: Uuid,
    /// Whether this was a buy or sell.
    pub side: ExecutionSide,
    /// Timestamp when the trading decision was made.
    pub decision_time: DateTime<Utc>,
    /// Price at the time the decision was made.
    pub decision_price: Decimal,
    /// Timestamp when the order was actually executed.
    pub execution_time: DateTime<Utc>,
    /// Actual execution price.
    pub execution_price: Decimal,
    /// Quantity traded (token count or notional in a consistent unit).
    pub quantity: Decimal,
    /// Benchmark price used for comparison (e.g. mid-point at decision time).
    pub benchmark_price: Decimal,
    /// Daily return standard deviation at execution time (dimensionless, e.g. 0.05 = 5 %/day).
    ///
    /// Required by the square-root impact model when `post_trade_price` is
    /// absent.  If `None`, the model falls back to returning
    /// [`MarketImpactError::InsufficientData`].
    #[serde(default)]
    pub volatility_daily: Option<f64>,
    /// Average daily volume in the same unit as [`quantity`].
    ///
    /// Required by the square-root impact model to compute the participation
    /// rate `Q / V`.  If `None`, the model returns
    /// [`MarketImpactError::MissingAverageDailyVolume`].
    #[serde(default)]
    pub average_daily_volume: Option<Decimal>,
}

/// Side of a trade execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionSide {
    /// Buy order.
    Buy,
    /// Sell order.
    Sell,
}

/// Implementation shortfall analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplementationShortfall {
    /// Total implementation shortfall (basis points).
    pub total_shortfall_bps: f64,
    /// Delay cost (decision to submission).
    pub delay_cost_bps: f64,
    /// Market impact cost.
    pub market_impact_bps: f64,
    /// Opportunity cost (unfilled orders).
    pub opportunity_cost_bps: f64,
    /// Total cost in absolute terms.
    pub total_cost: Decimal,
}

/// Market impact decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketImpact {
    /// Temporary impact (mean reversion).
    pub temporary_impact_bps: f64,
    /// Permanent impact.
    pub permanent_impact_bps: f64,
    /// Total impact.
    pub total_impact_bps: f64,
}

/// Trade cost analyzer.
#[derive(Debug)]
pub struct TradeCostAnalyzer {
    /// Historical executions.
    executions: Vec<ExecutionRecord>,
    /// Analyser configuration (impact model parameters).
    config: TradeCostAnalyzerConfig,
}

impl TradeCostAnalyzer {
    /// Create a new trade cost analyzer with default configuration.
    pub fn new() -> Self {
        Self {
            executions: Vec::new(),
            config: TradeCostAnalyzerConfig::default(),
        }
    }

    /// Create a new trade cost analyzer with a custom configuration.
    pub fn with_config(config: TradeCostAnalyzerConfig) -> Self {
        Self {
            executions: Vec::new(),
            config,
        }
    }

    /// Add execution record.
    pub fn add_execution(&mut self, execution: ExecutionRecord) {
        self.executions.push(execution);
    }

    /// Calculate implementation shortfall.
    ///
    /// Returns `Err(MarketImpactError::InsufficientData)` when `post_trade_price`
    /// is absent **and** the required model inputs
    /// ([`ExecutionRecord::volatility_daily`] / [`ExecutionRecord::average_daily_volume`])
    /// are also missing, because a silent `0.0` impact would be misleading.
    pub fn calculate_implementation_shortfall(
        &self,
        execution: &ExecutionRecord,
        post_trade_price: Option<Decimal>,
    ) -> Result<ImplementationShortfall, MarketImpactError> {
        // Arrival price (decision price).
        let arrival_price = execution.decision_price;

        // Execution price.
        let exec_price = execution.execution_price;

        // Calculate slippage.
        let slippage = match execution.side {
            ExecutionSide::Buy => exec_price - arrival_price,
            ExecutionSide::Sell => arrival_price - exec_price,
        };

        let slippage_bps = self.to_basis_points(slippage, arrival_price);

        // Delay cost (if decision and execution times differ).
        let delay_cost = self.calculate_delay_cost(execution);

        // Market impact — may return an error if inputs are genuinely unavailable.
        let market_impact_bps = self.estimate_market_impact(execution, post_trade_price)?;

        // Opportunity cost (assumed 0 for filled orders).
        let opportunity_cost_bps = 0.0;

        // Total cost.
        let total_cost = slippage * execution.quantity;

        Ok(ImplementationShortfall {
            total_shortfall_bps: slippage_bps,
            delay_cost_bps: delay_cost,
            market_impact_bps,
            opportunity_cost_bps,
            total_cost,
        })
    }

    /// Calculate delay cost.
    fn calculate_delay_cost(&self, execution: &ExecutionRecord) -> f64 {
        let price_change = match execution.side {
            ExecutionSide::Buy => execution.execution_price - execution.decision_price,
            ExecutionSide::Sell => execution.decision_price - execution.execution_price,
        };

        self.to_basis_points(price_change, execution.decision_price)
    }

    /// Estimate market impact in basis points.
    ///
    /// # Realized path (post_trade_price is `Some`)
    ///
    /// Uses the observed price reversal after execution.  For a buy, impact is
    /// the drop from execution price back toward mid-market; for a sell, the
    /// corresponding rise.  This reflects the temporary (mean-reverting) component
    /// of market impact and is expressed in basis points relative to the execution
    /// price.
    ///
    /// # Modelled path (post_trade_price is `None`)
    ///
    /// Applies an Almgren-style square-root model:
    ///
    /// ```text
    /// impact_bps = η · σ · √(Q / V) · 10 000
    /// ```
    ///
    /// where:
    /// - η = [`TradeCostAnalyzerConfig::impact_coefficient`] (default 0.5)
    /// - σ = [`ExecutionRecord::volatility_daily`] (daily return std-dev)
    /// - Q = [`ExecutionRecord::quantity`] (order size)
    /// - V = [`ExecutionRecord::average_daily_volume`] (ADV, same unit as Q)
    ///
    /// Returns [`MarketImpactError::InsufficientData`] when σ is absent,
    /// [`MarketImpactError::MissingAverageDailyVolume`] when V is absent, and
    /// [`MarketImpactError::ZeroAverageDailyVolume`] when V = 0.
    fn estimate_market_impact(
        &self,
        execution: &ExecutionRecord,
        post_trade_price: Option<Decimal>,
    ) -> Result<f64, MarketImpactError> {
        if let Some(post_price) = post_trade_price {
            // ---------------------------------------------------------------
            // REALIZED PATH: measure price reversion after the trade.
            // For a buy, the temporary impact shows as execution_price > post_price
            // (market rebounds toward mid after the buy pressure).
            // For a sell, the symmetric case applies.
            // ---------------------------------------------------------------
            let impact = match execution.side {
                ExecutionSide::Buy => execution.execution_price - post_price,
                ExecutionSide::Sell => post_price - execution.execution_price,
            };

            Ok(self.to_basis_points(impact, execution.execution_price))
        } else {
            // ---------------------------------------------------------------
            // MODELLED PATH: Almgren square-root impact model.
            //
            // We need both volatility and ADV to form a well-defined estimate.
            // Returning 0.0 when either is absent would silently understate costs
            // and violate the no-fabrication contract.
            // ---------------------------------------------------------------
            let volatility = execution
                .volatility_daily
                .ok_or(MarketImpactError::InsufficientData)?;

            let adv = execution
                .average_daily_volume
                .ok_or(MarketImpactError::MissingAverageDailyVolume)?;

            if adv == Decimal::ZERO {
                return Err(MarketImpactError::ZeroAverageDailyVolume);
            }

            // participation_rate = Q / V  (dimensionless)
            let participation_rate: f64 = (execution.quantity / adv)
                .to_string()
                .parse()
                .unwrap_or(0.0_f64);

            // impact_bps = η · σ · √(Q/V) · 10 000
            let impact_bps = self.config.impact_coefficient
                * volatility
                * participation_rate.sqrt()
                * 10_000.0_f64;

            if !impact_bps.is_finite() {
                return Err(MarketImpactError::NonFiniteResult {
                    detail: format!(
                        "coef={coef}, volatility={vol}, participation={par}",
                        coef = self.config.impact_coefficient,
                        vol = volatility,
                        par = participation_rate
                    ),
                });
            }

            // Impact is inherently non-negative (cost, not benefit).
            Ok(impact_bps.max(0.0))
        }
    }

    /// Analyze market impact with decomposition.
    pub fn analyze_market_impact(
        &self,
        execution: &ExecutionRecord,
        prices_after: &[(DateTime<Utc>, Decimal)],
    ) -> MarketImpact {
        let exec_price = execution.execution_price;

        // Find price shortly after (temporary).
        let temp_price = prices_after
            .iter()
            .take(5) // First 5 observations.
            .map(|(_, p)| *p)
            .sum::<Decimal>()
            / Decimal::from(5.min(prices_after.len()));

        // Find price after longer period (permanent).
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

    /// Calculate effective spread.
    pub fn effective_spread(&self, execution: &ExecutionRecord) -> f64 {
        let spread = match execution.side {
            ExecutionSide::Buy => execution.execution_price - execution.benchmark_price,
            ExecutionSide::Sell => execution.benchmark_price - execution.execution_price,
        };

        self.to_basis_points(spread * Decimal::from(2), execution.benchmark_price)
    }

    /// Calculate realized spread.
    pub fn realized_spread(&self, execution: &ExecutionRecord, midpoint_after: Decimal) -> f64 {
        let spread = match execution.side {
            ExecutionSide::Buy => midpoint_after - execution.execution_price,
            ExecutionSide::Sell => execution.execution_price - midpoint_after,
        };

        self.to_basis_points(spread * Decimal::from(2), execution.benchmark_price)
    }

    /// Calculate price improvement.
    pub fn price_improvement(&self, execution: &ExecutionRecord, reference_price: Decimal) -> f64 {
        let improvement = match execution.side {
            ExecutionSide::Buy => reference_price - execution.execution_price,
            ExecutionSide::Sell => execution.execution_price - reference_price,
        };

        self.to_basis_points(improvement, reference_price)
    }

    /// Convert to basis points.
    fn to_basis_points(&self, value: Decimal, base: Decimal) -> f64 {
        if base == Decimal::ZERO {
            return 0.0;
        }
        ((value / base) * Decimal::from(10000))
            .to_string()
            .parse()
            .unwrap_or(0.0)
    }

    /// Get execution statistics.
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

/// Execution statistics summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    /// Number of executions analysed.
    pub total_executions: usize,
    /// Total traded volume.
    pub total_volume: Decimal,
    /// Average slippage across all executions in basis points.
    pub average_slippage_bps: f64,
    /// Average time from decision to execution in milliseconds.
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

/// VWAP performance analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VwapPerformance {
    /// Execution VWAP.
    pub execution_vwap: Decimal,
    /// Benchmark VWAP.
    pub benchmark_vwap: Decimal,
    /// Deviation from benchmark (bps).
    pub deviation_bps: f64,
    /// Participation rate.
    pub participation_rate: f64,
}

/// VWAP analyzer.
#[derive(Debug)]
pub struct VwapAnalyzer;

impl VwapAnalyzer {
    /// Calculate VWAP performance.
    pub fn analyze_vwap(
        executions: &[ExecutionRecord],
        benchmark_trades: &[(Decimal, Decimal)], // (price, volume)
    ) -> VwapPerformance {
        // Calculate execution VWAP.
        let exec_vwap = Self::calculate_vwap(
            &executions
                .iter()
                .map(|e| (e.execution_price, e.quantity))
                .collect::<Vec<_>>(),
        );

        // Calculate benchmark VWAP.
        let benchmark_vwap = Self::calculate_vwap(benchmark_trades);

        // Calculate deviation.
        let deviation = exec_vwap - benchmark_vwap;
        let deviation_bps = if benchmark_vwap > Decimal::ZERO {
            ((deviation / benchmark_vwap) * Decimal::from(10000))
                .to_string()
                .parse()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Calculate participation rate.
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

    /// Calculate VWAP from price-volume pairs.
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

    /// Build a minimal [`ExecutionRecord`] with the new optional fields absent.
    fn make_execution(side: ExecutionSide) -> ExecutionRecord {
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
            volatility_daily: None,
            average_daily_volume: None,
        }
    }

    /// Build an [`ExecutionRecord`] with model inputs populated.
    fn make_execution_with_model_inputs(
        side: ExecutionSide,
        quantity: Decimal,
        volatility: f64,
        adv: Decimal,
    ) -> ExecutionRecord {
        let now = Utc::now();
        ExecutionRecord {
            trade_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            side,
            decision_time: now,
            decision_price: dec!(100),
            execution_time: now,
            execution_price: dec!(101),
            quantity,
            benchmark_price: dec!(100.5),
            volatility_daily: Some(volatility),
            average_daily_volume: Some(adv),
        }
    }

    // -----------------------------------------------------------------------
    // Realized impact path tests
    // -----------------------------------------------------------------------

    /// Realized impact: buy at 101, post-trade price 100.5
    /// impact = 101 - 100.5 = 0.5 on base 101 → 0.5/101 * 10 000 ≈ 49.5 bps
    #[test]
    fn test_realized_impact_buy_correct_bps() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution(ExecutionSide::Buy);

        let shortfall = analyzer
            .calculate_implementation_shortfall(&execution, Some(dec!(100.5)))
            .expect("realized path must not return error");

        // impact = (101 - 100.5) / 101 * 10 000 = 0.5/101 * 10 000 ≈ 49.5 bps
        let expected = 0.5_f64 / 101.0 * 10_000.0;
        assert!(
            (shortfall.market_impact_bps - expected).abs() < 0.5,
            "expected ≈{expected:.2} bps, got {got:.2} bps",
            got = shortfall.market_impact_bps
        );
    }

    /// Realized impact: sell at 101, post-trade price 101.5
    /// For a sell, impact = post_price - exec_price = 101.5 - 101 = 0.5 on base 101 ≈ 49.5 bps
    #[test]
    fn test_realized_impact_sell_correct_bps() {
        let analyzer = TradeCostAnalyzer::new();
        let mut execution = make_execution(ExecutionSide::Sell);
        execution.execution_price = dec!(101);

        let shortfall = analyzer
            .calculate_implementation_shortfall(&execution, Some(dec!(101.5)))
            .expect("realized path must not return error");

        let expected = 0.5_f64 / 101.0 * 10_000.0;
        assert!(
            (shortfall.market_impact_bps - expected).abs() < 0.5,
            "expected ≈{expected:.2} bps, got {got:.2} bps",
            got = shortfall.market_impact_bps
        );
    }

    /// When post_trade_price equals execution_price there is no observed impact.
    #[test]
    fn test_realized_impact_zero_reversion() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution(ExecutionSide::Buy);
        // post price == exec price → zero measured temporary impact
        let shortfall = analyzer
            .calculate_implementation_shortfall(&execution, Some(dec!(101)))
            .expect("realized path must not return error");

        assert!(
            shortfall.market_impact_bps.abs() < 1e-6,
            "expected 0 bps, got {bps}",
            bps = shortfall.market_impact_bps
        );
    }

    // -----------------------------------------------------------------------
    // Modelled path tests (Almgren square-root law)
    // -----------------------------------------------------------------------

    /// Model path: positive impact for a buy with valid inputs.
    #[test]
    fn test_modelled_impact_positive() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution_with_model_inputs(
            ExecutionSide::Buy,
            dec!(1000),    // order quantity
            0.05,          // 5 %/day volatility
            dec!(100_000), // ADV
        );

        let shortfall = analyzer
            .calculate_implementation_shortfall(&execution, None)
            .expect("model path with valid inputs must succeed");

        assert!(
            shortfall.market_impact_bps > 0.0,
            "expected positive impact, got {bps}",
            bps = shortfall.market_impact_bps
        );
    }

    /// sqrt law: 4× order size → ~2× impact (square-root relationship).
    ///
    /// Precisely: impact(4Q) / impact(Q) = √4 = 2.0.
    /// We tolerate a small relative error from f64 rounding.
    #[test]
    fn test_modelled_impact_sqrt_scaling_with_quantity() {
        let analyzer = TradeCostAnalyzer::new();

        let base_impact = {
            let exec = make_execution_with_model_inputs(
                ExecutionSide::Buy,
                dec!(1000), // Q
                0.05,
                dec!(100_000),
            );
            analyzer
                .calculate_implementation_shortfall(&exec, None)
                .expect("valid inputs")
                .market_impact_bps
        };

        let quad_impact = {
            let exec = make_execution_with_model_inputs(
                ExecutionSide::Buy,
                dec!(4000), // 4 × Q
                0.05,
                dec!(100_000),
            );
            analyzer
                .calculate_implementation_shortfall(&exec, None)
                .expect("valid inputs")
                .market_impact_bps
        };

        let ratio = quad_impact / base_impact;
        assert!(
            (ratio - 2.0).abs() < 0.01,
            "expected sqrt ratio ≈ 2.0, got {ratio:.4}"
        );
    }

    /// sqrt law: 4× volatility → 4× impact (linear in volatility).
    #[test]
    fn test_modelled_impact_linear_scaling_with_volatility() {
        let analyzer = TradeCostAnalyzer::new();

        let base_impact = {
            let exec = make_execution_with_model_inputs(
                ExecutionSide::Buy,
                dec!(1000),
                0.05, // σ
                dec!(100_000),
            );
            analyzer
                .calculate_implementation_shortfall(&exec, None)
                .expect("valid inputs")
                .market_impact_bps
        };

        let quad_vol_impact = {
            let exec = make_execution_with_model_inputs(
                ExecutionSide::Buy,
                dec!(1000),
                0.20, // 4 × σ
                dec!(100_000),
            );
            analyzer
                .calculate_implementation_shortfall(&exec, None)
                .expect("valid inputs")
                .market_impact_bps
        };

        let ratio = quad_vol_impact / base_impact;
        assert!(
            (ratio - 4.0).abs() < 0.01,
            "expected linear ratio ≈ 4.0, got {ratio:.4}"
        );
    }

    /// Custom coefficient scales impact proportionally.
    #[test]
    fn test_modelled_impact_coefficient_scaling() {
        let config_half = TradeCostAnalyzerConfig {
            impact_coefficient: 0.25,
        };
        let config_double = TradeCostAnalyzerConfig {
            impact_coefficient: 1.0,
        };

        let exec = || {
            make_execution_with_model_inputs(ExecutionSide::Buy, dec!(1000), 0.05, dec!(100_000))
        };

        let impact_half = TradeCostAnalyzer::with_config(config_half)
            .calculate_implementation_shortfall(&exec(), None)
            .expect("valid")
            .market_impact_bps;

        let impact_double = TradeCostAnalyzer::with_config(config_double)
            .calculate_implementation_shortfall(&exec(), None)
            .expect("valid")
            .market_impact_bps;

        let ratio = impact_double / impact_half;
        assert!(
            (ratio - 4.0).abs() < 0.01,
            "expected coefficient ratio ≈ 4.0 (1.0/0.25), got {ratio:.4}"
        );
    }

    // -----------------------------------------------------------------------
    // Error / degenerate input tests
    // -----------------------------------------------------------------------

    /// No post_trade_price and no volatility → InsufficientData error.
    #[test]
    fn test_modelled_impact_no_volatility_returns_error() {
        let analyzer = TradeCostAnalyzer::new();
        let mut execution = make_execution(ExecutionSide::Buy);
        execution.volatility_daily = None;
        execution.average_daily_volume = Some(dec!(100_000));

        let result = analyzer.calculate_implementation_shortfall(&execution, None);
        assert_eq!(result, Err(MarketImpactError::InsufficientData));
    }

    /// No post_trade_price and no ADV → MissingAverageDailyVolume error.
    #[test]
    fn test_modelled_impact_no_adv_returns_error() {
        let analyzer = TradeCostAnalyzer::new();
        let mut execution = make_execution(ExecutionSide::Buy);
        execution.volatility_daily = Some(0.05);
        execution.average_daily_volume = None;

        let result = analyzer.calculate_implementation_shortfall(&execution, None);
        assert_eq!(result, Err(MarketImpactError::MissingAverageDailyVolume));
    }

    /// Zero ADV → ZeroAverageDailyVolume error (not a silent 0.0).
    #[test]
    fn test_modelled_impact_zero_adv_returns_error() {
        let analyzer = TradeCostAnalyzer::new();
        let mut execution = make_execution(ExecutionSide::Buy);
        execution.volatility_daily = Some(0.05);
        execution.average_daily_volume = Some(Decimal::ZERO);

        let result = analyzer.calculate_implementation_shortfall(&execution, None);
        assert_eq!(result, Err(MarketImpactError::ZeroAverageDailyVolume));
    }

    // -----------------------------------------------------------------------
    // Legacy tests preserved (updated to use the new Result-returning API)
    // -----------------------------------------------------------------------

    #[test]
    fn test_implementation_shortfall() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution(ExecutionSide::Buy);

        let shortfall = analyzer
            .calculate_implementation_shortfall(&execution, Some(dec!(100.5)))
            .expect("realized path must succeed");

        // Buy at 101 when decision was at 100 = 1 unit slippage = 100 bps
        assert!((shortfall.total_shortfall_bps - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_effective_spread() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution(ExecutionSide::Buy);

        let spread = analyzer.effective_spread(&execution);

        // (101 - 100.5) * 2 / 100.5 * 10000 ≈ 99.5 bps
        assert!((spread - 99.5).abs() < 1.0);
    }

    #[test]
    fn test_price_improvement() {
        let analyzer = TradeCostAnalyzer::new();
        let execution = make_execution(ExecutionSide::Buy);

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
                volatility_daily: None,
                average_daily_volume: None,
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
                volatility_daily: None,
                average_daily_volume: None,
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

        analyzer.add_execution(make_execution(ExecutionSide::Buy));
        analyzer.add_execution(make_execution(ExecutionSide::Sell));

        let stats = analyzer.get_statistics();

        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.total_volume, dec!(2000));
    }
}
