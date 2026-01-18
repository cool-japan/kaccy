//! Collateral management system
//!
//! This module provides collateral management functionality including:
//! - Multi-collateral support
//! - Collateral valuation
//! - Haircut calculations
//! - Cross-collateralization

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Collateral asset configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralConfig {
    /// Asset symbol
    pub symbol: String,

    /// Collateral factor (0.0 to 1.0)
    /// e.g., 0.8 means $100 of collateral can support $80 of borrowing
    pub collateral_factor: Decimal,

    /// Liquidation threshold (0.0 to 1.0)
    /// e.g., 0.85 means liquidation starts when debt/collateral > 0.85
    pub liquidation_threshold: Decimal,

    /// Haircut percentage (reduction in value for risk)
    /// e.g., 0.1 means 10% haircut
    pub haircut: Decimal,

    /// Minimum deposit amount
    pub min_deposit: Decimal,

    /// Maximum single deposit
    pub max_deposit: Option<Decimal>,

    /// Is this asset enabled as collateral
    pub enabled: bool,

    /// Asset category
    pub category: AssetCategory,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Asset category for risk classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetCategory {
    /// Stablecoins (USDT, USDC, etc.)
    Stablecoin,

    /// Major cryptocurrencies (BTC, ETH)
    MajorCrypto,

    /// Altcoins
    Altcoin,

    /// Platform tokens
    PlatformToken,

    /// LP tokens
    LiquidityToken,
}

impl AssetCategory {
    /// Get default collateral factor for category
    pub fn default_collateral_factor(&self) -> Decimal {
        match self {
            AssetCategory::Stablecoin => dec!(0.95),
            AssetCategory::MajorCrypto => dec!(0.80),
            AssetCategory::Altcoin => dec!(0.60),
            AssetCategory::PlatformToken => dec!(0.70),
            AssetCategory::LiquidityToken => dec!(0.50),
        }
    }

    /// Get default liquidation threshold for category
    pub fn default_liquidation_threshold(&self) -> Decimal {
        match self {
            AssetCategory::Stablecoin => dec!(0.98),
            AssetCategory::MajorCrypto => dec!(0.85),
            AssetCategory::Altcoin => dec!(0.75),
            AssetCategory::PlatformToken => dec!(0.80),
            AssetCategory::LiquidityToken => dec!(0.70),
        }
    }

    /// Get default haircut for category
    pub fn default_haircut(&self) -> Decimal {
        match self {
            AssetCategory::Stablecoin => dec!(0.02),     // 2%
            AssetCategory::MajorCrypto => dec!(0.10),    // 10%
            AssetCategory::Altcoin => dec!(0.20),        // 20%
            AssetCategory::PlatformToken => dec!(0.15),  // 15%
            AssetCategory::LiquidityToken => dec!(0.25), // 25%
        }
    }
}

impl CollateralConfig {
    /// Create new collateral config with defaults
    pub fn new(symbol: String, category: AssetCategory) -> Self {
        Self {
            symbol,
            collateral_factor: category.default_collateral_factor(),
            liquidation_threshold: category.default_liquidation_threshold(),
            haircut: category.default_haircut(),
            min_deposit: dec!(10),
            max_deposit: None,
            enabled: true,
            category,
            updated_at: Utc::now(),
        }
    }

    /// Calculate effective value after haircut
    pub fn effective_value(&self, amount: Decimal, price: Decimal) -> Decimal {
        let gross_value = amount * price;
        gross_value * (Decimal::ONE - self.haircut)
    }

    /// Calculate borrowing power
    pub fn borrowing_power(&self, amount: Decimal, price: Decimal) -> Decimal {
        let effective_value = self.effective_value(amount, price);
        effective_value * self.collateral_factor
    }
}

/// Collateral deposit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralAssetDeposit {
    /// Deposit ID
    pub deposit_id: String,

    /// User ID
    pub user_id: String,

    /// Asset symbol
    pub symbol: String,

    /// Amount deposited
    pub amount: Decimal,

    /// Price at deposit (USD)
    pub deposit_price: Decimal,

    /// Current price (USD)
    pub current_price: Decimal,

    /// Is this deposit locked
    pub locked: bool,

    /// Lock expiry (if locked)
    pub lock_expiry: Option<DateTime<Utc>>,

    /// Deposited timestamp
    pub deposited_at: DateTime<Utc>,
}

