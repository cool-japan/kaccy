//! Liquidity pool and LP token models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// AMM liquidity pool using constant product (x*y=k) formula
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiquidityPool {
    pub pool_id: Uuid,
    /// First token in the pair (e.g., $KACCY)
    pub token_a_id: Uuid,
    /// Second token in the pair (e.g., BTC or another token)
    pub token_b_id: Uuid,
    /// Reserve of token A
    pub reserve_a: Decimal,
    /// Reserve of token B
    pub reserve_b: Decimal,
    /// Total LP tokens minted
    pub total_lp_tokens: Decimal,
    /// Trading fee percentage (e.g., 0.003 for 0.3%)
    pub fee_percentage: Decimal,
    /// Pool status
    pub status: PoolStatus,
    /// Cumulative trading volume in token A
    pub cumulative_volume_a: Decimal,
    /// Cumulative trading volume in token B
    pub cumulative_volume_b: Decimal,
    /// Total fees collected in token A
    pub total_fees_a: Decimal,
    /// Total fees collected in token B
    pub total_fees_b: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum PoolStatus {
    #[default]
    Active,
    Paused,
    Closed,
}

impl fmt::Display for PoolStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolStatus::Active => write!(f, "active"),
            PoolStatus::Paused => write!(f, "paused"),
            PoolStatus::Closed => write!(f, "closed"),
        }
    }
}

impl LiquidityPool {
    /// Calculate the constant product k (x * y)
    pub fn calculate_k(&self) -> Decimal {
        self.reserve_a * self.reserve_b
    }

    /// Get the current price of token A in terms of token B
    pub fn price_a_in_b(&self) -> Decimal {
        if self.reserve_a == dec!(0) {
            return dec!(0);
        }
        self.reserve_b / self.reserve_a
    }

    /// Get the current price of token B in terms of token A
    pub fn price_b_in_a(&self) -> Decimal {
        if self.reserve_b == dec!(0) {
            return dec!(0);
        }
        self.reserve_a / self.reserve_b
    }

    /// Calculate output amount for a given input (with fee)
    /// Uses formula: dy = (y * dx * (1-fee)) / (x + dx * (1-fee))
    pub fn calculate_output(&self, input_amount: Decimal, input_is_a: bool) -> Decimal {
        if input_amount <= dec!(0) {
            return dec!(0);
        }

        let (reserve_in, reserve_out) = if input_is_a {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };

        if reserve_in == dec!(0) || reserve_out == dec!(0) {
            return dec!(0);
        }

        // Apply fee to input amount
        let input_with_fee = input_amount * (dec!(1) - self.fee_percentage);

        // Calculate output: dy = (y * dx') / (x + dx')
        // where dx' = dx * (1 - fee)
        let numerator = reserve_out * input_with_fee;
        let denominator = reserve_in + input_with_fee;

        if denominator == dec!(0) {
            return dec!(0);
        }

        numerator / denominator
    }

    /// Calculate input amount needed for a desired output (with fee)
    /// Uses formula: dx = (x * dy) / ((y - dy) * (1-fee))
    pub fn calculate_input(&self, output_amount: Decimal, output_is_a: bool) -> Decimal {
        if output_amount <= dec!(0) {
            return dec!(0);
        }

        let (reserve_in, reserve_out) = if output_is_a {
            (self.reserve_b, self.reserve_a)
        } else {
            (self.reserve_a, self.reserve_b)
        };

        if reserve_in == dec!(0) || reserve_out == dec!(0) || output_amount >= reserve_out {
            return Decimal::MAX;
        }

        // Calculate input before fee: dx' = (x * dy) / (y - dy)
        let numerator = reserve_in * output_amount;
        let denominator = reserve_out - output_amount;

        if denominator <= dec!(0) {
            return Decimal::MAX;
        }

        let input_before_fee = numerator / denominator;

        // Adjust for fee: dx = dx' / (1 - fee)
        input_before_fee / (dec!(1) - self.fee_percentage)
    }

    /// Calculate price impact for a trade
    pub fn calculate_price_impact(&self, input_amount: Decimal, input_is_a: bool) -> Decimal {
        let price_before = if input_is_a {
            self.price_a_in_b()
        } else {
            self.price_b_in_a()
        };

        if price_before == dec!(0) {
            return dec!(0);
        }

        let output_amount = self.calculate_output(input_amount, input_is_a);
        if output_amount == dec!(0) {
            return dec!(0);
        }

        // Effective price = output / input
        let effective_price = output_amount / input_amount;

        // Price impact = (price_before - effective_price) / price_before
        ((price_before - effective_price) / price_before).abs()
    }

    /// Validate the pool state
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.reserve_a < dec!(0) {
            return Err(ValidationError("Reserve A cannot be negative".to_string()));
        }
        if self.reserve_b < dec!(0) {
            return Err(ValidationError("Reserve B cannot be negative".to_string()));
        }
        if self.total_lp_tokens < dec!(0) {
            return Err(ValidationError(
                "Total LP tokens cannot be negative".to_string(),
            ));
        }
        if self.fee_percentage < dec!(0) || self.fee_percentage >= dec!(1) {
            return Err(ValidationError(
                "Fee percentage must be between 0 and 1".to_string(),
            ));
        }
        Ok(())
    }
}

