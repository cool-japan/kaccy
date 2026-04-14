//! Reputation tiers

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Reputation tier classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReputationTier {
    /// Score 0–199: account not yet verified.
    Unverified, // 0-199
    /// Score 200–399.
    Bronze, // 200-399
    /// Score 400–599.
    Silver, // 400-599
    /// Score 600–799.
    Gold, // 600-799
    /// Score 800–899.
    Platinum, // 800-899
    /// Score 900–1000.
    Diamond, // 900-1000
}

impl fmt::Display for ReputationTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unverified => write!(f, "Unverified"),
            Self::Bronze => write!(f, "Bronze"),
            Self::Silver => write!(f, "Silver"),
            Self::Gold => write!(f, "Gold"),
            Self::Platinum => write!(f, "Platinum"),
            Self::Diamond => write!(f, "Diamond"),
        }
    }
}

impl std::str::FromStr for ReputationTier {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unverified" => Ok(Self::Unverified),
            "bronze" => Ok(Self::Bronze),
            "silver" => Ok(Self::Silver),
            "gold" => Ok(Self::Gold),
            "platinum" => Ok(Self::Platinum),
            "diamond" => Ok(Self::Diamond),
            _ => Err(format!("Invalid reputation tier: {}", s)),
        }
    }
}

impl TryFrom<String> for ReputationTier {
    type Error = String;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for ReputationTier {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl ReputationTier {
    /// Determine the tier that corresponds to the given reputation score.
    pub fn from_score(score: Decimal) -> Self {
        let score_f64 = score.try_into().unwrap_or(0.0);
        match score_f64 {
            s if s >= 900.0 => Self::Diamond,
            s if s >= 800.0 => Self::Platinum,
            s if s >= 600.0 => Self::Gold,
            s if s >= 400.0 => Self::Silver,
            s if s >= 200.0 => Self::Bronze,
            _ => Self::Unverified,
        }
    }

    /// Get the minimum score required for this tier
    pub fn min_score(&self) -> Decimal {
        use rust_decimal_macros::dec;
        match self {
            Self::Unverified => dec!(0),
            Self::Bronze => dec!(200),
            Self::Silver => dec!(400),
            Self::Gold => dec!(600),
            Self::Platinum => dec!(800),
            Self::Diamond => dec!(900),
        }
    }

    /// Get the maximum score for this tier (inclusive)
    pub fn max_score(&self) -> Decimal {
        use rust_decimal_macros::dec;
        match self {
            Self::Unverified => dec!(199),
            Self::Bronze => dec!(399),
            Self::Silver => dec!(599),
            Self::Gold => dec!(799),
            Self::Platinum => dec!(899),
            Self::Diamond => dec!(1000),
        }
    }

    /// Get the score range for this tier as (min, max)
    pub fn score_range(&self) -> (Decimal, Decimal) {
        (self.min_score(), self.max_score())
    }

    /// Check if this tier allows token issuance
    pub fn is_verified(&self) -> bool {
        self.benefits().can_issue_tokens
    }

    /// Get the next higher tier, if any
    pub fn next_tier(&self) -> Option<Self> {
        match self {
            Self::Unverified => Some(Self::Bronze),
            Self::Bronze => Some(Self::Silver),
            Self::Silver => Some(Self::Gold),
            Self::Gold => Some(Self::Platinum),
            Self::Platinum => Some(Self::Diamond),
            Self::Diamond => None,
        }
    }

    /// Get the previous lower tier, if any
    pub fn previous_tier(&self) -> Option<Self> {
        match self {
            Self::Unverified => None,
            Self::Bronze => Some(Self::Unverified),
            Self::Silver => Some(Self::Bronze),
            Self::Gold => Some(Self::Silver),
            Self::Platinum => Some(Self::Gold),
            Self::Diamond => Some(Self::Platinum),
        }
    }

    /// Get all tier variants in order from lowest to highest
    pub fn all_variants() -> [Self; 6] {
        [
            Self::Unverified,
            Self::Bronze,
            Self::Silver,
            Self::Gold,
            Self::Platinum,
            Self::Diamond,
        ]
    }

    /// Get tier as string (lowercase)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
            Self::Platinum => "platinum",
            Self::Diamond => "diamond",
        }
    }

    /// Return the benefits granted by this tier.
    pub fn benefits(&self) -> TierBenefits {
        match self {
            Self::Diamond => TierBenefits {
                max_token_issuance: 5,
                fee_discount_percent: 50,
                can_issue_tokens: true,
                verified_badge: true,
            },
            Self::Platinum => TierBenefits {
                max_token_issuance: 3,
                fee_discount_percent: 30,
                can_issue_tokens: true,
                verified_badge: true,
            },
            Self::Gold => TierBenefits {
                max_token_issuance: 2,
                fee_discount_percent: 15,
                can_issue_tokens: true,
                verified_badge: true,
            },
            Self::Silver => TierBenefits {
                max_token_issuance: 1,
                fee_discount_percent: 5,
                can_issue_tokens: true,
                verified_badge: false,
            },
            Self::Bronze => TierBenefits {
                max_token_issuance: 1,
                fee_discount_percent: 0,
                can_issue_tokens: false,
                verified_badge: false,
            },
            Self::Unverified => TierBenefits {
                max_token_issuance: 0,
                fee_discount_percent: 0,
                can_issue_tokens: false,
                verified_badge: false,
            },
        }
    }

