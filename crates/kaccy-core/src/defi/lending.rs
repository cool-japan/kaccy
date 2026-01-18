//! Lending and borrowing system for DeFi primitives
//!
//! This module provides:
//! - Collateralized lending pools
//! - Interest rate models (linear and exponential)
//! - Liquidation engine with health factor monitoring
//! - Borrow/repay operations with automatic interest accrual

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Interest rate model type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InterestRateModel {
    /// Linear interest rate: base_rate + utilization * slope
    Linear { base_rate: Decimal, slope: Decimal },
    /// Exponential interest rate: base_rate * e^(utilization * multiplier)
    Exponential {
        base_rate: Decimal,
        multiplier: Decimal,
    },
    /// Kinked rate model (optimal utilization with kink)
    Kinked {
        base_rate: Decimal,
        slope1: Decimal,
        slope2: Decimal,
        optimal_utilization: Decimal,
    },
}

impl InterestRateModel {
    /// Calculate interest rate based on utilization
    pub fn calculate_rate(&self, utilization: Decimal) -> Decimal {
        match self {
            Self::Linear { base_rate, slope } => base_rate + (utilization * slope),
            Self::Exponential {
                base_rate,
                multiplier,
            } => {
                // Approximate e^x using Taylor series for small x
                let x = utilization * multiplier;
                let exp_x = dec!(1) + x + (x * x / dec!(2)) + (x * x * x / dec!(6));
                base_rate * exp_x
            }
            Self::Kinked {
                base_rate,
                slope1,
                slope2,
                optimal_utilization,
            } => {
                if utilization <= *optimal_utilization {
                    base_rate + (utilization * slope1)
                } else {
                    let base = base_rate + (optimal_utilization * slope1);
                    base + ((utilization - optimal_utilization) * slope2)
                }
            }
        }
    }
}

/// Lending pool for a specific asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendingPool {
    pub pool_id: Uuid,
    pub asset_id: Uuid,
    pub total_deposits: Decimal,
    pub total_borrows: Decimal,
    pub interest_rate_model: InterestRateModel,
    /// Collateral factor (0.0 to 1.0) - how much can be borrowed against deposits
    pub collateral_factor: Decimal,
    /// Liquidation threshold (0.0 to 1.0)
    pub liquidation_threshold: Decimal,
    /// Liquidation penalty (e.g., 0.05 = 5%)
    pub liquidation_penalty: Decimal,
    /// Reserve factor (portion of interest going to reserves)
    pub reserve_factor: Decimal,
    pub created_at: i64,
    pub last_update: i64,
}

impl LendingPool {
    pub fn new(
        asset_id: Uuid,
        interest_rate_model: InterestRateModel,
        collateral_factor: Decimal,
        liquidation_threshold: Decimal,
        liquidation_penalty: Decimal,
    ) -> Result<Self, &'static str> {
        if collateral_factor > dec!(1) || collateral_factor < Decimal::ZERO {
            return Err("Collateral factor must be between 0 and 1");
        }

        if liquidation_threshold > dec!(1) || liquidation_threshold < Decimal::ZERO {
            return Err("Liquidation threshold must be between 0 and 1");
        }

        if liquidation_penalty > dec!(1) || liquidation_penalty < Decimal::ZERO {
            return Err("Liquidation penalty must be between 0 and 1");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            pool_id: Uuid::new_v4(),
            asset_id,
            total_deposits: Decimal::ZERO,
            total_borrows: Decimal::ZERO,
            interest_rate_model,
            collateral_factor,
            liquidation_threshold,
            liquidation_penalty,
            reserve_factor: dec!(0.1), // 10% to reserves by default
            created_at: now,
            last_update: now,
        })
    }

    /// Calculate current utilization rate
    pub fn utilization_rate(&self) -> Decimal {
        if self.total_deposits == Decimal::ZERO {
            Decimal::ZERO
        } else {
            self.total_borrows / self.total_deposits
        }
    }

    /// Calculate current borrow APR
    pub fn borrow_apr(&self) -> Decimal {
        let utilization = self.utilization_rate();
        self.interest_rate_model.calculate_rate(utilization)
    }

    /// Calculate current supply APR
    pub fn supply_apr(&self) -> Decimal {
        let borrow_apr = self.borrow_apr();
        let utilization = self.utilization_rate();
        borrow_apr * utilization * (dec!(1) - self.reserve_factor)
    }

    /// Get available liquidity
    pub fn available_liquidity(&self) -> Decimal {
        self.total_deposits - self.total_borrows
    }
}

