//! Fee calculation for trades

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Fee schedule for the platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeSchedule {
    /// Platform fee rate (e.g., 0.025 = 2.5%)
    pub platform_fee_rate: Decimal,
    /// Issuer royalty rate (e.g., 0.005 = 0.5%)
    pub issuer_royalty_rate: Decimal,
}

impl Default for FeeSchedule {
    fn default() -> Self {
        Self {
            platform_fee_rate: dec!(0.025),   // 2.5%
            issuer_royalty_rate: dec!(0.005), // 0.5%
        }
    }
}

impl FeeSchedule {
    /// Create a new fee schedule with custom rates
    pub fn new(platform_fee_rate: Decimal, issuer_royalty_rate: Decimal) -> Self {
        Self {
            platform_fee_rate,
            issuer_royalty_rate,
        }
    }

    /// Get total fee rate
    pub fn total_rate(&self) -> Decimal {
        self.platform_fee_rate + self.issuer_royalty_rate
    }

    /// Calculate fees for a given trade amount
    pub fn calculate(&self, trade_amount_btc: Decimal) -> FeeBreakdown {
        let platform_fee = trade_amount_btc * self.platform_fee_rate;
        let issuer_royalty = trade_amount_btc * self.issuer_royalty_rate;
        let total_fees = platform_fee + issuer_royalty;
        let net_amount = trade_amount_btc - total_fees;

        FeeBreakdown {
            trade_amount_btc,
            platform_fee_btc: platform_fee,
            issuer_royalty_btc: issuer_royalty,
            total_fees_btc: total_fees,
            net_amount_btc: net_amount,
        }
    }

    /// Apply discount based on $KACCY staked amount
    pub fn with_discount(&self, discount_percent: Decimal) -> Self {
        let discount = discount_percent.min(dec!(50)) / dec!(100); // Max 50% discount
        Self {
            platform_fee_rate: self.platform_fee_rate * (dec!(1) - discount),
            issuer_royalty_rate: self.issuer_royalty_rate, // Royalty not discounted
        }
    }
}

/// Breakdown of fees for a trade
#[derive(Debug, Clone, Serialize)]
pub struct FeeBreakdown {
    /// Original trade amount in BTC
    pub trade_amount_btc: Decimal,
    /// Platform fee in BTC
    pub platform_fee_btc: Decimal,
    /// Issuer royalty in BTC
    pub issuer_royalty_btc: Decimal,
    /// Total fees in BTC
    pub total_fees_btc: Decimal,
    /// Net amount after fees in BTC
    pub net_amount_btc: Decimal,
}

impl FeeBreakdown {
    /// Calculate fee percentage
    pub fn fee_percentage(&self) -> Decimal {
        if self.trade_amount_btc == dec!(0) {
            return dec!(0);
        }
        (self.total_fees_btc / self.trade_amount_btc) * dec!(100)
    }
}

/// Fee discount tiers based on reputation
pub fn reputation_discount(reputation_score: Decimal) -> Decimal {
    // Higher reputation = higher discount
    if reputation_score >= dec!(900) {
        dec!(50) // Diamond: 50% discount
    } else if reputation_score >= dec!(800) {
        dec!(30) // Platinum: 30% discount
    } else if reputation_score >= dec!(600) {
        dec!(15) // Gold: 15% discount
    } else if reputation_score >= dec!(400) {
        dec!(5) // Silver: 5% discount
    } else {
        dec!(0) // Bronze/Unverified: No discount
    }
}

/// Volume-based fee discount tiers
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VolumeTier {
    /// Minimum 30-day trading volume in BTC
    pub min_volume_btc: Decimal,
    /// Fee discount percentage
    pub discount_percent: Decimal,
    /// Maker fee discount (additional on top of base discount)
    pub maker_bonus_percent: Decimal,
}

impl VolumeTier {
    /// Define standard volume tiers
    pub fn standard_tiers() -> Vec<VolumeTier> {
        vec![
            VolumeTier {
                min_volume_btc: dec!(0),
                discount_percent: dec!(0),
                maker_bonus_percent: dec!(0),
            },
            VolumeTier {
                min_volume_btc: dec!(1),
                discount_percent: dec!(5),
                maker_bonus_percent: dec!(2),
            },
            VolumeTier {
                min_volume_btc: dec!(10),
                discount_percent: dec!(10),
                maker_bonus_percent: dec!(5),
            },
            VolumeTier {
                min_volume_btc: dec!(50),
                discount_percent: dec!(15),
                maker_bonus_percent: dec!(7),
            },
            VolumeTier {
                min_volume_btc: dec!(100),
                discount_percent: dec!(20),
                maker_bonus_percent: dec!(10),
            },
            VolumeTier {
                min_volume_btc: dec!(500),
                discount_percent: dec!(25),
                maker_bonus_percent: dec!(15),
            },
        ]
    }

    /// Get the appropriate tier for a given volume
    pub fn get_tier_for_volume(volume_btc: Decimal, tiers: &[VolumeTier]) -> VolumeTier {
        tiers
            .iter()
            .rev()
            .find(|tier| volume_btc >= tier.min_volume_btc)
            .copied()
            .unwrap_or(VolumeTier {
                min_volume_btc: dec!(0),
                discount_percent: dec!(0),
                maker_bonus_percent: dec!(0),
            })
    }
}

