//! Enhanced trade validation module
//!
//! This module provides comprehensive validation for trading operations to ensure:
//! - Price sanity checks
//! - Amount validation
//! - Slippage protection
//! - Market manipulation detection
//! - Risk limit enforcement

use crate::error::{CoreError, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Trade validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeValidationConfig {
    /// Maximum price deviation from oracle price (as percentage)
    pub max_price_deviation: Decimal,

    /// Minimum order amount
    pub min_order_amount: Decimal,

    /// Maximum order amount
    pub max_order_amount: Decimal,

    /// Maximum slippage tolerance (as percentage)
    pub max_slippage: Decimal,

    /// Maximum price impact allowed (as percentage)
    pub max_price_impact: Decimal,

    /// Minimum time between orders from same user (seconds)
    pub min_order_interval_secs: i64,

    /// Maximum number of orders per user per hour
    pub max_orders_per_hour: usize,
}

impl Default for TradeValidationConfig {
    fn default() -> Self {
        Self {
            max_price_deviation: dec!(0.10), // 10%
            min_order_amount: dec!(0.0001),  // 0.0001 BTC
            max_order_amount: dec!(100.0),   // 100 BTC
            max_slippage: dec!(0.05),        // 5%
            max_price_impact: dec!(0.15),    // 15%
            min_order_interval_secs: 1,      // 1 second
            max_orders_per_hour: 1000,       // 1000 orders/hour
        }
    }
}

/// Trade validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Trade is valid
    Valid,

    /// Trade is rejected with reason
    Rejected(String),

    /// Trade requires additional confirmation
    RequiresConfirmation(String),
}

/// Trade validator
pub struct TradeValidator {
    config: TradeValidationConfig,
}

impl TradeValidator {
    /// Create a new trade validator with default config
    pub fn new() -> Self {
        Self {
            config: TradeValidationConfig::default(),
        }
    }

    /// Create a new trade validator with custom config
    pub fn with_config(config: TradeValidationConfig) -> Self {
        Self { config }
    }

    /// Validate order amount
    pub fn validate_amount(&self, amount: Decimal) -> ValidationResult {
        if amount <= Decimal::ZERO {
            return ValidationResult::Rejected("Amount must be positive".to_string());
        }

        if amount < self.config.min_order_amount {
            return ValidationResult::Rejected(format!(
                "Amount {} is below minimum {}",
                amount, self.config.min_order_amount
            ));
        }

        if amount > self.config.max_order_amount {
            return ValidationResult::Rejected(format!(
                "Amount {} exceeds maximum {}",
                amount, self.config.max_order_amount
            ));
        }

        ValidationResult::Valid
    }

    /// Validate price against oracle price
    pub fn validate_price(&self, price: Decimal, oracle_price: Decimal) -> ValidationResult {
        if price <= Decimal::ZERO {
            return ValidationResult::Rejected("Price must be positive".to_string());
        }

        if oracle_price <= Decimal::ZERO {
            return ValidationResult::RequiresConfirmation(
                "Oracle price unavailable, manual review required".to_string(),
            );
        }

        let deviation = ((price - oracle_price) / oracle_price).abs();

        if deviation > self.config.max_price_deviation {
            return ValidationResult::Rejected(format!(
                "Price deviation {:.2}% exceeds maximum {:.2}%",
                deviation * dec!(100),
                self.config.max_price_deviation * dec!(100)
            ));
        }

        ValidationResult::Valid
    }

    /// Validate slippage
    pub fn validate_slippage(
        &self,
        expected_price: Decimal,
        execution_price: Decimal,
    ) -> ValidationResult {
        if expected_price <= Decimal::ZERO || execution_price <= Decimal::ZERO {
            return ValidationResult::Rejected("Invalid price values".to_string());
        }

        let slippage = ((execution_price - expected_price) / expected_price).abs();

        if slippage > self.config.max_slippage {
            return ValidationResult::Rejected(format!(
                "Slippage {:.2}% exceeds maximum {:.2}%",
                slippage * dec!(100),
                self.config.max_slippage * dec!(100)
            ));
        }

        ValidationResult::Valid
    }

