//! Liquidity mining rewards and incentives

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Liquidity mining program for a pool
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiquidityMiningProgram {
    pub program_id: Uuid,
    pub pool_id: Uuid,
    /// Reward token ID (usually $KACCY)
    pub reward_token_id: Uuid,
    /// Total rewards allocated
    pub total_rewards: Decimal,
    /// Rewards distributed so far
    pub distributed_rewards: Decimal,
    /// Reward rate per second
    pub reward_rate_per_second: Decimal,
    /// Program start time
    pub start_time: DateTime<Utc>,
    /// Program end time
    pub end_time: DateTime<Utc>,
    /// Minimum liquidity lock duration (in seconds)
    pub min_lock_duration: i64,
    /// Status
    pub status: ProgramStatus,
    /// Last reward calculation time
    pub last_update_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum ProgramStatus {
    #[default]
    Active,
    Paused,
    Ended,
}

impl fmt::Display for ProgramStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramStatus::Active => write!(f, "active"),
            ProgramStatus::Paused => write!(f, "paused"),
            ProgramStatus::Ended => write!(f, "ended"),
        }
    }
}

impl LiquidityMiningProgram {
    /// Create a new liquidity mining program
    pub fn new(
        pool_id: Uuid,
        reward_token_id: Uuid,
        total_rewards: Decimal,
        duration_days: i64,
        min_lock_duration: i64,
    ) -> Result<Self> {
        if total_rewards <= dec!(0) {
            return Err(CoreError::Validation(
                "Total rewards must be positive".to_string(),
            ));
        }

        if duration_days <= 0 {
            return Err(CoreError::Validation(
                "Duration must be positive".to_string(),
            ));
        }

        let now = Utc::now();
        let duration_seconds = duration_days * 86400;
        let reward_rate_per_second = total_rewards / Decimal::from(duration_seconds);

        Ok(Self {
            program_id: Uuid::new_v4(),
            pool_id,
            reward_token_id,
            total_rewards,
            distributed_rewards: dec!(0),
            reward_rate_per_second,
            start_time: now,
            end_time: now + chrono::Duration::seconds(duration_seconds),
            min_lock_duration,
            status: ProgramStatus::Active,
            last_update_time: now,
            created_at: now,
            updated_at: now,
        })
    }

    /// Check if program is active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        self.status == ProgramStatus::Active && now >= self.start_time && now < self.end_time
    }

    /// Get remaining rewards
    pub fn remaining_rewards(&self) -> Decimal {
        self.total_rewards - self.distributed_rewards
    }

    /// Calculate rewards accrued since last update
    pub fn calculate_pending_rewards(&self, total_staked_lp: Decimal) -> Decimal {
        if !self.is_active() || total_staked_lp == dec!(0) {
            return dec!(0);
        }

        let now = Utc::now();
        let time_elapsed = (now - self.last_update_time).num_seconds();

        if time_elapsed <= 0 {
            return dec!(0);
        }

        let rewards = self.reward_rate_per_second * Decimal::from(time_elapsed);
        rewards.min(self.remaining_rewards())
    }
}

