//! $KACCY staking for fee discounts and fee distribution
//!
//! This module provides:
//! - Fee discounts based on $KACCY holdings
//! - Fee distribution to $KACCY stakers
//! - Staking tiers and rewards

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// $KACCY staking tiers for fee discounts
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StakingTier {
    /// No staking (0 $KACCY)
    None,
    /// Basic tier (100+ $KACCY)
    Basic,
    /// Silver tier (1,000+ $KACCY)
    Silver,
    /// Gold tier (10,000+ $KACCY)
    Gold,
    /// Platinum tier (100,000+ $KACCY)
    Platinum,
    /// Diamond tier (1,000,000+ $KACCY)
    Diamond,
}

impl StakingTier {
    /// Get tier from staked amount
    pub fn from_staked_amount(amount: Decimal) -> Self {
        if amount >= dec!(1_000_000) {
            StakingTier::Diamond
        } else if amount >= dec!(100_000) {
            StakingTier::Platinum
        } else if amount >= dec!(10_000) {
            StakingTier::Gold
        } else if amount >= dec!(1_000) {
            StakingTier::Silver
        } else if amount >= dec!(100) {
            StakingTier::Basic
        } else {
            StakingTier::None
        }
    }

    /// Get fee discount percentage for this tier
    pub fn fee_discount_percent(&self) -> Decimal {
        match self {
            StakingTier::None => dec!(0),
            StakingTier::Basic => dec!(5),
            StakingTier::Silver => dec!(10),
            StakingTier::Gold => dec!(20),
            StakingTier::Platinum => dec!(35),
            StakingTier::Diamond => dec!(50),
        }
    }

    /// Get minimum stake required for this tier
    pub fn minimum_stake(&self) -> Decimal {
        match self {
            StakingTier::None => dec!(0),
            StakingTier::Basic => dec!(100),
            StakingTier::Silver => dec!(1_000),
            StakingTier::Gold => dec!(10_000),
            StakingTier::Platinum => dec!(100_000),
            StakingTier::Diamond => dec!(1_000_000),
        }
    }

    /// Get share of fee pool for this tier
    pub fn fee_share_multiplier(&self) -> Decimal {
        match self {
            StakingTier::None => dec!(0),
            StakingTier::Basic => dec!(1),
            StakingTier::Silver => dec!(1.5),
            StakingTier::Gold => dec!(2),
            StakingTier::Platinum => dec!(3),
            StakingTier::Diamond => dec!(5),
        }
    }

    /// Get name of tier
    pub fn name(&self) -> &'static str {
        match self {
            StakingTier::None => "None",
            StakingTier::Basic => "Basic",
            StakingTier::Silver => "Silver",
            StakingTier::Gold => "Gold",
            StakingTier::Platinum => "Platinum",
            StakingTier::Diamond => "Diamond",
        }
    }
}

impl std::fmt::Display for StakingTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A user's staking position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingPosition {
    pub user_id: Uuid,
    pub staked_amount: Decimal,
    pub tier: StakingTier,
    pub staked_at: DateTime<Utc>,
    pub lock_until: Option<DateTime<Utc>>,
    pub accumulated_rewards: Decimal,
    pub last_claim_at: Option<DateTime<Utc>>,
}

impl StakingPosition {
    /// Create a new staking position
    pub fn new(user_id: Uuid, amount: Decimal, lock_days: Option<u32>) -> Self {
        let now = Utc::now();
        let lock_until = lock_days.map(|days| now + Duration::days(i64::from(days)));

        Self {
            user_id,
            staked_amount: amount,
            tier: StakingTier::from_staked_amount(amount),
            staked_at: now,
            lock_until,
            accumulated_rewards: dec!(0),
            last_claim_at: None,
        }
    }

    /// Check if position is locked
    pub fn is_locked(&self) -> bool {
        self.lock_until
            .map(|until| Utc::now() < until)
            .unwrap_or(false)
    }

    /// Get days staked
    pub fn days_staked(&self) -> i64 {
        (Utc::now() - self.staked_at).num_days()
    }