impl CollateralAssetDeposit {
    /// Create new collateral deposit
    pub fn new(
        deposit_id: String,
        user_id: String,
        symbol: String,
        amount: Decimal,
        price: Decimal,
    ) -> Self {
        Self {
            deposit_id,
            user_id,
            symbol,
            amount,
            deposit_price: price,
            current_price: price,
            locked: false,
            lock_expiry: None,
            deposited_at: Utc::now(),
        }
    }

    /// Get current value in USD
    pub fn current_value(&self) -> Decimal {
        self.amount * self.current_price
    }

    /// Get PnL since deposit
    pub fn unrealized_pnl(&self) -> Decimal {
        self.amount * (self.current_price - self.deposit_price)
    }

    /// Update current price
    pub fn update_price(&mut self, price: Decimal) {
        self.current_price = price;
    }

    /// Check if deposit is locked
    pub fn is_locked(&self) -> bool {
        if !self.locked {
            return false;
        }

        if let Some(expiry) = self.lock_expiry {
            Utc::now() < expiry
        } else {
            true
        }
    }
}

/// Collateral portfolio for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralPortfolio {
    /// User ID
    pub user_id: String,

    /// All deposits
    pub deposits: Vec<CollateralAssetDeposit>,

    /// Total borrowed amount (in USD)
    pub total_borrowed: Decimal,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl CollateralPortfolio {
    /// Create new collateral portfolio
    pub fn new(user_id: String) -> Self {
        Self {
            user_id,
            deposits: Vec::new(),
            total_borrowed: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Add collateral deposit
    pub fn add_deposit(&mut self, deposit: CollateralAssetDeposit) {
        self.deposits.push(deposit);
        self.updated_at = Utc::now();
    }

    /// Remove collateral deposit
    pub fn remove_deposit(&mut self, deposit_id: &str) -> Result<CollateralAssetDeposit> {
        let idx = self
            .deposits
            .iter()
            .position(|d| d.deposit_id == deposit_id)
            .ok_or_else(|| CoreError::NotFound(format!("Deposit {} not found", deposit_id)))?;

        let deposit = self.deposits.remove(idx);

        if deposit.is_locked() {
            return Err(CoreError::Validation(
                "Cannot withdraw locked collateral".to_string(),
            ));
        }

        self.updated_at = Utc::now();
        Ok(deposit)
    }

    /// Calculate total collateral value
    pub fn total_collateral_value(&self) -> Decimal {
        self.deposits.iter().map(|d| d.current_value()).sum()
    }

    /// Calculate effective collateral value (with configs)
    pub fn effective_collateral_value(
        &self,
        configs: &HashMap<String, CollateralConfig>,
    ) -> Decimal {
        self.deposits
            .iter()
            .filter_map(|d| {
                configs
                    .get(&d.symbol)
                    .map(|config| config.effective_value(d.amount, d.current_price))
            })
            .sum()
    }

    /// Calculate total borrowing power
    pub fn total_borrowing_power(&self, configs: &HashMap<String, CollateralConfig>) -> Decimal {
        self.deposits
            .iter()
            .filter_map(|d| {
                configs
                    .get(&d.symbol)
                    .map(|config| config.borrowing_power(d.amount, d.current_price))
            })
            .sum()
    }

    /// Calculate available borrowing power
    pub fn available_borrowing_power(
        &self,
        configs: &HashMap<String, CollateralConfig>,
    ) -> Decimal {
        let total_power = self.total_borrowing_power(configs);
        (total_power - self.total_borrowed).max(Decimal::ZERO)
    }

    /// Calculate health factor
    /// Health factor = (collateral value * liquidation threshold) / total borrowed
    /// < 1.0 means account is subject to liquidation
    pub fn health_factor(&self, configs: &HashMap<String, CollateralConfig>) -> Decimal {
        if self.total_borrowed.is_zero() {
            return Decimal::MAX;
        }

        let liquidation_value: Decimal = self
            .deposits
            .iter()
            .filter_map(|d| {
                configs.get(&d.symbol).map(|config| {
                    let value = d.current_value();
                    value * config.liquidation_threshold
                })
            })
            .sum();

        liquidation_value / self.total_borrowed
    }

    /// Check if portfolio is healthy
    pub fn is_healthy(&self, configs: &HashMap<String, CollateralConfig>) -> bool {
        self.health_factor(configs) > Decimal::ONE
    }

    /// Update deposit prices
    pub fn update_prices(&mut self, prices: &HashMap<String, Decimal>) {
        for deposit in &mut self.deposits {
            if let Some(&price) = prices.get(&deposit.symbol) {
                deposit.update_price(price);
            }
        }
        self.updated_at = Utc::now();
    }

    /// Add borrowing
    pub fn add_borrowing(&mut self, amount: Decimal) {
        self.total_borrowed += amount;
        self.updated_at = Utc::now();
    }

    /// Repay borrowing
    pub fn repay_borrowing(&mut self, amount: Decimal) {
        self.total_borrowed = (self.total_borrowed - amount).max(Decimal::ZERO);
        self.updated_at = Utc::now();
    }

    /// Get collateral breakdown by asset
    pub fn breakdown(&self) -> HashMap<String, Decimal> {
        let mut breakdown = HashMap::new();

        for deposit in &self.deposits {
            *breakdown
                .entry(deposit.symbol.clone())
                .or_insert(Decimal::ZERO) += deposit.current_value();
        }

        breakdown
    }
}

/// Collateral manager
pub struct CollateralManager {
    /// Collateral configurations
    configs: HashMap<String, CollateralConfig>,

    /// User portfolios
    portfolios: HashMap<String, CollateralPortfolio>,
}

impl CollateralManager {
    /// Create new collateral manager
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            portfolios: HashMap::new(),
        }
    }

    /// Add collateral config
    pub fn add_config(&mut self, config: CollateralConfig) {
        self.configs.insert(config.symbol.clone(), config);
    }

    /// Get collateral config
    pub fn get_config(&self, symbol: &str) -> Option<&CollateralConfig> {
        self.configs.get(symbol)
    }

    /// Get or create user portfolio
    pub fn get_or_create_portfolio(&mut self, user_id: &str) -> &mut CollateralPortfolio {
        self.portfolios
            .entry(user_id.to_string())
            .or_insert_with(|| CollateralPortfolio::new(user_id.to_string()))
    }

    /// Get user portfolio
    pub fn get_portfolio(&self, user_id: &str) -> Option<&CollateralPortfolio> {
        self.portfolios.get(user_id)
    }

    /// Deposit collateral
    pub fn deposit_collateral(&mut self, deposit: CollateralAssetDeposit) -> Result<()> {
        // Validate asset is enabled
        let config = self.configs.get(&deposit.symbol).ok_or_else(|| {
            CoreError::NotFound(format!(
                "Collateral config for {} not found",
                deposit.symbol
            ))
        })?;

        if !config.enabled {
            return Err(CoreError::Validation(format!(
                "{} is not enabled as collateral",
                deposit.symbol
            )));
        }

        // Validate amount
        if deposit.amount < config.min_deposit {
            return Err(CoreError::Validation(format!(
                "Deposit amount {} below minimum {}",
                deposit.amount, config.min_deposit
            )));
        }

        if let Some(max) = config.max_deposit {
            if deposit.amount > max {
                return Err(CoreError::Validation(format!(
                    "Deposit amount {} exceeds maximum {}",
                    deposit.amount, max
                )));
            }
        }

        let portfolio = self.get_or_create_portfolio(&deposit.user_id);
        portfolio.add_deposit(deposit);

        Ok(())
    }

    /// Withdraw collateral
    pub fn withdraw_collateral(
        &mut self,
        user_id: &str,
        deposit_id: &str,
    ) -> Result<CollateralAssetDeposit> {
        let portfolio = self
            .portfolios
            .get_mut(user_id)
            .ok_or_else(|| CoreError::NotFound(format!("Portfolio for {} not found", user_id)))?;

        let deposit = portfolio.remove_deposit(deposit_id)?;

        // Check if withdrawal would make portfolio unhealthy
        if !portfolio.is_healthy(&self.configs) {
            // Restore deposit
            portfolio.add_deposit(deposit.clone());
            return Err(CoreError::Validation(
                "Withdrawal would make portfolio unhealthy".to_string(),
            ));
        }

        Ok(deposit)
    }

    /// Update all prices
    pub fn update_prices(&mut self, prices: &HashMap<String, Decimal>) {
        for portfolio in self.portfolios.values_mut() {
            portfolio.update_prices(prices);
        }
    }

    /// Get unhealthy portfolios
    pub fn get_unhealthy_portfolios(&self) -> Vec<String> {
        self.portfolios
            .iter()
            .filter(|(_, portfolio)| !portfolio.is_healthy(&self.configs))
            .map(|(user_id, _)| user_id.clone())
            .collect()
    }

    /// Calculate total value locked (TVL)
    pub fn total_value_locked(&self) -> Decimal {
        self.portfolios
            .values()
            .map(|p| p.total_collateral_value())
            .sum()
    }
}

