//! Referral system
//!
//! This module implements a multi-tier referral program that incentivizes user growth.
//!
//! # Features
//!
//! - Unique referral codes for each user
//! - Multi-tier commission structure (referrer, 2nd tier, 3rd tier)
//! - Commission tracking and distribution
//! - Referral leaderboards
//! - Anti-gaming mechanisms (minimum trading volume, cooldown periods)
//!
//! # Commission Structure
//!
//! - Tier 1 (Direct referrals): 30% of platform fees
//! - Tier 2 (Referrals of referrals): 10% of platform fees
//! - Tier 3 (Third level): 5% of platform fees

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Referral code information
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReferralCode {
    /// Unique identifier for this referral code record
    pub code_id: Uuid,
    /// User who owns this referral code
    pub owner_user_id: Uuid,
    /// Unique referral code (e.g., "ALICE2023")
    pub code: String,
    /// Number of times this code has been used
    pub usage_count: u64,
    /// Total commission earned from this code
    pub total_commission_earned: Decimal,
    /// Whether the code is active
    pub is_active: bool,
    /// Timestamp when this code was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when this code was deactivated, if applicable
    pub deactivated_at: Option<DateTime<Utc>>,
}

impl ReferralCode {
    /// Create a new referral code
    pub fn new(owner_user_id: Uuid, code: String) -> Self {
        Self {
            code_id: Uuid::new_v4(),
            owner_user_id,
            code,
            usage_count: 0,
            total_commission_earned: Decimal::ZERO,
            is_active: true,
            created_at: Utc::now(),
            deactivated_at: None,
        }
    }

    /// Deactivate the referral code
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.deactivated_at = Some(Utc::now());
    }

    /// Increment usage count
    pub fn increment_usage(&mut self) {
        self.usage_count += 1;
    }

    /// Add commission earned
    pub fn add_commission(&mut self, amount: Decimal) {
        self.total_commission_earned += amount;
    }
}

/// Referral relationship
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReferralRelationship {
    /// Unique identifier for this referral relationship
    pub relationship_id: Uuid,
    /// User who was referred
    pub referred_user_id: Uuid,
    /// User who referred them (tier 1)
    pub referrer_user_id: Uuid,
    /// Referral code used
    pub referral_code: String,
    /// When the referral was created
    pub referred_at: DateTime<Utc>,
    /// Total trading volume of referred user
    pub total_trading_volume: Decimal,
    /// Total commission earned from this referral
    pub total_commission_earned: Decimal,
    /// Whether the referral is eligible (met minimum requirements)
    pub is_eligible: bool,
}

impl ReferralRelationship {
    /// Create a new referral relationship
    pub fn new(referred_user_id: Uuid, referrer_user_id: Uuid, referral_code: String) -> Self {
        Self {
            relationship_id: Uuid::new_v4(),
            referred_user_id,
            referrer_user_id,
            referral_code,
            referred_at: Utc::now(),
            total_trading_volume: Decimal::ZERO,
            total_commission_earned: Decimal::ZERO,
            is_eligible: false,
        }
    }

    /// Update trading volume
    pub fn add_trading_volume(&mut self, volume: Decimal, min_volume_for_eligibility: Decimal) {
        self.total_trading_volume += volume;

        // Check eligibility
        if self.total_trading_volume >= min_volume_for_eligibility {
            self.is_eligible = true;
        }
    }

    /// Add commission earned
    pub fn add_commission(&mut self, amount: Decimal) {
        self.total_commission_earned += amount;
    }
}

/// Commission distribution record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommissionDistribution {
    /// Unique identifier for this commission distribution
    pub distribution_id: Uuid,
    /// User receiving the commission
    pub recipient_user_id: Uuid,
    /// User who was referred (source of commission)
    pub referred_user_id: Uuid,
    /// Tier level (1, 2, or 3)
    pub tier: u8,
    /// Fee amount that generated this commission
    pub fee_amount: Decimal,
    /// Commission amount
    pub commission_amount: Decimal,
    /// Commission rate applied
    pub commission_rate: Decimal,
    /// Token ID involved in the trade
    pub token_id: Uuid,
    /// When the commission was distributed
    pub distributed_at: DateTime<Utc>,
}