    /// Calculate loyalty bonus (longer staking = higher rewards)
    pub fn loyalty_multiplier(&self) -> Decimal {
        let days = self.days_staked();
        if days >= 365 {
            dec!(2.0) // 2x after 1 year
        } else if days >= 180 {
            dec!(1.5) // 1.5x after 6 months
        } else if days >= 90 {
            dec!(1.25) // 1.25x after 3 months
        } else if days >= 30 {
            dec!(1.1) // 1.1x after 1 month
        } else {
            dec!(1.0)
        }
    }
}

/// Request to stake $KACCY
#[derive(Debug, Clone, Deserialize)]
pub struct StakeRequest {
    pub amount: Decimal,
    /// Lock period in days (optional, longer lock = bonus)
    pub lock_days: Option<u32>,
}

impl StakeRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.amount <= dec!(0) {
            return Err("Amount must be positive");
        }
        if let Some(days) = self.lock_days {
            if days > 365 {
                return Err("Lock period cannot exceed 365 days");
            }
        }
        Ok(())
    }
}

/// Fee pool for distribution to stakers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeePool {
    pub pool_id: Uuid,
    pub total_fees_btc: Decimal,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_weight: Decimal,
    pub distributed: bool,
    pub created_at: DateTime<Utc>,
}

/// Distribution record for a staker
#[derive(Debug, Clone, Serialize)]
pub struct FeeDistribution {
    pub user_id: Uuid,
    pub pool_id: Uuid,
    pub staked_amount: Decimal,
    pub weight: Decimal,
    pub share_btc: Decimal,
    pub claimed: bool,
    pub claimed_at: Option<DateTime<Utc>>,
}

/// Calculator for fee discounts
#[derive(Debug, Clone)]
pub struct FeeDiscountCalculator {
    /// Whether to combine discounts (reputation + staking)
    pub combine_discounts: bool,
    /// Maximum combined discount
    pub max_discount_percent: Decimal,
}

impl Default for FeeDiscountCalculator {
    fn default() -> Self {
        Self {
            combine_discounts: true,
            max_discount_percent: dec!(60), // Max 60% total discount
        }
    }
}

impl FeeDiscountCalculator {
    /// Calculate total discount from reputation and staking
    pub fn calculate_discount(
        &self,
        reputation_score: Decimal,
        staked_amount: Decimal,
    ) -> DiscountBreakdown {
        // Reputation-based discount
        let reputation_discount = Self::reputation_discount_percent(reputation_score);

        // Staking-based discount
        let staking_tier = StakingTier::from_staked_amount(staked_amount);
        let staking_discount = staking_tier.fee_discount_percent();

        // Combine discounts
        let combined = if self.combine_discounts {
            // Multiplicative combination: (1-r) * (1-s) = 1 - total
            // E.g., 20% rep + 20% stake = 36% total (not 40%)
            let rep_factor = dec!(1) - reputation_discount / dec!(100);
            let stake_factor = dec!(1) - staking_discount / dec!(100);
            (dec!(1) - rep_factor * stake_factor) * dec!(100)
        } else {
            // Take the higher discount
            reputation_discount.max(staking_discount)
        };

        // Cap at maximum
        let total_discount = combined.min(self.max_discount_percent);

        DiscountBreakdown {
            reputation_score,
            reputation_discount,
            staked_amount,
            staking_tier,
            staking_discount,
            total_discount,
        }
    }

    /// Get reputation-based discount percentage
    fn reputation_discount_percent(score: Decimal) -> Decimal {
        if score >= dec!(900) {
            dec!(30) // Diamond rep: 30%
        } else if score >= dec!(800) {
            dec!(20) // Platinum rep: 20%
        } else if score >= dec!(600) {
            dec!(10) // Gold rep: 10%
        } else if score >= dec!(400) {
            dec!(5) // Silver rep: 5%
        } else {
            dec!(0)
        }
    }
}

/// Breakdown of fee discount
#[derive(Debug, Clone, Serialize)]
pub struct DiscountBreakdown {
    pub reputation_score: Decimal,
    pub reputation_discount: Decimal,
    pub staked_amount: Decimal,
    pub staking_tier: StakingTier,
    pub staking_discount: Decimal,
    pub total_discount: Decimal,
}

/// Distributor for fee pool to stakers
#[derive(Debug)]
pub struct FeeDistributor {
    positions: HashMap<Uuid, StakingPosition>,
}

