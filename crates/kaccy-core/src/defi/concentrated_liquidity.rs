//! Concentrated liquidity (Uniswap V3 style)
//!
//! This module implements concentrated liquidity pools where liquidity providers
//! can specify price ranges for their liquidity, improving capital efficiency.
//!
//! Key features:
//! - Price ranges (ticks) for liquidity provision
//! - Virtual reserves within active ranges
//! - Fee accumulation per tick
//! - Position NFT representation
//! - Multiple positions per LP
//! - Improved capital efficiency over constant product AMMs

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::error::{CoreError, Result};

/// Tick spacing (minimum price movement)
/// Lower spacing = more granularity but higher gas costs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TickSpacing {
    /// 0.01% spacing (most granular)
    VeryFine = 1,
    /// 0.05% spacing
    Fine = 5,
    /// 0.1% spacing
    Medium = 10,
    /// 0.3% spacing
    Coarse = 30,
    /// 1% spacing (least granular)
    VeryCoarse = 100,
}

impl TickSpacing {
    /// Get the spacing value
    pub fn value(&self) -> i32 {
        *self as i32
    }
}

/// Fee tier for the pool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeeTier {
    /// 0.01% fee
    VeryLow = 1,
    /// 0.05% fee (stablecoins)
    Low = 5,
    /// 0.3% fee (standard)
    Medium = 30,
    /// 1% fee (exotic pairs)
    High = 100,
}

impl FeeTier {
    /// Get fee as decimal (e.g., 0.003 for 0.3%)
    pub fn as_decimal(&self) -> Decimal {
        match self {
            FeeTier::VeryLow => dec!(0.0001),
            FeeTier::Low => dec!(0.0005),
            FeeTier::Medium => dec!(0.003),
            FeeTier::High => dec!(0.01),
        }
    }
}

/// Liquidity position in a concentrated liquidity pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentratedPosition {
    /// Position ID (NFT token ID)
    pub id: String,
    /// Owner address
    pub owner: String,
    /// Pool ID
    pub pool_id: String,
    /// Lower tick (price bound)
    pub tick_lower: i32,
    /// Upper tick (price bound)
    pub tick_upper: i32,
    /// Liquidity amount
    pub liquidity: Decimal,
    /// Amount of token0 deposited
    pub amount0: Decimal,
    /// Amount of token1 deposited
    pub amount1: Decimal,
    /// Fees earned in token0
    pub fees_earned0: Decimal,
    /// Fees earned in token1
    pub fees_earned1: Decimal,
    /// Fee growth inside last (for calculating earned fees)
    pub fee_growth_inside0_last: Decimal,
    /// Fee growth inside last for token1
    pub fee_growth_inside1_last: Decimal,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl ConcentratedPosition {
    /// Create a new concentrated position
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        owner: String,
        pool_id: String,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: Decimal,
        amount0: Decimal,
        amount1: Decimal,
    ) -> Result<Self> {
        if tick_lower >= tick_upper {
            return Err(CoreError::Validation(
                "Lower tick must be less than upper tick".to_string(),
            ));
        }
        if liquidity <= dec!(0) {
            return Err(CoreError::Validation(
                "Liquidity must be positive".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id,
            owner,
            pool_id,
            tick_lower,
            tick_upper,
            liquidity,
            amount0,
            amount1,
            fees_earned0: dec!(0),
            fees_earned1: dec!(0),
            fee_growth_inside0_last: dec!(0),
            fee_growth_inside1_last: dec!(0),
            created_at: now,
            updated_at: now,
        })
    }

    /// Check if position is in range at current tick
    pub fn is_in_range(&self, current_tick: i32) -> bool {
        current_tick >= self.tick_lower && current_tick < self.tick_upper
    }

    /// Update fees earned
    pub fn update_fees(&mut self, fee_growth_inside0: Decimal, fee_growth_inside1: Decimal) {
        // Calculate fees earned since last update
        let fee_growth_delta0 = fee_growth_inside0 - self.fee_growth_inside0_last;
        let fee_growth_delta1 = fee_growth_inside1 - self.fee_growth_inside1_last;

        self.fees_earned0 += self.liquidity * fee_growth_delta0;
        self.fees_earned1 += self.liquidity * fee_growth_delta1;

        self.fee_growth_inside0_last = fee_growth_inside0;
        self.fee_growth_inside1_last = fee_growth_inside1;
        self.updated_at = Utc::now();
    }

    /// Collect fees
    pub fn collect_fees(&mut self) -> (Decimal, Decimal) {
        let fees0 = self.fees_earned0;
        let fees1 = self.fees_earned1;

        self.fees_earned0 = dec!(0);
        self.fees_earned1 = dec!(0);
        self.updated_at = Utc::now();

        (fees0, fees1)
    }
}

