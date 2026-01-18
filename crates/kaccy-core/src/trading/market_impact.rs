//! Market Impact Models
//!
//! Implements various market impact models for estimating the price impact of trades.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Market impact estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEstimate {
    /// Temporary impact (bps)
    pub temporary_impact_bps: f64,
    /// Permanent impact (bps)
    pub permanent_impact_bps: f64,
    /// Total impact (bps)
    pub total_impact_bps: f64,
    /// Expected cost
    pub expected_cost: Decimal,
}

/// Almgren-Chriss market impact model
#[derive(Debug, Clone)]
pub struct AlmgrenChrissModel {
    /// Daily volume
    pub daily_volume: Decimal,
    /// Volatility (annualized)
    pub volatility: f64,
    /// Temporary impact parameter (eta)
    pub eta: f64,
    /// Permanent impact parameter (gamma)
    pub gamma: f64,
}

impl AlmgrenChrissModel {
    pub fn new(daily_volume: Decimal, volatility: f64) -> Self {
        Self {
            daily_volume,
            volatility,
            eta: 0.1,    // Default temporary impact
            gamma: 0.01, // Default permanent impact
        }
    }

    pub fn with_parameters(mut self, eta: f64, gamma: f64) -> Self {
        self.eta = eta;
        self.gamma = gamma;
        self
    }

    /// Estimate market impact for a given trade
    pub fn estimate_impact(
        &self,
        quantity: Decimal,
        execution_time: f64, // in hours
        current_price: Decimal,
    ) -> ImpactEstimate {
        if self.daily_volume == Decimal::ZERO {
            return ImpactEstimate {
                temporary_impact_bps: 0.0,
                permanent_impact_bps: 0.0,
                total_impact_bps: 0.0,
                expected_cost: Decimal::ZERO,
            };
        }

        // Participation rate
        let participation_rate = (quantity / self.daily_volume)
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        // Trading rate (quantity / time)
        let trading_rate = if execution_time > 0.0 {
            participation_rate / execution_time
        } else {
            participation_rate
        };

        // Temporary impact: eta * (v / V)
        let temporary_impact = self.eta * trading_rate;

        // Permanent impact: gamma * (Q / V)
        let permanent_impact = self.gamma * participation_rate;

        // Total impact
        let total_impact = temporary_impact + permanent_impact;

        // Convert to basis points
        let temporary_impact_bps = temporary_impact * 10000.0;
        let permanent_impact_bps = permanent_impact * 10000.0;
        let total_impact_bps = total_impact * 10000.0;

        // Expected cost
        let impact_fraction = Decimal::from_f64_retain(total_impact).unwrap_or(Decimal::ZERO);
        let expected_cost = current_price * quantity * impact_fraction;

        ImpactEstimate {
            temporary_impact_bps,
            permanent_impact_bps,
            total_impact_bps,
            expected_cost,
        }
    }

    /// Calculate optimal execution time
    pub fn optimal_execution_time(&self, quantity: Decimal, risk_aversion: f64) -> f64 {
        if self.daily_volume == Decimal::ZERO {
            return 1.0;
        }

        let participation = (quantity / self.daily_volume)
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.01);

        // Simplified optimal time: T = sqrt(eta / (lambda * sigma^2))
        // where lambda is risk aversion
        let numerator = self.eta * participation;
        let denominator = risk_aversion * self.volatility.powi(2);

        if denominator > 0.0 {
            (numerator / denominator).sqrt()
        } else {
            1.0 // Default to 1 hour
        }
    }
}

/// Square-root impact model (commonly used in practice)
#[derive(Debug, Clone)]
pub struct SquareRootImpactModel {
    /// Market depth parameter
    pub market_depth: Decimal,
    /// Impact coefficient
    pub impact_coef: f64,
}

impl SquareRootImpactModel {
    pub fn new(market_depth: Decimal) -> Self {
        Self {
            market_depth,
            impact_coef: 1.0,
        }
    }

    /// Estimate impact: impact = coef * sqrt(Q / market_depth)
    pub fn estimate_impact(&self, quantity: Decimal, current_price: Decimal) -> ImpactEstimate {
        if self.market_depth == Decimal::ZERO {
            return ImpactEstimate {
                temporary_impact_bps: 0.0,
                permanent_impact_bps: 0.0,
                total_impact_bps: 0.0,
                expected_cost: Decimal::ZERO,
            };
        }

        let ratio = (quantity / self.market_depth)
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        let impact = self.impact_coef * ratio.sqrt();

        // Assume 70% temporary, 30% permanent
        let temporary_fraction = 0.7;
        let permanent_fraction = 0.3;

        let temporary_impact_bps = impact * temporary_fraction * 10000.0;
        let permanent_impact_bps = impact * permanent_fraction * 10000.0;
        let total_impact_bps = impact * 10000.0;

        let impact_decimal = Decimal::from_f64_retain(impact).unwrap_or(Decimal::ZERO);
        let expected_cost = current_price * quantity * impact_decimal;

        ImpactEstimate {
            temporary_impact_bps,
            permanent_impact_bps,
            total_impact_bps,
            expected_cost,
        }
    }
}

/// Linear impact model (simple but effective)
#[derive(Debug, Clone)]
pub struct LinearImpactModel {
    /// Impact per unit (in bps per 1% of daily volume)
    pub impact_per_percent: f64,
    /// Daily volume
    pub daily_volume: Decimal,
}

