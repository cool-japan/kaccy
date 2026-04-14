//! Liquid staking implementation
//!
//! Allows users to stake tokens while receiving a liquid derivative token (stToken)
//! that can be traded or used in DeFi while still earning staking rewards.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Liquid staking pool that mints stTokens for staked assets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidStakingPool {
    /// Pool ID
    pub id: Uuid,
    /// Underlying asset token ID
    pub asset_token_id: Uuid,
    /// Staked token (stToken) ID
    pub st_token_id: Uuid,
    /// Total amount of underlying asset staked
    pub total_staked: Decimal,
    /// Total supply of stTokens
    pub st_token_supply: Decimal,
    /// Accumulated rewards
    pub accumulated_rewards: Decimal,
    /// Exchange rate (stToken to underlying)
    pub exchange_rate: Decimal,
    /// Annual percentage yield
    pub apy: Decimal,
    /// Last reward distribution time
    pub last_reward_time: DateTime<Utc>,
    /// Pool creation time
    pub created_at: DateTime<Utc>,
    /// Slashing events (if any)
    pub slashing_events: Vec<SlashingEvent>,
}

/// Represents a slashing event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvent {
    /// Event ID
    pub id: Uuid,
    /// Amount slashed
    pub amount: Decimal,
    /// Reason for slashing
    pub reason: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// User's liquid staking position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidStakingPosition {
    /// Position ID
    pub id: Uuid,
    /// Pool ID
    pub pool_id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Amount of stTokens held
    pub st_token_amount: Decimal,
    /// Original stake amount (in underlying asset)
    pub original_stake: Decimal,
    /// Timestamp when staked
    pub staked_at: DateTime<Utc>,
    /// Last claimed rewards
    pub last_claimed_at: Option<DateTime<Utc>>,
    /// Total rewards earned
    pub total_rewards_earned: Decimal,
}

/// Redemption request for unstaking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionRequest {
    /// Request ID
    pub id: Uuid,
    /// Pool ID
    pub pool_id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Amount of stTokens to redeem
    pub st_token_amount: Decimal,
    /// Expected underlying amount
    pub expected_amount: Decimal,
    /// Request time
    pub requested_at: DateTime<Utc>,
    /// Unlock time (after unbonding period)
    pub unlock_at: DateTime<Utc>,
    /// Status
    pub status: RedemptionStatus,
}

/// Status of a redemption request
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RedemptionStatus {
    /// Pending unbonding
    Pending,
    /// Ready to claim
    ReadyToClaim,
    /// Claimed
    Claimed,
    /// Cancelled
    Cancelled,
}

