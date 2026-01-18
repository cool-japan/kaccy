//! Uniswap V4 Style Hooks System
//!
//! This module implements a flexible hooks system that allows custom logic to be executed
//! at various points in the pool lifecycle (before/after swaps, before/after liquidity operations).
//!
//! Hooks enable powerful customization including:
//! - Limit orders
//! - Time-weighted average market makers (TWAMM)
//! - Volatility oracles
//! - Custom fee logic
//! - On-chain stop-loss/take-profit orders

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Hook execution context for swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapContext {
    /// Pool identifier
    pub pool_id: Uuid,
    /// User performing the swap
    pub user_id: Uuid,
    /// Token being sold
    pub token_in: Uuid,
    /// Amount being sold
    pub amount_in: Decimal,
    /// Token being bought
    pub token_out: Uuid,
    /// Expected amount out (can be modified by hooks)
    pub amount_out: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional context data
    pub data: HashMap<String, String>,
}

/// Hook execution context for liquidity operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityContext {
    /// Pool identifier
    pub pool_id: Uuid,
    /// Liquidity provider
    pub provider_id: Uuid,
    /// Token 0
    pub token0: Uuid,
    /// Amount of token 0
    pub amount0: Decimal,
    /// Token 1
    pub token1: Uuid,
    /// Amount of token 1
    pub amount1: Decimal,
    /// Is this an add or remove operation?
    pub is_add: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Additional context data
    pub data: HashMap<String, String>,
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    /// Should continue with the operation?
    pub should_continue: bool,
    /// Modified context (if any)
    pub modified_context: Option<String>,
    /// Fee adjustment (if applicable)
    pub fee_adjustment: Option<Decimal>,
    /// Custom data to pass to next hook
    pub custom_data: HashMap<String, String>,
}

impl HookResult {
    /// Create a success result
    pub fn success() -> Self {
        Self {
            should_continue: true,
            modified_context: None,
            fee_adjustment: None,
            custom_data: HashMap::new(),
        }
    }

    /// Create a failure result (stops execution)
    pub fn failure() -> Self {
        Self {
            should_continue: false,
            modified_context: None,
            fee_adjustment: None,
            custom_data: HashMap::new(),
        }
    }

    /// Create a result with fee adjustment
    pub fn with_fee_adjustment(mut self, fee: Decimal) -> Self {
        self.fee_adjustment = Some(fee);
        self
    }

    /// Add custom data
    pub fn with_data(mut self, key: String, value: String) -> Self {
        self.custom_data.insert(key, value);
        self
    }
}

/// Hook trait that all hooks must implement
pub trait Hook: Send + Sync {
    /// Hook name/identifier
    fn name(&self) -> &str;

    /// Execute before swap
    fn before_swap(&self, _context: &mut SwapContext) -> Result<HookResult> {
        Ok(HookResult::success())
    }

    /// Execute after swap
    fn after_swap(&self, _context: &SwapContext) -> Result<HookResult> {
        Ok(HookResult::success())
    }

    /// Execute before liquidity add/remove
    fn before_liquidity(&self, _context: &mut LiquidityContext) -> Result<HookResult> {
        Ok(HookResult::success())
    }

    /// Execute after liquidity add/remove
    fn after_liquidity(&self, _context: &LiquidityContext) -> Result<HookResult> {
        Ok(HookResult::success())
    }

    /// Dynamic fee calculation based on current state
    fn compute_dynamic_fee(&self, _context: &SwapContext) -> Result<Option<Decimal>> {
        Ok(None)
    }

    /// Gas limit for this hook (prevents infinite loops)
    fn gas_limit(&self) -> u64 {
        100_000
    }
}

/// Limit order hook implementation
pub struct LimitOrderHook {
    /// Active limit orders
    orders: HashMap<Uuid, LimitOrder>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct LimitOrder {
    user_id: Uuid,
    token_in: Uuid,
    amount_in: Decimal,
    token_out: Uuid,
    limit_price: Decimal,
    created_at: DateTime<Utc>,
}

impl LimitOrderHook {
    /// Create a new limit order hook
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
        }
    }

    /// Add a limit order
    pub fn add_order(
        &mut self,
        user_id: Uuid,
        token_in: Uuid,
        amount_in: Decimal,
        token_out: Uuid,
        limit_price: Decimal,
    ) -> Uuid {
        let order_id = Uuid::new_v4();
        let order = LimitOrder {
            user_id,
            token_in,
            amount_in,
            token_out,
            limit_price,
            created_at: Utc::now(),
        };
        self.orders.insert(order_id, order);
        order_id
    }
}

