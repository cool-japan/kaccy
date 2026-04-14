//! Margin trading system
//!
//! This module provides margin trading functionality including:
//! - Margin account management
//! - Leverage calculation
//! - Maintenance margin requirements
//! - Margin call triggers

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Margin account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginAccount {
    /// Account ID
    pub account_id: String,

    /// User ID
    pub user_id: String,

    /// Collateral balance (in base currency)
    pub collateral: Decimal,

    /// Borrowed amount (in base currency)
    pub borrowed: Decimal,

    /// Current positions
    pub positions: Vec<MarginPosition>,

    /// Maximum leverage allowed
    pub max_leverage: Decimal,

    /// Maintenance margin ratio (e.g., 0.25 for 25%)
    pub maintenance_margin_ratio: Decimal,

    /// Initial margin ratio (e.g., 0.5 for 50%)
    pub initial_margin_ratio: Decimal,

    /// Account status
    pub status: MarginAccountStatus,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Margin account status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarginAccountStatus {
    /// Account is active and can trade
    Active,

    /// Account is under margin call
    MarginCall,

    /// Account is being liquidated
    Liquidating,

    /// Account is suspended
    Suspended,

    /// Account is closed
    Closed,
}

/// Margin position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginPosition {
    /// Position ID
    pub position_id: String,

    /// Token symbol
    pub token_symbol: String,

    /// Position size (positive for long, negative for short)
    pub size: Decimal,

    /// Entry price
    pub entry_price: Decimal,

    /// Current price
    pub current_price: Decimal,

    /// Leverage used
    pub leverage: Decimal,

    /// Borrowed amount for this position
    pub borrowed: Decimal,

    /// Unrealized PnL
    pub unrealized_pnl: Decimal,

    /// Position side
    pub side: PositionSide,

    /// Opened timestamp
    pub opened_at: DateTime<Utc>,
}

/// Position side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    /// Long position (profits when price rises).
    Long,
    /// Short position (profits when price falls).
    Short,
}

impl MarginPosition {
    /// Calculate unrealized PnL
    pub fn calculate_unrealized_pnl(&self) -> Decimal {
        match self.side {
            PositionSide::Long => (self.current_price - self.entry_price) * self.size.abs(),
            PositionSide::Short => (self.entry_price - self.current_price) * self.size.abs(),
        }
    }

    /// Update current price and recalculate PnL
    pub fn update_price(&mut self, current_price: Decimal) {
        self.current_price = current_price;
        self.unrealized_pnl = self.calculate_unrealized_pnl();
    }
}