/// User position in lending pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LendingPosition {
    pub position_id: Uuid,
    pub user_id: Uuid,
    pub pool_id: Uuid,
    pub deposited: Decimal,
    pub borrowed: Decimal,
    /// Accumulated interest since last update
    pub accrued_interest: Decimal,
    pub last_update: i64,
}

impl LendingPosition {
    pub fn new(user_id: Uuid, pool_id: Uuid) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            position_id: Uuid::new_v4(),
            user_id,
            pool_id,
            deposited: Decimal::ZERO,
            borrowed: Decimal::ZERO,
            accrued_interest: Decimal::ZERO,
            last_update: now,
        }
    }

    /// Calculate total debt (borrowed + accrued interest)
    pub fn total_debt(&self) -> Decimal {
        self.borrowed + self.accrued_interest
    }

    /// Accrue interest based on time elapsed and borrow rate
    pub fn accrue_interest(&mut self, borrow_apr: Decimal, current_time: i64) {
        if self.borrowed == Decimal::ZERO {
            self.last_update = current_time;
            return;
        }

        let time_elapsed = Decimal::from(current_time - self.last_update);
        let seconds_per_year = dec!(31536000); // 365 days

        // Interest = principal * rate * time
        let interest = self.borrowed * borrow_apr * time_elapsed / seconds_per_year;
        self.accrued_interest += interest;
        self.last_update = current_time;
    }
}

/// Health factor calculation for liquidation monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFactor {
    pub user_id: Uuid,
    pub total_collateral_value: Decimal,
    pub total_debt_value: Decimal,
    pub health_factor: Decimal,
    pub liquidation_threshold: Decimal,
}

impl HealthFactor {
    /// Calculate health factor
    /// health_factor = (collateral_value * liquidation_threshold) / debt_value
    /// health_factor < 1.0 means position can be liquidated
    pub fn calculate(
        total_collateral_value: Decimal,
        total_debt_value: Decimal,
        liquidation_threshold: Decimal,
    ) -> Decimal {
        if total_debt_value == Decimal::ZERO {
            return Decimal::MAX; // No debt = infinite health
        }

        (total_collateral_value * liquidation_threshold) / total_debt_value
    }

    pub fn is_liquidatable(&self) -> bool {
        self.health_factor < dec!(1)
    }

    pub fn liquidation_amount(&self) -> Decimal {
        if !self.is_liquidatable() {
            return Decimal::ZERO;
        }

        // Can liquidate up to 50% of debt to restore health
        self.total_debt_value / dec!(2)
    }
}

/// Liquidation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Liquidation {
    pub liquidation_id: Uuid,
    pub liquidator_id: Uuid,
    pub borrower_id: Uuid,
    pub pool_id: Uuid,
    pub debt_repaid: Decimal,
    pub collateral_seized: Decimal,
    pub liquidation_bonus: Decimal,
    pub timestamp: i64,
}

/// Lending manager
pub struct LendingManager {
    pools: Arc<RwLock<HashMap<Uuid, LendingPool>>>,
    positions: Arc<RwLock<HashMap<(Uuid, Uuid), LendingPosition>>>, // (user_id, pool_id)
    asset_prices: Arc<RwLock<HashMap<Uuid, Decimal>>>,              // For health factor calculation
}

