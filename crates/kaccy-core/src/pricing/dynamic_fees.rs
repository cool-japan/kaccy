//! Advanced dynamic fee models with time-based, liquidity-based,
//! and conditional fee structures

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Time of day for fee scheduling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDay {
    /// Peak trading hours (high fees)
    Peak,
    /// Regular trading hours (normal fees)
    Regular,
    /// Off-peak hours (reduced fees)
    OffPeak,
}

impl TimeOfDay {
    /// Determine time of day from UTC hour
    pub fn from_hour(hour: u32) -> Self {
        match hour {
            // Peak hours: 13:00-21:00 UTC (corresponds to major market hours)
            13..=20 => Self::Peak,
            // Off-peak: 0:00-6:00 UTC
            0..=5 => Self::OffPeak,
            // Regular: everything else
            _ => Self::Regular,
        }
    }

    /// Get fee multiplier for this time of day
    pub fn fee_multiplier(&self) -> Decimal {
        match self {
            Self::Peak => dec!(1.2),    // 20% higher fees
            Self::Regular => dec!(1.0), // Normal fees
            Self::OffPeak => dec!(0.8), // 20% lower fees
        }
    }
}

/// Liquidity level for fee adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidityLevel {
    /// Very high liquidity
    VeryHigh,
    /// High liquidity
    High,
    /// Normal liquidity
    Normal,
    /// Low liquidity
    Low,
    /// Very low liquidity
    VeryLow,
}

impl LiquidityLevel {
    /// Determine liquidity level from pool depth
    pub fn from_depth(depth: Decimal, reference_depth: Decimal) -> Self {
        let ratio = if reference_depth.is_zero() {
            dec!(1.0)
        } else {
            depth / reference_depth
        };

        if ratio >= dec!(2.0) {
            Self::VeryHigh
        } else if ratio >= dec!(1.5) {
            Self::High
        } else if ratio >= dec!(0.75) {
            Self::Normal
        } else if ratio >= dec!(0.5) {
            Self::Low
        } else {
            Self::VeryLow
        }
    }

    /// Get fee multiplier for this liquidity level
    pub fn fee_multiplier(&self) -> Decimal {
        match self {
            Self::VeryHigh => dec!(0.7), // 30% lower fees
            Self::High => dec!(0.85),    // 15% lower fees
            Self::Normal => dec!(1.0),   // Normal fees
            Self::Low => dec!(1.2),      // 20% higher fees
            Self::VeryLow => dec!(1.5),  // 50% higher fees
        }
    }
}

/// Token-specific fee schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenFeeSchedule {
    /// Token ID
    pub token_id: Uuid,
    /// Base fee rate (e.g., 0.0025 = 0.25%)
    pub base_fee_rate: Decimal,
    /// Maker fee rate
    pub maker_fee_rate: Decimal,
    /// Taker fee rate
    pub taker_fee_rate: Decimal,
    /// Minimum fee amount
    pub min_fee: Decimal,
    /// Maximum fee amount (None = no cap)
    pub max_fee: Option<Decimal>,
    /// Whether this token has premium features (lower fees)
    pub is_premium: bool,
}

impl TokenFeeSchedule {
    /// Create a standard fee schedule
    pub fn standard(token_id: Uuid) -> Self {
        Self {
            token_id,
            base_fee_rate: dec!(0.0025), // 0.25%
            maker_fee_rate: dec!(0.002), // 0.20%
            taker_fee_rate: dec!(0.003), // 0.30%
            min_fee: dec!(0.0001),
            max_fee: None,
            is_premium: false,
        }
    }

    /// Create a premium fee schedule (lower fees)
    pub fn premium(token_id: Uuid) -> Self {
        Self {
            token_id,
            base_fee_rate: dec!(0.002),   // 0.20%
            maker_fee_rate: dec!(0.0015), // 0.15%
            taker_fee_rate: dec!(0.0025), // 0.25%
            min_fee: dec!(0.0001),
            max_fee: None,
            is_premium: true,
        }
    }
}

/// Conditional fee rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalFeeRule {
    /// Rule ID
    pub id: Uuid,
    /// Rule name
    pub name: String,
    /// Condition description
    pub condition: FeeCondition,
    /// Fee adjustment when condition is met
    pub adjustment: FeeAdjustment,
    /// Whether the rule is active
    pub active: bool,
}

/// Fee condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeeCondition {
    /// Volume threshold (in USD)
    VolumeAbove(Decimal),
    /// Trade size threshold
    TradeSizeAbove(Decimal),
    /// User holding specific token
    HoldsToken { token_id: Uuid, min_amount: Decimal },
    /// Time-based condition
    TimeRange { start_hour: u32, end_hour: u32 },
    /// User is market maker
    IsMarketMaker,
    /// Specific user
    UserId(Uuid),
}