impl Default for CollateralManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_asset_category_defaults() {
        assert_eq!(
            AssetCategory::Stablecoin.default_collateral_factor(),
            dec!(0.95)
        );
        assert_eq!(
            AssetCategory::MajorCrypto.default_collateral_factor(),
            dec!(0.80)
        );
    }

    #[test]
    fn test_collateral_config() {
        let config = CollateralConfig::new("BTC".to_string(), AssetCategory::MajorCrypto);

        let effective_value = config.effective_value(dec!(1), dec!(50000));
        assert_eq!(effective_value, dec!(45000)); // 50000 * (1 - 0.1)

        let borrowing_power = config.borrowing_power(dec!(1), dec!(50000));
        assert_eq!(borrowing_power, dec!(36000)); // 45000 * 0.8
    }

    #[test]
    fn test_collateral_deposit() {
        let deposit = CollateralAssetDeposit::new(
            "dep1".to_string(),
            "user1".to_string(),
            "BTC".to_string(),
            dec!(10),
            dec!(50000),
        );

        assert_eq!(deposit.current_value(), dec!(500000));
        assert_eq!(deposit.unrealized_pnl(), Decimal::ZERO);
    }

    #[test]
    fn test_collateral_portfolio() {
        let mut portfolio = CollateralPortfolio::new("user1".to_string());

        let deposit = CollateralAssetDeposit::new(
            "dep1".to_string(),
            "user1".to_string(),
            "BTC".to_string(),
            dec!(1),
            dec!(50000),
        );

        portfolio.add_deposit(deposit);
        assert_eq!(portfolio.total_collateral_value(), dec!(50000));
    }

    #[test]
    fn test_health_factor() {
        let mut portfolio = CollateralPortfolio::new("user1".to_string());

        let deposit = CollateralAssetDeposit::new(
            "dep1".to_string(),
            "user1".to_string(),
            "BTC".to_string(),
            dec!(1),
            dec!(50000),
        );

        portfolio.add_deposit(deposit);
        portfolio.add_borrowing(dec!(30000));

        let mut configs = HashMap::new();
        configs.insert(
            "BTC".to_string(),
            CollateralConfig::new("BTC".to_string(), AssetCategory::MajorCrypto),
        );

        let health = portfolio.health_factor(&configs);
        assert!(health > Decimal::ONE);
        assert!(portfolio.is_healthy(&configs));
    }

    #[test]
    fn test_collateral_manager() {
        let mut manager = CollateralManager::new();

        manager.add_config(CollateralConfig::new(
            "BTC".to_string(),
            AssetCategory::MajorCrypto,
        ));

        let deposit = CollateralAssetDeposit::new(
            "dep1".to_string(),
            "user1".to_string(),
            "BTC".to_string(),
            dec!(10), // Meets minimum deposit requirement
            dec!(50000),
        );

        manager.deposit_collateral(deposit).unwrap();

        let portfolio = manager.get_portfolio("user1").unwrap();
        assert_eq!(portfolio.deposits.len(), 1);
    }
}