impl Hook for LimitOrderHook {
    fn name(&self) -> &str {
        "LimitOrderHook"
    }

    fn after_swap(&self, context: &SwapContext) -> Result<HookResult> {
        // Check if any limit orders can be executed at current price
        let current_price = context.amount_out / context.amount_in;

        let mut result = HookResult::success();

        for (order_id, order) in &self.orders {
            if order.token_in == context.token_in
                && order.token_out == context.token_out
                && current_price >= order.limit_price
            {
                // Order can be executed
                result
                    .custom_data
                    .insert(format!("executable_order_{}", order_id), "true".to_string());
            }
        }

        Ok(result)
    }
}

impl Default for LimitOrderHook {
    fn default() -> Self {
        Self::new()
    }
}

/// TWAMM (Time-Weighted Average Market Maker) hook
pub struct TwammHook {
    /// Active TWAMM orders
    orders: HashMap<Uuid, TwammOrder>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TwammOrder {
    user_id: Uuid,
    token_in: Uuid,
    total_amount: Decimal,
    token_out: Uuid,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    executed_amount: Decimal,
}

impl TwammHook {
    /// Create a new TWAMM hook
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
        }
    }

    /// Add a TWAMM order
    #[allow(clippy::too_many_arguments)]
    pub fn add_order(
        &mut self,
        user_id: Uuid,
        token_in: Uuid,
        total_amount: Decimal,
        token_out: Uuid,
        duration_seconds: i64,
    ) -> Uuid {
        let order_id = Uuid::new_v4();
        let start_time = Utc::now();
        let end_time = start_time + chrono::Duration::seconds(duration_seconds);

        let order = TwammOrder {
            user_id,
            token_in,
            total_amount,
            token_out,
            start_time,
            end_time,
            executed_amount: Decimal::ZERO,
        };

        self.orders.insert(order_id, order);
        order_id
    }

    /// Calculate how much should be executed for an order at current time
    fn calculate_executable_amount(&self, order: &TwammOrder) -> Decimal {
        let now = Utc::now();
        if now >= order.end_time {
            // Order period has ended, execute remaining amount
            return order.total_amount - order.executed_amount;
        }

        if now <= order.start_time {
            // Order hasn't started yet
            return Decimal::ZERO;
        }

        let total_duration = (order.end_time - order.start_time).num_seconds();
        let elapsed_duration = (now - order.start_time).num_seconds();

        let progress = Decimal::from(elapsed_duration) / Decimal::from(total_duration);
        let should_have_executed = order.total_amount * progress;

        should_have_executed - order.executed_amount
    }
}

impl Hook for TwammHook {
    fn name(&self) -> &str {
        "TwammHook"
    }

    fn before_swap(&self, context: &mut SwapContext) -> Result<HookResult> {
        let mut result = HookResult::success();
        let mut total_twamm_amount = Decimal::ZERO;

        // Aggregate all executable TWAMM orders
        for (order_id, order) in &self.orders {
            if order.token_in == context.token_in && order.token_out == context.token_out {
                let executable = self.calculate_executable_amount(order);
                if executable > Decimal::ZERO {
                    total_twamm_amount += executable;
                    result
                        .custom_data
                        .insert(format!("twamm_order_{}", order_id), executable.to_string());
                }
            }
        }

        if total_twamm_amount > Decimal::ZERO {
            result.custom_data.insert(
                "total_twamm_amount".to_string(),
                total_twamm_amount.to_string(),
            );
        }

        Ok(result)
    }
}

impl Default for TwammHook {
    fn default() -> Self {
        Self::new()
    }
}

/// Volatility oracle hook that adjusts fees based on volatility
pub struct VolatilityOracleHook {
    /// Base fee percentage
    base_fee: Decimal,
    /// Recent price observations
    price_observations: Vec<PriceObservation>,
    /// Maximum observations to keep
    max_observations: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PriceObservation {
    price: Decimal,
    timestamp: DateTime<Utc>,
}

impl VolatilityOracleHook {
    /// Create a new volatility oracle hook
    pub fn new(base_fee: Decimal, max_observations: usize) -> Self {
        Self {
            base_fee,
            price_observations: Vec::new(),
            max_observations,
        }
    }

    /// Calculate volatility from recent observations
    fn calculate_volatility(&self) -> Decimal {
        if self.price_observations.len() < 2 {
            return Decimal::ZERO;
        }

        let prices: Vec<Decimal> = self.price_observations.iter().map(|o| o.price).collect();

        // Calculate returns
        let mut returns = Vec::new();
        for i in 1..prices.len() {
            let ret = (prices[i] - prices[i - 1]) / prices[i - 1];
            returns.push(ret);
        }

        // Calculate standard deviation of returns
        let mean = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        let variance: Decimal = returns
            .iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        // Return standard deviation as volatility measure
        // In production, would use proper sqrt
        variance
    }