impl MarginAccount {
    /// Create a new margin account
    pub fn new(
        account_id: String,
        user_id: String,
        collateral: Decimal,
        max_leverage: Decimal,
    ) -> Self {
        Self {
            account_id,
            user_id,
            collateral,
            borrowed: Decimal::ZERO,
            positions: Vec::new(),
            max_leverage,
            maintenance_margin_ratio: dec!(0.25), // 25%
            initial_margin_ratio: dec!(0.5),      // 50%
            status: MarginAccountStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Calculate total account value
    pub fn calculate_account_value(&self) -> Decimal {
        let unrealized_pnl: Decimal = self.positions.iter().map(|p| p.unrealized_pnl).sum();

        self.collateral + unrealized_pnl
    }

    /// Calculate equity (account value - borrowed)
    pub fn calculate_equity(&self) -> Decimal {
        self.calculate_account_value() - self.borrowed
    }

    /// Calculate margin ratio (equity / position value)
    pub fn calculate_margin_ratio(&self) -> Decimal {
        let position_value: Decimal = self
            .positions
            .iter()
            .map(|p| p.current_price * p.size.abs())
            .sum();

        if position_value.is_zero() {
            return Decimal::ONE;
        }

        self.calculate_equity() / position_value
    }

    /// Calculate current leverage (position value / equity)
    pub fn calculate_current_leverage(&self) -> Decimal {
        let equity = self.calculate_equity();

        if equity <= Decimal::ZERO {
            return Decimal::MAX;
        }

        let position_value: Decimal = self
            .positions
            .iter()
            .map(|p| p.current_price * p.size.abs())
            .sum();

        if position_value.is_zero() {
            return Decimal::ZERO;
        }

        position_value / equity
    }

    /// Check if account is under margin call
    pub fn is_margin_call(&self) -> bool {
        let margin_ratio = self.calculate_margin_ratio();
        margin_ratio < self.maintenance_margin_ratio && !self.positions.is_empty()
    }

    /// Check if account should be liquidated
    pub fn should_liquidate(&self) -> bool {
        let equity = self.calculate_equity();
        equity <= Decimal::ZERO || self.calculate_margin_ratio() < self.maintenance_margin_ratio
    }

    /// Calculate maximum position size for given leverage
    pub fn calculate_max_position_size(
        &self,
        price: Decimal,
        leverage: Decimal,
    ) -> Result<Decimal> {
        if leverage > self.max_leverage {
            return Err(CoreError::Validation(format!(
                "Leverage {} exceeds maximum {}",
                leverage, self.max_leverage
            )));
        }

        let available_equity = self.calculate_equity();
        let max_position_value = available_equity * leverage;

        Ok(max_position_value / price)
    }

    /// Open a new margin position
    pub fn open_position(
        &mut self,
        position_id: String,
        token_symbol: String,
        size: Decimal,
        price: Decimal,
        leverage: Decimal,
        side: PositionSide,
    ) -> Result<()> {
        // Validate leverage
        if leverage > self.max_leverage {
            return Err(CoreError::Validation(format!(
                "Leverage {} exceeds maximum {}",
                leverage, self.max_leverage
            )));
        }

        // Calculate position value
        let position_value = size.abs() * price;

        // Calculate required collateral
        let required_collateral = position_value / leverage;

        // Check if enough equity
        let available_equity = self.calculate_equity();
        if required_collateral > available_equity {
            return Err(CoreError::InsufficientBalance {
                required: required_collateral,
                available: available_equity,
            });
        }

        // Calculate borrowed amount
        let borrowed = position_value - required_collateral;

        // Create position
        let position = MarginPosition {
            position_id,
            token_symbol,
            size,
            entry_price: price,
            current_price: price,
            leverage,
            borrowed,
            unrealized_pnl: Decimal::ZERO,
            side,
            opened_at: Utc::now(),
        };

        self.positions.push(position);
        self.borrowed += borrowed;
        self.updated_at = Utc::now();

        // Check if margin call needed
        if self.is_margin_call() {
            self.status = MarginAccountStatus::MarginCall;
        }

        Ok(())
    }

    /// Close a margin position
    pub fn close_position(&mut self, position_id: &str, closing_price: Decimal) -> Result<Decimal> {
        let position_idx = self
            .positions
            .iter()
            .position(|p| p.position_id == position_id)
            .ok_or_else(|| CoreError::NotFound(format!("Position {} not found", position_id)))?;

        let mut position = self.positions.remove(position_idx);

        // Update to closing price
        position.update_price(closing_price);

        // Calculate realized PnL
        let realized_pnl = position.unrealized_pnl;

        // Update account
        self.collateral += realized_pnl;
        self.borrowed -= position.borrowed;
        self.updated_at = Utc::now();

        // Update status if no longer under margin call
        if !self.is_margin_call() && self.status == MarginAccountStatus::MarginCall {
            self.status = MarginAccountStatus::Active;
        }

        Ok(realized_pnl)
    }

    /// Update all position prices
    pub fn update_prices(&mut self, prices: &HashMap<String, Decimal>) {
        for position in &mut self.positions {
            if let Some(&price) = prices.get(&position.token_symbol) {
                position.update_price(price);
            }
        }

        self.updated_at = Utc::now();

        // Check margin status
        if self.is_margin_call() {
            self.status = MarginAccountStatus::MarginCall;
        } else if self.status == MarginAccountStatus::MarginCall {
            self.status = MarginAccountStatus::Active;
        }
    }

    /// Add collateral to account
    pub fn add_collateral(&mut self, amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        self.collateral += amount;
        self.updated_at = Utc::now();

        // Update status if no longer under margin call
        if !self.is_margin_call() && self.status == MarginAccountStatus::MarginCall {
            self.status = MarginAccountStatus::Active;
        }

        Ok(())
    }

    /// Withdraw collateral from account
    pub fn withdraw_collateral(&mut self, amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        let available = self.calculate_equity();
        if amount > available {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available,
            });
        }

        self.collateral -= amount;
        self.updated_at = Utc::now();

        // Check if withdrawal triggers margin call
        if self.is_margin_call() {
            return Err(CoreError::Validation(
                "Withdrawal would trigger margin call".to_string(),
            ));
        }

        Ok(())
    }
}

/// Margin trading manager
pub struct MarginTradingManager {
    /// All margin accounts
    accounts: HashMap<String, MarginAccount>,

    /// Default maximum leverage
    default_max_leverage: Decimal,

    /// Interest rate per day (e.g., 0.001 for 0.1% per day)
    daily_interest_rate: Decimal,
}

impl MarginTradingManager {
    /// Create a new margin trading manager
    pub fn new(default_max_leverage: Decimal, daily_interest_rate: Decimal) -> Self {
        Self {
            accounts: HashMap::new(),
            default_max_leverage,
            daily_interest_rate,
        }
    }

    /// Create a new margin account
    pub fn create_account(
        &mut self,
        account_id: String,
        user_id: String,
        collateral: Decimal,
        max_leverage: Option<Decimal>,
    ) -> Result<MarginAccount> {
        if self.accounts.contains_key(&account_id) {
            return Err(CoreError::Validation(format!(
                "Account {} already exists",
                account_id
            )));
        }

        let account = MarginAccount::new(
            account_id.clone(),
            user_id,
            collateral,
            max_leverage.unwrap_or(self.default_max_leverage),
        );

        self.accounts.insert(account_id, account.clone());

        Ok(account)
    }

    /// Get margin account
    pub fn get_account(&self, account_id: &str) -> Option<&MarginAccount> {
        self.accounts.get(account_id)
    }

    /// Get mutable margin account
    pub fn get_account_mut(&mut self, account_id: &str) -> Option<&mut MarginAccount> {
        self.accounts.get_mut(account_id)
    }