impl LendingManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            positions: Arc::new(RwLock::new(HashMap::new())),
            asset_prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new lending pool
    pub async fn create_pool(&self, pool: LendingPool) -> Result<Uuid, &'static str> {
        let pool_id = pool.pool_id;
        self.pools.write().await.insert(pool_id, pool);
        Ok(pool_id)
    }

    /// Update asset price (for oracle integration)
    pub async fn update_price(&self, asset_id: Uuid, price: Decimal) {
        self.asset_prices.write().await.insert(asset_id, price);
    }

    /// Deposit assets into lending pool
    pub async fn deposit(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Deposit amount must be positive");
        }

        // Update pool
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;
        pool.total_deposits += amount;
        pool.last_update = chrono::Utc::now().timestamp();

        // Update position
        let mut positions = self.positions.write().await;
        let position = positions
            .entry((user_id, pool_id))
            .or_insert_with(|| LendingPosition::new(user_id, pool_id));
        position.deposited += amount;

        Ok(())
    }

    /// Withdraw assets from lending pool
    pub async fn withdraw(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Withdraw amount must be positive");
        }

        let mut positions = self.positions.write().await;
        let position = positions
            .get_mut(&(user_id, pool_id))
            .ok_or("Position not found")?;

        if position.deposited < amount {
            return Err("Insufficient deposits");
        }

        // Check if withdrawal would affect health factor
        drop(positions);
        let health = self.calculate_health_factor(user_id).await?;
        let collateral_value_after = health.total_collateral_value - amount;
        let new_health = HealthFactor::calculate(
            collateral_value_after,
            health.total_debt_value,
            health.liquidation_threshold,
        );

        if new_health < dec!(1.2) {
            // Require 20% buffer above liquidation
            return Err("Withdrawal would put position at risk of liquidation");
        }

        // Update pool
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        if pool.available_liquidity() < amount {
            return Err("Insufficient pool liquidity");
        }

        pool.total_deposits -= amount;
        pool.last_update = chrono::Utc::now().timestamp();

        // Update position
        let mut positions = self.positions.write().await;
        let position = positions.get_mut(&(user_id, pool_id)).unwrap();
        position.deposited -= amount;

        Ok(())
    }

    /// Borrow assets from lending pool
    pub async fn borrow(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Borrow amount must be positive");
        }

        let pools = self.pools.read().await;
        let pool = pools.get(&pool_id).ok_or("Pool not found")?;

        if pool.available_liquidity() < amount {
            return Err("Insufficient pool liquidity");
        }

        // Check if borrow would exceed collateral
        drop(pools);
        let health = self.calculate_health_factor(user_id).await?;
        let new_debt = health.total_debt_value + amount;
        let new_health = HealthFactor::calculate(
            health.total_collateral_value,
            new_debt,
            health.liquidation_threshold,
        );

        if new_health < dec!(1.5) {
            // Require 50% buffer
            return Err("Insufficient collateral");
        }

        // Update pool
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).unwrap();
        pool.total_borrows += amount;
        pool.last_update = chrono::Utc::now().timestamp();

        // Accrue interest and update position
        let borrow_apr = pool.borrow_apr();
        drop(pools);

        let mut positions = self.positions.write().await;
        let position = positions
            .entry((user_id, pool_id))
            .or_insert_with(|| LendingPosition::new(user_id, pool_id));

        let current_time = chrono::Utc::now().timestamp();
        position.accrue_interest(borrow_apr, current_time);
        position.borrowed += amount;

        Ok(())
    }

    /// Repay borrowed assets
    pub async fn repay(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
    ) -> Result<Decimal, &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Repay amount must be positive");
        }

        let pools = self.pools.read().await;
        let pool = pools.get(&pool_id).ok_or("Pool not found")?;
        let borrow_apr = pool.borrow_apr();
        drop(pools);

        let mut positions = self.positions.write().await;
        let position = positions
            .get_mut(&(user_id, pool_id))
            .ok_or("Position not found")?;

        // Accrue interest first
        let current_time = chrono::Utc::now().timestamp();
        position.accrue_interest(borrow_apr, current_time);

        let total_debt = position.total_debt();
        if total_debt == Decimal::ZERO {
            return Err("No debt to repay");
        }

        let actual_repay = amount.min(total_debt);

        // Repay accrued interest first, then principal
        if actual_repay >= position.accrued_interest {
            let remaining = actual_repay - position.accrued_interest;
            position.accrued_interest = Decimal::ZERO;
            position.borrowed -= remaining;
        } else {
            position.accrued_interest -= actual_repay;
        }

        // Update pool
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).unwrap();
        pool.total_borrows -= actual_repay.min(position.borrowed);
        pool.last_update = chrono::Utc::now().timestamp();

        Ok(actual_repay)
    }

    /// Calculate user's health factor across all positions
    pub async fn calculate_health_factor(
        &self,
        user_id: Uuid,
    ) -> Result<HealthFactor, &'static str> {
        let positions = self.positions.read().await;
        let pools = self.pools.read().await;
        let prices = self.asset_prices.read().await;

        let mut total_collateral_value = Decimal::ZERO;
        let mut total_debt_value = Decimal::ZERO;
        let mut weighted_threshold = Decimal::ZERO;

        for ((uid, pool_id), position) in positions.iter() {
            if *uid != user_id {
                continue;
            }

            let pool = pools.get(pool_id).ok_or("Pool not found")?;
            let price = prices.get(&pool.asset_id).copied().unwrap_or(dec!(1));

            // Add collateral value
            let collateral_value = position.deposited * price;
            total_collateral_value += collateral_value;
            weighted_threshold += collateral_value * pool.liquidation_threshold;

            // Add debt value
            total_debt_value += position.total_debt() * price;
        }

        let avg_threshold = if total_collateral_value > Decimal::ZERO {
            weighted_threshold / total_collateral_value
        } else {
            dec!(0.75) // Default threshold
        };

        let health_factor =
            HealthFactor::calculate(total_collateral_value, total_debt_value, avg_threshold);

        Ok(HealthFactor {
            user_id,
            total_collateral_value,
            total_debt_value,
            health_factor,
            liquidation_threshold: avg_threshold,
        })
    }

    /// Liquidate undercollateralized position
    pub async fn liquidate(
        &self,
        liquidator_id: Uuid,
        borrower_id: Uuid,
        pool_id: Uuid,
        debt_to_repay: Decimal,
    ) -> Result<Liquidation, &'static str> {
        // Check if borrower is liquidatable
        let health = self.calculate_health_factor(borrower_id).await?;
        if !health.is_liquidatable() {
            return Err("Position is not liquidatable");
        }

        let max_liquidation = health.liquidation_amount();
        if debt_to_repay > max_liquidation {
            return Err("Exceeds maximum liquidation amount");
        }

        let pools = self.pools.read().await;
        let pool = pools.get(&pool_id).ok_or("Pool not found")?;
        let liquidation_penalty = pool.liquidation_penalty;
        let asset_id = pool.asset_id;
        drop(pools);

        let prices = self.asset_prices.read().await;
        let price = prices.get(&asset_id).copied().unwrap_or(dec!(1));
        drop(prices);

        // Calculate collateral to seize (debt + penalty)
        let collateral_seized = debt_to_repay * (dec!(1) + liquidation_penalty) / price;
        let liquidation_bonus = debt_to_repay * liquidation_penalty;

        // Update borrower position
        let mut positions = self.positions.write().await;
        let borrower_pos = positions
            .get_mut(&(borrower_id, pool_id))
            .ok_or("Borrower position not found")?;

        if borrower_pos.deposited < collateral_seized {
            return Err("Insufficient collateral");
        }

        borrower_pos.deposited -= collateral_seized;
        borrower_pos.borrowed -= debt_to_repay;

        // Transfer collateral to liquidator
        let liquidator_pos = positions
            .entry((liquidator_id, pool_id))
            .or_insert_with(|| LendingPosition::new(liquidator_id, pool_id));
        liquidator_pos.deposited += collateral_seized;

        drop(positions);

        // Update pool
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).unwrap();
        pool.total_borrows -= debt_to_repay;
        pool.last_update = chrono::Utc::now().timestamp();

        Ok(Liquidation {
            liquidation_id: Uuid::new_v4(),
            liquidator_id,
            borrower_id,
            pool_id,
            debt_repaid: debt_to_repay,
            collateral_seized,
            liquidation_bonus,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Get pool information
    pub async fn get_pool(&self, pool_id: Uuid) -> Option<LendingPool> {
        self.pools.read().await.get(&pool_id).cloned()
    }

    /// Get user position
    pub async fn get_position(&self, user_id: Uuid, pool_id: Uuid) -> Option<LendingPosition> {
        self.positions
            .read()
            .await
            .get(&(user_id, pool_id))
            .cloned()
    }

    /// Get all positions for a user
    pub async fn get_user_positions(&self, user_id: Uuid) -> Vec<LendingPosition> {
        self.positions
            .read()
            .await
            .iter()
            .filter(|((uid, _), _)| *uid == user_id)
            .map(|(_, pos)| pos.clone())
            .collect()
    }
}

impl Default for LendingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_interest_rate() {
        let model = InterestRateModel::Linear {
            base_rate: dec!(0.02),
            slope: dec!(0.10),
        };

        // 50% utilization
        let rate = model.calculate_rate(dec!(0.5));
        assert_eq!(rate, dec!(0.07)); // 2% + 50% * 10% = 7%
    }

    #[test]
    fn test_kinked_interest_rate() {
        let model = InterestRateModel::Kinked {
            base_rate: dec!(0.02),
            slope1: dec!(0.05),
            slope2: dec!(0.50),
            optimal_utilization: dec!(0.8),
        };

        // Below kink
        let rate1 = model.calculate_rate(dec!(0.5));
        assert_eq!(rate1, dec!(0.045)); // 2% + 50% * 5% = 4.5%

        // Above kink
        let rate2 = model.calculate_rate(dec!(0.9));
        // 2% + 80% * 5% + (90% - 80%) * 50% = 2% + 4% + 5% = 11%
        assert_eq!(rate2, dec!(0.11));
    }

    #[test]
    fn test_lending_pool_creation() {
        let pool = LendingPool::new(
            Uuid::new_v4(),
            InterestRateModel::Linear {
                base_rate: dec!(0.02),
                slope: dec!(0.10),
            },
            dec!(0.75),
            dec!(0.80),
            dec!(0.05),
        );
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pool_utilization() {
        let mut pool = LendingPool::new(
            Uuid::new_v4(),
            InterestRateModel::Linear {
                base_rate: dec!(0.02),
                slope: dec!(0.10),
            },
            dec!(0.75),
            dec!(0.80),
            dec!(0.05),
        )
        .unwrap();

        pool.total_deposits = dec!(1000);
        pool.total_borrows = dec!(750);

        assert_eq!(pool.utilization_rate(), dec!(0.75));
        assert_eq!(pool.borrow_apr(), dec!(0.095)); // 2% + 75% * 10%
    }

    #[test]
    fn test_health_factor_calculation() {
        let health = HealthFactor::calculate(dec!(1000), dec!(600), dec!(0.80));
        // (1000 * 0.80) / 600 = 800 / 600 = 1.333...
        assert!(health > dec!(1.33) && health < dec!(1.34));
        assert!(
            !HealthFactor {
                user_id: Uuid::new_v4(),
                total_collateral_value: dec!(1000),
                total_debt_value: dec!(600),
                health_factor: health,
                liquidation_threshold: dec!(0.80),
            }
            .is_liquidatable()
        );
    }

    #[test]
    fn test_health_factor_liquidatable() {
        let health = HealthFactor::calculate(dec!(1000), dec!(900), dec!(0.80));
        // (1000 * 0.80) / 900 = 800 / 900 = 0.888...
        assert!(health < dec!(1));
        assert!(
            HealthFactor {
                user_id: Uuid::new_v4(),
                total_collateral_value: dec!(1000),
                total_debt_value: dec!(900),
                health_factor: health,
                liquidation_threshold: dec!(0.80),
            }
            .is_liquidatable()
        );
    }

    #[tokio::test]
    async fn test_lending_deposit_withdraw() {
        let manager = LendingManager::new();
        let asset_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Create pool
        let pool = LendingPool::new(
            asset_id,
            InterestRateModel::Linear {
                base_rate: dec!(0.02),
                slope: dec!(0.10),
            },
            dec!(0.75),
            dec!(0.80),
            dec!(0.05),
        )
        .unwrap();
        let pool_id = pool.pool_id;
        manager.create_pool(pool).await.unwrap();
        manager.update_price(asset_id, dec!(1)).await;

        // Deposit
        manager.deposit(user_id, pool_id, dec!(1000)).await.unwrap();

        let position = manager.get_position(user_id, pool_id).await.unwrap();
        assert_eq!(position.deposited, dec!(1000));

        // Withdraw
        manager.withdraw(user_id, pool_id, dec!(300)).await.unwrap();

        let position = manager.get_position(user_id, pool_id).await.unwrap();
        assert_eq!(position.deposited, dec!(700));
    }

    #[tokio::test]
    async fn test_borrow_repay() {
        let manager = LendingManager::new();
        let asset_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Create pool
        let pool = LendingPool::new(
            asset_id,
            InterestRateModel::Linear {
                base_rate: dec!(0.02),
                slope: dec!(0.10),
            },
            dec!(0.75),
            dec!(0.80),
            dec!(0.05),
        )
        .unwrap();
        let pool_id = pool.pool_id;
        manager.create_pool(pool).await.unwrap();
        manager.update_price(asset_id, dec!(1)).await;

        // Deposit collateral
        manager.deposit(user_id, pool_id, dec!(1000)).await.unwrap();

        // Borrow (can borrow up to 75% * collateral with safety buffer)
        manager.borrow(user_id, pool_id, dec!(400)).await.unwrap();

        let position = manager.get_position(user_id, pool_id).await.unwrap();
        assert_eq!(position.borrowed, dec!(400));

        // Repay
        let repaid = manager.repay(user_id, pool_id, dec!(200)).await.unwrap();
        assert_eq!(repaid, dec!(200));

        let position = manager.get_position(user_id, pool_id).await.unwrap();
        assert_eq!(position.borrowed, dec!(200));
    }
}