    /// Add price observation
    #[allow(dead_code)]
    fn add_observation(&mut self, price: Decimal) {
        self.price_observations.push(PriceObservation {
            price,
            timestamp: Utc::now(),
        });

        // Keep only recent observations
        if self.price_observations.len() > self.max_observations {
            self.price_observations.remove(0);
        }
    }
}

impl Hook for VolatilityOracleHook {
    fn name(&self) -> &str {
        "VolatilityOracleHook"
    }

    fn compute_dynamic_fee(&self, _context: &SwapContext) -> Result<Option<Decimal>> {
        let volatility = self.calculate_volatility();

        // Adjust fee based on volatility: higher volatility = higher fee
        // Fee = base_fee * (1 + volatility_multiplier)
        let volatility_multiplier = volatility * Decimal::from(10);
        let adjusted_fee = self.base_fee * (Decimal::ONE + volatility_multiplier);

        // Cap at 5x base fee
        let capped_fee = adjusted_fee.min(self.base_fee * Decimal::from(5));

        Ok(Some(capped_fee))
    }

    fn after_swap(&self, context: &SwapContext) -> Result<HookResult> {
        // Record price for volatility calculation
        let price = context.amount_out / context.amount_in;

        // Note: In a real implementation, we'd need mutable access
        // This is a simplified version for demonstration
        let result =
            HookResult::success().with_data("current_price".to_string(), price.to_string());

        Ok(result)
    }
}

/// Hook manager that coordinates multiple hooks
pub struct HookManager {
    /// Registered hooks for each pool
    pool_hooks: HashMap<Uuid, Vec<Box<dyn Hook>>>,
    /// Hook permissions (which operations are allowed)
    hook_permissions: HashMap<Uuid, HookPermissions>,
}

#[derive(Debug, Clone)]
pub struct HookPermissions {
    pub allow_before_swap: bool,
    pub allow_after_swap: bool,
    pub allow_before_liquidity: bool,
    pub allow_after_liquidity: bool,
    pub allow_dynamic_fee: bool,
}

impl Default for HookPermissions {
    fn default() -> Self {
        Self {
            allow_before_swap: true,
            allow_after_swap: true,
            allow_before_liquidity: true,
            allow_after_liquidity: true,
            allow_dynamic_fee: true,
        }
    }
}

impl HookManager {
    /// Create a new hook manager
    pub fn new() -> Self {
        Self {
            pool_hooks: HashMap::new(),
            hook_permissions: HashMap::new(),
        }
    }

    /// Register a hook for a pool
    pub fn register_hook(
        &mut self,
        pool_id: Uuid,
        hook: Box<dyn Hook>,
        permissions: HookPermissions,
    ) -> Result<()> {
        self.pool_hooks.entry(pool_id).or_default().push(hook);

        self.hook_permissions.insert(pool_id, permissions);
        Ok(())
    }

    /// Execute before swap hooks
    pub fn execute_before_swap(&self, pool_id: Uuid, context: &mut SwapContext) -> Result<()> {
        let permissions = self
            .hook_permissions
            .get(&pool_id)
            .cloned()
            .unwrap_or_default();

        if !permissions.allow_before_swap {
            return Ok(());
        }

        if let Some(hooks) = self.pool_hooks.get(&pool_id) {
            for hook in hooks {
                let result = hook.before_swap(context)?;
                if !result.should_continue {
                    return Err(CoreError::Validation(format!(
                        "Hook {} rejected swap",
                        hook.name()
                    )));
                }

                // Apply fee adjustment if provided
                if let Some(fee) = result.fee_adjustment {
                    context
                        .data
                        .insert("hook_fee_adjustment".to_string(), fee.to_string());
                }
            }
        }

        Ok(())
    }

