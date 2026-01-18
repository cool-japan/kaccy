//! Token burn mechanism

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Record of a token burn event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenBurn {
    pub burn_id: Uuid,
    pub token_id: Uuid,
    pub burner_user_id: Uuid,
    /// Amount of tokens burned
    pub amount: Decimal,
    /// Reason for burning
    pub burn_type: BurnType,
    /// Optional notes or reason description
    pub notes: Option<String>,
    /// Transaction hash if applicable
    pub tx_hash: Option<String>,
    pub burned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum BurnType {
    /// User voluntarily burns their tokens
    #[default]
    Voluntary,
    /// Protocol-enforced burn (e.g., fee burn)
    Protocol,
    /// Deflationary mechanism (automatic burns)
    Deflationary,
    /// Buyback and burn by issuer
    Buyback,
    /// Penalty or slashing
    Penalty,
    /// Other reason
    Other,
}

impl fmt::Display for BurnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BurnType::Voluntary => write!(f, "voluntary"),
            BurnType::Protocol => write!(f, "protocol"),
            BurnType::Deflationary => write!(f, "deflationary"),
            BurnType::Buyback => write!(f, "buyback"),
            BurnType::Penalty => write!(f, "penalty"),
            BurnType::Other => write!(f, "other"),
        }
    }
}

impl fmt::Display for TokenBurn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TokenBurn({}, amount={}, type={})",
            self.burn_id, self.amount, self.burn_type
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct BurnTokensRequest {
    pub token_id: Uuid,
    pub amount: Decimal,
    pub burn_type: BurnType,
    pub notes: Option<String>,
}