/// User's liquidity mining stake
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LiquidityStake {
    pub stake_id: Uuid,
    pub program_id: Uuid,
    pub user_id: Uuid,
    pub pool_id: Uuid,
    /// Amount of LP tokens staked
    pub lp_tokens_staked: Decimal,
    /// Rewards earned but not claimed
    pub pending_rewards: Decimal,
    /// Total rewards claimed
    pub claimed_rewards: Decimal,
    /// Stake start time
    pub staked_at: DateTime<Utc>,
    /// Lock end time (for boost calculation)
    pub lock_end_time: Option<DateTime<Utc>>,
    /// Boost multiplier (1.0 = no boost, 2.0 = 2x rewards)
    pub boost_multiplier: Decimal,
    /// Last reward calculation time
    pub last_reward_time: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LiquidityStake {
    /// Create a new stake
    pub fn new(
        program_id: Uuid,
        user_id: Uuid,
        pool_id: Uuid,
        lp_tokens: Decimal,
        lock_duration_seconds: Option<i64>,
    ) -> Self {
        let now = Utc::now();
        let lock_end_time =
            lock_duration_seconds.map(|duration| now + chrono::Duration::seconds(duration));

        // Calculate boost multiplier based on lock duration
        let boost_multiplier = Self::calculate_boost_multiplier(lock_duration_seconds);

        Self {
            stake_id: Uuid::new_v4(),
            program_id,
            user_id,
            pool_id,
            lp_tokens_staked: lp_tokens,
            pending_rewards: dec!(0),
            claimed_rewards: dec!(0),
            staked_at: now,
            lock_end_time,
            boost_multiplier,
            last_reward_time: now,
            updated_at: now,
        }
    }

    /// Calculate boost multiplier based on lock duration
    /// - No lock: 1.0x
    /// - 1 month: 1.1x
    /// - 3 months: 1.25x
    /// - 6 months: 1.5x
    /// - 1 year: 2.0x
    fn calculate_boost_multiplier(lock_duration_seconds: Option<i64>) -> Decimal {
        match lock_duration_seconds {
            None => dec!(1.0),
            Some(duration) => {
                let days = duration / 86400;
                if days >= 365 {
                    dec!(2.0)
                } else if days >= 180 {
                    dec!(1.5)
                } else if days >= 90 {
                    dec!(1.25)
                } else if days >= 30 {
                    dec!(1.1)
                } else {
                    dec!(1.0)
                }
            }
        }
    }

    /// Check if stake is locked
    pub fn is_locked(&self) -> bool {
        if let Some(lock_end) = self.lock_end_time {
            Utc::now() < lock_end
        } else {
            false
        }
    }

    /// Calculate effective staked amount (with boost)
    pub fn effective_stake(&self) -> Decimal {
        self.lp_tokens_staked * self.boost_multiplier
    }
}

/// Liquidity mining reward distributor
pub struct RewardDistributor;

impl RewardDistributor {
    /// Update rewards for a stake
    pub fn update_stake_rewards(
        program: &mut LiquidityMiningProgram,
        stake: &mut LiquidityStake,
        total_effective_stake: Decimal,
    ) -> Result<Decimal> {
        if !program.is_active() {
            return Ok(dec!(0));
        }

        if total_effective_stake == dec!(0) {
            return Ok(dec!(0));
        }

        let now = Utc::now();
        let time_elapsed = (now - stake.last_reward_time).num_seconds();

        if time_elapsed <= 0 {
            return Ok(dec!(0));
        }

        // Calculate this stake's share of rewards
        let stake_share = stake.effective_stake() / total_effective_stake;
        let period_rewards = program.reward_rate_per_second * Decimal::from(time_elapsed);
        let stake_rewards = period_rewards * stake_share;

        // Update stake
        stake.pending_rewards += stake_rewards;
        stake.last_reward_time = now;
        stake.updated_at = now;

        // Update program
        program.distributed_rewards += stake_rewards;
        program.last_update_time = now;
        program.updated_at = now;

        Ok(stake_rewards)
    }

    /// Claim rewards from a stake
    pub fn claim_rewards(stake: &mut LiquidityStake) -> Result<Decimal> {
        if stake.pending_rewards == dec!(0) {
            return Ok(dec!(0));
        }

        let claimed = stake.pending_rewards;
        stake.claimed_rewards += claimed;
        stake.pending_rewards = dec!(0);
        stake.updated_at = Utc::now();

        Ok(claimed)
    }

    /// Unstake LP tokens (if not locked)
    pub fn unstake(stake: &mut LiquidityStake, amount: Decimal) -> Result<Decimal> {
        if stake.is_locked() {
            return Err(CoreError::Validation(
                "Cannot unstake while locked".to_string(),
            ));
        }

        if amount > stake.lp_tokens_staked {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available: stake.lp_tokens_staked,
            });
        }

        stake.lp_tokens_staked -= amount;
        stake.updated_at = Utc::now();

        Ok(amount)
    }

    /// Add more LP tokens to existing stake
    pub fn add_to_stake(stake: &mut LiquidityStake, amount: Decimal) {
        stake.lp_tokens_staked += amount;
        stake.updated_at = Utc::now();
    }
}