impl LinearImpactModel {
    pub fn new(daily_volume: Decimal, impact_per_percent: f64) -> Self {
        Self {
            impact_per_percent,
            daily_volume,
        }
    }

    pub fn estimate_impact(&self, quantity: Decimal, current_price: Decimal) -> ImpactEstimate {
        if self.daily_volume == Decimal::ZERO {
            return ImpactEstimate {
                temporary_impact_bps: 0.0,
                permanent_impact_bps: 0.0,
                total_impact_bps: 0.0,
                expected_cost: Decimal::ZERO,
            };
        }

        let participation_pct = (quantity / self.daily_volume * dec!(100))
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0);

        let total_impact_bps = self.impact_per_percent * participation_pct;

        // Split 60/40 temporary/permanent
        let temporary_impact_bps = total_impact_bps * 0.6;
        let permanent_impact_bps = total_impact_bps * 0.4;

        let impact = total_impact_bps / 10000.0;
        let impact_decimal = Decimal::from_f64_retain(impact).unwrap_or(Decimal::ZERO);
        let expected_cost = current_price * quantity * impact_decimal;

        ImpactEstimate {
            temporary_impact_bps,
            permanent_impact_bps,
            total_impact_bps,
            expected_cost,
        }
    }
}

/// Impact decay function
#[derive(Debug, Clone)]
pub struct ImpactDecayModel {
    /// Half-life of temporary impact (in minutes)
    pub half_life: f64,
}

impl ImpactDecayModel {
    pub fn new(half_life: f64) -> Self {
        Self { half_life }
    }

    /// Calculate remaining impact after time elapsed
    pub fn decay_factor(&self, time_elapsed: f64) -> f64 {
        if self.half_life <= 0.0 {
            return 0.0;
        }

        // Exponential decay: e^(-ln(2) * t / T_half)
        let decay_rate = (2.0_f64).ln() / self.half_life;
        (-decay_rate * time_elapsed).exp()
    }

    /// Calculate expected impact including decay
    pub fn impact_with_decay(&self, initial_impact_bps: f64, time_elapsed: f64) -> f64 {
        initial_impact_bps * self.decay_factor(time_elapsed)
    }
}

/// Transient impact analyzer
#[derive(Debug)]
pub struct TransientImpactAnalyzer {
    #[allow(dead_code)]
    decay_model: ImpactDecayModel,
}

impl TransientImpactAnalyzer {
    pub fn new(half_life: f64) -> Self {
        Self {
            decay_model: ImpactDecayModel::new(half_life),
        }
    }

    /// Decompose impact into transient and permanent components
    pub fn decompose_impact(
        &self,
        immediate_impact_bps: f64,
        impact_after_time_bps: f64,
        _time_elapsed: f64,
    ) -> (f64, f64) {
        // Permanent impact is what remains after full decay
        let permanent_impact = impact_after_time_bps;

        // Transient impact is the difference that has decayed
        let transient_impact = immediate_impact_bps - permanent_impact;

        (transient_impact.max(0.0), permanent_impact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_almgren_chriss_impact() {
        let model = AlmgrenChrissModel::new(dec!(1000000), 0.3).with_parameters(0.1, 0.01);

        let impact = model.estimate_impact(
            dec!(10000), // 1% of daily volume
            1.0,         // 1 hour
            dec!(100),   // price
        );

        assert!(impact.total_impact_bps > 0.0);
        assert!(impact.temporary_impact_bps > 0.0);
        assert!(impact.permanent_impact_bps > 0.0);
    }

    #[test]
    fn test_square_root_impact() {
        let model = SquareRootImpactModel::new(dec!(500000));

        let impact = model.estimate_impact(dec!(10000), dec!(100));

        assert!(impact.total_impact_bps > 0.0);
        // Temporary should be larger than permanent (70/30 split)
        assert!(impact.temporary_impact_bps > impact.permanent_impact_bps);
    }

    #[test]
    fn test_linear_impact() {
        let model = LinearImpactModel::new(dec!(1000000), 10.0); // 10 bps per 1%

        let impact = model.estimate_impact(dec!(10000), dec!(100)); // 1% of volume

        // Should be approximately 10 bps total
        assert!((impact.total_impact_bps - 10.0).abs() < 1.0);
    }

    #[test]
    fn test_impact_decay() {
        let decay_model = ImpactDecayModel::new(30.0); // 30 minute half-life

        let factor_0 = decay_model.decay_factor(0.0);
        let factor_30 = decay_model.decay_factor(30.0);
        let factor_60 = decay_model.decay_factor(60.0);

        assert!((factor_0 - 1.0).abs() < 0.01); // No decay at t=0
        assert!((factor_30 - 0.5).abs() < 0.01); // 50% remaining at half-life
        assert!((factor_60 - 0.25).abs() < 0.01); // 25% remaining at 2*half-life
    }

    #[test]
    fn test_optimal_execution_time() {
        let model = AlmgrenChrissModel::new(dec!(1000000), 0.3);

        let optimal_time = model.optimal_execution_time(dec!(50000), 0.5);

        // Should return a positive time
        assert!(optimal_time > 0.0);
    }

    #[test]
    fn test_transient_impact_decomposition() {
        let analyzer = TransientImpactAnalyzer::new(30.0);

        let (transient, permanent) = analyzer.decompose_impact(
            100.0, // immediate impact
            30.0,  // impact after time
            60.0,  // time elapsed
        );

        assert_eq!(transient, 70.0);
        assert_eq!(permanent, 30.0);
    }
}