/// Tick information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tick {
    /// Tick index
    pub index: i32,
    /// Liquidity gross (total liquidity at this tick)
    pub liquidity_gross: Decimal,
    /// Liquidity net (liquidity added/removed when crossing)
    pub liquidity_net: Decimal,
    /// Fee growth outside in token0
    pub fee_growth_outside0: Decimal,
    /// Fee growth outside in token1
    pub fee_growth_outside1: Decimal,
    /// Initialized flag
    pub initialized: bool,
}

impl Tick {
    /// Create a new uninitialized tick
    pub fn new(index: i32) -> Self {
        Self {
            index,
            liquidity_gross: dec!(0),
            liquidity_net: dec!(0),
            fee_growth_outside0: dec!(0),
            fee_growth_outside1: dec!(0),
            initialized: false,
        }
    }

    /// Update tick with liquidity delta
    pub fn update(&mut self, liquidity_delta: Decimal, upper: bool) {
        self.liquidity_gross += liquidity_delta.abs();

        if upper {
            self.liquidity_net -= liquidity_delta;
        } else {
            self.liquidity_net += liquidity_delta;
        }

        self.initialized = self.liquidity_gross != dec!(0);
    }
}

/// Concentrated liquidity pool (Uniswap V3 style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentratedLiquidityPool {
    /// Pool ID
    pub id: String,
    /// Token0 symbol
    pub token0: String,
    /// Token1 symbol
    pub token1: String,
    /// Fee tier
    pub fee_tier: FeeTier,
    /// Tick spacing
    pub tick_spacing: TickSpacing,
    /// Current tick
    pub current_tick: i32,
    /// Current sqrt price (Q64.64 format simulation)
    pub sqrt_price: Decimal,
    /// Active liquidity at current price
    pub liquidity: Decimal,
    /// Reserve of token0
    pub reserve0: Decimal,
    /// Reserve of token1
    pub reserve1: Decimal,
    /// Global fee growth for token0
    pub fee_growth_global0: Decimal,
    /// Global fee growth for token1
    pub fee_growth_global1: Decimal,
    /// Tick data
    pub ticks: BTreeMap<i32, Tick>,
    /// Total value locked
    pub tvl: Decimal,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl ConcentratedLiquidityPool {
    /// Create a new concentrated liquidity pool
    pub fn new(
        id: String,
        token0: String,
        token1: String,
        fee_tier: FeeTier,
        tick_spacing: TickSpacing,
        initial_sqrt_price: Decimal,
    ) -> Self {
        let current_tick = Self::sqrt_price_to_tick(initial_sqrt_price);
        let now = Utc::now();

        Self {
            id,
            token0,
            token1,
            fee_tier,
            tick_spacing,
            current_tick,
            sqrt_price: initial_sqrt_price,
            liquidity: dec!(0),
            reserve0: dec!(0),
            reserve1: dec!(0),
            fee_growth_global0: dec!(0),
            fee_growth_global1: dec!(0),
            ticks: BTreeMap::new(),
            tvl: dec!(0),
            created_at: now,
            updated_at: now,
        }
    }

    /// Convert sqrt price to tick (simplified)
    fn sqrt_price_to_tick(sqrt_price: Decimal) -> i32 {
        // Simplified: tick = log1.0001(price)
        // In production, would use proper fixed-point math
        let price = sqrt_price * sqrt_price;
        if price <= dec!(0) {
            return 0;
        }

        let log_price = (price.to_string().parse::<f64>().unwrap_or(1.0)).ln();
        let log_base = 1.0001_f64.ln();
        (log_price / log_base) as i32
    }

    /// Convert tick to sqrt price (simplified)
    fn tick_to_sqrt_price(tick: i32) -> Decimal {
        // price = 1.0001^tick
        // sqrt_price = 1.0001^(tick/2)
        let price = 1.0001_f64.powi(tick);
        Decimal::from_f64_retain(price.sqrt()).unwrap_or(dec!(1))
    }

    /// Add liquidity to a price range
    pub fn add_liquidity(
        &mut self,
        tick_lower: i32,
        tick_upper: i32,
        amount0_desired: Decimal,
        amount1_desired: Decimal,
    ) -> Result<(Decimal, Decimal, Decimal)> {
        // Validate ticks are aligned to spacing
        if tick_lower % self.tick_spacing.value() != 0
            || tick_upper % self.tick_spacing.value() != 0
        {
            return Err(CoreError::Validation(
                "Ticks must be aligned to tick spacing".to_string(),
            ));
        }

        // Calculate liquidity from amounts
        let liquidity =
            self.calculate_liquidity(tick_lower, tick_upper, amount0_desired, amount1_desired)?;

        // Update ticks
        self.update_tick(tick_lower, liquidity, false)?;
        self.update_tick(tick_upper, liquidity, true)?;

        // If position is in range, update active liquidity
        if self.current_tick >= tick_lower && self.current_tick < tick_upper {
            self.liquidity += liquidity;
        }

        // Calculate actual amounts
        let (amount0, amount1) = self.calculate_amounts(tick_lower, tick_upper, liquidity)?;

        self.reserve0 += amount0;
        self.reserve1 += amount1;
        self.tvl = self.reserve0 + self.reserve1; // Simplified TVL

        self.updated_at = Utc::now();

        Ok((liquidity, amount0, amount1))
    }

    /// Calculate liquidity from token amounts
    fn calculate_liquidity(
        &self,
        tick_lower: i32,
        tick_upper: i32,
        amount0: Decimal,
        amount1: Decimal,
    ) -> Result<Decimal> {
        let sqrt_price_lower = Self::tick_to_sqrt_price(tick_lower);
        let sqrt_price_upper = Self::tick_to_sqrt_price(tick_upper);

        // Simplified liquidity calculation
        // L = amount1 / (sqrt_upper - sqrt_lower)
        // or L = amount0 * sqrt_upper * sqrt_lower / (sqrt_upper - sqrt_lower)

        let liquidity_from_amount1 = if sqrt_price_upper > sqrt_price_lower {
            amount1 / (sqrt_price_upper - sqrt_price_lower)
        } else {
            dec!(0)
        };

        let liquidity_from_amount0 = if sqrt_price_upper > dec!(0) && sqrt_price_lower > dec!(0) {
            amount0 * sqrt_price_upper * sqrt_price_lower / (sqrt_price_upper - sqrt_price_lower)
        } else {
            dec!(0)
        };

        // Take the minimum to ensure both amounts are satisfied
        Ok(liquidity_from_amount0
            .min(liquidity_from_amount1)
            .max(dec!(1)))
    }

    /// Calculate token amounts from liquidity
    fn calculate_amounts(
        &self,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: Decimal,
    ) -> Result<(Decimal, Decimal)> {
        let sqrt_price_lower = Self::tick_to_sqrt_price(tick_lower);
        let sqrt_price_upper = Self::tick_to_sqrt_price(tick_upper);
        let sqrt_price_current = self.sqrt_price;

        let amount0;
        let amount1;

        if self.current_tick < tick_lower {
            // Position is entirely in token0
            amount0 = liquidity * (sqrt_price_upper - sqrt_price_lower)
                / (sqrt_price_upper * sqrt_price_lower);
            amount1 = dec!(0);
        } else if self.current_tick >= tick_upper {
            // Position is entirely in token1
            amount0 = dec!(0);
            amount1 = liquidity * (sqrt_price_upper - sqrt_price_lower);
        } else {
            // Position is active (mixed)
            amount0 = liquidity * (sqrt_price_upper - sqrt_price_current)
                / (sqrt_price_upper * sqrt_price_current);
            amount1 = liquidity * (sqrt_price_current - sqrt_price_lower);
        }

        Ok((amount0.max(dec!(0)), amount1.max(dec!(0))))
    }

    /// Update tick with liquidity delta
    fn update_tick(&mut self, tick: i32, liquidity_delta: Decimal, upper: bool) -> Result<()> {
        let tick_data = self.ticks.entry(tick).or_insert_with(|| Tick::new(tick));
        tick_data.update(liquidity_delta, upper);
        Ok(())
    }

    /// Swap tokens (simplified)
    pub fn swap(&mut self, zero_for_one: bool, amount_in: Decimal) -> Result<Decimal> {
        if amount_in <= dec!(0) {
            return Err(CoreError::Validation(
                "Amount in must be positive".to_string(),
            ));
        }

        // Calculate fee
        let fee_amount = amount_in * self.fee_tier.as_decimal();
        let amount_in_less_fee = amount_in - fee_amount;

        // Simplified constant product swap within active liquidity
        let amount_out = if zero_for_one {
            // Swapping token0 for token1
            if self.reserve0 == dec!(0) || self.liquidity == dec!(0) {
                return Err(CoreError::Validation("Insufficient liquidity".to_string()));
            }

            let amount_out =
                (self.reserve1 * amount_in_less_fee) / (self.reserve0 + amount_in_less_fee);

            self.reserve0 += amount_in;
            self.reserve1 -= amount_out;

            // Update fee growth
            if self.liquidity > dec!(0) {
                self.fee_growth_global0 += fee_amount / self.liquidity;
            }

            amount_out
        } else {
            // Swapping token1 for token0
            if self.reserve1 == dec!(0) || self.liquidity == dec!(0) {
                return Err(CoreError::Validation("Insufficient liquidity".to_string()));
            }

            let amount_out =
                (self.reserve0 * amount_in_less_fee) / (self.reserve1 + amount_in_less_fee);

            self.reserve1 += amount_in;
            self.reserve0 -= amount_out;

            // Update fee growth
            if self.liquidity > dec!(0) {
                self.fee_growth_global1 += fee_amount / self.liquidity;
            }

            amount_out
        };

        // Update price and tick (simplified)
        if self.reserve0 > dec!(0) && self.reserve1 > dec!(0) {
            let new_price = self.reserve1 / self.reserve0;
            self.sqrt_price = Decimal::from_f64_retain(
                new_price.to_string().parse::<f64>().unwrap_or(1.0).sqrt(),
            )
            .unwrap_or(dec!(1));
            self.current_tick = Self::sqrt_price_to_tick(self.sqrt_price);
        }

        self.updated_at = Utc::now();

        Ok(amount_out)
    }

    /// Get price at current tick
    pub fn current_price(&self) -> Decimal {
        self.sqrt_price * self.sqrt_price
    }

    /// Calculate fee growth inside a range
    pub fn calculate_fee_growth_inside(
        &self,
        tick_lower: i32,
        tick_upper: i32,
    ) -> (Decimal, Decimal) {
        let lower_tick = self.ticks.get(&tick_lower);
        let upper_tick = self.ticks.get(&tick_upper);

        let fee_growth_below0 = lower_tick.map(|t| t.fee_growth_outside0).unwrap_or(dec!(0));
        let fee_growth_below1 = lower_tick.map(|t| t.fee_growth_outside1).unwrap_or(dec!(0));

        let fee_growth_above0 = upper_tick.map(|t| t.fee_growth_outside0).unwrap_or(dec!(0));
        let fee_growth_above1 = upper_tick.map(|t| t.fee_growth_outside1).unwrap_or(dec!(0));

        let fee_growth_inside0 = self.fee_growth_global0 - fee_growth_below0 - fee_growth_above0;
        let fee_growth_inside1 = self.fee_growth_global1 - fee_growth_below1 - fee_growth_above1;

        (fee_growth_inside0, fee_growth_inside1)
    }
}