/// Fee adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeeAdjustment {
    /// Multiply fee by this factor
    Multiply(Decimal),
    /// Add this amount to fee
    Add(Decimal),
    /// Subtract this amount from fee
    Subtract(Decimal),
    /// Set fee to this exact amount
    SetTo(Decimal),
    /// Set fee to percentage
    SetPercentage(Decimal),
}

/// Market maker fee rebate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerRebate {
    /// Market maker user ID
    pub user_id: Uuid,
    /// Rebate rate (e.g., 0.0001 = 0.01% rebate)
    pub rebate_rate: Decimal,
    /// Minimum monthly volume to qualify
    pub min_monthly_volume: Decimal,
    /// Current month volume
    pub current_volume: Decimal,
    /// Whether currently qualified
    pub qualified: bool,
}

impl MarketMakerRebate {
    /// Create a new rebate
    pub fn new(user_id: Uuid, rebate_rate: Decimal, min_monthly_volume: Decimal) -> Self {
        Self {
            user_id,
            rebate_rate,
            min_monthly_volume,
            current_volume: Decimal::ZERO,
            qualified: false,
        }
    }

    /// Update volume and check qualification
    pub fn update_volume(&mut self, volume: Decimal) {
        self.current_volume += volume;
        self.qualified = self.current_volume >= self.min_monthly_volume;
    }

    /// Reset monthly volume
    pub fn reset_monthly(&mut self) {
        self.current_volume = Decimal::ZERO;
        self.qualified = false;
    }

    /// Calculate rebate amount
    pub fn calculate_rebate(&self, fee_paid: Decimal) -> Decimal {
        if self.qualified {
            fee_paid * self.rebate_rate
        } else {
            Decimal::ZERO
        }
    }
}

/// Dynamic fee calculator
pub struct DynamicFeeCalculator {
    /// Token-specific fee schedules
    token_schedules: HashMap<Uuid, TokenFeeSchedule>,
    /// Conditional fee rules
    rules: Vec<ConditionalFeeRule>,
    /// Market maker rebates
    rebates: HashMap<Uuid, MarketMakerRebate>,
    /// Reference liquidity depth for calculations
    reference_depth: Decimal,
}

impl DynamicFeeCalculator {
    /// Create a new dynamic fee calculator
    pub fn new(reference_depth: Decimal) -> Self {
        Self {
            token_schedules: HashMap::new(),
            rules: Vec::new(),
            rebates: HashMap::new(),
            reference_depth,
        }
    }

    /// Add a token fee schedule
    pub fn add_token_schedule(&mut self, schedule: TokenFeeSchedule) {
        self.token_schedules.insert(schedule.token_id, schedule);
    }

    /// Add a conditional rule
    pub fn add_rule(&mut self, rule: ConditionalFeeRule) {
        self.rules.push(rule);
    }

    /// Add a market maker rebate
    pub fn add_rebate(&mut self, rebate: MarketMakerRebate) {
        self.rebates.insert(rebate.user_id, rebate);
    }

    /// Calculate fee for a trade
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_fee(
        &self,
        token_id: Uuid,
        user_id: Uuid,
        trade_amount: Decimal,
        is_maker: bool,
        liquidity_depth: Decimal,
        timestamp: DateTime<Utc>,
        user_holdings: &HashMap<Uuid, Decimal>,
    ) -> Decimal {
        // Get base fee rate
        let schedule = self
            .token_schedules
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| TokenFeeSchedule::standard(token_id));

        let mut base_fee = if is_maker {
            schedule.maker_fee_rate
        } else {
            schedule.taker_fee_rate
        };

        // Apply time-based adjustment
        let time_of_day = TimeOfDay::from_hour(timestamp.hour());
        base_fee *= time_of_day.fee_multiplier();

        // Apply liquidity-based adjustment
        let liquidity_level = LiquidityLevel::from_depth(liquidity_depth, self.reference_depth);
        base_fee *= liquidity_level.fee_multiplier();

