//! Position limits and exposure controls
//!
//! This module provides position limit management including:
//! - Per-token position limits
//! - Concentration limits
//! - Exposure limits by user tier
//! - Dynamic limit adjustment

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User tier for position limits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserTier {
    Basic,
    Silver,
    Gold,
    Platinum,
    Diamond,
}

impl UserTier {
    /// Get tier multiplier for position limits
    pub fn multiplier(&self) -> Decimal {
        match self {
            UserTier::Basic => dec!(1.0),
            UserTier::Silver => dec!(2.0),
            UserTier::Gold => dec!(5.0),
            UserTier::Platinum => dec!(10.0),
            UserTier::Diamond => dec!(20.0),
        }
    }

    /// Get tier name
    pub fn name(&self) -> &str {
        match self {
            UserTier::Basic => "Basic",
            UserTier::Silver => "Silver",
            UserTier::Gold => "Gold",
            UserTier::Platinum => "Platinum",
            UserTier::Diamond => "Diamond",
        }
    }
}

/// Position limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimit {
    /// Token symbol
    pub token_symbol: String,

    /// Maximum position size (in base currency)
    pub max_position_size: Decimal,

    /// Maximum position value (in USD)
    pub max_position_value: Decimal,

    /// Maximum leverage allowed
    pub max_leverage: Decimal,

    /// Maximum number of open positions
    pub max_open_positions: usize,

    /// User tier
    pub tier: UserTier,

    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

impl PositionLimit {
    /// Create new position limit
    pub fn new(token_symbol: String, tier: UserTier) -> Self {
        let base_position_size = dec!(10000); // Base: $10,000
        let base_position_value = dec!(100000); // Base: $100,000
        let base_max_positions = 10;

        let multiplier = tier.multiplier();

        Self {
            token_symbol,
            max_position_size: base_position_size * multiplier,
            max_position_value: base_position_value * multiplier,
            max_leverage: match tier {
                UserTier::Basic => dec!(2),
                UserTier::Silver => dec!(5),
                UserTier::Gold => dec!(10),
                UserTier::Platinum => dec!(20),
                UserTier::Diamond => dec!(50),
            },
            max_open_positions: (base_max_positions as f64 * multiplier.to_f64().unwrap_or(1.0))
                as usize,
            tier,
            created_at: Utc::now(),
        }
    }

    /// Check if position size is within limit
    pub fn check_position_size(&self, size: Decimal) -> Result<()> {
        if size > self.max_position_size {
            return Err(CoreError::Validation(format!(
                "Position size {} exceeds limit {}",
                size, self.max_position_size
            )));
        }

        Ok(())
    }

    /// Check if position value is within limit
    pub fn check_position_value(&self, value: Decimal) -> Result<()> {
        if value > self.max_position_value {
            return Err(CoreError::Validation(format!(
                "Position value {} exceeds limit {}",
                value, self.max_position_value
            )));
        }

        Ok(())
    }

    /// Check if leverage is within limit
    pub fn check_leverage(&self, leverage: Decimal) -> Result<()> {
        if leverage > self.max_leverage {
            return Err(CoreError::Validation(format!(
                "Leverage {} exceeds limit {}",
                leverage, self.max_leverage
            )));
        }

        Ok(())
    }
}

/// Concentration limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcentrationLimit {
    /// Maximum percentage of portfolio in single token
    pub max_single_token_percentage: Decimal,

    /// Maximum percentage of portfolio in single sector
    pub max_sector_percentage: Decimal,

    /// Minimum number of tokens for diversification
    pub min_tokens: usize,

    /// User tier
    pub tier: UserTier,
}

impl ConcentrationLimit {
    /// Create new concentration limit
    pub fn new(tier: UserTier) -> Self {
        match tier {
            UserTier::Basic => Self {
                max_single_token_percentage: dec!(0.5), // 50%
                max_sector_percentage: dec!(0.7),       // 70%
                min_tokens: 3,
                tier,
            },
            UserTier::Silver => Self {
                max_single_token_percentage: dec!(0.6),
                max_sector_percentage: dec!(0.8),
                min_tokens: 2,
                tier,
            },
            UserTier::Gold => Self {
                max_single_token_percentage: dec!(0.7),
                max_sector_percentage: dec!(0.85),
                min_tokens: 2,
                tier,
            },
            UserTier::Platinum | UserTier::Diamond => Self {
                max_single_token_percentage: dec!(1.0), // No limit
                max_sector_percentage: dec!(1.0),
                min_tokens: 1,
                tier,
            },
        }
    }