    /// Process margin calls for all accounts
    pub fn process_margin_calls(&mut self, prices: &HashMap<String, Decimal>) -> Vec<String> {
        let mut margin_call_accounts = Vec::new();

        for (account_id, account) in &mut self.accounts {
            account.update_prices(prices);

            if account.is_margin_call() && account.status != MarginAccountStatus::MarginCall {
                account.status = MarginAccountStatus::MarginCall;
                margin_call_accounts.push(account_id.clone());
            }
        }

        margin_call_accounts
    }

    /// Get accounts that should be liquidated
    pub fn get_liquidation_candidates(&self) -> Vec<String> {
        self.accounts
            .iter()
            .filter(|(_, account)| account.should_liquidate())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Liquidate an account
    pub fn liquidate_account(
        &mut self,
        account_id: &str,
        prices: &HashMap<String, Decimal>,
    ) -> Result<LiquidationResult> {
        let account = self
            .accounts
            .get_mut(account_id)
            .ok_or_else(|| CoreError::NotFound(format!("Account {} not found", account_id)))?;

        account.status = MarginAccountStatus::Liquidating;
        account.update_prices(prices);

        let mut total_pnl = Decimal::ZERO;
        let positions_closed = account.positions.len();

        // Close all positions
        while !account.positions.is_empty() {
            let position = account.positions.remove(0);
            let _closing_price = prices
                .get(&position.token_symbol)
                .copied()
                .unwrap_or(position.current_price);

            total_pnl += position.calculate_unrealized_pnl();
            account.borrowed -= position.borrowed;
        }

        account.collateral += total_pnl;
        account.status = MarginAccountStatus::Closed;
        account.updated_at = Utc::now();

        Ok(LiquidationResult {
            account_id: account_id.to_string(),
            positions_closed,
            total_pnl,
            remaining_equity: account.calculate_equity(),
        })
    }

    /// Apply interest to all accounts
    pub fn apply_interest(&mut self) {
        for account in self.accounts.values_mut() {
            let interest = account.borrowed * self.daily_interest_rate;
            account.borrowed += interest;
            account.updated_at = Utc::now();
        }
    }
}

/// Liquidation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationResult {
    /// Identifier of the account that was liquidated.
    pub account_id: String,
    /// Number of positions closed during liquidation.
    pub positions_closed: usize,
    /// Total realized profit and loss from closed positions.
    pub total_pnl: Decimal,
    /// Equity remaining after liquidation penalties.
    pub remaining_equity: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_margin_account_creation() {
        let account =
            MarginAccount::new("acc1".to_string(), "user1".to_string(), dec!(1000), dec!(5));

        assert_eq!(account.collateral, dec!(1000));
        assert_eq!(account.max_leverage, dec!(5));
        assert_eq!(account.status, MarginAccountStatus::Active);
    }

    #[test]
    fn test_open_long_position() {
        let mut account = MarginAccount::new(
            "acc1".to_string(),
            "user1".to_string(),
            dec!(30000), // Increased collateral
            dec!(5),
        );

        account
            .open_position(
                "pos1".to_string(),
                "BTC".to_string(),
                dec!(1),
                dec!(50000),
                dec!(2),
                PositionSide::Long,
            )
            .unwrap();

        assert_eq!(account.positions.len(), 1);
        assert!(account.borrowed > Decimal::ZERO);
    }

    #[test]
    fn test_margin_call() {
        let mut account = MarginAccount::new(
            "acc1".to_string(),
            "user1".to_string(),
            dec!(15000), // Increased collateral
            dec!(5),
        );

        // Open a position
        account
            .open_position(
                "pos1".to_string(),
                "BTC".to_string(),
                dec!(1),
                dec!(50000),
                dec!(4),
                PositionSide::Long,
            )
            .unwrap();

        // Price drops significantly
        let mut prices = HashMap::new();
        prices.insert("BTC".to_string(), dec!(40000));
        account.update_prices(&prices);

        assert!(account.is_margin_call());
    }

    #[test]
    fn test_close_position_with_profit() {
        let mut account = MarginAccount::new(
            "acc1".to_string(),
            "user1".to_string(),
            dec!(30000), // Increased collateral
            dec!(5),
        );

        account
            .open_position(
                "pos1".to_string(),
                "BTC".to_string(),
                dec!(1),
                dec!(50000),
                dec!(2),
                PositionSide::Long,
            )
            .unwrap();

        let initial_collateral = account.collateral;

        // Close at higher price
        let pnl = account.close_position("pos1", dec!(55000)).unwrap();

        assert!(pnl > Decimal::ZERO);
        assert!(account.collateral > initial_collateral);
        assert_eq!(account.positions.len(), 0);
    }

    #[test]
    fn test_margin_trading_manager() {
        let mut manager = MarginTradingManager::new(dec!(5), dec!(0.001));

        let account = manager
            .create_account("acc1".to_string(), "user1".to_string(), dec!(1000), None)
            .unwrap();

        assert_eq!(account.account_id, "acc1");
        assert!(manager.get_account("acc1").is_some());
    }
}