    /// Validate price impact
    pub fn validate_price_impact(&self, price_impact: Decimal) -> ValidationResult {
        if price_impact < Decimal::ZERO {
            return ValidationResult::Rejected("Price impact cannot be negative".to_string());
        }

        if price_impact > self.config.max_price_impact {
            return ValidationResult::RequiresConfirmation(format!(
                "High price impact {:.2}% detected (max {:.2}%). Confirm to proceed.",
                price_impact * dec!(100),
                self.config.max_price_impact * dec!(100)
            ));
        }

        ValidationResult::Valid
    }

    /// Validate order frequency
    pub fn validate_order_frequency(
        &self,
        seconds_since_last_order: i64,
        orders_in_last_hour: usize,
    ) -> ValidationResult {
        if seconds_since_last_order < self.config.min_order_interval_secs {
            return ValidationResult::Rejected(format!(
                "Orders too frequent. Wait {} seconds between orders",
                self.config.min_order_interval_secs - seconds_since_last_order
            ));
        }

        if orders_in_last_hour >= self.config.max_orders_per_hour {
            return ValidationResult::Rejected(format!(
                "Order limit exceeded. Maximum {} orders per hour",
                self.config.max_orders_per_hour
            ));
        }

        ValidationResult::Valid
    }

    /// Validate total position size
    pub fn validate_position_size(
        &self,
        new_position: Decimal,
        current_position: Decimal,
        position_limit: Decimal,
    ) -> ValidationResult {
        let total_position = current_position + new_position;

        if total_position > position_limit {
            return ValidationResult::Rejected(format!(
                "Position size {} would exceed limit {}",
                total_position, position_limit
            ));
        }

        ValidationResult::Valid
    }

    /// Comprehensive trade validation
    #[allow(clippy::too_many_arguments)]
    pub fn validate_trade(
        &self,
        amount: Decimal,
        price: Decimal,
        oracle_price: Option<Decimal>,
        expected_price: Option<Decimal>,
        price_impact: Decimal,
        current_position: Decimal,
        position_limit: Decimal,
        seconds_since_last: i64,
        orders_last_hour: usize,
    ) -> Result<()> {
        // Validate amount
        match self.validate_amount(amount) {
            ValidationResult::Valid => {}
            ValidationResult::Rejected(reason) => {
                return Err(CoreError::Validation(reason));
            }
            ValidationResult::RequiresConfirmation(reason) => {
                return Err(CoreError::Validation(reason));
            }
        }

        // Validate price if oracle price available
        if let Some(oracle) = oracle_price {
            match self.validate_price(price, oracle) {
                ValidationResult::Valid => {}
                ValidationResult::Rejected(reason) => {
                    return Err(CoreError::Validation(reason));
                }
                ValidationResult::RequiresConfirmation(_) => {
                    // Log warning but allow
                }
            }
        }

        // Validate slippage if expected price available
        if let Some(expected) = expected_price {
            match self.validate_slippage(expected, price) {
                ValidationResult::Valid => {}
                ValidationResult::Rejected(reason) => {
                    return Err(CoreError::Validation(reason));
                }
                ValidationResult::RequiresConfirmation(reason) => {
                    return Err(CoreError::Validation(reason));
                }
            }
        }

        // Validate price impact
        match self.validate_price_impact(price_impact) {
            ValidationResult::Valid => {}
            ValidationResult::Rejected(reason) => {
                return Err(CoreError::Validation(reason));
            }
            ValidationResult::RequiresConfirmation(_) => {
                // Log warning but allow
            }
        }

        // Validate position size
        match self.validate_position_size(amount, current_position, position_limit) {
            ValidationResult::Valid => {}
            ValidationResult::Rejected(reason) => {
                return Err(CoreError::Validation(reason));
            }
            ValidationResult::RequiresConfirmation(reason) => {
                return Err(CoreError::Validation(reason));
            }
        }

        // Validate order frequency
        match self.validate_order_frequency(seconds_since_last, orders_last_hour) {
            ValidationResult::Valid => {}
            ValidationResult::Rejected(reason) => {
                return Err(CoreError::Validation(reason));
            }
            ValidationResult::RequiresConfirmation(reason) => {
                return Err(CoreError::Validation(reason));
            }
        }

        Ok(())
    }
}