impl LiquidStakingPool {
    /// Create a new liquid staking pool
    pub fn new(asset_token_id: Uuid, st_token_id: Uuid, initial_apy: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            asset_token_id,
            st_token_id,
            total_staked: Decimal::ZERO,
            st_token_supply: Decimal::ZERO,
            accumulated_rewards: Decimal::ZERO,
            exchange_rate: dec!(1.0), // Initially 1:1
            apy: initial_apy,
            last_reward_time: Utc::now(),
            created_at: Utc::now(),
            slashing_events: Vec::new(),
        }
    }

    /// Stake assets and mint stTokens
    pub fn stake(&mut self, user_id: Uuid, amount: Decimal) -> Result<LiquidStakingPosition> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Stake amount must be positive".to_string(),
            ));
        }

        // Calculate how many stTokens to mint based on current exchange rate
        let st_tokens_to_mint = amount / self.exchange_rate;

        // Update pool state
        self.total_staked += amount;
        self.st_token_supply += st_tokens_to_mint;

        // Create position
        Ok(LiquidStakingPosition {
            id: Uuid::new_v4(),
            pool_id: self.id,
            user_id,
            st_token_amount: st_tokens_to_mint,
            original_stake: amount,
            staked_at: Utc::now(),
            last_claimed_at: None,
            total_rewards_earned: Decimal::ZERO,
        })
    }

    /// Request redemption (start unbonding)
    pub fn request_redemption(
        &self,
        user_id: Uuid,
        st_token_amount: Decimal,
        unbonding_period_days: i64,
    ) -> Result<RedemptionRequest> {
        if st_token_amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Redemption amount must be positive".to_string(),
            ));
        }

        if st_token_amount > self.st_token_supply {
            return Err(CoreError::InsufficientBalance {
                required: st_token_amount,
                available: self.st_token_supply,
            });
        }

        let expected_amount = st_token_amount * self.exchange_rate;
        let now = Utc::now();
        let unlock_at = now + chrono::Duration::days(unbonding_period_days);

        Ok(RedemptionRequest {
            id: Uuid::new_v4(),
            pool_id: self.id,
            user_id,
            st_token_amount,
            expected_amount,
            requested_at: now,
            unlock_at,
            status: RedemptionStatus::Pending,
        })
    }

    /// Complete redemption (after unbonding period)
    pub fn complete_redemption(&mut self, request: &mut RedemptionRequest) -> Result<Decimal> {
        if request.status != RedemptionStatus::Pending {
            return Err(CoreError::Validation(
                "Redemption request is not pending".to_string(),
            ));
        }

        if Utc::now() < request.unlock_at {
            return Err(CoreError::Validation(
                "Unbonding period not complete".to_string(),
            ));
        }

        // Calculate actual amount based on current exchange rate
        let actual_amount = request.st_token_amount * self.exchange_rate;

        // Update pool state
        self.total_staked -= actual_amount;
        self.st_token_supply -= request.st_token_amount;

        // Update request status
        request.status = RedemptionStatus::Claimed;

        Ok(actual_amount)
    }

    /// Distribute rewards and update exchange rate
    pub fn distribute_rewards(&mut self, reward_amount: Decimal) -> Result<()> {
        if reward_amount < Decimal::ZERO {
            return Err(CoreError::Validation(
                "Reward amount cannot be negative".to_string(),
            ));
        }

        self.accumulated_rewards += reward_amount;
        self.total_staked += reward_amount;

        // Update exchange rate (stTokens become worth more)
        if self.st_token_supply > Decimal::ZERO {
            self.exchange_rate = self.total_staked / self.st_token_supply;
        }

        self.last_reward_time = Utc::now();

        Ok(())
    }

    /// Record a slashing event
    pub fn slash(&mut self, amount: Decimal, reason: String) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Slash amount must be positive".to_string(),
            ));
        }

        if amount > self.total_staked {
            return Err(CoreError::Validation(
                "Slash amount exceeds total staked".to_string(),
            ));
        }

        // Record slashing event
        self.slashing_events.push(SlashingEvent {
            id: Uuid::new_v4(),
            amount,
            reason,
            timestamp: Utc::now(),
        });

        // Reduce total staked
        self.total_staked -= amount;

        // Update exchange rate (stTokens become worth less)
        if self.st_token_supply > Decimal::ZERO {
            self.exchange_rate = self.total_staked / self.st_token_supply;
        }

        Ok(())
    }

    /// Get current value of stTokens in underlying asset
    pub fn get_underlying_value(&self, st_token_amount: Decimal) -> Decimal {
        st_token_amount * self.exchange_rate
    }

    /// Calculate APY including compound effect
    pub fn calculate_compound_apy(&self, compounds_per_year: u32) -> Decimal {
        if compounds_per_year == 0 {
            return self.apy;
        }

        let rate_per_period = self.apy / Decimal::from(compounds_per_year * 100);
        let base = dec!(1.0) + rate_per_period;

        // (1 + r/n)^n - 1, approximation for small r
        let mut result = dec!(1.0);
        for _ in 0..compounds_per_year {
            result *= base;
        }
        (result - dec!(1.0)) * dec!(100)
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        let total_slashed: Decimal = self.slashing_events.iter().map(|e| e.amount).sum();

        PoolStats {
            total_staked: self.total_staked,
            st_token_supply: self.st_token_supply,
            exchange_rate: self.exchange_rate,
            apy: self.apy,
            accumulated_rewards: self.accumulated_rewards,
            total_slashed,
            slashing_events_count: self.slashing_events.len(),
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Total amount of native tokens staked in the pool.
    pub total_staked: Decimal,
    /// Total supply of the liquid staking token.
    pub st_token_supply: Decimal,
    /// Current exchange rate between staking token and native token.
    pub exchange_rate: Decimal,
    /// Current annual percentage yield for stakers.
    pub apy: Decimal,
    /// Total rewards accumulated but not yet distributed.
    pub accumulated_rewards: Decimal,
    /// Total amount slashed from validators.
    pub total_slashed: Decimal,
    /// Number of slashing events that have occurred.
    pub slashing_events_count: usize,
}

/// Manager for multiple liquid staking pools
#[derive(Debug, Clone, Default)]
pub struct LiquidStakingManager {
    pools: HashMap<Uuid, LiquidStakingPool>,
    positions: HashMap<Uuid, Vec<LiquidStakingPosition>>,
    redemptions: HashMap<Uuid, Vec<RedemptionRequest>>,
}

impl LiquidStakingManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new pool
    pub fn create_pool(
        &mut self,
        asset_token_id: Uuid,
        st_token_id: Uuid,
        initial_apy: Decimal,
    ) -> Uuid {
        let pool = LiquidStakingPool::new(asset_token_id, st_token_id, initial_apy);
        let pool_id = pool.id;
        self.pools.insert(pool_id, pool);
        pool_id
    }

    /// Stake in a pool
    pub fn stake(
        &mut self,
        pool_id: Uuid,
        user_id: Uuid,
        amount: Decimal,
    ) -> Result<LiquidStakingPosition> {
        let pool = self
            .pools
            .get_mut(&pool_id)
            .ok_or(CoreError::NotFound("Pool not found".to_string()))?;

        let position = pool.stake(user_id, amount)?;

        self.positions
            .entry(user_id)
            .or_default()
            .push(position.clone());

        Ok(position)
    }

    /// Request redemption
    pub fn request_redemption(
        &mut self,
        pool_id: Uuid,
        user_id: Uuid,
        st_token_amount: Decimal,
        unbonding_period_days: i64,
    ) -> Result<RedemptionRequest> {
        let pool = self
            .pools
            .get(&pool_id)
            .ok_or(CoreError::NotFound("Pool not found".to_string()))?;

        let request = pool.request_redemption(user_id, st_token_amount, unbonding_period_days)?;

        self.redemptions
            .entry(user_id)
            .or_default()
            .push(request.clone());

        Ok(request)
    }

    /// Complete a redemption
    pub fn complete_redemption(
        &mut self,
        pool_id: Uuid,
        user_id: Uuid,
        request_id: Uuid,
    ) -> Result<Decimal> {
        let pool = self
            .pools
            .get_mut(&pool_id)
            .ok_or(CoreError::NotFound("Pool not found".to_string()))?;

        let user_redemptions = self
            .redemptions
            .get_mut(&user_id)
            .ok_or(CoreError::NotFound("No redemptions found".to_string()))?;

        let request = user_redemptions
            .iter_mut()
            .find(|r| r.id == request_id)
            .ok_or(CoreError::NotFound(
                "Redemption request not found".to_string(),
            ))?;

        pool.complete_redemption(request)
    }

    /// Get pool
    pub fn get_pool(&self, pool_id: Uuid) -> Option<&LiquidStakingPool> {
        self.pools.get(&pool_id)
    }

    /// Get user positions
    pub fn get_user_positions(&self, user_id: Uuid) -> Vec<&LiquidStakingPosition> {
        self.positions
            .get(&user_id)
            .map(|positions| positions.iter().collect())
            .unwrap_or_default()
    }

    /// Get user redemptions
    pub fn get_user_redemptions(&self, user_id: Uuid) -> Vec<&RedemptionRequest> {
        self.redemptions
            .get(&user_id)
            .map(|redemptions| redemptions.iter().collect())
            .unwrap_or_default()
    }

    /// Distribute rewards to a pool
    pub fn distribute_rewards(&mut self, pool_id: Uuid, reward_amount: Decimal) -> Result<()> {
        let pool = self
            .pools
            .get_mut(&pool_id)
            .ok_or(CoreError::NotFound("Pool not found".to_string()))?;

        pool.distribute_rewards(reward_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquid_staking_pool_creation() {
        let pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));
        assert_eq!(pool.total_staked, Decimal::ZERO);
        assert_eq!(pool.exchange_rate, dec!(1.0));
        assert_eq!(pool.apy, dec!(10.0));
    }

    #[test]
    fn test_stake_and_mint() {
        let mut pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));
        let user_id = Uuid::new_v4();

        let position = pool.stake(user_id, dec!(1000)).unwrap();

        assert_eq!(position.st_token_amount, dec!(1000)); // 1:1 initially
        assert_eq!(pool.total_staked, dec!(1000));
        assert_eq!(pool.st_token_supply, dec!(1000));
    }

    #[test]
    fn test_reward_distribution_updates_exchange_rate() {
        let mut pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));
        pool.stake(Uuid::new_v4(), dec!(1000)).unwrap();

        // Distribute 100 rewards
        pool.distribute_rewards(dec!(100)).unwrap();

        // Exchange rate should increase
        assert_eq!(pool.exchange_rate, dec!(1.1)); // 1100 / 1000
        assert_eq!(pool.total_staked, dec!(1100));
    }

    #[test]
    fn test_redemption_flow() {
        let mut pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));
        let user_id = Uuid::new_v4();
        pool.stake(user_id, dec!(1000)).unwrap();

        // Request redemption
        let mut request = pool.request_redemption(user_id, dec!(500), 7).unwrap();
        assert_eq!(request.status, RedemptionStatus::Pending);

        // Try to complete before unbonding period - should fail
        let result = pool.complete_redemption(&mut request);
        assert!(result.is_err());
    }

    #[test]
    fn test_slashing_reduces_exchange_rate() {
        let mut pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));
        pool.stake(Uuid::new_v4(), dec!(1000)).unwrap();

        // Slash 10%
        pool.slash(dec!(100), "Validator misbehavior".to_string())
            .unwrap();

        assert_eq!(pool.exchange_rate, dec!(0.9)); // 900 / 1000
        assert_eq!(pool.total_staked, dec!(900));
        assert_eq!(pool.slashing_events.len(), 1);
    }

    #[test]
    fn test_compound_apy_calculation() {
        let pool = LiquidStakingPool::new(Uuid::new_v4(), Uuid::new_v4(), dec!(10.0));

        // Daily compounding
        let compound_apy = pool.calculate_compound_apy(365);
        assert!(compound_apy > dec!(10.0)); // Should be higher than simple APY
    }

    #[test]
    fn test_manager_operations() {
        let mut manager = LiquidStakingManager::new();
        let asset_token = Uuid::new_v4();
        let st_token = Uuid::new_v4();

        // Create pool
        let pool_id = manager.create_pool(asset_token, st_token, dec!(10.0));

        // Stake
        let user_id = Uuid::new_v4();
        let position = manager.stake(pool_id, user_id, dec!(1000)).unwrap();
        assert_eq!(position.st_token_amount, dec!(1000));

        // Check user positions
        let positions = manager.get_user_positions(user_id);
        assert_eq!(positions.len(), 1);
    }
}
