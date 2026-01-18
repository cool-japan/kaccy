//! Automated Market Maker (AMM) executor for liquidity pools

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::{
    AddLiquidityRequest, CreatePoolRequest, LiquidityPool, LpPosition, PoolStatus,
    RemoveLiquidityRequest, SwapRequest,
};

/// Oracle price data for pool price bounds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePrice {
    pub pool_id: Uuid,
    /// Reference price from oracle
    pub reference_price: Decimal,
    /// Maximum deviation allowed from oracle price (e.g., 0.05 = 5%)
    pub max_deviation: Decimal,
    /// Timestamp of last oracle update
    pub updated_at: DateTime<Utc>,
    /// Oracle source identifier
    pub oracle_source: String,
}

impl OraclePrice {
    /// Check if pool price is within acceptable bounds
    pub fn is_price_within_bounds(&self, pool_price: Decimal) -> bool {
        let lower_bound = self.reference_price * (dec!(1) - self.max_deviation);
        let upper_bound = self.reference_price * (dec!(1) + self.max_deviation);
        pool_price >= lower_bound && pool_price <= upper_bound
    }

    /// Calculate deviation from oracle price
    pub fn calculate_deviation(&self, pool_price: Decimal) -> Decimal {
        if self.reference_price == dec!(0) {
            return dec!(0);
        }
        ((pool_price - self.reference_price) / self.reference_price).abs()
    }
}