        // Apply conditional rules
        for rule in &self.rules {
            if !rule.active {
                continue;
            }

            let condition_met = match &rule.condition {
                FeeCondition::VolumeAbove(threshold) => trade_amount >= *threshold,
                FeeCondition::TradeSizeAbove(threshold) => trade_amount >= *threshold,
                FeeCondition::HoldsToken {
                    token_id,
                    min_amount,
                } => {
                    user_holdings
                        .get(token_id)
                        .copied()
                        .unwrap_or(Decimal::ZERO)
                        >= *min_amount
                }
                FeeCondition::TimeRange {
                    start_hour,
                    end_hour,
                } => {
                    let hour = timestamp.hour();
                    if start_hour <= end_hour {
                        hour >= *start_hour && hour < *end_hour
                    } else {
                        hour >= *start_hour || hour < *end_hour
                    }
                }
                FeeCondition::IsMarketMaker => is_maker,
                FeeCondition::UserId(id) => user_id == *id,
            };

            if condition_met {
                base_fee = match &rule.adjustment {
                    FeeAdjustment::Multiply(factor) => base_fee * factor,
                    FeeAdjustment::Add(amount) => base_fee + amount,
                    FeeAdjustment::Subtract(amount) => (base_fee - amount).max(Decimal::ZERO),
                    FeeAdjustment::SetTo(amount) => *amount,
                    FeeAdjustment::SetPercentage(pct) => *pct,
                };
            }
        }

        // Calculate final fee amount
        let mut fee = trade_amount * base_fee;

        // Apply min/max limits
        fee = fee.max(schedule.min_fee);
        if let Some(max_fee) = schedule.max_fee {
            fee = fee.min(max_fee);
        }

        fee
    }

    /// Calculate fee with rebate
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_fee_with_rebate(
        &self,
        token_id: Uuid,
        user_id: Uuid,
        trade_amount: Decimal,
        is_maker: bool,
        liquidity_depth: Decimal,
        timestamp: DateTime<Utc>,
        user_holdings: &HashMap<Uuid, Decimal>,
    ) -> (Decimal, Decimal) {
        let fee = self.calculate_fee(
            token_id,
            user_id,
            trade_amount,
            is_maker,
            liquidity_depth,
            timestamp,
            user_holdings,
        );

        let rebate = self
            .rebates
            .get(&user_id)
            .map(|r| r.calculate_rebate(fee))
            .unwrap_or(Decimal::ZERO);

        (fee, rebate)
    }

    /// Get optimal trading time (lowest fees)
    pub fn optimal_trading_time(&self) -> TimeOfDay {
        TimeOfDay::OffPeak
    }

    /// Get fee estimate for a future trade
    pub fn estimate_fee(&self, token_id: Uuid, trade_amount: Decimal, is_maker: bool) -> Decimal {
        let schedule = self
            .token_schedules
            .get(&token_id)
            .cloned()
            .unwrap_or_else(|| TokenFeeSchedule::standard(token_id));

        let base_fee = if is_maker {
            schedule.maker_fee_rate
        } else {
            schedule.taker_fee_rate
        };

        trade_amount * base_fee
    }
}

impl Default for DynamicFeeCalculator {
    fn default() -> Self {
        Self::new(dec!(1000000)) // 1M default reference depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_classification() {
        assert_eq!(TimeOfDay::from_hour(15), TimeOfDay::Peak);
        assert_eq!(TimeOfDay::from_hour(10), TimeOfDay::Regular);
        assert_eq!(TimeOfDay::from_hour(3), TimeOfDay::OffPeak);
    }

    #[test]
    fn test_time_of_day_multiplier() {
        assert_eq!(TimeOfDay::Peak.fee_multiplier(), dec!(1.2));
        assert_eq!(TimeOfDay::Regular.fee_multiplier(), dec!(1.0));
        assert_eq!(TimeOfDay::OffPeak.fee_multiplier(), dec!(0.8));
    }

    #[test]
    fn test_liquidity_level() {
        let reference = dec!(1000);

        assert_eq!(
            LiquidityLevel::from_depth(dec!(2500), reference),
            LiquidityLevel::VeryHigh
        );
        assert_eq!(
            LiquidityLevel::from_depth(dec!(1600), reference),
            LiquidityLevel::High
        );
        assert_eq!(
            LiquidityLevel::from_depth(dec!(900), reference),
            LiquidityLevel::Normal
        );
        assert_eq!(
            LiquidityLevel::from_depth(dec!(600), reference),
            LiquidityLevel::Low
        );
        assert_eq!(
            LiquidityLevel::from_depth(dec!(300), reference),
            LiquidityLevel::VeryLow
        );
    }

    #[test]
    fn test_token_fee_schedule() {
        let token_id = Uuid::new_v4();
        let standard = TokenFeeSchedule::standard(token_id);
        assert_eq!(standard.base_fee_rate, dec!(0.0025));
        assert!(!standard.is_premium);

        let premium = TokenFeeSchedule::premium(token_id);
        assert_eq!(premium.base_fee_rate, dec!(0.002));
        assert!(premium.is_premium);
        assert!(premium.maker_fee_rate < standard.maker_fee_rate);
    }