    /// Get numeric ranking of tier (0 = lowest, 5 = highest)
    ///
    /// Useful for comparisons and sorting.
    pub fn tier_rank(&self) -> u8 {
        match self {
            Self::Unverified => 0,
            Self::Bronze => 1,
            Self::Silver => 2,
            Self::Gold => 3,
            Self::Platinum => 4,
            Self::Diamond => 5,
        }
    }

    /// Calculate tiers that are within reach (specified point distance)
    ///
    /// Returns all tiers that can be reached by improving score within the given distance.
    pub fn tiers_within_distance(&self, current_score: Decimal, distance: Decimal) -> Vec<Self> {
        let target_score = current_score + distance;
        let mut reachable = Vec::new();

        for tier in Self::all_variants() {
            if tier.min_score() <= target_score && tier.tier_rank() > self.tier_rank() {
                reachable.push(tier);
            }
        }

        reachable
    }

    /// Get tier color for UI display
    pub fn color_hex(&self) -> &'static str {
        match self {
            Self::Unverified => "#808080", // Gray
            Self::Bronze => "#CD7F32",     // Bronze
            Self::Silver => "#C0C0C0",     // Silver
            Self::Gold => "#FFD700",       // Gold
            Self::Platinum => "#E5E4E2",   // Platinum
            Self::Diamond => "#B9F2FF",    // Diamond blue
        }
    }

    /// Get tier emoji for quick visual display
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Unverified => "❓",
            Self::Bronze => "🥉",
            Self::Silver => "🥈",
            Self::Gold => "🥇",
            Self::Platinum => "💿",
            Self::Diamond => "💎",
        }
    }

    /// Get all tiers with their thresholds and benefits
    ///
    /// Useful for generating tier comparison tables or documentation.
    pub fn all_with_info() -> Vec<TierInfo> {
        Self::all_variants()
            .into_iter()
            .map(|tier| TierInfo {
                tier,
                min_score: tier.min_score(),
                max_score: tier.max_score(),
                benefits: tier.benefits(),
            })
            .collect()
    }
}

/// Benefits unlocked at a specific reputation tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TierBenefits {
    /// Maximum number of tokens this tier may issue simultaneously.
    pub max_token_issuance: u32,
    /// Fee discount percentage (0–100) applied to platform fees.
    pub fee_discount_percent: u32,
    /// Whether this tier may issue new tokens.
    pub can_issue_tokens: bool,
    /// Whether this tier is shown with a verified badge.
    pub verified_badge: bool,
}

impl TierBenefits {
    /// Compare benefits with another tier
    ///
    /// Returns a comparison showing what changes between tiers.
    pub fn compare_with(&self, other: &TierBenefits) -> BenefitComparison {
        BenefitComparison {
            token_issuance_delta: self.max_token_issuance as i32 - other.max_token_issuance as i32,
            fee_discount_delta: self.fee_discount_percent as i32
                - other.fee_discount_percent as i32,
            can_issue_tokens_gained: self.can_issue_tokens && !other.can_issue_tokens,
            verified_badge_gained: self.verified_badge && !other.verified_badge,
        }
    }

    /// Calculate actual fee discount amount for a given fee
    pub fn calculate_fee_discount(&self, base_fee: Decimal) -> Decimal {
        use rust_decimal_macros::dec;
        base_fee * (Decimal::from(self.fee_discount_percent) / dec!(100))
    }

    /// Calculate actual fee after discount
    pub fn calculate_discounted_fee(&self, base_fee: Decimal) -> Decimal {
        base_fee - self.calculate_fee_discount(base_fee)
    }
}

impl fmt::Display for TierBenefits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Max tokens: {}, Discount: {}%, Can issue: {}, Verified: {}",
            self.max_token_issuance,
            self.fee_discount_percent,
            self.can_issue_tokens,
            self.verified_badge
        )
    }
}

/// Result of comparing two tier benefits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenefitComparison {
    /// Change in max token issuance (positive = gain, negative = loss)
    pub token_issuance_delta: i32,
    /// Change in fee discount percentage (positive = better discount)
    pub fee_discount_delta: i32,
    /// Whether token issuance capability was gained
    pub can_issue_tokens_gained: bool,
    /// Whether verified badge was gained
    pub verified_badge_gained: bool,
}

