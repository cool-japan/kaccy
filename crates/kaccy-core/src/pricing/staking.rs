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
    /// User who owns this staking position
    pub user_id: Uuid,
    /// Amount of $KACCY currently staked
    pub staked_amount: Decimal,
    /// Tier computed from the staked amount
    pub tier: StakingTier,
    /// Timestamp when the stake was created
    pub staked_at: DateTime<Utc>,
    /// Optional timestamp until which the stake is locked
    pub lock_until: Option<DateTime<Utc>>,
    /// Total rewards accumulated since last claim
    pub accumulated_rewards: Decimal,
    /// Timestamp of the most recent reward claim
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
    /// Amount of $KACCY to stake
    pub amount: Decimal,
    /// Lock period in days (optional, longer lock = bonus)
    pub lock_days: Option<u32>,
}

impl StakeRequest {
    /// Validate the stake request
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
    /// Unique identifier for this fee pool
    pub pool_id: Uuid,
    /// Total BTC fees accumulated for distribution
    pub total_fees_btc: Decimal,
    /// Start of the fee collection period
    pub period_start: DateTime<Utc>,
    /// End of the fee collection period
    pub period_end: DateTime<Utc>,
    /// Sum of all staker weights at distribution time
    pub total_weight: Decimal,
    /// Whether fees have already been distributed
    pub distributed: bool,
    /// When this fee pool was created
    pub created_at: DateTime<Utc>,
}

/// Distribution record for a staker
#[derive(Debug, Clone, Serialize)]
pub struct FeeDistribution {
    /// User receiving this distribution
    pub user_id: Uuid,
    /// Fee pool this distribution is from
    pub pool_id: Uuid,
    /// Amount staked at distribution time
    pub staked_amount: Decimal,
    /// Staker's weight used to calculate their share
    pub weight: Decimal,
    /// BTC amount allocated to this staker
    pub share_btc: Decimal,
    /// Whether the staker has claimed their distribution
    pub claimed: bool,
    /// Timestamp when the staker claimed their distribution
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
    /// User's reputation score used to compute the reputation discount
    pub reputation_score: Decimal,
    /// Discount percentage earned from reputation (0–30%)
    pub reputation_discount: Decimal,
    /// User's staked $KACCY amount
    pub staked_amount: Decimal,
    /// Staking tier derived from the staked amount
    pub staking_tier: StakingTier,
    /// Discount percentage earned from staking (0–50%)
    pub staking_discount: Decimal,
    /// Final combined discount after applying the cap
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
    /// Create a new fee distributor with no staking positions
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
    /// Total $KACCY staked across all users
    pub total_staked: Decimal,
    /// Number of distinct stakers
    pub total_stakers: usize,
    /// Sum of all staker weights
    pub total_weight: Decimal,
    /// Per-tier breakdown of staker counts and amounts
    pub by_tier: Vec<TierStats>,
}

/// Statistics for a single tier
#[derive(Debug, Clone, Serialize)]
pub struct TierStats {
    /// The staking tier this entry describes
    pub tier: StakingTier,
    /// Number of stakers in this tier
    pub staker_count: usize,
    /// Total $KACCY staked across all stakers in this tier
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    // ------------------------------------------------------------------ //
    // Helper: build a StakingPosition without going through new()        //
    // ------------------------------------------------------------------ //
    fn make_position(
        user_id: Uuid,
        amount: Decimal,
        staked_at: DateTime<Utc>,
        lock_until: Option<DateTime<Utc>>,
    ) -> StakingPosition {
        StakingPosition {
            user_id,
            staked_amount: amount,
            tier: StakingTier::from_staked_amount(amount),
            staked_at,
            lock_until,
            accumulated_rewards: dec!(0),
            last_claim_at: None,
        }
    }

