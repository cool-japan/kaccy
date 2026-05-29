//! Yield farming mechanisms
//!
//! This module provides:
//! - Liquidity mining programs
//! - Reward distribution algorithms
//! - Farming pool management
//! - APR/APY calculations
//! - Staking and reward mechanisms

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Reward emission schedule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmissionSchedule {
    /// Fixed emission per block/time period
    Fixed {
        /// Constant reward tokens emitted per second.
        rate_per_second: Decimal,
    },
    /// Decreasing emission over time (halving)
    Decreasing {
        /// Starting emission rate at launch.
        initial_rate: Decimal,
        /// Duration in seconds between each halving event.
        halving_period: i64,
        /// Minimum emission floor after repeated halvings.
        min_rate: Decimal,
    },
    /// Custom emission curve
    Custom {
        /// List of (timestamp, rate) pairs defining the emission curve.
        emissions: Vec<(i64, Decimal)>,
    },
}

impl EmissionSchedule {
    /// Calculate emission rate at a specific time
    pub fn rate_at(&self, timestamp: i64, start_time: i64) -> Decimal {
        match self {
            Self::Fixed { rate_per_second } => *rate_per_second,
            Self::Decreasing {
                initial_rate,
                halving_period,
                min_rate,
            } => {
                let elapsed = timestamp - start_time;
                if elapsed < 0 {
                    return Decimal::ZERO;
                }

                let halvings = elapsed / halving_period;
                let mut rate = *initial_rate;

                for _ in 0..halvings {
                    rate /= dec!(2);
                    if rate < *min_rate {
                        return *min_rate;
                    }
                }

                rate.max(*min_rate)
            }
            Self::Custom { emissions } => {
                // Find the appropriate emission rate
                let mut current_rate = Decimal::ZERO;
                for (time, rate) in emissions {
                    if timestamp >= *time {
                        current_rate = *rate;
                    } else {
                        break;
                    }
                }
                current_rate
            }
        }
    }
}

/// Farming pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarmingPool {
    /// Unique identifier for this pool
    pub pool_id: Uuid,
    /// Human-readable name for this pool
    pub name: String,
    /// Staking token (LP token or single token)
    pub stake_token_id: Uuid,
    /// Reward token
    pub reward_token_id: Uuid,
    /// Emission schedule for rewards
    pub emission_schedule: EmissionSchedule,
    /// Total staked amount
    pub total_staked: Decimal,
    /// Accumulated rewards per share (scaled by 1e18)
    pub acc_reward_per_share: Decimal,
    /// Last reward calculation time
    pub last_reward_time: i64,
    /// Pool start time
    pub start_time: i64,
    /// Pool end time (if any)
    pub end_time: Option<i64>,
    /// Lock period (optional minimum staking duration)
    pub lock_period: Option<i64>,
    /// Boost multiplier based on lock duration
    /// Whether boost multipliers for locked stakes are enabled
    pub boost_enabled: bool,
    /// Unix timestamp when this pool was created
    pub created_at: i64,
}

impl FarmingPool {
    /// Create a new farming pool with the given parameters
    pub fn new(
        name: String,
        stake_token_id: Uuid,
        reward_token_id: Uuid,
        emission_schedule: EmissionSchedule,
        start_time: i64,
        end_time: Option<i64>,
        lock_period: Option<i64>,
    ) -> Result<Self, &'static str> {
        if let Some(end) = end_time {
            if end <= start_time {
                return Err("End time must be after start time");
            }
        }