impl BenefitComparison {
    /// Check if this represents an overall improvement
    pub fn is_improvement(&self) -> bool {
        self.token_issuance_delta > 0
            || self.fee_discount_delta > 0
            || self.can_issue_tokens_gained
            || self.verified_badge_gained
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if self.token_issuance_delta != 0 {
            parts.push(format!(
                "Token issuance: {} {} to {}",
                if self.token_issuance_delta > 0 {
                    "increased"
                } else {
                    "decreased"
                },
                self.token_issuance_delta.abs(),
                if self.token_issuance_delta > 0 {
                    "more"
                } else {
                    "fewer"
                }
            ));
        }

        if self.fee_discount_delta != 0 {
            parts.push(format!(
                "Fee discount: {} by {}%",
                if self.fee_discount_delta > 0 {
                    "increased"
                } else {
                    "decreased"
                },
                self.fee_discount_delta.abs()
            ));
        }

        if self.can_issue_tokens_gained {
            parts.push("Gained token issuance capability".to_string());
        }

        if self.verified_badge_gained {
            parts.push("Earned verified badge".to_string());
        }

        if parts.is_empty() {
            "No changes in benefits".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Tier information with thresholds and benefits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierInfo {
    /// The tier
    pub tier: ReputationTier,
    /// Minimum score required
    pub min_score: Decimal,
    /// Maximum score for this tier
    pub max_score: Decimal,
    /// Benefits at this tier
    pub benefits: TierBenefits,
}

impl TierInfo {
    /// Calculate score needed to reach this tier from a given score
    pub fn points_needed_from(&self, current_score: Decimal) -> Decimal {
        if current_score >= self.min_score {
            use rust_decimal_macros::dec;
            dec!(0) // Already at or above this tier
        } else {
            self.min_score - current_score
        }
    }
}

/// Direction of tier change
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TierChangeDirection {
    /// User was promoted to a higher tier
    Promotion,
    /// User was demoted to a lower tier
    Demotion,
}

impl TierChangeDirection {
    /// Check if this is a positive tier change (promotion)
    pub fn is_positive(&self) -> bool {
        matches!(self, TierChangeDirection::Promotion)
    }

    /// Check if this is a negative tier change (demotion)
    pub fn is_negative(&self) -> bool {
        matches!(self, TierChangeDirection::Demotion)
    }
}

impl fmt::Display for TierChangeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Promotion => write!(f, "Promotion"),
            Self::Demotion => write!(f, "Demotion"),
        }
    }
}

impl std::str::FromStr for TierChangeDirection {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "promotion" => Ok(TierChangeDirection::Promotion),
            "demotion" => Ok(TierChangeDirection::Demotion),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid tier change direction: {}. Valid values: promotion, demotion",
                s
            ))),
        }
    }
}

/// Tier change event
#[derive(Debug, Clone, Serialize)]
pub struct TierChangeEvent {
    /// User whose tier changed
    pub user_id: Uuid,
    /// Previous tier
    pub previous_tier: ReputationTier,
    /// New tier
    pub new_tier: ReputationTier,
    /// Direction of change
    pub direction: TierChangeDirection,
    /// Previous score
    pub previous_score: Decimal,
    /// New score
    pub new_score: Decimal,
    /// Timestamp of the change
    pub changed_at: chrono::DateTime<chrono::Utc>,
}

impl TierChangeEvent {
    /// Create a new tier change event if tiers differ
    pub fn detect(user_id: Uuid, previous_score: Decimal, new_score: Decimal) -> Option<Self> {
        let previous_tier = ReputationTier::from_score(previous_score);
        let new_tier = ReputationTier::from_score(new_score);

        if previous_tier == new_tier {
            return None;
        }

        let direction = if new_tier > previous_tier {
            TierChangeDirection::Promotion
        } else {
            TierChangeDirection::Demotion
        };

        Some(Self {
            user_id,
            previous_tier,
            new_tier,
            direction,
            previous_score,
            new_score,
            changed_at: chrono::Utc::now(),
        })
    }

    /// Check if this is a promotion
    pub fn is_promotion(&self) -> bool {
        self.direction == TierChangeDirection::Promotion
    }

    /// Check if this is a demotion
    pub fn is_demotion(&self) -> bool {
        self.direction == TierChangeDirection::Demotion
    }

    /// Get human-readable message for the tier change
    pub fn message(&self) -> String {
        match self.direction {
            TierChangeDirection::Promotion => {
                format!(
                    "Congratulations! You've been promoted from {} to {} tier.",
                    self.previous_tier, self.new_tier
                )
            }
            TierChangeDirection::Demotion => {
                format!(
                    "Your tier has changed from {} to {}.",
                    self.previous_tier, self.new_tier
                )
            }
        }
    }

    /// Get benefits gained or lost
    pub fn benefits_changed(&self) -> BenefitsChange {
        let previous = self.previous_tier.benefits();
        let new = self.new_tier.benefits();

        BenefitsChange {
            token_issuance_delta: new.max_token_issuance as i32
                - previous.max_token_issuance as i32,
            fee_discount_delta: new.fee_discount_percent as i32
                - previous.fee_discount_percent as i32,
            gained_token_issuance: !previous.can_issue_tokens && new.can_issue_tokens,
            lost_token_issuance: previous.can_issue_tokens && !new.can_issue_tokens,
            gained_verified_badge: !previous.verified_badge && new.verified_badge,
            lost_verified_badge: previous.verified_badge && !new.verified_badge,
        }
    }
}

/// Summary of benefits changes due to tier change
#[derive(Debug, Clone, Serialize)]
pub struct BenefitsChange {
    /// Change in max token issuance count
    pub token_issuance_delta: i32,
    /// Change in fee discount percentage
    pub fee_discount_delta: i32,
    /// Whether token issuance was gained
    pub gained_token_issuance: bool,
    /// Whether token issuance was lost
    pub lost_token_issuance: bool,
    /// Whether verified badge was gained
    pub gained_verified_badge: bool,
    /// Whether verified badge was lost
    pub lost_verified_badge: bool,
}