    /// Check if concentration is within limits
    pub fn check_concentration(
        &self,
        portfolio_value: Decimal,
        token_value: Decimal,
    ) -> Result<()> {
        if portfolio_value.is_zero() {
            return Ok(());
        }

        let concentration = token_value / portfolio_value;

        if concentration > self.max_single_token_percentage {
            return Err(CoreError::Validation(format!(
                "Token concentration {}% exceeds limit {}%",
                (concentration * dec!(100)),
                (self.max_single_token_percentage * dec!(100))
            )));
        }

        Ok(())
    }

    /// Check sector concentration
    pub fn check_sector_concentration(
        &self,
        portfolio_value: Decimal,
        sector_value: Decimal,
    ) -> Result<()> {
        if portfolio_value.is_zero() {
            return Ok(());
        }

        let concentration = sector_value / portfolio_value;

        if concentration > self.max_sector_percentage {
            return Err(CoreError::Validation(format!(
                "Sector concentration {}% exceeds limit {}%",
                (concentration * dec!(100)),
                (self.max_sector_percentage * dec!(100))
            )));
        }

        Ok(())
    }
}

/// Exposure limit tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureLimit {
    /// User ID
    pub user_id: String,

    /// User tier
    pub tier: UserTier,

    /// Total exposure limit (in USD)
    pub total_exposure_limit: Decimal,

    /// Current total exposure
    pub current_exposure: Decimal,

    /// Per-token exposure limits
    pub token_limits: HashMap<String, PositionLimit>,

    /// Concentration limits
    pub concentration_limit: ConcentrationLimit,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl ExposureLimit {
    /// Create new exposure limit
    pub fn new(user_id: String, tier: UserTier) -> Self {
        let total_exposure_limit = match tier {
            UserTier::Basic => dec!(100000),      // $100,000
            UserTier::Silver => dec!(500000),     // $500,000
            UserTier::Gold => dec!(2000000),      // $2,000,000
            UserTier::Platinum => dec!(10000000), // $10,000,000
            UserTier::Diamond => dec!(100000000), // $100,000,000
        };

        Self {
            user_id,
            tier,
            total_exposure_limit,
            current_exposure: Decimal::ZERO,
            token_limits: HashMap::new(),
            concentration_limit: ConcentrationLimit::new(tier),
            updated_at: Utc::now(),
        }
    }

    /// Get or create position limit for a token
    pub fn get_or_create_limit(&mut self, token_symbol: &str) -> &PositionLimit {
        self.token_limits
            .entry(token_symbol.to_string())
            .or_insert_with(|| PositionLimit::new(token_symbol.to_string(), self.tier))
    }

    /// Check if new position is within limits
    pub fn check_new_position(
        &mut self,
        token_symbol: &str,
        size: Decimal,
        price: Decimal,
        leverage: Decimal,
    ) -> Result<()> {
        let position_value = size * price;

        // Check total exposure limit
        if self.current_exposure + position_value > self.total_exposure_limit {
            return Err(CoreError::Validation(format!(
                "New position would exceed total exposure limit: current={}, new={}, limit={}",
                self.current_exposure, position_value, self.total_exposure_limit
            )));
        }

        // Check token-specific limits
        let limit = self.get_or_create_limit(token_symbol);
        // Note: We check position value instead of size since size units vary by token
        limit.check_position_value(position_value)?;
        limit.check_leverage(leverage)?;

        // Check concentration limits
        self.concentration_limit
            .check_concentration(self.current_exposure + position_value, position_value)?;

        Ok(())
    }

    /// Add exposure
    pub fn add_exposure(&mut self, amount: Decimal) {
        self.current_exposure += amount;
        self.updated_at = Utc::now();
    }

    /// Remove exposure
    pub fn remove_exposure(&mut self, amount: Decimal) {
        self.current_exposure = (self.current_exposure - amount).max(Decimal::ZERO);
        self.updated_at = Utc::now();
    }

    /// Get available exposure
    pub fn available_exposure(&self) -> Decimal {
        (self.total_exposure_limit - self.current_exposure).max(Decimal::ZERO)
    }

    /// Get utilization percentage
    pub fn utilization_percentage(&self) -> Decimal {
        if self.total_exposure_limit.is_zero() {
            return Decimal::ZERO;
        }

        (self.current_exposure / self.total_exposure_limit * dec!(100)).min(dec!(100))
    }
}

/// Position limit manager
pub struct PositionLimitManager {
    /// User exposure limits
    user_limits: HashMap<String, ExposureLimit>,

    /// Global position limits
    global_limits: HashMap<String, Decimal>,
}

impl PositionLimitManager {
    /// Create new position limit manager
    pub fn new() -> Self {
        Self {
            user_limits: HashMap::new(),
            global_limits: HashMap::new(),
        }
    }

    /// Set user tier
    pub fn set_user_tier(&mut self, user_id: String, tier: UserTier) {
        self.user_limits
            .insert(user_id.clone(), ExposureLimit::new(user_id, tier));
    }