    // ------------------------------------------------------------------ //
    // Test 1: Tier classification from staked amount                     //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_tier_from_staked_amount_boundary_values() {
        assert_eq!(StakingTier::from_staked_amount(dec!(0)), StakingTier::None);
        assert_eq!(StakingTier::from_staked_amount(dec!(99)), StakingTier::None);
        assert_eq!(
            StakingTier::from_staked_amount(dec!(100)),
            StakingTier::Basic
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(999)),
            StakingTier::Basic
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(1_000)),
            StakingTier::Silver
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(9_999)),
            StakingTier::Silver
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(10_000)),
            StakingTier::Gold
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(99_999)),
            StakingTier::Gold
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(100_000)),
            StakingTier::Platinum
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(999_999)),
            StakingTier::Platinum
        );
        assert_eq!(
            StakingTier::from_staked_amount(dec!(1_000_000)),
            StakingTier::Diamond
        );
    }

    // ------------------------------------------------------------------ //
    // Test 2: Fee discount percentages per tier                          //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_discount_percentages_per_tier() {
        assert_eq!(StakingTier::None.fee_discount_percent(), dec!(0));
        assert_eq!(StakingTier::Basic.fee_discount_percent(), dec!(5));
        assert_eq!(StakingTier::Silver.fee_discount_percent(), dec!(10));
        assert_eq!(StakingTier::Gold.fee_discount_percent(), dec!(20));
        assert_eq!(StakingTier::Platinum.fee_discount_percent(), dec!(35));
        assert_eq!(StakingTier::Diamond.fee_discount_percent(), dec!(50));
    }

    // ------------------------------------------------------------------ //
    // Test 3: Fee share multipliers ordered correctly                    //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_share_multipliers_increase_with_tier() {
        let none_mult = StakingTier::None.fee_share_multiplier();
        let basic_mult = StakingTier::Basic.fee_share_multiplier();
        let silver_mult = StakingTier::Silver.fee_share_multiplier();
        let gold_mult = StakingTier::Gold.fee_share_multiplier();
        let platinum_mult = StakingTier::Platinum.fee_share_multiplier();
        let diamond_mult = StakingTier::Diamond.fee_share_multiplier();

        assert!(none_mult < basic_mult, "None < Basic multiplier");
        assert!(basic_mult < silver_mult, "Basic < Silver multiplier");
        assert!(silver_mult < gold_mult, "Silver < Gold multiplier");
        assert!(gold_mult < platinum_mult, "Gold < Platinum multiplier");
        assert!(
            platinum_mult < diamond_mult,
            "Platinum < Diamond multiplier"
        );

        // Exact values
        assert_eq!(none_mult, dec!(0));
        assert_eq!(basic_mult, dec!(1));
        assert_eq!(silver_mult, dec!(1.5));
        assert_eq!(diamond_mult, dec!(5));
    }

    // ------------------------------------------------------------------ //
    // Test 4: Minimum stake for each tier                                //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_minimum_stake_per_tier() {
        assert_eq!(StakingTier::None.minimum_stake(), dec!(0));
        assert_eq!(StakingTier::Basic.minimum_stake(), dec!(100));
        assert_eq!(StakingTier::Silver.minimum_stake(), dec!(1_000));
        assert_eq!(StakingTier::Gold.minimum_stake(), dec!(10_000));
        assert_eq!(StakingTier::Platinum.minimum_stake(), dec!(100_000));
        assert_eq!(StakingTier::Diamond.minimum_stake(), dec!(1_000_000));
    }

    // ------------------------------------------------------------------ //
    // Test 5: StakeRequest validation enforces positive amount            //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_stake_request_validation_positive_amount() {
        let valid = StakeRequest {
            amount: dec!(100),
            lock_days: None,
        };
        assert!(valid.validate().is_ok(), "positive amount should be valid");

        let zero_amount = StakeRequest {
            amount: dec!(0),
            lock_days: None,
        };
        assert!(
            zero_amount.validate().is_err(),
            "zero amount should fail validation"
        );

        let negative = StakeRequest {
            amount: dec!(-1),
            lock_days: None,
        };
        assert!(
            negative.validate().is_err(),
            "negative amount should fail validation"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 6: StakeRequest validation rejects lock_days > 365            //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_stake_request_validation_lock_days_cap() {
        let exactly_365 = StakeRequest {
            amount: dec!(100),
            lock_days: Some(365),
        };
        assert!(
            exactly_365.validate().is_ok(),
            "365 lock days should be valid"
        );

        let too_long = StakeRequest {
            amount: dec!(100),
            lock_days: Some(366),
        };
        assert!(
            too_long.validate().is_err(),
            "366 lock days should fail validation"
        );

        let no_lock = StakeRequest {
            amount: dec!(100),
            lock_days: None,
        };
        assert!(no_lock.validate().is_ok(), "no lock should be valid");
    }

    // ------------------------------------------------------------------ //
    // Test 7: is_locked() reflects lock_until relative to now            //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_staking_position_is_locked() {
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        // Future lock
        let locked = make_position(
            user_id,
            dec!(1000),
            now - Duration::days(1),
            Some(now + Duration::days(10)),
        );
        assert!(
            locked.is_locked(),
            "position with future lock_until should be locked"
        );

        // Past lock
        let unlocked = make_position(
            user_id,
            dec!(1000),
            now - Duration::days(30),
            Some(now - Duration::days(1)),
        );
        assert!(
            !unlocked.is_locked(),
            "position with past lock_until should not be locked"
        );

        // No lock
        let no_lock = make_position(user_id, dec!(1000), now - Duration::days(5), None);
        assert!(
            !no_lock.is_locked(),
            "position without lock should not be locked"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 8: loyalty_multiplier() returns correct values by days staked //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_loyalty_multiplier_by_staking_duration() {
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let fresh = make_position(user_id, dec!(100), now - Duration::days(0), None);
        assert_eq!(fresh.loyalty_multiplier(), dec!(1.0), "0 days => 1.0x");

        let one_month = make_position(user_id, dec!(100), now - Duration::days(30), None);
        assert_eq!(one_month.loyalty_multiplier(), dec!(1.1), "30 days => 1.1x");

        let three_months = make_position(user_id, dec!(100), now - Duration::days(90), None);
        assert_eq!(
            three_months.loyalty_multiplier(),
            dec!(1.25),
            "90 days => 1.25x"
        );

        let six_months = make_position(user_id, dec!(100), now - Duration::days(180), None);
        assert_eq!(
            six_months.loyalty_multiplier(),
            dec!(1.5),
            "180 days => 1.5x"
        );

        let one_year = make_position(user_id, dec!(100), now - Duration::days(365), None);
        assert_eq!(one_year.loyalty_multiplier(), dec!(2.0), "365 days => 2.0x");
    }

    // ------------------------------------------------------------------ //
    // Test 9: FeeDistributor distributes proportionally to weight        //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_distributor_proportional_distribution() {
        let now = Utc::now();
        let user_a = Uuid::new_v4();
        let user_b = Uuid::new_v4();

        let mut distributor = FeeDistributor::new();

        // Both users at Basic tier (multiplier=1), same staked amount, fresh (loyalty=1)
        distributor.set_position(make_position(user_a, dec!(100), now, None));
        distributor.set_position(make_position(user_b, dec!(100), now, None));

        let total_fees = dec!(1000);
        let distributions = distributor.distribute(total_fees);

        assert_eq!(
            distributions.len(),
            2,
            "should produce one distribution per staker"
        );

        // Equal weight => equal share
        let share_a = distributions
            .iter()
            .find(|d| d.user_id == user_a)
            .expect("distribution for user_a must exist")
            .share_btc;
        let share_b = distributions
            .iter()
            .find(|d| d.user_id == user_b)
            .expect("distribution for user_b must exist")
            .share_btc;

        assert_eq!(share_a, share_b, "equal weight => equal share");

        // Shares must sum to exactly total_fees (no rounding leak)
        let total_distributed: Decimal = distributions.iter().map(|d| d.share_btc).sum();
        assert_eq!(
            total_distributed, total_fees,
            "distributed shares must sum to total fees"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 10: Higher tier receives proportionally larger share           //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_distributor_higher_tier_receives_larger_share() {
        let now = Utc::now();
        let user_basic = Uuid::new_v4();
        let user_gold = Uuid::new_v4();

        let mut distributor = FeeDistributor::new();

        // Basic: 100 KACCY (multiplier=1), Gold: 10_000 KACCY (multiplier=2)
        distributor.set_position(make_position(user_basic, dec!(100), now, None));
        distributor.set_position(make_position(user_gold, dec!(10_000), now, None));

        let distributions = distributor.distribute(dec!(100));

        let share_basic = distributions
            .iter()
            .find(|d| d.user_id == user_basic)
            .expect("basic distribution must exist")
            .share_btc;
        let share_gold = distributions
            .iter()
            .find(|d| d.user_id == user_gold)
            .expect("gold distribution must exist")
            .share_btc;

        assert!(
            share_gold > share_basic,
            "Gold staker (higher amount + multiplier) must receive larger share: gold={share_gold}, basic={share_basic}"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 11: Empty distributor returns no distributions                 //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_distributor_empty_returns_no_distributions() {
        let distributor = FeeDistributor::new();
        let distributions = distributor.distribute(dec!(1000));
        assert!(
            distributions.is_empty(),
            "empty distributor should return no distributions"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 12: get_user_share returns None for empty pool                 //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_get_user_share_none_when_empty() {
        let distributor = FeeDistributor::new();
        let result = distributor.get_user_share(Uuid::new_v4(), dec!(100));
        assert!(
            result.is_none(),
            "get_user_share with no positions should return None"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 13: Multiplicative fee discount calculation (combine=true)     //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_discount_calculator_multiplicative_combine() {
        // 20% rep + 20% staking => (1 - 0.8 * 0.8) * 100 = 36%, not 40%
        let calc = FeeDiscountCalculator {
            combine_discounts: true,
            max_discount_percent: dec!(60),
        };
        // Score >= 800 => 20% rep discount; 10_000 KACCY => Gold tier => 20% staking discount
        let breakdown = calc.calculate_discount(dec!(800), dec!(10_000));

        assert_eq!(
            breakdown.reputation_discount,
            dec!(20),
            "rep discount should be 20%"
        );
        assert_eq!(
            breakdown.staking_discount,
            dec!(20),
            "staking discount should be 20%"
        );
        assert_eq!(
            breakdown.total_discount,
            dec!(36),
            "combined multiplicative discount should be 36%"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 14: Max discount cap is enforced                               //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_discount_max_cap_enforced() {
        // Diamond rep (30%) + Diamond staking (50%) would combine to high value,
        // cap at max_discount_percent
        let calc = FeeDiscountCalculator {
            combine_discounts: true,
            max_discount_percent: dec!(60),
        };
        let breakdown = calc.calculate_discount(dec!(900), dec!(1_000_000));

        assert!(
            breakdown.total_discount <= dec!(60),
            "total discount must not exceed max cap of 60%, got {}",
            breakdown.total_discount
        );
        assert_eq!(
            breakdown.total_discount,
            dec!(60),
            "should be exactly capped at 60%"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 15: StakingConfig default values are sane                      //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_staking_config_default_values() {
        let config = StakingConfig::default();

        assert_eq!(config.min_stake, dec!(100), "min_stake should be 100");
        assert_eq!(
            config.distribution_period_hours, 24,
            "distribution period should be daily"
        );
        assert_eq!(
            config.staker_share_percent,
            dec!(50),
            "stakers should get 50% of platform fees"
        );
        assert!(
            config.lock_bonus_enabled,
            "lock bonus should be enabled by default"
        );
        assert_eq!(
            config.lock_bonus_multiplier,
            dec!(1.5),
            "lock bonus multiplier should be 1.5x"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 16: StakingStats correctly aggregates multiple positions       //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_staking_stats_aggregation() {
        let now = Utc::now();
        let mut distributor = FeeDistributor::new();

        distributor.set_position(make_position(Uuid::new_v4(), dec!(100), now, None));
        distributor.set_position(make_position(Uuid::new_v4(), dec!(100), now, None));
        distributor.set_position(make_position(Uuid::new_v4(), dec!(1_000), now, None));

        let stats = distributor.get_stats();

        assert_eq!(stats.total_stakers, 3, "should count 3 stakers");
        assert_eq!(
            stats.total_staked,
            dec!(1_200),
            "total staked = 100 + 100 + 1000"
        );
        assert!(
            stats.total_weight > dec!(0),
            "total weight must be positive"
        );

        // Two Basic, one Silver
        let basic_stats = stats.by_tier.iter().find(|t| t.tier == StakingTier::Basic);
        let silver_stats = stats.by_tier.iter().find(|t| t.tier == StakingTier::Silver);
        assert!(basic_stats.is_some(), "Basic tier stats should be present");
        assert!(
            silver_stats.is_some(),
            "Silver tier stats should be present"
        );

        let basic = basic_stats.expect("basic stats must be present");
        assert_eq!(basic.staker_count, 2, "2 stakers at Basic tier");
        assert_eq!(basic.total_staked, dec!(200), "total staked in Basic = 200");
    }

    // ------------------------------------------------------------------ //
    // Test 17: remove_position removes the staker                         //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_remove_position_removes_staker() {
        let now = Utc::now();
        let user_id = Uuid::new_v4();
        let mut distributor = FeeDistributor::new();

        distributor.set_position(make_position(user_id, dec!(500), now, None));
        assert!(
            distributor.get_position(user_id).is_some(),
            "position should exist before removal"
        );

        let removed = distributor
            .remove_position(user_id)
            .expect("should return removed position");
        assert_eq!(removed.user_id, user_id);
        assert!(
            distributor.get_position(user_id).is_none(),
            "position should be gone after removal"
        );

        let stats = distributor.get_stats();
        assert_eq!(stats.total_stakers, 0);
    }

    // ------------------------------------------------------------------ //
    // Test 18: Non-combining discount takes higher of the two             //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fee_discount_non_combining_takes_higher() {
        let calc = FeeDiscountCalculator {
            combine_discounts: false,
            max_discount_percent: dec!(60),
        };

        // Score=400 => 5% rep, 10_000 KACCY => 20% staking => higher is 20%
        let breakdown = calc.calculate_discount(dec!(400), dec!(10_000));
        assert_eq!(
            breakdown.total_discount,
            dec!(20),
            "non-combining should take max(5%, 20%) = 20%"
        );

        // Score=900 => 30% rep, 100 KACCY => 5% staking => higher is 30%
        let breakdown2 = calc.calculate_discount(dec!(900), dec!(100));
        assert_eq!(
            breakdown2.total_discount,
            dec!(30),
            "non-combining should take max(30%, 5%) = 30%"
        );
    }
}
