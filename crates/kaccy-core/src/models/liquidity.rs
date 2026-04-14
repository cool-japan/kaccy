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
    /// Unique identifier for this liquidity pool
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
    /// Timestamp when the pool was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the pool was last updated
    pub updated_at: DateTime<Utc>,
}

/// Operational status of a liquidity pool
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum PoolStatus {
    /// Pool is accepting swaps and liquidity operations
    #[default]
    Active,
    /// Pool is temporarily suspended; no new swaps accepted
    Paused,
    /// Pool has been permanently closed
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
    /// Unique identifier for this LP position
    pub position_id: Uuid,
    /// Pool this position belongs to
    pub pool_id: Uuid,
    /// User who owns this LP position
    pub user_id: Uuid,
    /// Amount of LP tokens held
    pub lp_tokens: Decimal,
    /// Initial reserve A when LP tokens were minted
    pub initial_reserve_a: Decimal,
    /// Initial reserve B when LP tokens were minted
    pub initial_reserve_b: Decimal,
    /// Share of the pool (0-1)
    pub pool_share: Decimal,
    /// Timestamp when the position was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the position was last updated
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

/// Request to create a new AMM liquidity pool
#[derive(Debug, Deserialize)]
pub struct CreatePoolRequest {
    /// First token in the pair
    pub token_a_id: Uuid,
    /// Second token in the pair
    pub token_b_id: Uuid,
    /// Initial deposit of token A
    pub initial_reserve_a: Decimal,
    /// Initial deposit of token B
    pub initial_reserve_b: Decimal,
    /// Trading fee percentage; defaults to 0.3% if not specified
    pub fee_percentage: Option<Decimal>,
}

impl CreatePoolRequest {
    /// Validate the create-pool request fields
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

/// Request to add liquidity to an existing pool
#[derive(Debug, Deserialize)]
pub struct AddLiquidityRequest {
    /// Pool to add liquidity to
    pub pool_id: Uuid,
    /// Amount of token A to deposit
    pub amount_a: Decimal,
    /// Amount of token B to deposit
    pub amount_b: Decimal,
    /// Minimum LP tokens to receive; reverts if fewer would be minted
    pub min_lp_tokens: Option<Decimal>,
}

impl AddLiquidityRequest {
    /// Validate the add-liquidity request fields
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

/// Request to remove liquidity by burning LP tokens
#[derive(Debug, Deserialize)]
pub struct RemoveLiquidityRequest {
    /// Pool to remove liquidity from
    pub pool_id: Uuid,
    /// Amount of LP tokens to burn
    pub lp_tokens: Decimal,
    /// Minimum token A to receive; reverts if pool would return less
    pub min_amount_a: Option<Decimal>,
    /// Minimum token B to receive; reverts if pool would return less
    pub min_amount_b: Option<Decimal>,
}

impl RemoveLiquidityRequest {
    /// Validate the remove-liquidity request fields
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.lp_tokens <= dec!(0) {
            return Err(ValidationError("LP tokens must be positive".to_string()));
        }
        Ok(())
    }
}

/// Request to execute a token swap through a pool
#[derive(Debug, Deserialize)]
pub struct SwapRequest {
    /// Pool to swap through
    pub pool_id: Uuid,
    /// Amount of the input token to sell
    pub input_amount: Decimal,
    /// True if the input token is token A; false if it is token B
    pub input_is_a: bool,
    /// Minimum output tokens to receive; reverts if pool would return less
    pub min_output: Option<Decimal>,
    /// Maximum acceptable price impact (0–1); reverts if exceeded
    pub max_slippage: Option<Decimal>,
}

impl SwapRequest {
    /// Validate the swap request fields
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
    /// Pool being described
    pub pool_id: Uuid,
    /// Current reserve of token A
    pub reserve_a: Decimal,
    /// Current reserve of token B
    pub reserve_b: Decimal,
    /// Spot price of token A denominated in token B
    pub price_a_in_b: Decimal,
    /// Spot price of token B denominated in token A
    pub price_b_in_a: Decimal,
    /// Total LP tokens outstanding
    pub total_lp_tokens: Decimal,
    /// Lifetime trading volume in token A
    pub cumulative_volume_a: Decimal,
    /// Lifetime trading volume in token B
    pub cumulative_volume_b: Decimal,
    /// Lifetime fees collected in token A
    pub total_fees_a: Decimal,
    /// Lifetime fees collected in token B
    pub total_fees_b: Decimal,
    /// Annual Percentage Rate for LP token holders
    pub apr: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // ----------------------------------------------------------------
    // Helpers
    // ----------------------------------------------------------------

    /// Build a basic balanced pool with reserves 1000:1000, fee 0.3%, 1000 LP tokens.
    fn make_pool(reserve_a: Decimal, reserve_b: Decimal) -> LiquidityPool {
        let now = Utc::now();
        LiquidityPool {
            pool_id: Uuid::new_v4(),
            token_a_id: Uuid::new_v4(),
            token_b_id: Uuid::new_v4(),
            reserve_a,
            reserve_b,
            total_lp_tokens: dec!(1000),
            fee_percentage: dec!(0.003),
            status: PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: now,
            updated_at: now,
        }
    }

    fn make_lp_position(pool: &LiquidityPool, lp_tokens: Decimal) -> LpPosition {
        let now = Utc::now();
        LpPosition {
            position_id: Uuid::new_v4(),
            pool_id: pool.pool_id,
            user_id: Uuid::new_v4(),
            lp_tokens,
            initial_reserve_a: pool.reserve_a,
            initial_reserve_b: pool.reserve_b,
            pool_share: lp_tokens / pool.total_lp_tokens,
            created_at: now,
            updated_at: now,
        }
    }

    // ----------------------------------------------------------------
    // Pool creation and initial state
    // ----------------------------------------------------------------

    #[test]
    fn test_pool_initial_constant_product() {
        let pool = make_pool(dec!(1000), dec!(500));
        // k = 1000 * 500 = 500_000
        assert_eq!(pool.calculate_k(), dec!(500000));
    }

    #[test]
    fn test_pool_price_a_in_b() {
        let pool = make_pool(dec!(1000), dec!(500));
        // price of A = reserve_b / reserve_a = 500 / 1000 = 0.5
        assert_eq!(pool.price_a_in_b(), dec!(0.5));
    }

    #[test]
    fn test_pool_price_b_in_a() {
        let pool = make_pool(dec!(1000), dec!(500));
        // price of B = reserve_a / reserve_b = 1000 / 500 = 2
        assert_eq!(pool.price_b_in_a(), dec!(2));
    }

    #[test]
    fn test_pool_price_a_in_b_zero_reserve_returns_zero() {
        let pool = make_pool(dec!(0), dec!(500));
        assert_eq!(pool.price_a_in_b(), dec!(0));
    }

    // ----------------------------------------------------------------
    // Constant-product invariant after output calculation
    // ----------------------------------------------------------------

    #[test]
    fn test_calculate_output_respects_constant_product_approximately() {
        // With fee=0 the constant product must hold exactly.
        // We use a zero-fee pool for this invariant check.
        let mut pool = make_pool(dec!(1000), dec!(1000));
        pool.fee_percentage = dec!(0);

        let input = dec!(100);
        let output = pool.calculate_output(input, true);

        // After swap: new_reserve_a = 1000 + 100 = 1100
        //             new_reserve_b = 1000 - output
        // k_before = 1_000_000; k_after = 1100 * (1000 - output)
        // output = (1000 * 100) / (1000 + 100) = 1000000/1100 ≈ 90.909...
        // We only check that k stays within a small epsilon due to Decimal precision.
        let new_k = (pool.reserve_a + input) * (pool.reserve_b - output);
        let k_before = pool.calculate_k();
        // Difference must be tiny (< 1 unit)
        let diff = (new_k - k_before).abs();
        assert!(
            diff < dec!(1),
            "Constant product should be preserved; diff={diff}"
        );
    }

    #[test]
    fn test_calculate_output_positive_for_positive_input() {
        let pool = make_pool(dec!(1000), dec!(1000));
        let out = pool.calculate_output(dec!(50), true);
        assert!(
            out > dec!(0),
            "Output must be positive for a positive input"
        );
    }

    #[test]
    fn test_calculate_output_zero_for_zero_input() {
        let pool = make_pool(dec!(1000), dec!(1000));
        assert_eq!(pool.calculate_output(dec!(0), true), dec!(0));
    }

    #[test]
    fn test_calculate_output_is_less_with_fee_than_without() {
        let pool_with_fee = make_pool(dec!(1000), dec!(1000));
        let mut pool_no_fee = make_pool(dec!(1000), dec!(1000));
        pool_no_fee.pool_id = pool_with_fee.pool_id; // same ids not required, just reuse helper
        pool_no_fee.fee_percentage = dec!(0);

        let input = dec!(100);
        let out_with_fee = pool_with_fee.calculate_output(input, true);
        let out_no_fee = pool_no_fee.calculate_output(input, true);
        assert!(
            out_with_fee < out_no_fee,
            "Output with fee ({out_with_fee}) must be less than without fee ({out_no_fee})"
        );
    }

    // ----------------------------------------------------------------
    // calculate_input (inverse of calculate_output)
    // ----------------------------------------------------------------

    #[test]
    fn test_calculate_input_roundtrip() {
        let pool = make_pool(dec!(1000), dec!(1000));
        let desired_output = dec!(50);
        let required_input = pool.calculate_input(desired_output, false);
        // Feed that input back and verify we get at least the desired output
        let actual_output = pool.calculate_output(required_input, true);
        assert!(
            actual_output >= desired_output,
            "Round-trip: actual_output ({actual_output}) must be >= desired ({desired_output})"
        );
    }

    #[test]
    fn test_calculate_input_for_impossible_output_returns_max() {
        let pool = make_pool(dec!(100), dec!(100));
        // Asking for more than the entire reserve
        let result = pool.calculate_input(dec!(200), false);
        assert_eq!(
            result,
            Decimal::MAX,
            "Impossible output must return Decimal::MAX"
        );
    }

    // ----------------------------------------------------------------
    // Price impact
    // ----------------------------------------------------------------

    #[test]
    fn test_price_impact_increases_with_trade_size() {
        let pool = make_pool(dec!(1000), dec!(1000));
        let small_impact = pool.calculate_price_impact(dec!(10), true);
        let large_impact = pool.calculate_price_impact(dec!(500), true);
        assert!(
            large_impact > small_impact,
            "Larger trade must produce greater price impact ({large_impact} vs {small_impact})"
        );
    }

    #[test]
    fn test_price_impact_zero_for_zero_input() {
        let pool = make_pool(dec!(1000), dec!(1000));
        assert_eq!(pool.calculate_price_impact(dec!(0), true), dec!(0));
    }

    // ----------------------------------------------------------------
    // Pool validation
    // ----------------------------------------------------------------

    #[test]
    fn test_pool_validate_ok_for_valid_pool() {
        let pool = make_pool(dec!(1000), dec!(1000));
        assert!(pool.validate().is_ok(), "Valid pool must pass validation");
    }

    #[test]
    fn test_pool_validate_rejects_invalid_fee() {
        let mut pool = make_pool(dec!(1000), dec!(1000));
        pool.fee_percentage = dec!(1); // must be < 1
        let err = pool.validate().expect_err("Fee >= 1 must be rejected");
        assert!(err.0.contains("Fee"), "error: {}", err.0);
    }

    #[test]
    fn test_pool_validate_rejects_negative_reserve() {
        let mut pool = make_pool(dec!(1000), dec!(1000));
        pool.reserve_a = dec!(-1);
        let err = pool
            .validate()
            .expect_err("Negative reserve must be rejected");
        assert!(err.0.contains("Reserve A"), "error: {}", err.0);
    }

    // ----------------------------------------------------------------
    // LpPosition::current_value
    // ----------------------------------------------------------------

    #[test]
    fn test_lp_position_current_value_proportional() {
        let pool = make_pool(dec!(1000), dec!(2000));
        // 100 out of 1000 LP tokens → 10% of pool
        let pos = make_lp_position(&pool, dec!(100));
        let (val_a, val_b) = pos.current_value(&pool);
        assert_eq!(val_a, dec!(100), "10% of 1000 reserve_a must be 100");
        assert_eq!(val_b, dec!(200), "10% of 2000 reserve_b must be 200");
    }

    #[test]
    fn test_lp_position_current_value_zero_when_no_lp_tokens_minted() {
        let pool = make_pool(dec!(1000), dec!(1000));
        // Manually build the position so that pool_share computation does not divide
        // by the (later-zeroed) total_lp_tokens.
        let now = Utc::now();
        let pos = LpPosition {
            position_id: Uuid::new_v4(),
            pool_id: pool.pool_id,
            user_id: Uuid::new_v4(),
            lp_tokens: dec!(100),
            initial_reserve_a: pool.reserve_a,
            initial_reserve_b: pool.reserve_b,
            pool_share: dec!(0),
            created_at: now,
            updated_at: now,
        };
        // Now zero the pool's LP supply — current_value must guard against this.
        let mut empty_pool = pool.clone();
        empty_pool.total_lp_tokens = dec!(0);
        let (val_a, val_b) = pos.current_value(&empty_pool);
        assert_eq!(val_a, dec!(0));
        assert_eq!(val_b, dec!(0));
    }

    // ----------------------------------------------------------------
    // LpPosition::impermanent_loss
    // ----------------------------------------------------------------

    #[test]
    fn test_impermanent_loss_is_zero_when_price_unchanged() {
        let pool = make_pool(dec!(1000), dec!(1000));
        let pos = make_lp_position(&pool, dec!(500));
        // Price ratio is 1:1 both initially and now → no IL
        let il = pos.impermanent_loss(&pool);
        assert_eq!(
            il,
            dec!(0),
            "No price change must produce zero impermanent loss"
        );
    }

    #[test]
    fn test_impermanent_loss_is_positive_when_price_changed() {
        // Simulate a pool where the price has changed: initial was 1:1, now 4:1
        let initial_pool = make_pool(dec!(1000), dec!(1000));
        let mut pos = make_lp_position(&initial_pool, dec!(500));
        // Now the current pool reflects new price (e.g., reserve_a halved, reserve_b doubled)
        let current_pool = make_pool(dec!(500), dec!(2000));
        // Update position to reference the current pool
        pos.pool_id = current_pool.pool_id;

        let il = pos.impermanent_loss(&current_pool);
        assert!(
            il > dec!(0),
            "A 4x price change must produce positive impermanent loss, got {il}"
        );
    }

    // ----------------------------------------------------------------
    // CreatePoolRequest::validate
    // ----------------------------------------------------------------

    #[test]
    fn test_create_pool_request_same_token_rejected() {
        let id = Uuid::new_v4();
        let req = CreatePoolRequest {
            token_a_id: id,
            token_b_id: id,
            initial_reserve_a: dec!(100),
            initial_reserve_b: dec!(100),
            fee_percentage: None,
        };
        let err = req
            .validate()
            .expect_err("Same token pool must be rejected");
        assert!(err.0.contains("same token"), "error: {}", err.0);
    }

    #[test]
    fn test_create_pool_request_zero_reserve_rejected() {
        let req = CreatePoolRequest {
            token_a_id: Uuid::new_v4(),
            token_b_id: Uuid::new_v4(),
            initial_reserve_a: dec!(0),
            initial_reserve_b: dec!(100),
            fee_percentage: None,
        };
        let err = req.validate().expect_err("Zero reserve must be rejected");
        assert!(err.0.contains("reserve A"), "error: {}", err.0);
    }

    // ----------------------------------------------------------------
    // PoolStatus Display
    // ----------------------------------------------------------------

    #[test]
    fn test_pool_status_display() {
        assert_eq!(PoolStatus::Active.to_string(), "active");
        assert_eq!(PoolStatus::Paused.to_string(), "paused");
        assert_eq!(PoolStatus::Closed.to_string(), "closed");
    }
}
