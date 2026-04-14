//! Just-in-time (JIT) liquidity provisioning
//!
//! JIT liquidity allows liquidity providers to add liquidity right before a trade
//! and remove it immediately after, capturing fees while minimizing capital
//! requirements and impermanent loss exposure.

use crate::error::Result;
use rust_decimal::Decimal;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// JIT liquidity position
#[derive(Debug, Clone)]
pub struct JitPosition {
    /// Unique identifier for this JIT position
    pub id: Uuid,
    /// Liquidity provider user ID
    pub provider: Uuid,
    /// Token pool this position was added to
    pub token_id: Uuid,
    /// Amount of token A deposited
    pub amount_a: Decimal,
    /// Amount of token B deposited
    pub amount_b: Decimal,
    /// When this JIT position was added
    pub added_at: SystemTime,
    /// When this JIT position was removed (if applicable)
    pub removed_at: Option<SystemTime>,
    /// Fees earned while this position was active
    pub fees_earned: Decimal,
}

/// JIT liquidity opportunity
#[derive(Debug, Clone)]
pub struct JitOpportunity {
    /// Token pool where the opportunity exists
    pub token_id: Uuid,
    /// Volume of the pending trade that triggered this opportunity
    pub pending_trade_volume: Decimal,
    /// Current liquidity depth in the pool
    pub current_liquidity: Decimal,
    /// Expected fee income from providing JIT liquidity for this trade
    pub expected_fee: Decimal,
    /// Overall risk score for this opportunity (higher = riskier)
    pub risk_score: Decimal,
    /// Suggested JIT liquidity amount to deploy
    pub optimal_amount: Decimal,
    /// Time window within which liquidity must be added
    pub timing_window: Duration,
    /// Probability of MEV exploitation (0–1)
    pub mev_risk: Decimal,
}

/// JIT liquidity provider
#[derive(Debug)]
pub struct JitLiquidityProvider {
    /// Minimum profit threshold (as a ratio) to consider an opportunity viable
    pub min_profit_threshold: Decimal,
    /// Maximum capital to deploy in a single JIT position
    pub max_position_size: Decimal,
    /// Maximum acceptable MEV risk (0–1)
    pub max_mev_risk: Decimal,
    /// Milliseconds of buffer to allow before the trade executes
    pub timing_buffer_ms: u64,
}

impl Default for JitLiquidityProvider {
    fn default() -> Self {
        Self {
            min_profit_threshold: Decimal::new(5, 4), // 0.0005 or 0.05%
            max_position_size: Decimal::new(100000, 0),
            max_mev_risk: Decimal::new(3, 1), // 0.3 or 30%
            timing_buffer_ms: 100,
        }
    }
}

impl JitLiquidityProvider {
    /// Create a new JIT liquidity provider with custom parameters
    pub fn new(
        min_profit_threshold: Decimal,
        max_position_size: Decimal,
        max_mev_risk: Decimal,
        timing_buffer_ms: u64,
    ) -> Self {
        Self {
            min_profit_threshold,
            max_position_size,
            max_mev_risk,
            timing_buffer_ms,
        }
    }

    /// Detect JIT liquidity opportunity
    pub fn detect_opportunity(
        &self,
        token_id: Uuid,
        pending_trade_volume: Decimal,
        current_liquidity: Decimal,
        pool_fee_rate: Decimal,
        volatility: Decimal,
    ) -> Result<Option<JitOpportunity>> {
        // Calculate expected fee from the trade
        let expected_fee = pending_trade_volume * pool_fee_rate;

        // Calculate risk score based on volatility and liquidity depth
        let liquidity_ratio = if current_liquidity > Decimal::ZERO {
            pending_trade_volume / current_liquidity
        } else {
            Decimal::MAX
        };

        let risk_score = (volatility * Decimal::new(5, 1)) + (liquidity_ratio * Decimal::new(3, 1));

        // Calculate MEV risk (higher when trade is large relative to liquidity)
        let mev_risk = if current_liquidity > Decimal::ZERO {
            (pending_trade_volume / current_liquidity).min(Decimal::ONE)
        } else {
            Decimal::ONE
        };

        // Skip if MEV risk is too high
        if mev_risk > self.max_mev_risk {
            return Ok(None);
        }

        // Calculate optimal amount to add
        // Aim to provide enough liquidity to capture fees without over-committing
        let optimal_amount = self.calculate_optimal_amount(
            pending_trade_volume,
            current_liquidity,
            expected_fee,
            risk_score,
        )?;

        // Check if opportunity meets profit threshold
        if expected_fee < self.min_profit_threshold || optimal_amount > self.max_position_size {
            return Ok(None);
        }

        // Timing window - must add liquidity before the trade
        let timing_window = Duration::from_millis(self.timing_buffer_ms);

        Ok(Some(JitOpportunity {
            token_id,
            pending_trade_volume,
            current_liquidity,
            expected_fee,
            risk_score,
            optimal_amount,
            timing_window,
            mev_risk,
        }))
    }