/// Volatility tracker for dynamic fee adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityTracker {
    pub pool_id: Uuid,
    /// Recent price changes (stored as percentages)
    pub recent_changes: Vec<Decimal>,
    /// Maximum number of price changes to track
    pub max_samples: usize,
    /// Calculated volatility (standard deviation of price changes)
    pub current_volatility: Decimal,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl VolatilityTracker {
    /// Create a new volatility tracker
    pub fn new(pool_id: Uuid, max_samples: usize) -> Self {
        Self {
            pool_id,
            recent_changes: Vec::new(),
            max_samples,
            current_volatility: dec!(0),
            updated_at: Utc::now(),
        }
    }

    /// Add a price change observation
    pub fn add_price_change(&mut self, price_change_pct: Decimal) {
        self.recent_changes.push(price_change_pct);

        // Keep only recent samples
        if self.recent_changes.len() > self.max_samples {
            self.recent_changes.remove(0);
        }

        self.current_volatility = self.calculate_volatility();
        self.updated_at = Utc::now();
    }

    /// Calculate standard deviation of price changes
    fn calculate_volatility(&self) -> Decimal {
        if self.recent_changes.is_empty() {
            return dec!(0);
        }

        let n = Decimal::from(self.recent_changes.len());
        let mean = self.recent_changes.iter().sum::<Decimal>() / n;

        let variance = self
            .recent_changes
            .iter()
            .map(|x| {
                let diff = *x - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        // Approximate square root for volatility
        if let Some(var_f64) = variance.to_f64() {
            if let Some(std_dev) = Decimal::from_f64_retain(var_f64.sqrt()) {
                return std_dev;
            }
        }

        dec!(0)
    }

    /// Calculate dynamic fee based on volatility
    /// Higher volatility = higher fees to compensate LPs for impermanent loss
    pub fn calculate_dynamic_fee(&self, base_fee: Decimal, max_fee: Decimal) -> Decimal {
        if self.current_volatility == dec!(0) {
            return base_fee;
        }

        // Fee increases linearly with volatility
        // At 10% volatility, fee is at max
        let volatility_multiplier = dec!(1) + (self.current_volatility / dec!(0.1));
        let dynamic_fee = base_fee * volatility_multiplier;

        // Cap at max_fee
        dynamic_fee.min(max_fee)
    }
}

/// Flash swap request for atomic arbitrage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashSwapRequest {
    pub pool_id: Uuid,
    /// Amount to borrow
    pub borrow_amount: Decimal,
    /// Which token to borrow (true = token A, false = token B)
    pub borrow_token_a: bool,
    /// Expected repayment amount (including fees)
    pub expected_repayment: Decimal,
    /// Callback data for flash swap logic
    pub callback_data: Vec<u8>,
}

impl FlashSwapRequest {
    pub fn validate(&self) -> Result<()> {
        if self.borrow_amount <= dec!(0) {
            return Err(CoreError::Validation(
                "Borrow amount must be positive".to_string(),
            ));
        }
        if self.expected_repayment < self.borrow_amount {
            return Err(CoreError::Validation(
                "Repayment must be at least borrow amount".to_string(),
            ));
        }
        Ok(())
    }
}

/// Flash swap result
#[derive(Debug, Serialize)]
pub struct FlashSwapResult {
    pub borrowed_amount: Decimal,
    pub repaid_amount: Decimal,
    pub fee_amount: Decimal,
    pub profit: Decimal,
}

/// AMM executor for managing liquidity pools
pub struct AmmExecutor;

impl AmmExecutor {
    /// Create a new liquidity pool
    pub fn create_pool(request: CreatePoolRequest, _creator_id: Uuid) -> Result<LiquidityPool> {
        request.validate().map_err(|e| {
            CoreError::Validation(format!("Invalid pool creation request: {}", e.0))
        })?;

        let now = Utc::now();
        let pool_id = Uuid::new_v4();

        // Default fee is 0.3% (same as Uniswap V2)
        let fee_percentage = request.fee_percentage.unwrap_or(dec!(0.003));

        // Initial LP tokens = sqrt(reserve_a * reserve_b)
        let product = request.initial_reserve_a * request.initial_reserve_b;
        let total_lp_tokens = if let Some(val) = product.to_f64() {
            Decimal::from_f64_retain(val.sqrt())
                .ok_or_else(|| CoreError::Validation("Invalid initial reserves".to_string()))?
        } else {
            return Err(CoreError::Validation(
                "Invalid initial reserves".to_string(),
            ));
        };

        Ok(LiquidityPool {
            pool_id,
            token_a_id: request.token_a_id,
            token_b_id: request.token_b_id,
            reserve_a: request.initial_reserve_a,
            reserve_b: request.initial_reserve_b,
            total_lp_tokens,
            fee_percentage,
            status: PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: now,
            updated_at: now,
        })
    }

    /// Create initial LP position for pool creator
    pub fn create_initial_position(pool: &LiquidityPool, creator_id: Uuid) -> Result<LpPosition> {
        let now = Utc::now();

        Ok(LpPosition {
            position_id: Uuid::new_v4(),
            pool_id: pool.pool_id,
            user_id: creator_id,
            lp_tokens: pool.total_lp_tokens,
            initial_reserve_a: pool.reserve_a,
            initial_reserve_b: pool.reserve_b,
            pool_share: dec!(1), // 100% initially
            created_at: now,
            updated_at: now,
        })
    }

    /// Add liquidity to an existing pool
    /// Returns (updated_pool, lp_tokens_minted, actual_amount_a, actual_amount_b)
    pub fn add_liquidity(
        pool: &mut LiquidityPool,
        request: AddLiquidityRequest,
        _user_id: Uuid,
    ) -> Result<(Decimal, Decimal, Decimal)> {
        request.validate().map_err(|e| {
            CoreError::Validation(format!("Invalid add liquidity request: {}", e.0))
        })?;

        if pool.status != PoolStatus::Active {
            return Err(CoreError::Validation("Pool is not active".to_string()));
        }

        // Calculate optimal amounts to maintain pool ratio
        let ratio = pool.reserve_b / pool.reserve_a;
        let optimal_amount_b = request.amount_a * ratio;

        let (actual_amount_a, actual_amount_b) = if optimal_amount_b <= request.amount_b {
            // Use all of amount_a and calculated amount_b
            (request.amount_a, optimal_amount_b)
        } else {
            // Use calculated amount_a and all of amount_b
            let optimal_amount_a = request.amount_b / ratio;
            (optimal_amount_a, request.amount_b)
        };

        // Calculate LP tokens to mint: min(amount_a/reserve_a, amount_b/reserve_b) * total_lp_tokens
        let lp_tokens_a = (actual_amount_a * pool.total_lp_tokens) / pool.reserve_a;
        let lp_tokens_b = (actual_amount_b * pool.total_lp_tokens) / pool.reserve_b;
        let lp_tokens = lp_tokens_a.min(lp_tokens_b);

        if let Some(min_lp) = request.min_lp_tokens {
            if lp_tokens < min_lp {
                return Err(CoreError::SlippageExceeded {
                    expected: min_lp,
                    actual: lp_tokens,
                });
            }
        }

        // Update pool reserves and total LP tokens
        pool.reserve_a += actual_amount_a;
        pool.reserve_b += actual_amount_b;
        pool.total_lp_tokens += lp_tokens;
        pool.updated_at = Utc::now();

        Ok((lp_tokens, actual_amount_a, actual_amount_b))
    }

    /// Create or update LP position when adding liquidity
    pub fn update_or_create_position(
        pool: &LiquidityPool,
        user_id: Uuid,
        lp_tokens_added: Decimal,
        amount_a_added: Decimal,
        amount_b_added: Decimal,
        existing_position: Option<LpPosition>,
    ) -> Result<LpPosition> {
        let now = Utc::now();

        match existing_position {
            Some(mut pos) => {
                // Update existing position
                pos.lp_tokens += lp_tokens_added;
                pos.pool_share = pos.lp_tokens / pool.total_lp_tokens;
                pos.updated_at = now;
                Ok(pos)
            }
            None => {
                // Create new position
                Ok(LpPosition {
                    position_id: Uuid::new_v4(),
                    pool_id: pool.pool_id,
                    user_id,
                    lp_tokens: lp_tokens_added,
                    initial_reserve_a: amount_a_added,
                    initial_reserve_b: amount_b_added,
                    pool_share: lp_tokens_added / pool.total_lp_tokens,
                    created_at: now,
                    updated_at: now,
                })
            }
        }
    }

    /// Remove liquidity from pool
    /// Returns (amount_a, amount_b) that user receives
    pub fn remove_liquidity(
        pool: &mut LiquidityPool,
        request: RemoveLiquidityRequest,
        position: &LpPosition,
    ) -> Result<(Decimal, Decimal)> {
        request.validate().map_err(|e| {
            CoreError::Validation(format!("Invalid remove liquidity request: {}", e.0))
        })?;

        if pool.status == PoolStatus::Closed {
            return Err(CoreError::Validation("Pool is closed".to_string()));
        }

        if position.lp_tokens < request.lp_tokens {
            return Err(CoreError::InsufficientBalance {
                required: request.lp_tokens,
                available: position.lp_tokens,
            });
        }

        // Calculate share of pool
        let share = request.lp_tokens / pool.total_lp_tokens;

        // Calculate amounts to return
        let amount_a = pool.reserve_a * share;
        let amount_b = pool.reserve_b * share;

        // Check slippage
        if let Some(min_a) = request.min_amount_a {
            if amount_a < min_a {
                return Err(CoreError::SlippageExceeded {
                    expected: min_a,
                    actual: amount_a,
                });
            }
        }
        if let Some(min_b) = request.min_amount_b {
            if amount_b < min_b {
                return Err(CoreError::SlippageExceeded {
                    expected: min_b,
                    actual: amount_b,
                });
            }
        }

        // Update pool
        pool.reserve_a -= amount_a;
        pool.reserve_b -= amount_b;
        pool.total_lp_tokens -= request.lp_tokens;
        pool.updated_at = Utc::now();

        Ok((amount_a, amount_b))
    }

    /// Update LP position after removing liquidity
    pub fn update_position_after_removal(
        position: &mut LpPosition,
        pool: &LiquidityPool,
        lp_tokens_removed: Decimal,
    ) {
        position.lp_tokens -= lp_tokens_removed;
        position.pool_share = if pool.total_lp_tokens > dec!(0) {
            position.lp_tokens / pool.total_lp_tokens
        } else {
            dec!(0)
        };
        position.updated_at = Utc::now();
    }

    /// Execute a swap
    /// Returns (output_amount, fee_amount)
    pub fn swap(pool: &mut LiquidityPool, request: SwapRequest) -> Result<(Decimal, Decimal)> {
        request
            .validate()
            .map_err(|e| CoreError::Validation(format!("Invalid swap request: {}", e.0)))?;

        if pool.status != PoolStatus::Active {
            return Err(CoreError::Validation("Pool is not active".to_string()));
        }

        // Calculate output amount
        let output_amount = pool.calculate_output(request.input_amount, request.input_is_a);

        if output_amount == dec!(0) {
            return Err(CoreError::Validation(
                "Invalid swap: output is zero".to_string(),
            ));
        }

        // Check minimum output
        if let Some(min_output) = request.min_output {
            if output_amount < min_output {
                return Err(CoreError::SlippageExceeded {
                    expected: min_output,
                    actual: output_amount,
                });
            }
        }

        // Check slippage
        if let Some(max_slippage) = request.max_slippage {
            let price_impact =
                pool.calculate_price_impact(request.input_amount, request.input_is_a);
            if price_impact > max_slippage {
                return Err(CoreError::SlippageExceeded {
                    expected: max_slippage,
                    actual: price_impact,
                });
            }
        }

        // Calculate fee amount
        let fee_amount = request.input_amount * pool.fee_percentage;

        // Update reserves
        if request.input_is_a {
            pool.reserve_a += request.input_amount;
            pool.reserve_b -= output_amount;
            pool.cumulative_volume_a += request.input_amount;
            pool.total_fees_a += fee_amount;
        } else {
            pool.reserve_b += request.input_amount;
            pool.reserve_a -= output_amount;
            pool.cumulative_volume_b += request.input_amount;
            pool.total_fees_b += fee_amount;
        }

        pool.updated_at = Utc::now();

        Ok((output_amount, fee_amount))
    }

    /// Calculate optimal swap route for cross-token trades
    /// Enhanced with multi-hop optimization
    pub fn calculate_optimal_route(
        pools: &[LiquidityPool],
        input_token_id: Uuid,
        output_token_id: Uuid,
        input_amount: Decimal,
    ) -> Option<(Vec<Uuid>, Decimal)> {
        // Check for direct pool
        let mut best_route: Option<(Vec<Uuid>, Decimal)> = None;

        for pool in pools {
            if pool.status != PoolStatus::Active {
                continue;
            }

            if (pool.token_a_id == input_token_id && pool.token_b_id == output_token_id)
                || (pool.token_b_id == input_token_id && pool.token_a_id == output_token_id)
            {
                let input_is_a = pool.token_a_id == input_token_id;
                let output = pool.calculate_output(input_amount, input_is_a);
                best_route = Some((vec![pool.pool_id], output));
            }
        }

        // Check for 1-hop route and compare with direct route
        for pool1 in pools {
            if pool1.status != PoolStatus::Active {
                continue;
            }

            let (intermediate_token, input_is_a1) = if pool1.token_a_id == input_token_id {
                (Some(pool1.token_b_id), true)
            } else if pool1.token_b_id == input_token_id {
                (Some(pool1.token_a_id), false)
            } else {
                (None, false)
            };

            if let Some(intermediate) = intermediate_token {
                for pool2 in pools {
                    if pool2.status != PoolStatus::Active {
                        continue;
                    }

                    let input_is_a2 = if pool2.token_a_id == intermediate
                        && pool2.token_b_id == output_token_id
                    {
                        Some(true)
                    } else if pool2.token_b_id == intermediate
                        && pool2.token_a_id == output_token_id
                    {
                        Some(false)
                    } else {
                        None
                    };

                    if let Some(is_a2) = input_is_a2 {
                        // Calculate output through 2-hop route
                        let intermediate_output = pool1.calculate_output(input_amount, input_is_a1);
                        let final_output = pool2.calculate_output(intermediate_output, is_a2);

                        // Update best route if this is better
                        if let Some((_, current_best)) = best_route {
                            if final_output > current_best {
                                best_route =
                                    Some((vec![pool1.pool_id, pool2.pool_id], final_output));
                            }
                        } else {
                            best_route = Some((vec![pool1.pool_id, pool2.pool_id], final_output));
                        }
                    }
                }
            }
        }

        best_route
    }

    /// Execute flash swap for atomic arbitrage
    /// Flash swaps allow borrowing tokens, executing arbitrary logic, then repaying
    pub fn execute_flash_swap(
        pool: &mut LiquidityPool,
        request: FlashSwapRequest,
    ) -> Result<FlashSwapResult> {
        request.validate()?;

        if pool.status != PoolStatus::Active {
            return Err(CoreError::Validation("Pool is not active".to_string()));
        }

        // Calculate minimum fee (0.3% of borrowed amount)
        let min_fee = request.borrow_amount * pool.fee_percentage;
        let min_repayment = request.borrow_amount + min_fee;

        if request.expected_repayment < min_repayment {
            return Err(CoreError::Validation(format!(
                "Repayment {} insufficient, minimum required: {}",
                request.expected_repayment, min_repayment
            )));
        }

        // In a real implementation, this would:
        // 1. Transfer borrowed tokens to caller
        // 2. Execute callback with callback_data
        // 3. Verify repayment was made
        // 4. Update pool reserves

        // For now, simulate successful flash swap
        let fee_amount = request.expected_repayment - request.borrow_amount;
        let profit = request.expected_repayment - min_repayment;

        // Update pool reserves based on which token was borrowed
        if request.borrow_token_a {
            pool.total_fees_a += fee_amount;
        } else {
            pool.total_fees_b += fee_amount;
        }

        pool.updated_at = Utc::now();

        Ok(FlashSwapResult {
            borrowed_amount: request.borrow_amount,
            repaid_amount: request.expected_repayment,
            fee_amount,
            profit,
        })
    }

    /// Update pool fee dynamically based on volatility
    pub fn update_dynamic_fee(
        pool: &mut LiquidityPool,
        volatility_tracker: &VolatilityTracker,
        base_fee: Decimal,
        max_fee: Decimal,
    ) {
        let new_fee = volatility_tracker.calculate_dynamic_fee(base_fee, max_fee);
        pool.fee_percentage = new_fee;
        pool.updated_at = Utc::now();
    }

    /// Validate swap against oracle price bounds
    pub fn validate_swap_with_oracle(
        pool: &LiquidityPool,
        oracle: &OraclePrice,
        input_amount: Decimal,
        input_is_a: bool,
    ) -> Result<()> {
        // Calculate resulting pool price after swap
        let output_amount = pool.calculate_output(input_amount, input_is_a);

        let (new_reserve_a, new_reserve_b) = if input_is_a {
            (
                pool.reserve_a + input_amount,
                pool.reserve_b - output_amount,
            )
        } else {
            (
                pool.reserve_a - output_amount,
                pool.reserve_b + input_amount,
            )
        };

        let new_price = new_reserve_b / new_reserve_a;

        if !oracle.is_price_within_bounds(new_price) {
            let deviation = oracle.calculate_deviation(new_price);
            return Err(CoreError::Validation(format!(
                "Price deviation {} exceeds oracle bounds (max: {})",
                deviation, oracle.max_deviation
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pool() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let creator = Uuid::new_v4();

        let request = CreatePoolRequest {
            token_a_id: token_a,
            token_b_id: token_b,
            initial_reserve_a: dec!(1000),
            initial_reserve_b: dec!(2000),
            fee_percentage: Some(dec!(0.003)),
        };

        let pool = AmmExecutor::create_pool(request, creator).unwrap();

        assert_eq!(pool.reserve_a, dec!(1000));
        assert_eq!(pool.reserve_b, dec!(2000));
        assert_eq!(pool.fee_percentage, dec!(0.003));

        // sqrt(1000 * 2000) = sqrt(2,000,000) ≈ 1414.213...
        assert!(pool.total_lp_tokens > dec!(1414) && pool.total_lp_tokens < dec!(1415));
    }

    #[test]
    fn test_swap_calculation() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let creator = Uuid::new_v4();

        let request = CreatePoolRequest {
            token_a_id: token_a,
            token_b_id: token_b,
            initial_reserve_a: dec!(1000),
            initial_reserve_b: dec!(2000),
            fee_percentage: Some(dec!(0.003)),
        };

        let mut pool = AmmExecutor::create_pool(request, creator).unwrap();

        // Swap 100 token A for token B
        let swap_request = SwapRequest {
            pool_id: pool.pool_id,
            input_amount: dec!(100),
            input_is_a: true,
            min_output: None,
            max_slippage: None,
        };

        let (output, fee) = AmmExecutor::swap(&mut pool, swap_request).unwrap();

        // Fee should be 0.3% of input
        assert_eq!(fee, dec!(0.3));

        // Output should be less than proportional due to slippage and fees
        assert!(output < dec!(200)); // Would be 200 if no slippage/fees
        assert!(output > dec!(0));

        // Verify reserves updated
        assert_eq!(pool.reserve_a, dec!(1100));
        assert!(pool.reserve_b < dec!(2000));
    }

    #[test]
    fn test_price_impact() {
        let pool = LiquidityPool {
            pool_id: Uuid::new_v4(),
            token_a_id: Uuid::new_v4(),
            token_b_id: Uuid::new_v4(),
            reserve_a: dec!(1000),
            reserve_b: dec!(2000),
            total_lp_tokens: dec!(1414.213562373095),
            fee_percentage: dec!(0.003),
            status: PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Small trade should have minimal impact
        let small_impact = pool.calculate_price_impact(dec!(10), true);
        assert!(small_impact < dec!(0.02)); // Less than 2%

        // Large trade should have significant impact
        let large_impact = pool.calculate_price_impact(dec!(500), true);
        assert!(large_impact > dec!(0.1)); // More than 10%
    }

    #[test]
    fn test_add_remove_liquidity() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let creator = Uuid::new_v4();

        let request = CreatePoolRequest {
            token_a_id: token_a,
            token_b_id: token_b,
            initial_reserve_a: dec!(1000),
            initial_reserve_b: dec!(2000),
            fee_percentage: Some(dec!(0.003)),
        };

        let mut pool = AmmExecutor::create_pool(request, creator).unwrap();
        let initial_lp = pool.total_lp_tokens;

        // Add liquidity
        let add_request = AddLiquidityRequest {
            pool_id: pool.pool_id,
            amount_a: dec!(500),
            amount_b: dec!(1000),
            min_lp_tokens: None,
        };

        let user = Uuid::new_v4();
        let (lp_tokens, actual_a, actual_b) =
            AmmExecutor::add_liquidity(&mut pool, add_request, user).unwrap();

        assert_eq!(actual_a, dec!(500));
        assert_eq!(actual_b, dec!(1000));
        assert_eq!(pool.reserve_a, dec!(1500));
        assert_eq!(pool.reserve_b, dec!(3000));

        // LP tokens should increase by 50% (since we added 50% more liquidity)
        let expected_lp = initial_lp * dec!(0.5);
        assert!((lp_tokens - expected_lp).abs() < dec!(0.01));

        // Remove liquidity
        let position = LpPosition {
            position_id: Uuid::new_v4(),
            pool_id: pool.pool_id,
            user_id: user,
            lp_tokens,
            initial_reserve_a: actual_a,
            initial_reserve_b: actual_b,
            pool_share: lp_tokens / pool.total_lp_tokens,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let remove_request = RemoveLiquidityRequest {
            pool_id: pool.pool_id,
            lp_tokens: lp_tokens / dec!(2), // Remove half
            min_amount_a: None,
            min_amount_b: None,
        };

        let (removed_a, removed_b) =
            AmmExecutor::remove_liquidity(&mut pool, remove_request, &position).unwrap();

        // Should get back approximately half of what was added
        assert!((removed_a - dec!(250)).abs() < dec!(1));
        assert!((removed_b - dec!(500)).abs() < dec!(1));
    }

    #[test]
    fn test_volatility_tracker() {
        let pool_id = Uuid::new_v4();
        let mut tracker = VolatilityTracker::new(pool_id, 10);

        // Add some price changes
        tracker.add_price_change(dec!(0.05)); // 5% up
        tracker.add_price_change(dec!(-0.03)); // 3% down
        tracker.add_price_change(dec!(0.02)); // 2% up

        assert!(tracker.current_volatility > dec!(0));
        assert_eq!(tracker.recent_changes.len(), 3);

        // Test dynamic fee calculation
        let base_fee = dec!(0.003);
        let max_fee = dec!(0.01);
        let dynamic_fee = tracker.calculate_dynamic_fee(base_fee, max_fee);

        assert!(dynamic_fee >= base_fee);
        assert!(dynamic_fee <= max_fee);
    }

    #[test]
    fn test_oracle_price_bounds() {
        let pool_id = Uuid::new_v4();
        let oracle = OraclePrice {
            pool_id,
            reference_price: dec!(2),  // 2.0 token_b per token_a
            max_deviation: dec!(0.05), // 5% deviation allowed
            updated_at: Utc::now(),
            oracle_source: "test_oracle".to_string(),
        };

        // Price within bounds
        assert!(oracle.is_price_within_bounds(dec!(2.0)));
        assert!(oracle.is_price_within_bounds(dec!(2.05)));
        assert!(oracle.is_price_within_bounds(dec!(1.95)));

        // Price outside bounds
        assert!(!oracle.is_price_within_bounds(dec!(2.15)));
        assert!(!oracle.is_price_within_bounds(dec!(1.85)));

        // Test deviation calculation
        let deviation = oracle.calculate_deviation(dec!(2.1));
        assert_eq!(deviation, dec!(0.05)); // 5% deviation
    }

    #[test]
    fn test_flash_swap() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let creator = Uuid::new_v4();

        let request = CreatePoolRequest {
            token_a_id: token_a,
            token_b_id: token_b,
            initial_reserve_a: dec!(10000),
            initial_reserve_b: dec!(20000),
            fee_percentage: Some(dec!(0.003)),
        };

        let mut pool = AmmExecutor::create_pool(request, creator).unwrap();

        // Flash swap: borrow 100 token A
        let flash_request = FlashSwapRequest {
            pool_id: pool.pool_id,
            borrow_amount: dec!(100),
            borrow_token_a: true,
            expected_repayment: dec!(100.5), // Repay with 0.5 profit
            callback_data: vec![],
        };

        let result = AmmExecutor::execute_flash_swap(&mut pool, flash_request).unwrap();

        assert_eq!(result.borrowed_amount, dec!(100));
        assert_eq!(result.repaid_amount, dec!(100.5));
        assert_eq!(result.fee_amount, dec!(0.5));
        assert!(result.profit >= dec!(0));
    }

    #[test]
    fn test_enhanced_routing() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let token_c = Uuid::new_v4();

        // Create pool A-B
        let pool_ab = LiquidityPool {
            pool_id: Uuid::new_v4(),
            token_a_id: token_a,
            token_b_id: token_b,
            reserve_a: dec!(1000),
            reserve_b: dec!(2000),
            total_lp_tokens: dec!(1414.213562373095),
            fee_percentage: dec!(0.003),
            status: PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Create pool B-C
        let pool_bc = LiquidityPool {
            pool_id: Uuid::new_v4(),
            token_a_id: token_b,
            token_b_id: token_c,
            reserve_a: dec!(2000),
            reserve_b: dec!(4000),
            total_lp_tokens: dec!(2828.427124746190),
            fee_percentage: dec!(0.003),
            status: PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let pools = vec![pool_ab.clone(), pool_bc.clone()];

        // Test routing from A to C (should use 2-hop route)
        let route = AmmExecutor::calculate_optimal_route(&pools, token_a, token_c, dec!(100));
        assert!(route.is_some());

        let (route_pools, output) = route.unwrap();
        assert_eq!(route_pools.len(), 2);
        assert_eq!(route_pools[0], pool_ab.pool_id);
        assert_eq!(route_pools[1], pool_bc.pool_id);
        assert!(output > dec!(0));
    }

    #[test]
    fn test_dynamic_fee_update() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();
        let creator = Uuid::new_v4();

        let request = CreatePoolRequest {
            token_a_id: token_a,
            token_b_id: token_b,
            initial_reserve_a: dec!(1000),
            initial_reserve_b: dec!(2000),
            fee_percentage: Some(dec!(0.003)),
        };

        let mut pool = AmmExecutor::create_pool(request, creator).unwrap();
        let initial_fee = pool.fee_percentage;

        // Create volatility tracker with multiple price changes to create volatility
        let mut tracker = VolatilityTracker::new(pool.pool_id, 10);
        tracker.add_price_change(dec!(0.08)); // 8% up
        tracker.add_price_change(dec!(-0.05)); // 5% down
        tracker.add_price_change(dec!(0.06)); // 6% up
        tracker.add_price_change(dec!(-0.04)); // 4% down

        // Update fee based on volatility
        AmmExecutor::update_dynamic_fee(&mut pool, &tracker, dec!(0.003), dec!(0.01));

        // Fee should have increased due to volatility
        assert!(pool.fee_percentage > initial_fee);
        assert!(pool.fee_percentage <= dec!(0.01));
    }
}