        Ok(Self {
            pool_id: Uuid::new_v4(),
            name,
            stake_token_id,
            reward_token_id,
            emission_schedule,
            total_staked: Decimal::ZERO,
            acc_reward_per_share: Decimal::ZERO,
            last_reward_time: start_time,
            start_time,
            end_time,
            lock_period,
            boost_enabled: true,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Update reward variables
    pub fn update_rewards(&mut self, current_time: i64) -> Decimal {
        if current_time <= self.last_reward_time {
            return Decimal::ZERO;
        }

        if self.total_staked == Decimal::ZERO {
            self.last_reward_time = current_time;
            return Decimal::ZERO;
        }

        // Calculate time to consider
        let effective_time = match self.end_time {
            Some(end) => current_time.min(end),
            None => current_time,
        };

        let time_elapsed = effective_time - self.last_reward_time;
        if time_elapsed <= 0 {
            return Decimal::ZERO;
        }

        let emission_rate = self
            .emission_schedule
            .rate_at(effective_time, self.start_time);
        let rewards = emission_rate * Decimal::from(time_elapsed);

        // Update accumulated rewards per share (scaled by 1e18 for precision)
        let reward_per_share = (rewards * dec!(1000000000000000000)) / self.total_staked;
        self.acc_reward_per_share += reward_per_share;
        self.last_reward_time = effective_time;

        rewards
    }

    /// Calculate APR based on current emission rate
    pub fn calculate_apr(
        &self,
        reward_token_price: Decimal,
        stake_token_price: Decimal,
    ) -> Decimal {
        if self.total_staked == Decimal::ZERO {
            return Decimal::ZERO;
        }

        let current_rate = self
            .emission_schedule
            .rate_at(self.last_reward_time, self.start_time);
        let yearly_rewards = current_rate * dec!(31536000); // seconds in a year
        let reward_value = yearly_rewards * reward_token_price;
        let staked_value = self.total_staked * stake_token_price;

        if staked_value == Decimal::ZERO {
            Decimal::ZERO
        } else {
            reward_value / staked_value
        }
    }

    /// Calculate APY (compound interest)
    pub fn calculate_apy(
        &self,
        reward_token_price: Decimal,
        stake_token_price: Decimal,
    ) -> Decimal {
        let apr = self.calculate_apr(reward_token_price, stake_token_price);

        // APY = (1 + APR/n)^n - 1, where n = compounding periods (365 for daily)
        // Approximate (1 + r)^365 using Taylor series expansion
        // (1 + r)^n ≈ 1 + nr + n(n-1)/2 * r^2 + ...
        // For small apr, use approximation: APY ≈ APR * (1 + APR/2)
        apr * (dec!(1) + apr / dec!(2))
    }
}

/// User stake in a farming pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStake {
    /// Unique identifier for this stake
    pub stake_id: Uuid,
    /// User who owns this stake
    pub user_id: Uuid,
    /// Pool this stake belongs to
    pub pool_id: Uuid,
    /// Amount of tokens currently staked
    pub amount: Decimal,
    /// Accumulated reward debt used to calculate incremental rewards
    pub reward_debt: Decimal,
    /// Rewards accrued but not yet claimed
    pub pending_rewards: Decimal,
    /// Unix timestamp when the stake was created
    pub stake_time: i64,
    /// Unix timestamp when the lock expires, if applicable
    pub unlock_time: Option<i64>,
    /// Boost multiplier applied to this stake
    pub boost_multiplier: Decimal,
}

impl UserStake {
    /// Create a new empty user stake for the given pool
    pub fn new(user_id: Uuid, pool_id: Uuid, lock_period: Option<i64>) -> Self {
        let stake_time = chrono::Utc::now().timestamp();
        let unlock_time = lock_period.map(|period| stake_time + period);

        Self {
            stake_id: Uuid::new_v4(),
            user_id,
            pool_id,
            amount: Decimal::ZERO,
            reward_debt: Decimal::ZERO,
            pending_rewards: Decimal::ZERO,
            stake_time,
            unlock_time,
            boost_multiplier: dec!(1),
        }
    }

    /// Calculate pending rewards
    pub fn calculate_pending(&self, acc_reward_per_share: Decimal) -> Decimal {
        let accrued = (self.amount * self.boost_multiplier * acc_reward_per_share)
            / dec!(1000000000000000000);
        accrued - self.reward_debt + self.pending_rewards
    }

    /// Can withdraw (check lock period)
    pub fn can_withdraw(&self, current_time: i64) -> bool {
        match self.unlock_time {
            Some(unlock) => current_time >= unlock,
            None => true,
        }
    }

    /// Calculate boost multiplier based on lock duration
    pub fn calculate_boost(lock_duration: i64) -> Decimal {
        // 1x for no lock, up to 2x for max lock (2 years)
        let days = lock_duration / 86400;
        if days <= 0 {
            return dec!(1);
        }

        let max_days = 730; // 2 years
        let boost = dec!(1) + (Decimal::from(days.min(max_days)) / Decimal::from(max_days));
        boost.min(dec!(2))
    }
}

/// Yield farming manager
pub struct YieldFarmingManager {
    /// All farming pools keyed by pool ID
    pools: Arc<RwLock<HashMap<Uuid, FarmingPool>>>,
    /// User stakes keyed by (user_id, pool_id)
    user_stakes: Arc<RwLock<HashMap<(Uuid, Uuid), UserStake>>>,
    /// Current token prices keyed by token ID
    token_prices: Arc<RwLock<HashMap<Uuid, Decimal>>>,
}

impl YieldFarmingManager {
    /// Create a new yield farming manager
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            user_stakes: Arc::new(RwLock::new(HashMap::new())),
            token_prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new farming pool
    pub async fn create_pool(&self, pool: FarmingPool) -> Result<Uuid, &'static str> {
        let pool_id = pool.pool_id;
        self.pools.write().await.insert(pool_id, pool);
        Ok(pool_id)
    }