impl BurnTokensRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Burn amount must be positive".to_string()));
        }
        if let Some(ref notes) = self.notes {
            if notes.len() > 1000 {
                return Err(ValidationError(
                    "Notes must be at most 1000 characters".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Token burn statistics for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnStats {
    pub token_id: Uuid,
    /// Total tokens burned
    pub total_burned: Decimal,
    /// Number of burn events
    pub burn_count: i32,
    /// Burns by type
    pub burns_by_type: BurnsByType,
    /// Burn rate (tokens burned per day)
    pub daily_burn_rate: Option<Decimal>,
    /// Last burn timestamp
    pub last_burn_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnsByType {
    pub voluntary: Decimal,
    pub protocol: Decimal,
    pub deflationary: Decimal,
    pub buyback: Decimal,
    pub penalty: Decimal,
    pub other: Decimal,
}

impl Default for BurnsByType {
    fn default() -> Self {
        Self {
            voluntary: dec!(0),
            protocol: dec!(0),
            deflationary: dec!(0),
            buyback: dec!(0),
            penalty: dec!(0),
            other: dec!(0),
        }
    }
}

impl BurnsByType {
    /// Get total across all types
    pub fn total(&self) -> Decimal {
        self.voluntary
            + self.protocol
            + self.deflationary
            + self.buyback
            + self.penalty
            + self.other
    }

    /// Add a burn amount to the appropriate type
    pub fn add_burn(&mut self, burn_type: BurnType, amount: Decimal) {
        match burn_type {
            BurnType::Voluntary => self.voluntary += amount,
            BurnType::Protocol => self.protocol += amount,
            BurnType::Deflationary => self.deflationary += amount,
            BurnType::Buyback => self.buyback += amount,
            BurnType::Penalty => self.penalty += amount,
            BurnType::Other => self.other += amount,
        }
    }
}

/// Burn mechanism configuration for a token
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BurnMechanism {
    pub token_id: Uuid,
    /// Whether automatic deflationary burns are enabled
    pub deflationary_enabled: bool,
    /// Percentage of each trade to burn (0-1)
    pub burn_rate_per_trade: Decimal,
    /// Whether buyback and burn is enabled
    pub buyback_enabled: bool,
    /// Target burn amount per period
    pub target_burn_per_period: Option<Decimal>,
    /// Period duration in seconds
    pub burn_period_seconds: Option<i64>,
    /// Maximum supply after burns (soft cap)
    pub max_supply_after_burns: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BurnMechanism {
    /// Create default burn mechanism (disabled)
    pub fn default_for_token(token_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            token_id,
            deflationary_enabled: false,
            burn_rate_per_trade: dec!(0),
            buyback_enabled: false,
            target_burn_per_period: None,
            burn_period_seconds: None,
            max_supply_after_burns: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Calculate burn amount for a trade
    pub fn calculate_trade_burn(&self, trade_amount: Decimal) -> Decimal {
        if !self.deflationary_enabled {
            return dec!(0);
        }
        trade_amount * self.burn_rate_per_trade
    }

    /// Check if target burns for period are met
    pub fn is_burn_target_met(&self, period_burns: Decimal) -> bool {
        if let Some(target) = self.target_burn_per_period {
            period_burns >= target
        } else {
            true // No target set
        }
    }

    /// Validate the burn mechanism configuration
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.burn_rate_per_trade < dec!(0) || self.burn_rate_per_trade >= dec!(1) {
            return Err(ValidationError(
                "Burn rate must be between 0 and 1".to_string(),
            ));
        }
        if let Some(target) = self.target_burn_per_period {
            if target <= dec!(0) {
                return Err(ValidationError(
                    "Target burn per period must be positive".to_string(),
                ));
            }
        }
        if let Some(period) = self.burn_period_seconds {
            if period <= 0 {
                return Err(ValidationError("Burn period must be positive".to_string()));
            }
        }
        if let Some(max_supply) = self.max_supply_after_burns {
            if max_supply <= dec!(0) {
                return Err(ValidationError(
                    "Max supply after burns must be positive".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl fmt::Display for BurnMechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BurnMechanism(token={}, deflationary={}, burn_rate={})",
            self.token_id, self.deflationary_enabled, self.burn_rate_per_trade
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateBurnMechanismRequest {
    pub deflationary_enabled: Option<bool>,
    pub burn_rate_per_trade: Option<Decimal>,
    pub buyback_enabled: Option<bool>,
    pub target_burn_per_period: Option<Decimal>,
    pub burn_period_seconds: Option<i64>,
    pub max_supply_after_burns: Option<Decimal>,
}

/// Burn executor for processing burns
pub struct BurnExecutor;

impl BurnExecutor {
    /// Create a burn record
    pub fn create_burn(
        token_id: Uuid,
        burner_user_id: Uuid,
        amount: Decimal,
        burn_type: BurnType,
        notes: Option<String>,
    ) -> TokenBurn {
        TokenBurn {
            burn_id: Uuid::new_v4(),
            token_id,
            burner_user_id,
            amount,
            burn_type,
            notes,
            tx_hash: None,
            burned_at: Utc::now(),
        }
    }

    /// Calculate the impact of a burn on token price
    /// Assumes constant demand, reduced supply increases price
    pub fn calculate_price_impact(
        current_supply: Decimal,
        burn_amount: Decimal,
        current_price: Decimal,
    ) -> Decimal {
        if current_supply == dec!(0) || burn_amount >= current_supply {
            return dec!(0);
        }

        let new_supply = current_supply - burn_amount;
        let supply_ratio = current_supply / new_supply;

        // Price should increase proportionally to supply decrease
        // New price = old price * (old supply / new supply)
        current_price * supply_ratio
    }

    /// Calculate burn to achieve target price increase
    pub fn calculate_burn_for_price_target(
        current_supply: Decimal,
        current_price: Decimal,
        target_price: Decimal,
    ) -> Option<Decimal> {
        if target_price <= current_price {
            return None; // Can't decrease price by burning
        }

        // target_price = current_price * (current_supply / new_supply)
        // new_supply = current_supply * current_price / target_price
        // burn_amount = current_supply - new_supply

        let new_supply = (current_supply * current_price) / target_price;
        let burn_amount = current_supply - new_supply;

        if burn_amount <= dec!(0) || burn_amount >= current_supply {
            return None;
        }

        Some(burn_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_burns_by_type() {
        let mut burns = BurnsByType::default();

        burns.add_burn(BurnType::Voluntary, dec!(100));
        burns.add_burn(BurnType::Protocol, dec!(50));
        burns.add_burn(BurnType::Deflationary, dec!(25));

        assert_eq!(burns.voluntary, dec!(100));
        assert_eq!(burns.protocol, dec!(50));
        assert_eq!(burns.deflationary, dec!(25));
        assert_eq!(burns.total(), dec!(175));
    }

    #[test]
    fn test_trade_burn_calculation() {
        let mechanism = BurnMechanism {
            token_id: Uuid::new_v4(),
            deflationary_enabled: true,
            burn_rate_per_trade: dec!(0.01), // 1%
            buyback_enabled: false,
            target_burn_per_period: None,
            burn_period_seconds: None,
            max_supply_after_burns: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let trade_amount = dec!(1000);
        let burn_amount = mechanism.calculate_trade_burn(trade_amount);

        assert_eq!(burn_amount, dec!(10)); // 1% of 1000
    }

    #[test]
    fn test_burn_disabled() {
        let mechanism = BurnMechanism {
            token_id: Uuid::new_v4(),
            deflationary_enabled: false,
            burn_rate_per_trade: dec!(0.01),
            buyback_enabled: false,
            target_burn_per_period: None,
            burn_period_seconds: None,
            max_supply_after_burns: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let trade_amount = dec!(1000);
        let burn_amount = mechanism.calculate_trade_burn(trade_amount);

        assert_eq!(burn_amount, dec!(0)); // Disabled
    }

    #[test]
    fn test_price_impact() {
        let current_supply = dec!(10000);
        let burn_amount = dec!(1000); // Burn 10%
        let current_price = dec!(1);

        let new_price =
            BurnExecutor::calculate_price_impact(current_supply, burn_amount, current_price);

        // Burning 10% of supply should increase price by ~11.11%
        // new_price = 1 * (10000 / 9000) = 1.111...
        assert!(new_price > dec!(1.11) && new_price < dec!(1.12));
    }

    #[test]
    fn test_burn_for_price_target() {
        let current_supply = dec!(10000);
        let current_price = dec!(1);
        let target_price = dec!(2); // Double the price

        let burn_amount = BurnExecutor::calculate_burn_for_price_target(
            current_supply,
            current_price,
            target_price,
        )
        .unwrap();

        // To double price, need to halve supply
        assert_eq!(burn_amount, dec!(5000));
    }

    #[test]
    fn test_invalid_price_target() {
        let current_supply = dec!(10000);
        let current_price = dec!(2);
        let target_price = dec!(1); // Lower than current

        let result = BurnExecutor::calculate_burn_for_price_target(
            current_supply,
            current_price,
            target_price,
        );

        assert!(result.is_none()); // Can't decrease price by burning
    }
}