    /// Calculate optimal liquidity amount to provide
    fn calculate_optimal_amount(
        &self,
        trade_volume: Decimal,
        current_liquidity: Decimal,
        _expected_fee: Decimal,
        risk_score: Decimal,
    ) -> Result<Decimal> {
        // Base amount: provide liquidity proportional to trade size
        let base_amount = trade_volume * Decimal::new(2, 1); // 0.2 or 20% of trade volume

        // Adjust for current liquidity depth
        let liquidity_adjustment = if current_liquidity > Decimal::ZERO {
            let ratio = trade_volume / current_liquidity;
            if ratio > Decimal::ONE {
                // Low liquidity: increase amount
                Decimal::ONE + ratio
            } else {
                // Sufficient liquidity: reduce amount
                Decimal::ONE
            }
        } else {
            Decimal::new(2, 0) // No liquidity: double the base
        };

        // Adjust for risk
        let risk_adjustment = Decimal::ONE - (risk_score * Decimal::new(1, 1));
        let risk_adjustment = risk_adjustment.max(Decimal::new(2, 1)); // Min 0.2

        // Calculate optimal amount
        let optimal = base_amount * liquidity_adjustment * risk_adjustment;

        // Cap at max position size
        Ok(optimal.min(self.max_position_size))
    }

    /// Calculate risk-adjusted return for JIT opportunity
    pub fn calculate_risk_adjusted_return(&self, opportunity: &JitOpportunity) -> Result<Decimal> {
        // Risk-free return would be the expected fee
        let expected_return = opportunity.expected_fee;

        // Adjust for risks
        let mev_adjustment = Decimal::ONE - opportunity.mev_risk;
        let risk_adjustment = Decimal::ONE - (opportunity.risk_score * Decimal::new(5, 2)); // 0.05

        let adjusted_return = expected_return * mev_adjustment * risk_adjustment.max(Decimal::ZERO);

        Ok(adjusted_return)
    }

    /// Execute JIT liquidity provision
    pub fn provide_liquidity(
        &self,
        provider: Uuid,
        opportunity: &JitOpportunity,
    ) -> Result<JitPosition> {
        let amount = opportunity.optimal_amount;

        // In a real implementation, this would:
        // 1. Check provider has sufficient balance
        // 2. Add liquidity to the pool atomically before the trade
        // 3. Set up automatic removal after the trade

        let position = JitPosition {
            id: Uuid::new_v4(),
            provider,
            token_id: opportunity.token_id,
            amount_a: amount / Decimal::new(2, 0), // Split equally for AMM
            amount_b: amount / Decimal::new(2, 0),
            added_at: SystemTime::now(),
            removed_at: None,
            fees_earned: Decimal::ZERO,
        };

        Ok(position)
    }

    /// Remove JIT liquidity and collect fees
    pub fn remove_liquidity(&self, position: &mut JitPosition, fees_earned: Decimal) -> Result<()> {
        position.removed_at = Some(SystemTime::now());
        position.fees_earned = fees_earned;
        Ok(())
    }

    /// Calculate profitability of a completed JIT position
    pub fn calculate_profitability(&self, position: &JitPosition) -> Result<Decimal> {
        if position.removed_at.is_none() {
            return Ok(Decimal::ZERO);
        }

        // Total capital deployed
        let capital = position.amount_a + position.amount_b;

        // Fees earned
        let profit = position.fees_earned;

        // Calculate ROI
        let roi = if capital > Decimal::ZERO {
            (profit / capital) * Decimal::new(100, 0) // As percentage
        } else {
            Decimal::ZERO
        };

        Ok(roi)
    }
}