impl BenefitsChange {
    /// Check if any benefits were gained
    pub fn has_gains(&self) -> bool {
        self.token_issuance_delta > 0
            || self.fee_discount_delta > 0
            || self.gained_token_issuance
            || self.gained_verified_badge
    }

    /// Check if any benefits were lost
    pub fn has_losses(&self) -> bool {
        self.token_issuance_delta < 0
            || self.fee_discount_delta < 0
            || self.lost_token_issuance
            || self.lost_verified_badge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_tier_from_score_boundaries() {
        // Test each tier boundary
        assert_eq!(
            ReputationTier::from_score(dec!(0)),
            ReputationTier::Unverified
        );
        assert_eq!(
            ReputationTier::from_score(dec!(199)),
            ReputationTier::Unverified
        );
        assert_eq!(
            ReputationTier::from_score(dec!(200)),
            ReputationTier::Bronze
        );
        assert_eq!(
            ReputationTier::from_score(dec!(399)),
            ReputationTier::Bronze
        );
        assert_eq!(
            ReputationTier::from_score(dec!(400)),
            ReputationTier::Silver
        );
        assert_eq!(
            ReputationTier::from_score(dec!(599)),
            ReputationTier::Silver
        );
        assert_eq!(ReputationTier::from_score(dec!(600)), ReputationTier::Gold);
        assert_eq!(ReputationTier::from_score(dec!(799)), ReputationTier::Gold);
        assert_eq!(
            ReputationTier::from_score(dec!(800)),
            ReputationTier::Platinum
        );
        assert_eq!(
            ReputationTier::from_score(dec!(899)),
            ReputationTier::Platinum
        );
        assert_eq!(
            ReputationTier::from_score(dec!(900)),
            ReputationTier::Diamond
        );
        assert_eq!(
            ReputationTier::from_score(dec!(1000)),
            ReputationTier::Diamond
        );
    }

    #[test]
    fn test_tier_ordering() {
        assert!(ReputationTier::Diamond > ReputationTier::Platinum);
        assert!(ReputationTier::Platinum > ReputationTier::Gold);
        assert!(ReputationTier::Gold > ReputationTier::Silver);
        assert!(ReputationTier::Silver > ReputationTier::Bronze);
        assert!(ReputationTier::Bronze > ReputationTier::Unverified);
    }

    #[test]
    fn test_tier_benefits_unverified() {
        let benefits = ReputationTier::Unverified.benefits();
        assert_eq!(benefits.max_token_issuance, 0);
        assert_eq!(benefits.fee_discount_percent, 0);
        assert!(!benefits.can_issue_tokens);
        assert!(!benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_bronze() {
        let benefits = ReputationTier::Bronze.benefits();
        assert_eq!(benefits.max_token_issuance, 1);
        assert_eq!(benefits.fee_discount_percent, 0);
        assert!(!benefits.can_issue_tokens);
        assert!(!benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_silver() {
        let benefits = ReputationTier::Silver.benefits();
        assert_eq!(benefits.max_token_issuance, 1);
        assert_eq!(benefits.fee_discount_percent, 5);
        assert!(benefits.can_issue_tokens);
        assert!(!benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_gold() {
        let benefits = ReputationTier::Gold.benefits();
        assert_eq!(benefits.max_token_issuance, 2);
        assert_eq!(benefits.fee_discount_percent, 15);
        assert!(benefits.can_issue_tokens);
        assert!(benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_platinum() {
        let benefits = ReputationTier::Platinum.benefits();
        assert_eq!(benefits.max_token_issuance, 3);
        assert_eq!(benefits.fee_discount_percent, 30);
        assert!(benefits.can_issue_tokens);
        assert!(benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_diamond() {
        let benefits = ReputationTier::Diamond.benefits();
        assert_eq!(benefits.max_token_issuance, 5);
        assert_eq!(benefits.fee_discount_percent, 50);
        assert!(benefits.can_issue_tokens);
        assert!(benefits.verified_badge);
    }

    #[test]
    fn test_tier_change_detection_no_change() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(450), dec!(480));
        assert!(event.is_none());
    }

    #[test]
    fn test_tier_change_detection_promotion() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(350), dec!(450)).unwrap();
        assert_eq!(event.previous_tier, ReputationTier::Bronze);
        assert_eq!(event.new_tier, ReputationTier::Silver);
        assert_eq!(event.direction, TierChangeDirection::Promotion);
        assert!(event.is_promotion());
        assert!(!event.is_demotion());
    }

    #[test]
    fn test_tier_change_detection_demotion() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(650), dec!(350)).unwrap();
        assert_eq!(event.previous_tier, ReputationTier::Gold);
        assert_eq!(event.new_tier, ReputationTier::Bronze);
        assert_eq!(event.direction, TierChangeDirection::Demotion);
        assert!(!event.is_promotion());
        assert!(event.is_demotion());
    }

    #[test]
    fn test_tier_change_message_promotion() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(350), dec!(650)).unwrap();
        let message = event.message();
        assert!(message.contains("Congratulations"));
        assert!(message.contains("Bronze"));
        assert!(message.contains("Gold"));
    }

    #[test]
    fn test_tier_change_message_demotion() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(650), dec!(350)).unwrap();
        let message = event.message();
        assert!(message.contains("tier has changed"));
        assert!(message.contains("Gold"));
        assert!(message.contains("Bronze"));
    }

    #[test]
    fn test_benefits_change_gains() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(350), dec!(650)).unwrap();
        let changes = event.benefits_changed();