    /// Update token price
    pub async fn update_price(&self, token_id: Uuid, price: Decimal) {
        self.token_prices.write().await.insert(token_id, price);
    }

    /// Stake tokens in farming pool
    pub async fn stake(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
        lock_duration: Option<i64>,
    ) -> Result<(), &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Stake amount must be positive");
        }

        // Update pool rewards
        let current_time = chrono::Utc::now().timestamp();
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;

        // Check if pool has started
        if current_time < pool.start_time {
            return Err("Pool has not started yet");
        }

        // Check if pool has ended
        if let Some(end_time) = pool.end_time {
            if current_time > end_time {
                return Err("Pool has ended");
            }
        }

        pool.update_rewards(current_time);
        let acc_reward_per_share = pool.acc_reward_per_share;
        let lock_period = pool.lock_period;
        drop(pools);

        // Get or create user stake
        let mut stakes = self.user_stakes.write().await;
        let stake = stakes
            .entry((user_id, pool_id))
            .or_insert_with(|| UserStake::new(user_id, pool_id, lock_period));

        // Calculate pending rewards before updating
        stake.pending_rewards = stake.calculate_pending(acc_reward_per_share);

        // Apply boost multiplier if locking
        if let Some(duration) = lock_duration {
            stake.boost_multiplier = UserStake::calculate_boost(duration);
            stake.unlock_time = Some(current_time + duration);
        }

        // Update stake
        stake.amount += amount;
        stake.reward_debt = (stake.amount * stake.boost_multiplier * acc_reward_per_share)
            / dec!(1000000000000000000);

        drop(stakes);

        // Update pool total
        let mut pools = self.pools.write().await;
        let pool = pools
            .get_mut(&pool_id)
            .expect("pool_id was validated via ok_or earlier in this function");
        pool.total_staked += amount;

        Ok(())
    }

    /// Unstake tokens from farming pool
    pub async fn unstake(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
        amount: Decimal,
    ) -> Result<Decimal, &'static str> {
        if amount <= Decimal::ZERO {
            return Err("Unstake amount must be positive");
        }

        // Update pool rewards
        let current_time = chrono::Utc::now().timestamp();
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;
        pool.update_rewards(current_time);
        let acc_reward_per_share = pool.acc_reward_per_share;
        drop(pools);

        // Get user stake
        let mut stakes = self.user_stakes.write().await;
        let stake = stakes
            .get_mut(&(user_id, pool_id))
            .ok_or("No stake found")?;

        // Check lock period
        if !stake.can_withdraw(current_time) {
            return Err("Tokens are still locked");
        }

        if amount > stake.amount {
            return Err("Insufficient staked amount");
        }

        // Calculate and store pending rewards
        let pending = stake.calculate_pending(acc_reward_per_share);
        stake.pending_rewards = pending;

        // Update stake
        stake.amount -= amount;
        stake.reward_debt = (stake.amount * stake.boost_multiplier * acc_reward_per_share)
            / dec!(1000000000000000000);

        drop(stakes);

        // Update pool total
        let mut pools = self.pools.write().await;
        let pool = pools
            .get_mut(&pool_id)
            .expect("pool_id was validated via ok_or earlier in this function");
        pool.total_staked -= amount;

        Ok(pending)
    }

    /// Claim rewards without unstaking
    pub async fn claim_rewards(
        &self,
        user_id: Uuid,
        pool_id: Uuid,
    ) -> Result<Decimal, &'static str> {
        // Update pool rewards
        let current_time = chrono::Utc::now().timestamp();
        let mut pools = self.pools.write().await;
        let pool = pools.get_mut(&pool_id).ok_or("Pool not found")?;
        pool.update_rewards(current_time);
        let acc_reward_per_share = pool.acc_reward_per_share;
        drop(pools);

        // Get user stake
        let mut stakes = self.user_stakes.write().await;
        let stake = stakes
            .get_mut(&(user_id, pool_id))
            .ok_or("No stake found")?;

        let pending = stake.calculate_pending(acc_reward_per_share);

        // Reset pending rewards and update debt
        stake.pending_rewards = Decimal::ZERO;
        stake.reward_debt = (stake.amount * stake.boost_multiplier * acc_reward_per_share)
            / dec!(1000000000000000000);

        Ok(pending)
    }

    /// Get pending rewards for a user
    pub async fn get_pending_rewards(&self, user_id: Uuid, pool_id: Uuid) -> Decimal {
        // Update pool rewards
        let current_time = chrono::Utc::now().timestamp();
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.get_mut(&pool_id) {
            pool.update_rewards(current_time);
            let acc_reward_per_share = pool.acc_reward_per_share;
            drop(pools);

            let stakes = self.user_stakes.read().await;
            if let Some(stake) = stakes.get(&(user_id, pool_id)) {
                return stake.calculate_pending(acc_reward_per_share);
            }
        }

        Decimal::ZERO
    }

    /// Get pool info
    pub async fn get_pool(&self, pool_id: Uuid) -> Option<FarmingPool> {
        self.pools.read().await.get(&pool_id).cloned()
    }

    /// Get user stake info
    pub async fn get_stake(&self, user_id: Uuid, pool_id: Uuid) -> Option<UserStake> {
        self.user_stakes
            .read()
            .await
            .get(&(user_id, pool_id))
            .cloned()
    }

    /// Get all pools
    pub async fn get_all_pools(&self) -> Vec<FarmingPool> {
        self.pools.read().await.values().cloned().collect()
    }

    /// Get all stakes for a user
    pub async fn get_user_stakes(&self, user_id: Uuid) -> Vec<UserStake> {
        self.user_stakes
            .read()
            .await
            .iter()
            .filter(|((uid, _), _)| *uid == user_id)
            .map(|(_, stake)| stake.clone())
            .collect()
    }
}