/// LP (Liquidity Provider) token position
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LpPosition {
    pub position_id: Uuid,
    pub pool_id: Uuid,
    pub user_id: Uuid,
    /// Amount of LP tokens held
    pub lp_tokens: Decimal,
    /// Initial reserve A when LP tokens were minted
    pub initial_reserve_a: Decimal,
    /// Initial reserve B when LP tokens were minted
    pub initial_reserve_b: Decimal,
    /// Share of the pool (0-1)
    pub pool_share: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LpPosition {
    /// Calculate the current value of this position in terms of token A and B
    pub fn current_value(&self, pool: &LiquidityPool) -> (Decimal, Decimal) {
        if pool.total_lp_tokens == dec!(0) {
            return (dec!(0), dec!(0));
        }

        let share = self.lp_tokens / pool.total_lp_tokens;
        let current_a = pool.reserve_a * share;
        let current_b = pool.reserve_b * share;

        (current_a, current_b)
    }

    /// Calculate impermanent loss percentage
    /// IL = (value_if_held - value_in_pool) / value_if_held
    pub fn impermanent_loss(&self, pool: &LiquidityPool) -> Decimal {
        if self.initial_reserve_a == dec!(0) || self.initial_reserve_b == dec!(0) {
            return dec!(0);
        }

        let (current_a, current_b) = self.current_value(pool);

        // Calculate initial price ratio
        let initial_price_ratio = self.initial_reserve_b / self.initial_reserve_a;

        // Calculate current price ratio
        let current_price_ratio = if current_a == dec!(0) {
            return dec!(1); // 100% loss if somehow we have 0 tokens
        } else {
            current_b / current_a
        };

        // Price change ratio
        let price_ratio = current_price_ratio / initial_price_ratio;

        // Impermanent loss formula: IL = 2 * sqrt(ratio) / (1 + ratio) - 1
        let sqrt_ratio = if let Some(val) = price_ratio.to_f64() {
            Decimal::from_f64_retain(val.sqrt()).unwrap_or(dec!(1))
        } else {
            dec!(1)
        };
        let il = (dec!(2) * sqrt_ratio) / (dec!(1) + price_ratio) - dec!(1);

        // Return absolute value (IL is typically shown as a positive %)
        il.abs()
    }

    /// Calculate total return including fees
    /// Returns (total_return_percentage, fee_return_percentage, impermanent_loss_percentage)
    pub fn calculate_returns(&self, pool: &LiquidityPool) -> (Decimal, Decimal, Decimal) {
        let il = self.impermanent_loss(pool);

        // Calculate fee earnings based on pool share and accumulated fees
        let fee_share = if pool.total_lp_tokens == dec!(0) {
            dec!(0)
        } else {
            self.lp_tokens / pool.total_lp_tokens
        };

        let fees_a = pool.total_fees_a * fee_share;
        let _fees_b = pool.total_fees_b * fee_share;

        // Convert to percentage of initial position
        let initial_value_a = self.initial_reserve_a;
        let fee_return = if initial_value_a == dec!(0) {
            dec!(0)
        } else {
            fees_a / initial_value_a
        };

        // Total return = fee return - impermanent loss
        let total_return = fee_return - il;

        (total_return, fee_return, il)
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePoolRequest {
    pub token_a_id: Uuid,
    pub token_b_id: Uuid,
    pub initial_reserve_a: Decimal,
    pub initial_reserve_b: Decimal,
    pub fee_percentage: Option<Decimal>,
}

impl CreatePoolRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.token_a_id == self.token_b_id {
            return Err(ValidationError(
                "Cannot create pool with same token".to_string(),
            ));
        }
        if self.initial_reserve_a <= dec!(0) {
            return Err(ValidationError(
                "Initial reserve A must be positive".to_string(),
            ));
        }
        if self.initial_reserve_b <= dec!(0) {
            return Err(ValidationError(
                "Initial reserve B must be positive".to_string(),
            ));
        }
        if let Some(fee) = self.fee_percentage {
            if fee < dec!(0) || fee >= dec!(1) {
                return Err(ValidationError(
                    "Fee percentage must be between 0 and 1".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct AddLiquidityRequest {
    pub pool_id: Uuid,
    pub amount_a: Decimal,
    pub amount_b: Decimal,
    pub min_lp_tokens: Option<Decimal>,
}

impl AddLiquidityRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount_a <= dec!(0) {
            return Err(ValidationError("Amount A must be positive".to_string()));
        }
        if self.amount_b <= dec!(0) {
            return Err(ValidationError("Amount B must be positive".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct RemoveLiquidityRequest {
    pub pool_id: Uuid,
    pub lp_tokens: Decimal,
    pub min_amount_a: Option<Decimal>,
    pub min_amount_b: Option<Decimal>,
}

impl RemoveLiquidityRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.lp_tokens <= dec!(0) {
            return Err(ValidationError("LP tokens must be positive".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct SwapRequest {
    pub pool_id: Uuid,
    pub input_amount: Decimal,
    pub input_is_a: bool,
    pub min_output: Option<Decimal>,
    pub max_slippage: Option<Decimal>,
}

impl SwapRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.input_amount <= dec!(0) {
            return Err(ValidationError("Input amount must be positive".to_string()));
        }
        if let Some(slippage) = self.max_slippage {
            if slippage < dec!(0) || slippage > dec!(1) {
                return Err(ValidationError(
                    "Slippage must be between 0 and 1".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Pool statistics and analytics
#[derive(Debug, Serialize)]
pub struct PoolStats {
    pub pool_id: Uuid,
    pub reserve_a: Decimal,
    pub reserve_b: Decimal,
    pub price_a_in_b: Decimal,
    pub price_b_in_a: Decimal,
    pub total_lp_tokens: Decimal,
    pub cumulative_volume_a: Decimal,
    pub cumulative_volume_b: Decimal,
    pub total_fees_a: Decimal,
    pub total_fees_b: Decimal,
    pub apr: Option<Decimal>, // Annual Percentage Rate for LPs
}