/// JIT liquidity strategy optimizer
#[derive(Debug)]
pub struct JitStrategyOptimizer {
    /// Minimum liquidity gap (relative to pool) that warrants JIT provisioning
    pub min_liquidity_gap: Decimal,
    /// Target percentage of available fees to capture
    pub target_fee_capture: Decimal,
    /// Maximum gas cost as a ratio of expected fee income
    pub max_gas_cost_ratio: Decimal,
}

impl Default for JitStrategyOptimizer {
    fn default() -> Self {
        Self {
            min_liquidity_gap: Decimal::new(5, 1),  // 0.5 or 50%
            target_fee_capture: Decimal::new(7, 1), // 0.7 or 70%
            max_gas_cost_ratio: Decimal::new(1, 1), // 0.1 or 10%
        }
    }
}

impl JitStrategyOptimizer {
    /// Determine optimal timing for JIT liquidity addition
    pub fn calculate_optimal_timing(
        &self,
        opportunity: &JitOpportunity,
        block_time_ms: u64,
    ) -> Result<Duration> {
        // Calculate blocks needed to execute
        let execution_blocks = 1; // Minimum blocks for atomic execution

        // Add buffer for MEV protection
        let mev_buffer_blocks = if opportunity.mev_risk > Decimal::new(2, 1) {
            2 // High MEV risk: add 2 block buffer
        } else {
            1 // Low MEV risk: add 1 block buffer
        };

        let total_blocks = execution_blocks + mev_buffer_blocks;
        let timing_ms = total_blocks * block_time_ms;

        Ok(Duration::from_millis(timing_ms))
    }

    /// Evaluate if gas costs make JIT profitable
    pub fn evaluate_gas_efficiency(
        &self,
        expected_fee: Decimal,
        estimated_gas_cost: Decimal,
    ) -> Result<bool> {
        if expected_fee <= Decimal::ZERO {
            return Ok(false);
        }

        let gas_ratio = estimated_gas_cost / expected_fee;

        Ok(gas_ratio <= self.max_gas_cost_ratio)
    }