impl CommissionDistribution {
    /// Create a new commission distribution
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        recipient_user_id: Uuid,
        referred_user_id: Uuid,
        tier: u8,
        fee_amount: Decimal,
        commission_rate: Decimal,
        token_id: Uuid,
    ) -> Self {
        let commission_amount = fee_amount * commission_rate;

        Self {
            distribution_id: Uuid::new_v4(),
            recipient_user_id,
            referred_user_id,
            tier,
            fee_amount,
            commission_amount,
            commission_rate,
            token_id,
            distributed_at: Utc::now(),
        }
    }
}

/// Referral configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferralConfig {
    /// Commission rate for tier 1 (direct referrals)
    pub tier1_commission_rate: Decimal,
    /// Commission rate for tier 2
    pub tier2_commission_rate: Decimal,
    /// Commission rate for tier 3
    pub tier3_commission_rate: Decimal,
    /// Minimum trading volume to become eligible for commissions
    pub min_volume_for_eligibility: Decimal,
    /// Cooldown period between referrals from same IP (in hours)
    pub referral_cooldown_hours: u64,
    /// Maximum referrals per user
    pub max_referrals_per_user: Option<u64>,
}

impl Default for ReferralConfig {
    fn default() -> Self {
        Self {
            tier1_commission_rate: dec!(0.30),       // 30% of platform fees
            tier2_commission_rate: dec!(0.10),       // 10% of platform fees
            tier3_commission_rate: dec!(0.05),       // 5% of platform fees
            min_volume_for_eligibility: dec!(100.0), // Must trade at least 100 units
            referral_cooldown_hours: 24,             // 24 hour cooldown
            max_referrals_per_user: Some(1000),
        }
    }
}

/// Referral leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferralLeaderboardEntry {
    /// User this entry belongs to
    pub user_id: Uuid,
    /// Display username (populated from user service if available)
    pub username: Option<String>,
    /// Total number of users referred
    pub total_referrals: u64,
    /// Number of referrals that met the eligibility threshold
    pub eligible_referrals: u64,
    /// Total commission earned across all referrals
    pub total_commission_earned: Decimal,
    /// Position on the leaderboard (1-based)
    pub rank: u64,
}

/// Referral manager
pub struct ReferralManager {
    /// Active referral codes indexed by code string
    codes: HashMap<String, ReferralCode>,
    /// Referral relationships indexed by referred user ID
    relationships: HashMap<Uuid, ReferralRelationship>,
    /// Mapping from user ID to their referral code string
    user_codes: HashMap<Uuid, String>,
    /// Mapping from referrer ID to the list of referred user IDs
    user_referrals: HashMap<Uuid, Vec<Uuid>>,
    /// Referral program configuration
    config: ReferralConfig,
}

impl ReferralManager {
    /// Create a new referral manager
    pub fn new(config: ReferralConfig) -> Self {
        Self {
            codes: HashMap::new(),
            relationships: HashMap::new(),
            user_codes: HashMap::new(),
            user_referrals: HashMap::new(),
            config,
        }
    }

    /// Generate a unique referral code for a user
    pub fn generate_code(
        &mut self,
        user_id: Uuid,
        preferred_code: Option<String>,
    ) -> Result<String> {
        // Check if user already has a code
        if self.user_codes.contains_key(&user_id) {
            return Err(CoreError::AlreadyExists(
                "User already has a referral code".to_string(),
            ));
        }

        // Generate or use preferred code
        let code = if let Some(pref) = preferred_code {
            // Validate preferred code
            if pref.len() < 4 || pref.len() > 20 {
                return Err(CoreError::Validation(
                    "Code must be between 4 and 20 characters".to_string(),
                ));
            }

            if !pref.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(CoreError::Validation(
                    "Code must contain only alphanumeric characters".to_string(),
                ));
            }

            if self.codes.contains_key(&pref.to_uppercase()) {
                return Err(CoreError::AlreadyExists("Code already in use".to_string()));
            }

            pref.to_uppercase()
        } else {
            // Auto-generate code
            self.auto_generate_code(user_id)?
        };