/// Manager for concentrated liquidity positions
#[derive(Debug)]
pub struct ConcentratedLiquidityManager {
    /// Pools by ID
    pools: HashMap<String, ConcentratedLiquidityPool>,
    /// Positions by ID
    positions: HashMap<String, ConcentratedPosition>,
    /// Positions by owner
    positions_by_owner: HashMap<String, Vec<String>>,
    /// Next position ID
    next_position_id: u64,
}

impl ConcentratedLiquidityManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            positions: HashMap::new(),
            positions_by_owner: HashMap::new(),
            next_position_id: 1,
        }
    }

    /// Create a new pool
    pub fn create_pool(
        &mut self,
        id: String,
        token0: String,
        token1: String,
        fee_tier: FeeTier,
        tick_spacing: TickSpacing,
        initial_sqrt_price: Decimal,
    ) -> Result<()> {
        if self.pools.contains_key(&id) {
            return Err(CoreError::Validation("Pool already exists".to_string()));
        }

        let pool = ConcentratedLiquidityPool::new(
            id.clone(),
            token0,
            token1,
            fee_tier,
            tick_spacing,
            initial_sqrt_price,
        );

        self.pools.insert(id, pool);
        Ok(())
    }

    /// Mint a new position
    #[allow(clippy::too_many_arguments)]
    pub fn mint_position(
        &mut self,
        owner: String,
        pool_id: String,
        tick_lower: i32,
        tick_upper: i32,
        amount0_desired: Decimal,
        amount1_desired: Decimal,
    ) -> Result<String> {
        let pool = self
            .pools
            .get_mut(&pool_id)
            .ok_or_else(|| CoreError::Validation("Pool not found".to_string()))?;

        let (liquidity, amount0, amount1) =
            pool.add_liquidity(tick_lower, tick_upper, amount0_desired, amount1_desired)?;

        let position_id = format!("pos-{}", self.next_position_id);
        self.next_position_id += 1;

        let position = ConcentratedPosition::new(
            position_id.clone(),
            owner.clone(),
            pool_id,
            tick_lower,
            tick_upper,
            liquidity,
            amount0,
            amount1,
        )?;

        self.positions.insert(position_id.clone(), position);
        self.positions_by_owner
            .entry(owner)
            .or_default()
            .push(position_id.clone());

        Ok(position_id)
    }

    /// Get positions by owner
    pub fn get_positions_by_owner(&self, owner: &str) -> Vec<&ConcentratedPosition> {
        self.positions_by_owner
            .get(owner)
            .map(|ids| ids.iter().filter_map(|id| self.positions.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get pool
    pub fn get_pool(&self, pool_id: &str) -> Option<&ConcentratedLiquidityPool> {
        self.pools.get(pool_id)
    }

    /// Swap in a pool
    pub fn swap(
        &mut self,
        pool_id: &str,
        zero_for_one: bool,
        amount_in: Decimal,
    ) -> Result<Decimal> {
        let pool = self
            .pools
            .get_mut(pool_id)
            .ok_or_else(|| CoreError::Validation("Pool not found".to_string()))?;

        pool.swap(zero_for_one, amount_in)
    }
}

impl Default for ConcentratedLiquidityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concentrated_position_creation() {
        let position = ConcentratedPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "pool1".to_string(),
            -100,
            100,
            dec!(1000),
            dec!(100),
            dec!(100),
        )
        .unwrap();

        assert_eq!(position.liquidity, dec!(1000));
        assert_eq!(position.tick_lower, -100);
        assert_eq!(position.tick_upper, 100);
    }

    #[test]
    fn test_position_in_range() {
        let position = ConcentratedPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "pool1".to_string(),
            -100,
            100,
            dec!(1000),
            dec!(100),
            dec!(100),
        )
        .unwrap();

        assert!(position.is_in_range(0));
        assert!(position.is_in_range(-50));
        assert!(position.is_in_range(50));
        assert!(!position.is_in_range(-101));
        assert!(!position.is_in_range(100));
    }

    #[test]
    fn test_pool_creation() {
        let pool = ConcentratedLiquidityPool::new(
            "pool1".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            FeeTier::Low,
            TickSpacing::Fine,
            dec!(1),
        );

        assert_eq!(pool.token0, "USDC");
        assert_eq!(pool.token1, "USDT");
        assert_eq!(pool.liquidity, dec!(0));
    }

    #[test]
    fn test_manager_create_pool() {
        let mut manager = ConcentratedLiquidityManager::new();

        manager
            .create_pool(
                "pool1".to_string(),
                "USDC".to_string(),
                "USDT".to_string(),
                FeeTier::Low,
                TickSpacing::Fine,
                dec!(1),
            )
            .unwrap();

        assert!(manager.get_pool("pool1").is_some());
    }

    #[test]
    fn test_mint_position() {
        let mut manager = ConcentratedLiquidityManager::new();

        manager
            .create_pool(
                "pool1".to_string(),
                "USDC".to_string(),
                "USDT".to_string(),
                FeeTier::Low,
                TickSpacing::Medium,
                dec!(1),
            )
            .unwrap();

        let position_id = manager
            .mint_position(
                "user1".to_string(),
                "pool1".to_string(),
                -100,
                100,
                dec!(1000),
                dec!(1000),
            )
            .unwrap();

        assert!(position_id.starts_with("pos-"));

        let positions = manager.get_positions_by_owner("user1");
        assert_eq!(positions.len(), 1);
    }
}