    /// Calculate competitive advantage over passive LP
    pub fn calculate_capital_efficiency(
        &self,
        jit_capital: Decimal,
        jit_fees: Decimal,
        jit_duration_hours: Decimal,
        passive_capital: Decimal,
        passive_fees_annual: Decimal,
    ) -> Result<Decimal> {
        // Annualize JIT returns
        let hours_per_year = Decimal::new(8760, 0);
        let jit_annual_return = if jit_duration_hours > Decimal::ZERO && jit_capital > Decimal::ZERO
        {
            (jit_fees / jit_capital) * (hours_per_year / jit_duration_hours)
        } else {
            Decimal::ZERO
        };

        // Passive LP annual return
        let passive_annual_return = if passive_capital > Decimal::ZERO {
            passive_fees_annual / passive_capital
        } else {
            Decimal::ZERO
        };

        // Capital efficiency advantage
        let advantage = if passive_annual_return > Decimal::ZERO {
            (jit_annual_return - passive_annual_return) / passive_annual_return
                * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        };

        Ok(advantage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_opportunity_detection() {
        let provider = JitLiquidityProvider::default();
        let token_id = Uuid::new_v4();

        let result = provider
            .detect_opportunity(
                token_id,
                Decimal::new(10000, 0), // Trade volume
                Decimal::new(50000, 0), // Current liquidity
                Decimal::new(3, 3),     // 0.3% fee
                Decimal::new(15, 2),    // 15% volatility
            )
            .unwrap();

        assert!(result.is_some());
        let opp = result.unwrap();
        assert!(opp.expected_fee > Decimal::ZERO);
        assert!(opp.optimal_amount > Decimal::ZERO);
    }

    #[test]
    fn test_high_mev_risk_rejection() {
        let provider = JitLiquidityProvider::default();
        let token_id = Uuid::new_v4();

        // Very large trade relative to liquidity
        let result = provider
            .detect_opportunity(
                token_id,
                Decimal::new(100000, 0), // Large trade
                Decimal::new(10000, 0),  // Small liquidity
                Decimal::new(3, 3),      // 0.3% fee
                Decimal::new(20, 2),     // 20% volatility
            )
            .unwrap();

        // Should reject due to high MEV risk
        assert!(result.is_none());
    }

    #[test]
    fn test_provide_and_remove_liquidity() {
        let provider = JitLiquidityProvider::default();
        let token_id = Uuid::new_v4();
        let provider_id = Uuid::new_v4();

        let opportunity = JitOpportunity {
            token_id,
            pending_trade_volume: Decimal::new(10000, 0),
            current_liquidity: Decimal::new(50000, 0),
            expected_fee: Decimal::new(30, 0),
            risk_score: Decimal::new(2, 1),
            optimal_amount: Decimal::new(5000, 0),
            timing_window: Duration::from_millis(100),
            mev_risk: Decimal::new(1, 1),
        };

        let mut position = provider
            .provide_liquidity(provider_id, &opportunity)
            .unwrap();
        assert_eq!(position.removed_at, None);

        provider
            .remove_liquidity(&mut position, Decimal::new(30, 0))
            .unwrap();
        assert!(position.removed_at.is_some());
        assert_eq!(position.fees_earned, Decimal::new(30, 0));
    }

    #[test]
    fn test_profitability_calculation() {
        let provider = JitLiquidityProvider::default();

        let position = JitPosition {
            id: Uuid::new_v4(),
            provider: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount_a: Decimal::new(2500, 0),
            amount_b: Decimal::new(2500, 0),
            added_at: SystemTime::now(),
            removed_at: Some(SystemTime::now()),
            fees_earned: Decimal::new(30, 0),
        };

        let roi = provider.calculate_profitability(&position).unwrap();
        // ROI = (30 / 5000) * 100 = 0.6%
        assert_eq!(roi, Decimal::new(60, 2));
    }

    #[test]
    fn test_risk_adjusted_return() {
        let provider = JitLiquidityProvider::default();

        let opportunity = JitOpportunity {
            token_id: Uuid::new_v4(),
            pending_trade_volume: Decimal::new(10000, 0),
            current_liquidity: Decimal::new(50000, 0),
            expected_fee: Decimal::new(30, 0),
            risk_score: Decimal::new(2, 1),
            optimal_amount: Decimal::new(5000, 0),
            timing_window: Duration::from_millis(100),
            mev_risk: Decimal::new(1, 1),
        };

        let adjusted_return = provider
            .calculate_risk_adjusted_return(&opportunity)
            .unwrap();
        assert!(adjusted_return < opportunity.expected_fee);
        assert!(adjusted_return > Decimal::ZERO);
    }

    #[test]
    fn test_gas_efficiency_evaluation() {
        let optimizer = JitStrategyOptimizer::default();

        // Profitable case
        assert!(
            optimizer
                .evaluate_gas_efficiency(Decimal::new(100, 0), Decimal::new(5, 0))
                .unwrap()
        );

        // Unprofitable case (gas too high)
        assert!(
            !optimizer
                .evaluate_gas_efficiency(Decimal::new(100, 0), Decimal::new(20, 0))
                .unwrap()
        );
    }

    #[test]
    fn test_capital_efficiency() {
        let optimizer = JitStrategyOptimizer::default();

        let advantage = optimizer
            .calculate_capital_efficiency(
                Decimal::new(5000, 0),  // JIT capital
                Decimal::new(30, 0),    // JIT fees
                Decimal::new(1, 0),     // 1 hour
                Decimal::new(50000, 0), // Passive capital
                Decimal::new(500, 0),   // Passive annual fees
            )
            .unwrap();

        // JIT should have much higher capital efficiency
        assert!(advantage > Decimal::ZERO);
    }

    #[test]
    fn test_optimal_timing_calculation() {
        let optimizer = JitStrategyOptimizer::default();

        let opportunity = JitOpportunity {
            token_id: Uuid::new_v4(),
            pending_trade_volume: Decimal::new(10000, 0),
            current_liquidity: Decimal::new(50000, 0),
            expected_fee: Decimal::new(30, 0),
            risk_score: Decimal::new(2, 1),
            optimal_amount: Decimal::new(5000, 0),
            timing_window: Duration::from_millis(100),
            mev_risk: Decimal::new(3, 1), // High MEV risk
        };

        let timing = optimizer
            .calculate_optimal_timing(&opportunity, 12000)
            .unwrap();
        // Should include MEV buffer blocks
        assert!(timing.as_millis() >= 12000);
    }
}