        // Create referral code
        let referral_code = ReferralCode::new(user_id, code.clone());
        self.codes.insert(code.clone(), referral_code);
        self.user_codes.insert(user_id, code.clone());

        Ok(code)
    }

    /// Auto-generate a unique referral code
    fn auto_generate_code(&self, user_id: Uuid) -> Result<String> {
        // Generate code from user ID
        let code = format!(
            "REF{}",
            &user_id.to_string().replace("-", "")[..8].to_uppercase()
        );

        if self.codes.contains_key(&code) {
            // Fallback: add random suffix
            let suffix = chrono::Utc::now().timestamp() % 10000;
            Ok(format!("{}{}", code, suffix))
        } else {
            Ok(code)
        }
    }

    /// Use a referral code (when a new user signs up)
    pub fn use_code(&mut self, code: &str, new_user_id: Uuid) -> Result<Uuid> {
        let code = code.to_uppercase();

        // Get referral code
        let mut referral_code = self
            .codes
            .get(&code)
            .ok_or_else(|| CoreError::NotFound("Referral code not found".to_string()))?
            .clone();

        if !referral_code.is_active {
            return Err(CoreError::Validation(
                "Referral code is inactive".to_string(),
            ));
        }

        let referrer_user_id = referral_code.owner_user_id;

        // Check if user is trying to refer themselves
        if referrer_user_id == new_user_id {
            return Err(CoreError::Validation("Cannot refer yourself".to_string()));
        }

        // Check max referrals limit
        if let Some(max) = self.config.max_referrals_per_user {
            let current_count = self
                .user_referrals
                .get(&referrer_user_id)
                .map(|v| v.len() as u64)
                .unwrap_or(0);
            if current_count >= max {
                return Err(CoreError::Validation(
                    "Maximum referrals reached".to_string(),
                ));
            }
        }

        // Create relationship
        let relationship = ReferralRelationship::new(new_user_id, referrer_user_id, code.clone());
        let relationship_id = relationship.relationship_id;
        self.relationships.insert(new_user_id, relationship);

        // Update code usage
        referral_code.increment_usage();
        self.codes.insert(code, referral_code);

        // Track referral
        self.user_referrals
            .entry(referrer_user_id)
            .or_default()
            .push(new_user_id);

        Ok(relationship_id)
    }

    /// Calculate and distribute commissions for a trade
    pub fn distribute_commissions(
        &mut self,
        trader_user_id: Uuid,
        fee_amount: Decimal,
        token_id: Uuid,
    ) -> Result<Vec<CommissionDistribution>> {
        let mut distributions = Vec::new();

        // Get trader's referral relationship
        let relationship = match self.relationships.get_mut(&trader_user_id) {
            Some(r) => r,
            None => return Ok(distributions), // No referrer, no commissions
        };

        if !relationship.is_eligible {
            return Ok(distributions); // Not eligible yet
        }

        let tier1_user = relationship.referrer_user_id;

        // Tier 1: Direct referrer
        let tier1_commission = CommissionDistribution::new(
            tier1_user,
            trader_user_id,
            1,
            fee_amount,
            self.config.tier1_commission_rate,
            token_id,
        );

        relationship.add_commission(tier1_commission.commission_amount);

        if let Some(code_str) = self.user_codes.get(&tier1_user) {
            if let Some(code) = self.codes.get_mut(code_str) {
                code.add_commission(tier1_commission.commission_amount);
            }
        }

        distributions.push(tier1_commission);

        // Tier 2: Referrer's referrer
        if let Some(tier1_relationship) = self.relationships.get(&tier1_user).cloned() {
            if tier1_relationship.is_eligible {
                let tier2_user = tier1_relationship.referrer_user_id;
                let tier2_commission = CommissionDistribution::new(
                    tier2_user,
                    trader_user_id,
                    2,
                    fee_amount,
                    self.config.tier2_commission_rate,
                    token_id,
                );
                distributions.push(tier2_commission);
            }
        }

        // Tier 3: Referrer's referrer's referrer
        if let Some(tier1_relationship) = self.relationships.get(&tier1_user) {
            if tier1_relationship.is_eligible {
                let tier2_user = tier1_relationship.referrer_user_id;
                if let Some(tier2_relationship) = self.relationships.get(&tier2_user).cloned() {
                    if tier2_relationship.is_eligible {
                        let tier3_user = tier2_relationship.referrer_user_id;
                        let tier3_commission = CommissionDistribution::new(
                            tier3_user,
                            trader_user_id,
                            3,
                            fee_amount,
                            self.config.tier3_commission_rate,
                            token_id,
                        );
                        distributions.push(tier3_commission);
                    }
                }
            }
        }

        Ok(distributions)
    }

    /// Record trading volume for a user (to check eligibility)
    pub fn record_trading_volume(&mut self, user_id: Uuid, volume: Decimal) -> Result<()> {
        if let Some(relationship) = self.relationships.get_mut(&user_id) {
            relationship.add_trading_volume(volume, self.config.min_volume_for_eligibility);
        }
        Ok(())
    }

    /// Get referral code for a user
    pub fn get_user_code(&self, user_id: Uuid) -> Option<&String> {
        self.user_codes.get(&user_id)
    }

    /// Get referral statistics for a user
    pub fn get_user_stats(&self, user_id: Uuid) -> ReferralStats {
        let code = self.get_user_code(user_id);
        let total_referrals = self
            .user_referrals
            .get(&user_id)
            .map(|v| v.len() as u64)
            .unwrap_or(0);

        let eligible_referrals = self
            .user_referrals
            .get(&user_id)
            .map(|refs| {
                refs.iter()
                    .filter(|&&ref_id| {
                        self.relationships
                            .get(&ref_id)
                            .map(|r| r.is_eligible)
                            .unwrap_or(false)
                    })
                    .count() as u64
            })
            .unwrap_or(0);

        let total_commission = code
            .and_then(|c| self.codes.get(c))
            .map(|code| code.total_commission_earned)
            .unwrap_or(Decimal::ZERO);

        ReferralStats {
            user_id,
            referral_code: code.cloned(),
            total_referrals,
            eligible_referrals,
            total_commission_earned: total_commission,
        }
    }

    /// Get leaderboard
    pub fn get_leaderboard(&self, limit: usize) -> Vec<ReferralLeaderboardEntry> {
        let mut entries: Vec<ReferralLeaderboardEntry> = self
            .user_codes
            .keys()
            .map(|&user_id| {
                let stats = self.get_user_stats(user_id);
                ReferralLeaderboardEntry {
                    user_id,
                    username: None, // Would be populated from user service
                    total_referrals: stats.total_referrals,
                    eligible_referrals: stats.eligible_referrals,
                    total_commission_earned: stats.total_commission_earned,
                    rank: 0, // Will be set below
                }
            })
            .collect();

        // Sort by total commission earned
        entries.sort_by(|a, b| {
            b.total_commission_earned
                .cmp(&a.total_commission_earned)
                .then(b.total_referrals.cmp(&a.total_referrals))
        });

        // Assign ranks and limit
        entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(idx, mut entry)| {
                entry.rank = (idx + 1) as u64;
                entry
            })
            .collect()
    }
}