        assert_eq!(changes.token_issuance_delta, 1); // 1 -> 2
        assert_eq!(changes.fee_discount_delta, 15); // 0 -> 15 (Bronze has 0, Gold has 15)
        assert!(changes.gained_token_issuance); // Bronze can't issue, Gold can -> gained
        assert!(!changes.lost_token_issuance);
        assert!(changes.gained_verified_badge);
        assert!(!changes.lost_verified_badge);
        assert!(changes.has_gains());
        assert!(!changes.has_losses());
    }

    #[test]
    fn test_benefits_change_losses() {
        let user_id = Uuid::new_v4();
        let event = TierChangeEvent::detect(user_id, dec!(850), dec!(150)).unwrap();
        let changes = event.benefits_changed();

        assert_eq!(changes.token_issuance_delta, -3); // 3 -> 0
        assert_eq!(changes.fee_discount_delta, -30); // 30 -> 0
        assert!(!changes.gained_token_issuance);
        assert!(changes.lost_token_issuance);
        assert!(!changes.gained_verified_badge);
        assert!(changes.lost_verified_badge);
        assert!(!changes.has_gains());
        assert!(changes.has_losses());
    }

    #[test]
    fn test_benefits_change_token_issuance_gain() {
        let user_id = Uuid::new_v4();
        // Bronze -> Silver: gain token issuance ability
        let event = TierChangeEvent::detect(user_id, dec!(250), dec!(450)).unwrap();
        let changes = event.benefits_changed();

        assert!(changes.gained_token_issuance);
        assert!(!changes.lost_token_issuance);
    }

    #[test]
    fn test_benefits_change_token_issuance_loss() {
        let user_id = Uuid::new_v4();
        // Silver -> Bronze: lose token issuance ability
        let event = TierChangeEvent::detect(user_id, dec!(450), dec!(250)).unwrap();
        let changes = event.benefits_changed();

        assert!(!changes.gained_token_issuance);
        assert!(changes.lost_token_issuance);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(ReputationTier::Unverified.to_string(), "Unverified");
        assert_eq!(ReputationTier::Bronze.to_string(), "Bronze");
        assert_eq!(ReputationTier::Silver.to_string(), "Silver");
        assert_eq!(ReputationTier::Gold.to_string(), "Gold");
        assert_eq!(ReputationTier::Platinum.to_string(), "Platinum");
        assert_eq!(ReputationTier::Diamond.to_string(), "Diamond");
    }

    #[test]
    fn test_tier_min_score() {
        assert_eq!(ReputationTier::Unverified.min_score(), dec!(0));
        assert_eq!(ReputationTier::Bronze.min_score(), dec!(200));
        assert_eq!(ReputationTier::Silver.min_score(), dec!(400));
        assert_eq!(ReputationTier::Gold.min_score(), dec!(600));
        assert_eq!(ReputationTier::Platinum.min_score(), dec!(800));
        assert_eq!(ReputationTier::Diamond.min_score(), dec!(900));
    }

    #[test]
    fn test_tier_max_score() {
        assert_eq!(ReputationTier::Unverified.max_score(), dec!(199));
        assert_eq!(ReputationTier::Bronze.max_score(), dec!(399));
        assert_eq!(ReputationTier::Silver.max_score(), dec!(599));
        assert_eq!(ReputationTier::Gold.max_score(), dec!(799));
        assert_eq!(ReputationTier::Platinum.max_score(), dec!(899));
        assert_eq!(ReputationTier::Diamond.max_score(), dec!(1000));
    }

    #[test]
    fn test_tier_score_range() {
        assert_eq!(ReputationTier::Silver.score_range(), (dec!(400), dec!(599)));
        assert_eq!(ReputationTier::Gold.score_range(), (dec!(600), dec!(799)));
    }

    #[test]
    fn test_tier_is_verified() {
        assert!(!ReputationTier::Unverified.is_verified());
        assert!(!ReputationTier::Bronze.is_verified());
        assert!(ReputationTier::Silver.is_verified());
        assert!(ReputationTier::Gold.is_verified());
        assert!(ReputationTier::Platinum.is_verified());
        assert!(ReputationTier::Diamond.is_verified());
    }

    #[test]
    fn test_tier_next_tier() {
        assert_eq!(
            ReputationTier::Unverified.next_tier(),
            Some(ReputationTier::Bronze)
        );
        assert_eq!(
            ReputationTier::Bronze.next_tier(),
            Some(ReputationTier::Silver)
        );
        assert_eq!(
            ReputationTier::Silver.next_tier(),
            Some(ReputationTier::Gold)
        );
        assert_eq!(
            ReputationTier::Gold.next_tier(),
            Some(ReputationTier::Platinum)
        );
        assert_eq!(
            ReputationTier::Platinum.next_tier(),
            Some(ReputationTier::Diamond)
        );
        assert_eq!(ReputationTier::Diamond.next_tier(), None);
    }

    #[test]
    fn test_tier_previous_tier() {
        assert_eq!(ReputationTier::Unverified.previous_tier(), None);
        assert_eq!(
            ReputationTier::Bronze.previous_tier(),
            Some(ReputationTier::Unverified)
        );
        assert_eq!(
            ReputationTier::Silver.previous_tier(),
            Some(ReputationTier::Bronze)
        );
        assert_eq!(
            ReputationTier::Gold.previous_tier(),
            Some(ReputationTier::Silver)
        );
        assert_eq!(
            ReputationTier::Platinum.previous_tier(),
            Some(ReputationTier::Gold)
        );
        assert_eq!(
            ReputationTier::Diamond.previous_tier(),
            Some(ReputationTier::Platinum)
        );
    }

    #[test]
    fn test_tier_change_direction_display() {
        assert_eq!(TierChangeDirection::Promotion.to_string(), "Promotion");
        assert_eq!(TierChangeDirection::Demotion.to_string(), "Demotion");
    }

    #[test]
    fn test_tier_change_direction_from_str_valid() {
        assert_eq!(
            "promotion".parse::<TierChangeDirection>().unwrap(),
            TierChangeDirection::Promotion
        );
        assert_eq!(
            "demotion".parse::<TierChangeDirection>().unwrap(),
            TierChangeDirection::Demotion
        );
    }

    #[test]
    fn test_tier_change_direction_from_str_case_insensitive() {
        assert_eq!(
            "PROMOTION".parse::<TierChangeDirection>().unwrap(),
            TierChangeDirection::Promotion
        );
        assert_eq!(
            "Demotion".parse::<TierChangeDirection>().unwrap(),
            TierChangeDirection::Demotion
        );
    }

    #[test]
    fn test_tier_change_direction_from_str_invalid() {
        assert!("invalid".parse::<TierChangeDirection>().is_err());
        assert!("upgrade".parse::<TierChangeDirection>().is_err());
        assert!("".parse::<TierChangeDirection>().is_err());
    }

    // ========================================================================
    // Property-Based Tests
    // ========================================================================

    mod proptest_tier {
        use super::*;
        use proptest::prelude::*;
        use rust_decimal::Decimal;

        proptest! {
            /// Property: Any score should map to exactly one tier
            #[test]
            fn prop_score_maps_to_tier(score in 0i64..=1000i64) {
                let tier = ReputationTier::from_score(Decimal::new(score, 0));
                // Should not panic and should return a valid tier
                prop_assert!(matches!(tier, ReputationTier::Unverified | ReputationTier::Bronze | ReputationTier::Silver | ReputationTier::Gold | ReputationTier::Platinum | ReputationTier::Diamond));
            }

            /// Property: Score at tier minimum should map to that tier
            #[test]
            fn prop_tier_min_boundary(tier_index in 0usize..=5usize) {
                let tiers = [
                    ReputationTier::Unverified,
                    ReputationTier::Bronze,
                    ReputationTier::Silver,
                    ReputationTier::Gold,
                    ReputationTier::Platinum,
                    ReputationTier::Diamond,
                ];

                if tier_index < tiers.len() {
                    let tier = tiers[tier_index];
                    let min_score = tier.min_score();
                    let computed_tier = ReputationTier::from_score(min_score);
                    prop_assert_eq!(computed_tier, tier);
                }
            }

            /// Property: Score at tier maximum should map to that tier
            #[test]
            fn prop_tier_max_boundary(tier_index in 0usize..=5usize) {
                let tiers = [
                    ReputationTier::Unverified,
                    ReputationTier::Bronze,
                    ReputationTier::Silver,
                    ReputationTier::Gold,
                    ReputationTier::Platinum,
                    ReputationTier::Diamond,
                ];

                if tier_index < tiers.len() {
                    let tier = tiers[tier_index];
                    let max_score = tier.max_score();
                    let computed_tier = ReputationTier::from_score(max_score);
                    prop_assert_eq!(computed_tier, tier);
                }
            }

            /// Property: Tier ranges should be continuous (no gaps)
            #[test]
            fn prop_tier_continuity(score in 0i64..=999i64) {
                let tier1 = ReputationTier::from_score(Decimal::new(score, 0));
                let tier2 = ReputationTier::from_score(Decimal::new(score + 1, 0));

                // Either same tier or adjacent tier
                let tier1_ord = tier1 as i32;
                let tier2_ord = tier2 as i32;
                let diff = (tier2_ord - tier1_ord).abs();
                prop_assert!(diff <= 1);
            }

            /// Property: Higher scores should never result in lower tiers
            #[test]
            fn prop_tier_monotonicity(score1 in 0i64..=500i64, score2 in 500i64..=1000i64) {
                let tier1 = ReputationTier::from_score(Decimal::new(score1, 0));
                let tier2 = ReputationTier::from_score(Decimal::new(score2, 0));

                // tier2 should be >= tier1
                prop_assert!(tier2 >= tier1);
            }

            /// Property: next_tier().previous_tier() should return the original tier
            #[test]
            fn prop_tier_transition_reversibility(tier_index in 0usize..=4usize) {
                let tiers = [
                    ReputationTier::Unverified,
                    ReputationTier::Bronze,
                    ReputationTier::Silver,
                    ReputationTier::Gold,
                    ReputationTier::Platinum,
                ];

                if tier_index < tiers.len() {
                    let tier = tiers[tier_index];
                    if let Some(next) = tier.next_tier() {
                        if let Some(prev) = next.previous_tier() {
                            prop_assert_eq!(prev, tier);
                        }
                    }
                }
            }

            /// Property: Tier range max should be next tier's min - 1 (except Diamond)
            #[test]
            fn prop_tier_range_adjacency(tier_index in 0usize..=4usize) {
                let tiers = [
                    ReputationTier::Unverified,
                    ReputationTier::Bronze,
                    ReputationTier::Silver,
                    ReputationTier::Gold,
                    ReputationTier::Platinum,
                ];

                if tier_index < tiers.len() {
                    let tier = tiers[tier_index];
                    if let Some(next_tier) = tier.next_tier() {
                        let current_max = tier.max_score();
                        let next_min = next_tier.min_score();
                        // next_min should be current_max + 1
                        prop_assert_eq!(next_min, current_max + Decimal::new(1, 0));
                    }
                }
            }
        }
    }

    #[test]
    fn test_tier_change_direction_is_positive() {
        assert!(TierChangeDirection::Promotion.is_positive());
        assert!(!TierChangeDirection::Demotion.is_positive());
    }

    #[test]
    fn test_tier_change_direction_is_negative() {
        assert!(TierChangeDirection::Demotion.is_negative());
        assert!(!TierChangeDirection::Promotion.is_negative());
    }

    #[test]
    fn test_tier_change_direction_opposite() {
        // Test that positive and negative are opposites
        assert_eq!(
            TierChangeDirection::Promotion.is_positive(),
            !TierChangeDirection::Promotion.is_negative()
        );
        assert_eq!(
            TierChangeDirection::Demotion.is_positive(),
            !TierChangeDirection::Demotion.is_negative()
        );
    }

    // Phase 17 tests - New functionality
    #[test]
    fn test_tier_benefits_default() {
        let default_benefits = TierBenefits::default();
        assert_eq!(default_benefits.max_token_issuance, 0);
        assert_eq!(default_benefits.fee_discount_percent, 0);
        assert!(!default_benefits.can_issue_tokens);
        assert!(!default_benefits.verified_badge);
    }

    #[test]
    fn test_tier_benefits_display() {
        let benefits = TierBenefits {
            max_token_issuance: 3,
            fee_discount_percent: 30,
            can_issue_tokens: true,
            verified_badge: true,
        };

        let display_string = format!("{}", benefits);
        assert!(display_string.contains("Max tokens: 3"));
        assert!(display_string.contains("Discount: 30%"));
        assert!(display_string.contains("Can issue: true"));
        assert!(display_string.contains("Verified: true"));
    }

    #[test]
    fn test_tier_benefits_equality() {
        let benefits1 = TierBenefits {
            max_token_issuance: 2,
            fee_discount_percent: 15,
            can_issue_tokens: true,
            verified_badge: true,
        };

        let benefits2 = TierBenefits {
            max_token_issuance: 2,
            fee_discount_percent: 15,
            can_issue_tokens: true,
            verified_badge: true,
        };

        let benefits3 = TierBenefits {
            max_token_issuance: 3,
            fee_discount_percent: 30,
            can_issue_tokens: true,
            verified_badge: true,
        };

        assert_eq!(benefits1, benefits2);
        assert_ne!(benefits1, benefits3);
    }

    #[test]
    fn test_tier_benefits_clone() {
        let benefits = TierBenefits {
            max_token_issuance: 5,
            fee_discount_percent: 50,
            can_issue_tokens: true,
            verified_badge: true,
        };

        let cloned = benefits.clone();
        assert_eq!(benefits, cloned);
    }

    // Phase 18 tests - New functionality
    #[test]
    fn test_reputation_tier_all_variants() {
        let variants = ReputationTier::all_variants();
        assert_eq!(variants.len(), 6);
        assert_eq!(variants[0], ReputationTier::Unverified);
        assert_eq!(variants[1], ReputationTier::Bronze);
        assert_eq!(variants[2], ReputationTier::Silver);
        assert_eq!(variants[3], ReputationTier::Gold);
        assert_eq!(variants[4], ReputationTier::Platinum);
        assert_eq!(variants[5], ReputationTier::Diamond);
    }

    #[test]
    fn test_reputation_tier_as_str() {
        assert_eq!(ReputationTier::Unverified.as_str(), "unverified");
        assert_eq!(ReputationTier::Bronze.as_str(), "bronze");
        assert_eq!(ReputationTier::Silver.as_str(), "silver");
        assert_eq!(ReputationTier::Gold.as_str(), "gold");
        assert_eq!(ReputationTier::Platinum.as_str(), "platinum");
        assert_eq!(ReputationTier::Diamond.as_str(), "diamond");
    }

    // Phase 23 Tests

    #[test]
    fn test_tier_rank() {
        assert_eq!(ReputationTier::Unverified.tier_rank(), 0);
        assert_eq!(ReputationTier::Bronze.tier_rank(), 1);
        assert_eq!(ReputationTier::Silver.tier_rank(), 2);
        assert_eq!(ReputationTier::Gold.tier_rank(), 3);
        assert_eq!(ReputationTier::Platinum.tier_rank(), 4);
        assert_eq!(ReputationTier::Diamond.tier_rank(), 5);
    }

    #[test]
    fn test_tiers_within_distance() {
        use rust_decimal_macros::dec;

        let bronze = ReputationTier::Bronze;
        let current_score = dec!(250); // In Bronze tier

        // Can reach Silver and Gold with 400 points
        let reachable = bronze.tiers_within_distance(current_score, dec!(400));
        assert_eq!(reachable.len(), 2);
        assert!(reachable.contains(&ReputationTier::Silver));
        assert!(reachable.contains(&ReputationTier::Gold));

        // Can only reach Silver with 200 points
        let reachable = bronze.tiers_within_distance(current_score, dec!(200));
        assert_eq!(reachable.len(), 1);
        assert_eq!(reachable[0], ReputationTier::Silver);

        // Cannot reach any higher tier with 100 points
        let reachable = bronze.tiers_within_distance(current_score, dec!(100));
        assert_eq!(reachable.len(), 0);
    }

    #[test]
    fn test_tier_colors() {
        assert_eq!(ReputationTier::Unverified.color_hex(), "#808080");
        assert_eq!(ReputationTier::Bronze.color_hex(), "#CD7F32");
        assert_eq!(ReputationTier::Silver.color_hex(), "#C0C0C0");
        assert_eq!(ReputationTier::Gold.color_hex(), "#FFD700");
        assert_eq!(ReputationTier::Platinum.color_hex(), "#E5E4E2");
        assert_eq!(ReputationTier::Diamond.color_hex(), "#B9F2FF");
    }

    #[test]
    fn test_tier_emojis() {
        assert_eq!(ReputationTier::Unverified.emoji(), "❓");
        assert_eq!(ReputationTier::Bronze.emoji(), "🥉");
        assert_eq!(ReputationTier::Silver.emoji(), "🥈");
        assert_eq!(ReputationTier::Gold.emoji(), "🥇");
        assert_eq!(ReputationTier::Platinum.emoji(), "💿");
        assert_eq!(ReputationTier::Diamond.emoji(), "💎");
    }

    #[test]
    fn test_all_with_info() {
        let tier_infos = ReputationTier::all_with_info();
        assert_eq!(tier_infos.len(), 6);

        use rust_decimal_macros::dec;
        assert_eq!(tier_infos[0].tier, ReputationTier::Unverified);
        assert_eq!(tier_infos[0].min_score, dec!(0));
        assert_eq!(tier_infos[5].tier, ReputationTier::Diamond);
        assert_eq!(tier_infos[5].max_score, dec!(1000));
    }

    #[test]
    fn test_tier_info_points_needed() {
        use rust_decimal_macros::dec;

        let tier_infos = ReputationTier::all_with_info();
        let silver_info = &tier_infos[2]; // Silver tier

        // Need 150 points to reach Silver from 250
        assert_eq!(silver_info.points_needed_from(dec!(250)), dec!(150));

        // Already at Silver, need 0 points
        assert_eq!(silver_info.points_needed_from(dec!(450)), dec!(0));

        // Already above Silver, need 0 points
        assert_eq!(silver_info.points_needed_from(dec!(700)), dec!(0));
    }

    #[test]
    fn test_tier_benefits_compare() {
        let gold_benefits = ReputationTier::Gold.benefits();
        let silver_benefits = ReputationTier::Silver.benefits();

        let comparison = gold_benefits.compare_with(&silver_benefits);
        assert_eq!(comparison.token_issuance_delta, 1); // 2 - 1
        assert_eq!(comparison.fee_discount_delta, 10); // 15% - 5%
        assert!(comparison.is_improvement());
    }

    #[test]
    fn test_tier_benefits_fee_calculations() {
        use rust_decimal_macros::dec;

        let gold_benefits = ReputationTier::Gold.benefits();
        let base_fee = dec!(100);

        // Gold has 15% discount
        let discount = gold_benefits.calculate_fee_discount(base_fee);
        assert_eq!(discount, dec!(15));

        let discounted_fee = gold_benefits.calculate_discounted_fee(base_fee);
        assert_eq!(discounted_fee, dec!(85));
    }

    #[test]
    fn test_benefit_comparison_summary() {
        let gold_benefits = ReputationTier::Gold.benefits();
        let bronze_benefits = ReputationTier::Bronze.benefits();

        let comparison = gold_benefits.compare_with(&bronze_benefits);
        let summary = comparison.summary();

        // Gold vs Bronze: gains token issuance capability and fee discount
        assert!(summary.contains("Token issuance"));
        assert!(summary.contains("Gained token issuance capability"));
    }
}