impl Default for YieldFarmingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_emission() {
        let schedule = EmissionSchedule::Fixed {
            rate_per_second: dec!(10),
        };

        assert_eq!(schedule.rate_at(1000, 0), dec!(10));
        assert_eq!(schedule.rate_at(50000, 0), dec!(10));
    }

    #[test]
    fn test_decreasing_emission() {
        let schedule = EmissionSchedule::Decreasing {
            initial_rate: dec!(100),
            halving_period: 1000,
            min_rate: dec!(1),
        };

        // No halving yet
        assert_eq!(schedule.rate_at(500, 0), dec!(100));

        // One halving
        assert_eq!(schedule.rate_at(1000, 0), dec!(50));

        // Two halvings
        assert_eq!(schedule.rate_at(2000, 0), dec!(25));

        // Three halvings
        assert_eq!(schedule.rate_at(3000, 0), dec!(12.5));
    }

    #[test]
    fn test_boost_multiplier() {
        // No lock
        assert_eq!(UserStake::calculate_boost(0), dec!(1));

        // 1 year lock (365 days)
        let boost_1y = UserStake::calculate_boost(365 * 86400);
        assert!(boost_1y > dec!(1) && boost_1y < dec!(2));

        // 2 year lock (max)
        assert_eq!(UserStake::calculate_boost(730 * 86400), dec!(2));
    }

    #[test]
    fn test_farming_pool_creation() {
        let pool = FarmingPool::new(
            "ETH-USDC Pool".to_string(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            EmissionSchedule::Fixed {
                rate_per_second: dec!(10),
            },
            0,
            Some(86400 * 30), // 30 days
            None,
        );
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_stake_and_rewards() {
        let manager = YieldFarmingManager::new();
        let stake_token = Uuid::new_v4();
        let reward_token = Uuid::new_v4();

        // Create pool with fixed emission
        let pool = FarmingPool::new(
            "Test Pool".to_string(),
            stake_token,
            reward_token,
            EmissionSchedule::Fixed {
                rate_per_second: dec!(10),
            },
            0,
            None,
            None,
        )
        .unwrap();
        let pool_id = pool.pool_id;
        manager.create_pool(pool).await.unwrap();

        // Stake tokens
        let user_id = Uuid::new_v4();
        manager
            .stake(user_id, pool_id, dec!(100), None)
            .await
            .unwrap();

        // Wait a bit and check rewards (simulate by manually updating time)
        // In real scenario, time passes naturally
        let pending = manager.get_pending_rewards(user_id, pool_id).await;
        assert!(pending >= Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_unstake() {
        let manager = YieldFarmingManager::new();
        let stake_token = Uuid::new_v4();
        let reward_token = Uuid::new_v4();

        let pool = FarmingPool::new(
            "Test Pool".to_string(),
            stake_token,
            reward_token,
            EmissionSchedule::Fixed {
                rate_per_second: dec!(10),
            },
            0,
            None,
            None,
        )
        .unwrap();
        let pool_id = pool.pool_id;
        manager.create_pool(pool).await.unwrap();

        let user_id = Uuid::new_v4();
        manager
            .stake(user_id, pool_id, dec!(100), None)
            .await
            .unwrap();

        // Unstake
        let rewards = manager.unstake(user_id, pool_id, dec!(50)).await.unwrap();
        assert!(rewards >= Decimal::ZERO);

        let stake = manager.get_stake(user_id, pool_id).await.unwrap();
        assert_eq!(stake.amount, dec!(50));
    }
}