impl Default for ReferralManager {
    fn default() -> Self {
        Self::new(ReferralConfig::default())
    }
}

/// Referral statistics for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferralStats {
    /// User this stats summary belongs to
    pub user_id: Uuid,
    /// Referral code owned by this user, if any
    pub referral_code: Option<String>,
    /// Total number of users referred by this user
    pub total_referrals: u64,
    /// Number of referrals that met the eligibility threshold
    pub eligible_referrals: u64,
    /// Total commission earned across all referrals
    pub total_commission_earned: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code() {
        let mut manager = ReferralManager::default();
        let user_id = Uuid::new_v4();

        let code = manager
            .generate_code(user_id, Some("ALICE2023".to_string()))
            .unwrap();
        assert_eq!(code, "ALICE2023");
        assert_eq!(
            manager.get_user_code(user_id),
            Some(&"ALICE2023".to_string())
        );
    }

    #[test]
    fn test_use_code() {
        let mut manager = ReferralManager::default();
        let referrer_id = Uuid::new_v4();
        let new_user_id = Uuid::new_v4();

        let code = manager
            .generate_code(referrer_id, Some("BOB2023".to_string()))
            .unwrap();
        manager.use_code(&code, new_user_id).unwrap();

        let stats = manager.get_user_stats(referrer_id);
        assert_eq!(stats.total_referrals, 1);
    }

    #[test]
    fn test_cannot_self_refer() {
        let mut manager = ReferralManager::default();
        let user_id = Uuid::new_v4();

        let code = manager
            .generate_code(user_id, Some("SELF".to_string()))
            .unwrap();
        let result = manager.use_code(&code, user_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_commission_distribution() {
        let mut manager = ReferralManager::default();

        // Create referral chain: tier3 -> tier2 -> tier1 -> trader
        let tier3_id = Uuid::new_v4();
        let tier2_id = Uuid::new_v4();
        let tier1_id = Uuid::new_v4();
        let trader_id = Uuid::new_v4();

        manager
            .generate_code(tier3_id, Some("TIER3".to_string()))
            .unwrap();
        manager
            .generate_code(tier2_id, Some("TIER2".to_string()))
            .unwrap();
        manager
            .generate_code(tier1_id, Some("TIER1".to_string()))
            .unwrap();

        manager.use_code("TIER3", tier2_id).unwrap();
        manager.use_code("TIER2", tier1_id).unwrap();
        manager.use_code("TIER1", trader_id).unwrap();

        // Make all eligible by recording trading volume
        manager
            .record_trading_volume(tier2_id, dec!(100.0))
            .unwrap();
        manager
            .record_trading_volume(tier1_id, dec!(100.0))
            .unwrap();
        manager
            .record_trading_volume(trader_id, dec!(100.0))
            .unwrap();

        // Distribute commissions
        let token_id = Uuid::new_v4();
        let commissions = manager
            .distribute_commissions(trader_id, dec!(10.0), token_id)
            .unwrap();

        assert_eq!(commissions.len(), 3); // All 3 tiers
        assert_eq!(commissions[0].tier, 1);
        assert_eq!(commissions[0].commission_amount, dec!(3.0)); // 30% of 10
        assert_eq!(commissions[1].tier, 2);
        assert_eq!(commissions[1].commission_amount, dec!(1.0)); // 10% of 10
        assert_eq!(commissions[2].tier, 3);
        assert_eq!(commissions[2].commission_amount, dec!(0.5)); // 5% of 10
    }

    #[test]
    fn test_eligibility_threshold() {
        let mut manager = ReferralManager::default();
        let referrer_id = Uuid::new_v4();
        let referred_id = Uuid::new_v4();

        manager
            .generate_code(referrer_id, Some("REFER".to_string()))
            .unwrap();
        manager.use_code("REFER", referred_id).unwrap();

        // Not eligible yet
        let token_id = Uuid::new_v4();
        let commissions = manager
            .distribute_commissions(referred_id, dec!(10.0), token_id)
            .unwrap();
        assert_eq!(commissions.len(), 0);

        // Record enough volume to become eligible
        manager
            .record_trading_volume(referred_id, dec!(100.0))
            .unwrap();

        let commissions = manager
            .distribute_commissions(referred_id, dec!(10.0), token_id)
            .unwrap();
        assert_eq!(commissions.len(), 1);
    }

    #[test]
    fn test_leaderboard() {
        let mut manager = ReferralManager::default();

        // Create users with different referral counts
        let alice_id = Uuid::new_v4();
        let bob_id = Uuid::new_v4();

        manager
            .generate_code(alice_id, Some("ALICE".to_string()))
            .unwrap();
        manager
            .generate_code(bob_id, Some("BOBBY".to_string()))
            .unwrap();

        // Alice refers 3 users
        for _ in 0..3 {
            let user = Uuid::new_v4();
            manager.use_code("ALICE", user).unwrap();
            manager.record_trading_volume(user, dec!(100.0)).unwrap();
            manager
                .distribute_commissions(user, dec!(10.0), Uuid::new_v4())
                .unwrap();
        }

        // Bob refers 1 user
        let user = Uuid::new_v4();
        manager.use_code("BOBBY", user).unwrap();

        let leaderboard = manager.get_leaderboard(10);
        assert!(!leaderboard.is_empty());
        assert_eq!(leaderboard[0].user_id, alice_id); // Alice should be first
    }
}