/// Reward distribution summary
#[derive(Debug, Serialize)]
pub struct RewardSummary {
    pub program_id: Uuid,
    pub pool_id: Uuid,
    pub total_staked_lp: Decimal,
    pub total_effective_stake: Decimal,
    pub total_stakers: i32,
    pub current_apr: Decimal,
    pub total_distributed: Decimal,
    pub remaining_rewards: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_mining_program() {
        let pool_id = Uuid::new_v4();
        let reward_token_id = Uuid::new_v4();

        let program = LiquidityMiningProgram::new(
            pool_id,
            reward_token_id,
            dec!(10000), // 10,000 rewards
            30,          // 30 days
            86400,       // 1 day min lock
        )
        .unwrap();

        assert_eq!(program.total_rewards, dec!(10000));
        assert_eq!(program.distributed_rewards, dec!(0));
        assert!(program.reward_rate_per_second > dec!(0));
        assert!(program.is_active());
    }

    #[test]
    fn test_boost_multipliers() {
        let program_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let pool_id = Uuid::new_v4();

        // No lock
        let stake_no_lock = LiquidityStake::new(program_id, user_id, pool_id, dec!(100), None);
        assert_eq!(stake_no_lock.boost_multiplier, dec!(1.0));

        // 1 month lock
        let stake_1m =
            LiquidityStake::new(program_id, user_id, pool_id, dec!(100), Some(30 * 86400));
        assert_eq!(stake_1m.boost_multiplier, dec!(1.1));

        // 1 year lock
        let stake_1y =
            LiquidityStake::new(program_id, user_id, pool_id, dec!(100), Some(365 * 86400));
        assert_eq!(stake_1y.boost_multiplier, dec!(2.0));
    }

    #[test]
    fn test_effective_stake() {
        let program_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let pool_id = Uuid::new_v4();

        let stake = LiquidityStake::new(
            program_id,
            user_id,
            pool_id,
            dec!(100),
            Some(365 * 86400), // 1 year = 2x boost
        );

        assert_eq!(stake.effective_stake(), dec!(200));
    }

    #[test]
    fn test_reward_distribution() {
        let pool_id = Uuid::new_v4();
        let reward_token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut program =
            LiquidityMiningProgram::new(pool_id, reward_token_id, dec!(10000), 30, 0).unwrap();

        let mut stake = LiquidityStake::new(program.program_id, user_id, pool_id, dec!(100), None);

        // Simulate 1 day passing
        stake.last_reward_time = Utc::now() - chrono::Duration::days(1);

        // Total effective stake is just this stake
        let total_effective = stake.effective_stake();

        let rewards =
            RewardDistributor::update_stake_rewards(&mut program, &mut stake, total_effective)
                .unwrap();

        // Should have earned approximately 1 day worth of rewards (10000 / 30)
        let expected_daily = program.total_rewards / dec!(30);
        assert!((rewards - expected_daily).abs() < dec!(1));
        assert_eq!(stake.pending_rewards, rewards);
    }

    #[test]
    fn test_claim_rewards() {
        let program_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let pool_id = Uuid::new_v4();

        let mut stake = LiquidityStake::new(program_id, user_id, pool_id, dec!(100), None);

        stake.pending_rewards = dec!(500);

        let claimed = RewardDistributor::claim_rewards(&mut stake).unwrap();

        assert_eq!(claimed, dec!(500));
        assert_eq!(stake.pending_rewards, dec!(0));
        assert_eq!(stake.claimed_rewards, dec!(500));
    }

    #[test]
    fn test_locked_stake() {
        let program_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let pool_id = Uuid::new_v4();

        let mut stake = LiquidityStake::new(
            program_id,
            user_id,
            pool_id,
            dec!(100),
            Some(86400), // 1 day lock
        );

        assert!(stake.is_locked());

        // Try to unstake while locked
        let result = RewardDistributor::unstake(&mut stake, dec!(50));
        assert!(result.is_err());
    }
}