/// Fee calculator with volume and maker/taker differentiation
#[derive(Debug, Clone)]
pub struct AdvancedFeeCalculator {
    pub base_schedule: FeeSchedule,
    pub volume_tiers: Vec<VolumeTier>,
}

impl AdvancedFeeCalculator {
    pub fn new(base_schedule: FeeSchedule) -> Self {
        Self {
            base_schedule,
            volume_tiers: VolumeTier::standard_tiers(),
        }
    }

    /// Calculate fees with volume discount
    pub fn calculate_with_volume(
        &self,
        trade_amount_btc: Decimal,
        thirty_day_volume_btc: Decimal,
        is_maker: bool,
    ) -> FeeBreakdown {
        let tier = VolumeTier::get_tier_for_volume(thirty_day_volume_btc, &self.volume_tiers);

        let mut discount = tier.discount_percent;
        if is_maker {
            discount += tier.maker_bonus_percent;
        }

        // Apply discount (max 40% from volume alone)
        discount = discount.min(dec!(40));

        self.base_schedule
            .with_discount(discount)
            .calculate(trade_amount_btc)
    }

    /// Calculate combined fees with reputation and volume discounts
    pub fn calculate_with_all_discounts(
        &self,
        trade_amount_btc: Decimal,
        thirty_day_volume_btc: Decimal,
        reputation_score: Decimal,
        is_maker: bool,
    ) -> FeeBreakdown {
        let volume_tier =
            VolumeTier::get_tier_for_volume(thirty_day_volume_btc, &self.volume_tiers);
        let reputation_discount_pct = reputation_discount(reputation_score);

        let mut total_discount = volume_tier.discount_percent + reputation_discount_pct;
        if is_maker {
            total_discount += volume_tier.maker_bonus_percent;
        }

        // Max total discount: 60%
        total_discount = total_discount.min(dec!(60));

        self.base_schedule
            .with_discount(total_discount)
            .calculate(trade_amount_btc)
    }

    /// Get current tier information for a user
    pub fn get_user_tier(&self, thirty_day_volume_btc: Decimal) -> UserTierInfo {
        let current_tier =
            VolumeTier::get_tier_for_volume(thirty_day_volume_btc, &self.volume_tiers);

        // Find next tier
        let next_tier = self
            .volume_tiers
            .iter()
            .find(|tier| tier.min_volume_btc > thirty_day_volume_btc)
            .copied();

        UserTierInfo {
            current_volume_btc: thirty_day_volume_btc,
            current_tier,
            next_tier,
            volume_to_next_tier: next_tier.map(|t| t.min_volume_btc - thirty_day_volume_btc),
        }
    }
}

/// User tier information
#[derive(Debug, Clone, Serialize)]
pub struct UserTierInfo {
    pub current_volume_btc: Decimal,
    pub current_tier: VolumeTier,
    pub next_tier: Option<VolumeTier>,
    pub volume_to_next_tier: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_tier_selection() {
        let tiers = VolumeTier::standard_tiers();

        let tier0 = VolumeTier::get_tier_for_volume(dec!(0.5), &tiers);
        assert_eq!(tier0.discount_percent, dec!(0));

        let tier1 = VolumeTier::get_tier_for_volume(dec!(5), &tiers);
        assert_eq!(tier1.discount_percent, dec!(5));

        let tier2 = VolumeTier::get_tier_for_volume(dec!(50), &tiers);
        assert_eq!(tier2.discount_percent, dec!(15));

        let tier3 = VolumeTier::get_tier_for_volume(dec!(1000), &tiers);
        assert_eq!(tier3.discount_percent, dec!(25)); // Highest tier
    }

    #[test]
    fn test_maker_bonus() {
        let calculator = AdvancedFeeCalculator::new(FeeSchedule::default());

        // Same volume, different maker/taker status
        let taker_fees = calculator.calculate_with_volume(dec!(1), dec!(10), false);
        let maker_fees = calculator.calculate_with_volume(dec!(1), dec!(10), true);

        // Maker should pay less
        assert!(maker_fees.total_fees_btc < taker_fees.total_fees_btc);
    }

    #[test]
    fn test_combined_discounts_capped() {
        let calculator = AdvancedFeeCalculator::new(FeeSchedule::default());

        // High volume + high reputation + maker status
        let fees = calculator.calculate_with_all_discounts(
            dec!(1),
            dec!(1000), // High volume
            dec!(950),  // High reputation
            true,       // Maker
        );

        // Total discount should be capped at 60%
        let base_fee = dec!(1) * dec!(0.025);
        let max_discount = base_fee * dec!(0.60);
        let min_fee = base_fee - max_discount;

        assert!(fees.platform_fee_btc >= min_fee * dec!(0.99)); // Allow small rounding difference
    }

    #[test]
    fn test_user_tier_info() {
        let calculator = AdvancedFeeCalculator::new(FeeSchedule::default());

        let tier_info = calculator.get_user_tier(dec!(7));

        assert_eq!(tier_info.current_tier.discount_percent, dec!(5));
        assert!(tier_info.next_tier.is_some());
        assert_eq!(tier_info.next_tier.unwrap().min_volume_btc, dec!(10));
        assert_eq!(tier_info.volume_to_next_tier.unwrap(), dec!(3));
    }
}