    #[test]
    fn test_market_maker_rebate() {
        let user_id = Uuid::new_v4();
        let mut rebate = MarketMakerRebate::new(user_id, dec!(0.0001), dec!(10000));

        assert!(!rebate.qualified);

        rebate.update_volume(dec!(5000));
        assert!(!rebate.qualified);

        rebate.update_volume(dec!(6000));
        assert!(rebate.qualified);

        let rebate_amount = rebate.calculate_rebate(dec!(100));
        assert_eq!(rebate_amount, dec!(0.01)); // 100 * 0.0001
    }

    #[test]
    fn test_dynamic_fee_calculation() {
        let mut calculator = DynamicFeeCalculator::new(dec!(1000));
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        calculator.add_token_schedule(TokenFeeSchedule::standard(token_id));

        let fee = calculator.calculate_fee(
            token_id,
            user_id,
            dec!(1000),
            false, // taker
            dec!(1000),
            Utc::now(),
            &HashMap::new(),
        );

        assert!(fee > Decimal::ZERO);
    }

    #[test]
    fn test_conditional_rule() {
        let mut calculator = DynamicFeeCalculator::new(dec!(1000));
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        calculator.add_token_schedule(TokenFeeSchedule::standard(token_id));

        // Add rule: 50% discount for trades above 10000
        calculator.add_rule(ConditionalFeeRule {
            id: Uuid::new_v4(),
            name: "High volume discount".to_string(),
            condition: FeeCondition::VolumeAbove(dec!(10000)),
            adjustment: FeeAdjustment::Multiply(dec!(0.5)),
            active: true,
        });

        let small_trade_fee = calculator.calculate_fee(
            token_id,
            user_id,
            dec!(1000),
            false,
            dec!(1000),
            Utc::now(),
            &HashMap::new(),
        );

        let large_trade_fee = calculator.calculate_fee(
            token_id,
            user_id,
            dec!(15000),
            false,
            dec!(1000),
            Utc::now(),
            &HashMap::new(),
        );

        // Large trade should have lower fee rate
        assert!(large_trade_fee / dec!(15000) < small_trade_fee / dec!(1000));
    }

    #[test]
    fn test_fee_with_rebate() {
        let mut calculator = DynamicFeeCalculator::new(dec!(1000));
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        calculator.add_token_schedule(TokenFeeSchedule::standard(token_id));

        let mut rebate = MarketMakerRebate::new(user_id, dec!(0.0005), dec!(10000));
        rebate.update_volume(dec!(15000)); // Qualify for rebate
        calculator.add_rebate(rebate);

        let (fee, rebate_amount) = calculator.calculate_fee_with_rebate(
            token_id,
            user_id,
            dec!(1000),
            true, // maker
            dec!(1000),
            Utc::now(),
            &HashMap::new(),
        );

        assert!(fee > Decimal::ZERO);
        assert!(rebate_amount > Decimal::ZERO);
        assert!(rebate_amount < fee);
    }

    #[test]
    #[allow(dead_code)]
    fn test_time_range_condition() {
        let mut calculator = DynamicFeeCalculator::new(dec!(1000));
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        calculator.add_token_schedule(TokenFeeSchedule::standard(token_id));

        // Add rule for specific time range
        calculator.add_rule(ConditionalFeeRule {
            id: Uuid::new_v4(),
            name: "Morning discount".to_string(),
            condition: FeeCondition::TimeRange {
                start_hour: 6,
                end_hour: 10,
            },
            adjustment: FeeAdjustment::Multiply(dec!(0.9)),
            active: true,
        });

        // Create timestamp for 8 AM
        let morning_time = Utc::now()
            .date_naive()
            .and_hms_opt(8, 0, 0)
            .unwrap()
            .and_utc();

        let morning_fee = calculator.calculate_fee(
            token_id,
            user_id,
            dec!(1000),
            false,
            dec!(1000),
            morning_time,
            &HashMap::new(),
        );

        // Create timestamp for 3 PM
        let afternoon_time = Utc::now()
            .date_naive()
            .and_hms_opt(15, 0, 0)
            .unwrap()
            .and_utc();

        let afternoon_fee = calculator.calculate_fee(
            token_id,
            user_id,
            dec!(1000),
            false,
            dec!(1000),
            afternoon_time,
            &HashMap::new(),
        );

        // Morning should have discount
        assert!(morning_fee <= afternoon_fee);
    }

    #[test]
    fn test_estimate_fee() {
        let mut calculator = DynamicFeeCalculator::new(dec!(1000));
        let token_id = Uuid::new_v4();

        calculator.add_token_schedule(TokenFeeSchedule::standard(token_id));

        let estimate = calculator.estimate_fee(token_id, dec!(1000), true);

        assert!(estimate > Decimal::ZERO);
        // Should be 0.2% of 1000 = 2.0
        assert_eq!(estimate, dec!(2.0));
    }
}