    /// Execute after swap hooks
    pub fn execute_after_swap(&self, pool_id: Uuid, context: &SwapContext) -> Result<()> {
        let permissions = self
            .hook_permissions
            .get(&pool_id)
            .cloned()
            .unwrap_or_default();

        if !permissions.allow_after_swap {
            return Ok(());
        }

        if let Some(hooks) = self.pool_hooks.get(&pool_id) {
            for hook in hooks {
                let result = hook.after_swap(context)?;
                if !result.should_continue {
                    return Err(CoreError::Validation(format!(
                        "Hook {} failed in after_swap",
                        hook.name()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Execute before liquidity hooks
    pub fn execute_before_liquidity(
        &self,
        pool_id: Uuid,
        context: &mut LiquidityContext,
    ) -> Result<()> {
        let permissions = self
            .hook_permissions
            .get(&pool_id)
            .cloned()
            .unwrap_or_default();

        if !permissions.allow_before_liquidity {
            return Ok(());
        }

        if let Some(hooks) = self.pool_hooks.get(&pool_id) {
            for hook in hooks {
                let result = hook.before_liquidity(context)?;
                if !result.should_continue {
                    return Err(CoreError::Validation(format!(
                        "Hook {} rejected liquidity operation",
                        hook.name()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Execute after liquidity hooks
    pub fn execute_after_liquidity(&self, pool_id: Uuid, context: &LiquidityContext) -> Result<()> {
        let permissions = self
            .hook_permissions
            .get(&pool_id)
            .cloned()
            .unwrap_or_default();

        if !permissions.allow_after_liquidity {
            return Ok(());
        }

        if let Some(hooks) = self.pool_hooks.get(&pool_id) {
            for hook in hooks {
                let result = hook.after_liquidity(context)?;
                if !result.should_continue {
                    return Err(CoreError::Validation(format!(
                        "Hook {} failed in after_liquidity",
                        hook.name()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Compute dynamic fee from hooks
    pub fn compute_dynamic_fee(
        &self,
        pool_id: Uuid,
        context: &SwapContext,
    ) -> Result<Option<Decimal>> {
        let permissions = self
            .hook_permissions
            .get(&pool_id)
            .cloned()
            .unwrap_or_default();

        if !permissions.allow_dynamic_fee {
            return Ok(None);
        }

        if let Some(hooks) = self.pool_hooks.get(&pool_id) {
            for hook in hooks {
                if let Some(fee) = hook.compute_dynamic_fee(context)? {
                    return Ok(Some(fee));
                }
            }
        }

        Ok(None)
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_limit_order_hook() {
        let mut hook = LimitOrderHook::new();

        let user_id = Uuid::new_v4();
        let token_in = Uuid::new_v4();
        let token_out = Uuid::new_v4();

        // Add a limit order at price 1.5
        hook.add_order(user_id, token_in, dec!(100), token_out, dec!(1.5));

        // Create swap context with price 1.6 (above limit)
        let context = SwapContext {
            pool_id: Uuid::new_v4(),
            user_id,
            token_in,
            amount_in: dec!(100),
            token_out,
            amount_out: dec!(160), // Price = 1.6
            timestamp: Utc::now(),
            data: HashMap::new(),
        };

        let result = hook.after_swap(&context).unwrap();
        assert!(result.should_continue);
        assert!(!result.custom_data.is_empty());
    }

    #[test]
    fn test_twamm_hook() {
        let mut hook = TwammHook::new();

        let user_id = Uuid::new_v4();
        let token_in = Uuid::new_v4();
        let token_out = Uuid::new_v4();

        // Add a TWAMM order for 100 tokens over 3600 seconds
        hook.add_order(user_id, token_in, dec!(100), token_out, 3600);

        let mut context = SwapContext {
            pool_id: Uuid::new_v4(),
            user_id,
            token_in,
            amount_in: dec!(10),
            token_out,
            amount_out: dec!(15),
            timestamp: Utc::now(),
            data: HashMap::new(),
        };

        let result = hook.before_swap(&mut context).unwrap();
        assert!(result.should_continue);
    }

    #[test]
    fn test_volatility_oracle_hook() {
        let hook = VolatilityOracleHook::new(dec!(0.003), 100);

        let context = SwapContext {
            pool_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_in: Uuid::new_v4(),
            amount_in: dec!(100),
            token_out: Uuid::new_v4(),
            amount_out: dec!(150),
            timestamp: Utc::now(),
            data: HashMap::new(),
        };

        // With no observations, should return base fee
        let fee = hook.compute_dynamic_fee(&context).unwrap();
        assert!(fee.is_some());
    }

    #[test]
    fn test_hook_manager() {
        let mut manager = HookManager::new();
        let pool_id = Uuid::new_v4();

        let limit_hook = Box::new(LimitOrderHook::new());
        manager
            .register_hook(pool_id, limit_hook, HookPermissions::default())
            .unwrap();

        let mut context = SwapContext {
            pool_id,
            user_id: Uuid::new_v4(),
            token_in: Uuid::new_v4(),
            amount_in: dec!(100),
            token_out: Uuid::new_v4(),
            amount_out: dec!(150),
            timestamp: Utc::now(),
            data: HashMap::new(),
        };

        // Should execute without errors
        manager.execute_before_swap(pool_id, &mut context).unwrap();
        manager.execute_after_swap(pool_id, &context).unwrap();
    }
}