    /// Get user exposure limit
    pub fn get_user_limit(&self, user_id: &str) -> Option<&ExposureLimit> {
        self.user_limits.get(user_id)
    }

    /// Get mutable user exposure limit
    pub fn get_user_limit_mut(&mut self, user_id: &str) -> Option<&mut ExposureLimit> {
        self.user_limits.get_mut(user_id)
    }

    /// Check if user can open position
    pub fn can_open_position(
        &mut self,
        user_id: &str,
        token_symbol: &str,
        size: Decimal,
        price: Decimal,
        leverage: Decimal,
    ) -> Result<()> {
        let limit = self
            .get_user_limit_mut(user_id)
            .ok_or_else(|| CoreError::NotFound(format!("User {} not found", user_id)))?;

        limit.check_new_position(token_symbol, size, price, leverage)?;

        // Check global limits if any
        if let Some(&global_limit) = self.global_limits.get(token_symbol) {
            let position_value = size * price;
            if position_value > global_limit {
                return Err(CoreError::Validation(format!(
                    "Position exceeds global limit for {}",
                    token_symbol
                )));
            }
        }

        Ok(())
    }

    /// Record position opened
    pub fn record_position_opened(&mut self, user_id: &str, value: Decimal) -> Result<()> {
        let limit = self
            .get_user_limit_mut(user_id)
            .ok_or_else(|| CoreError::NotFound(format!("User {} not found", user_id)))?;

        limit.add_exposure(value);

        Ok(())
    }

    /// Record position closed
    pub fn record_position_closed(&mut self, user_id: &str, value: Decimal) -> Result<()> {
        let limit = self
            .get_user_limit_mut(user_id)
            .ok_or_else(|| CoreError::NotFound(format!("User {} not found", user_id)))?;

        limit.remove_exposure(value);

        Ok(())
    }

    /// Set global position limit for token
    pub fn set_global_limit(&mut self, token_symbol: String, limit: Decimal) {
        self.global_limits.insert(token_symbol, limit);
    }

    /// Get users exceeding limits
    pub fn get_users_exceeding_limits(&self) -> Vec<String> {
        self.user_limits
            .iter()
            .filter(|(_, limit)| limit.current_exposure > limit.total_exposure_limit)
            .map(|(user_id, _)| user_id.clone())
            .collect()
    }
}

impl Default for PositionLimitManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_user_tier_multiplier() {
        assert_eq!(UserTier::Basic.multiplier(), dec!(1.0));
        assert_eq!(UserTier::Diamond.multiplier(), dec!(20.0));
    }

    #[test]
    fn test_position_limit() {
        let limit = PositionLimit::new("BTC".to_string(), UserTier::Gold);

        assert!(limit.check_position_size(dec!(10000)).is_ok());
        assert!(limit.check_position_size(dec!(100000)).is_err());
    }

    #[test]
    fn test_concentration_limit() {
        let limit = ConcentrationLimit::new(UserTier::Basic);

        // 40% concentration should be OK
        assert!(limit.check_concentration(dec!(100000), dec!(40000)).is_ok());

        // 60% concentration should fail
        assert!(
            limit
                .check_concentration(dec!(100000), dec!(60000))
                .is_err()
        );
    }

    #[test]
    fn test_exposure_limit() {
        // Use Platinum tier which has no concentration limits
        let mut limit = ExposureLimit::new("user1".to_string(), UserTier::Platinum);

        // Position value = 0.2 * 50000 = 10000
        // This should be within limits for Platinum tier
        let result = limit.check_new_position("BTC", dec!(0.2), dec!(50000), dec!(2));
        assert!(result.is_ok());

        limit.add_exposure(dec!(10000));
        assert_eq!(limit.current_exposure, dec!(10000));

        limit.remove_exposure(dec!(5000));
        assert_eq!(limit.current_exposure, dec!(5000));
    }

    #[test]
    fn test_position_limit_manager() {
        let mut manager = PositionLimitManager::new();
        // Use Platinum tier which has no concentration limits
        manager.set_user_tier("user1".to_string(), UserTier::Platinum);

        assert!(
            manager
                .can_open_position("user1", "BTC", dec!(0.5), dec!(50000), dec!(2))
                .is_ok()
        );

        manager
            .record_position_opened("user1", dec!(25000))
            .unwrap();

        let limit = manager.get_user_limit("user1").unwrap();
        assert_eq!(limit.current_exposure, dec!(25000));
    }

    #[test]
    fn test_utilization_percentage() {
        let mut limit = ExposureLimit::new("user1".to_string(), UserTier::Basic);
        limit.add_exposure(dec!(50000));

        let utilization = limit.utilization_percentage();
        assert_eq!(utilization, dec!(50)); // 50%
    }
}