impl Default for FeeDistributor {
    fn default() -> Self {
        Self::new()
    }
}

impl FeeDistributor {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Add or update a staking position
    pub fn set_position(&mut self, position: StakingPosition) {
        self.positions.insert(position.user_id, position);
    }

    /// Remove a staking position
    pub fn remove_position(&mut self, user_id: Uuid) -> Option<StakingPosition> {
        self.positions.remove(&user_id)
    }

    /// Get a staking position
    pub fn get_position(&self, user_id: Uuid) -> Option<&StakingPosition> {
        self.positions.get(&user_id)
    }

    /// Calculate total staking weight
    pub fn total_weight(&self) -> Decimal {
        self.positions
            .values()
            .map(|p| self.calculate_weight(p))
            .sum()
    }

    /// Calculate weight for a single position
    fn calculate_weight(&self, position: &StakingPosition) -> Decimal {
        position.staked_amount
            * position.tier.fee_share_multiplier()
            * position.loyalty_multiplier()
    }

    /// Distribute fees to all stakers
    pub fn distribute(&self, total_fees_btc: Decimal) -> Vec<FeeDistribution> {
        let total_weight = self.total_weight();

        if total_weight == dec!(0) {
            return Vec::new();
        }

        let pool_id = Uuid::new_v4();

        self.positions
            .values()
            .map(|position| {
                let weight = self.calculate_weight(position);
                let share = total_fees_btc * weight / total_weight;

                FeeDistribution {
                    user_id: position.user_id,
                    pool_id,
                    staked_amount: position.staked_amount,
                    weight,
                    share_btc: share,
                    claimed: false,
                    claimed_at: None,
                }
            })
            .collect()
    }

    /// Get distribution for a specific user
    pub fn get_user_share(&self, user_id: Uuid, total_fees_btc: Decimal) -> Option<Decimal> {
        let position = self.positions.get(&user_id)?;
        let weight = self.calculate_weight(position);
        let total_weight = self.total_weight();

        if total_weight == dec!(0) {
            return None;
        }

        Some(total_fees_btc * weight / total_weight)
    }

    /// Get summary statistics
    pub fn get_stats(&self) -> StakingStats {
        let total_staked: Decimal = self.positions.values().map(|p| p.staked_amount).sum();
        let total_stakers = self.positions.len();

        let mut by_tier: HashMap<StakingTier, TierStats> = HashMap::new();
        for position in self.positions.values() {
            let entry = by_tier.entry(position.tier).or_insert(TierStats {
                tier: position.tier,
                staker_count: 0,
                total_staked: dec!(0),
            });
            entry.staker_count += 1;
            entry.total_staked += position.staked_amount;
        }

        StakingStats {
            total_staked,
            total_stakers,
            total_weight: self.total_weight(),
            by_tier: by_tier.into_values().collect(),
        }
    }
}

/// Staking statistics
#[derive(Debug, Clone, Serialize)]
pub struct StakingStats {
    pub total_staked: Decimal,
    pub total_stakers: usize,
    pub total_weight: Decimal,
    pub by_tier: Vec<TierStats>,
}

/// Statistics for a single tier
#[derive(Debug, Clone, Serialize)]
pub struct TierStats {
    pub tier: StakingTier,
    pub staker_count: usize,
    pub total_staked: Decimal,
}

/// Configuration for the staking system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingConfig {
    /// Minimum stake amount
    pub min_stake: Decimal,
    /// Distribution period in hours
    pub distribution_period_hours: u32,
    /// Percentage of platform fees that go to stakers
    pub staker_share_percent: Decimal,
    /// Whether locked stakes get bonus multiplier
    pub lock_bonus_enabled: bool,
    /// Bonus multiplier for locked stakes
    pub lock_bonus_multiplier: Decimal,
}

impl Default for StakingConfig {
    fn default() -> Self {
        Self {
            min_stake: dec!(100),
            distribution_period_hours: 24,  // Daily distribution
            staker_share_percent: dec!(50), // 50% of platform fees
            lock_bonus_enabled: true,
            lock_bonus_multiplier: dec!(1.5),
        }
    }
}