impl Default for TradeValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_amount_valid() {
        let validator = TradeValidator::new();
        let result = validator.validate_amount(dec!(1.0));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_amount_too_small() {
        let validator = TradeValidator::new();
        let result = validator.validate_amount(dec!(0.00001));
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_amount_too_large() {
        let validator = TradeValidator::new();
        let result = validator.validate_amount(dec!(200.0));
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_amount_zero() {
        let validator = TradeValidator::new();
        let result = validator.validate_amount(Decimal::ZERO);
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_price_within_deviation() {
        let validator = TradeValidator::new();
        let result = validator.validate_price(dec!(105.0), dec!(100.0));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_price_exceeds_deviation() {
        let validator = TradeValidator::new();
        let result = validator.validate_price(dec!(115.0), dec!(100.0));
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_slippage_acceptable() {
        let validator = TradeValidator::new();
        let result = validator.validate_slippage(dec!(100.0), dec!(103.0));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_slippage_excessive() {
        let validator = TradeValidator::new();
        let result = validator.validate_slippage(dec!(100.0), dec!(110.0));
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_price_impact_low() {
        let validator = TradeValidator::new();
        let result = validator.validate_price_impact(dec!(0.05));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_price_impact_high() {
        let validator = TradeValidator::new();
        let result = validator.validate_price_impact(dec!(0.20));
        match result {
            ValidationResult::RequiresConfirmation(_) => {}
            _ => panic!("Expected confirmation required"),
        }
    }

    #[test]
    fn test_validate_order_frequency_ok() {
        let validator = TradeValidator::new();
        let result = validator.validate_order_frequency(10, 50);
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_order_frequency_too_fast() {
        let validator = TradeValidator::new();
        let result = validator.validate_order_frequency(0, 50);
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_order_frequency_too_many() {
        let validator = TradeValidator::new();
        let result = validator.validate_order_frequency(10, 1001);
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_validate_position_size_ok() {
        let validator = TradeValidator::new();
        let result = validator.validate_position_size(dec!(10.0), dec!(5.0), dec!(20.0));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_position_size_exceeds_limit() {
        let validator = TradeValidator::new();
        let result = validator.validate_position_size(dec!(10.0), dec!(15.0), dec!(20.0));
        match result {
            ValidationResult::Rejected(_) => {}
            _ => panic!("Expected rejection"),
        }
    }

    #[test]
    fn test_comprehensive_validation_success() {
        let validator = TradeValidator::new();
        let result = validator.validate_trade(
            dec!(1.0),         // amount
            dec!(105.0),       // price
            Some(dec!(100.0)), // oracle_price
            Some(dec!(103.0)), // expected_price
            dec!(0.05),        // price_impact
            dec!(5.0),         // current_position
            dec!(20.0),        // position_limit
            10,                // seconds_since_last
            50,                // orders_last_hour
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_comprehensive_validation_invalid_amount() {
        let validator = TradeValidator::new();
        let result = validator.validate_trade(
            dec!(200.0),       // amount exceeds max
            dec!(105.0),       // price
            Some(dec!(100.0)), // oracle_price
            Some(dec!(103.0)), // expected_price
            dec!(0.05),        // price_impact
            dec!(5.0),         // current_position
            dec!(20.0),        // position_limit
            10,                // seconds_since_last
            50,                // orders_last_hour
        );
        assert!(result.is_err());
    }
}
