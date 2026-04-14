//! Cross-margining system for capital efficiency
//!
//! This module implements cross-margining which allows traders to use
//! unrealized profits from one position as collateral for another position,
//! reducing overall margin requirements and improving capital efficiency.
//!
//! Key features:
//! - Portfolio-level margin calculation
//! - Risk offsetting between correlated positions
//! - Margin optimization across all positions
//! - Netting of positions in opposite directions
//! - Dynamic margin requirements based on portfolio risk

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

/// Cross-margin position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossMarginPosition {
    /// Position ID
    pub id: String,
    /// Symbol
    pub symbol: String,
    /// Side (1 for long, -1 for short)
    pub side: Decimal,
    /// Position size
    pub size: Decimal,
    /// Entry price
    pub entry_price: Decimal,
    /// Current mark price
    pub mark_price: Decimal,
    /// Unrealized PnL
    pub unrealized_pnl: Decimal,
    /// Leverage
    pub leverage: Decimal,
    /// Position-specific margin
    pub position_margin: Decimal,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl CrossMarginPosition {
    /// Create a new cross-margin position
    pub fn new(
        id: String,
        symbol: String,
        side: Decimal,
        size: Decimal,
        entry_price: Decimal,
        leverage: Decimal,
    ) -> Result<Self> {
        if size <= dec!(0) {
            return Err(CoreError::Validation("Size must be positive".to_string()));
        }
        if entry_price <= dec!(0) {
            return Err(CoreError::Validation(
                "Entry price must be positive".to_string(),
            ));
        }
        if leverage <= dec!(0) {
            return Err(CoreError::Validation(
                "Leverage must be positive".to_string(),
            ));
        }

        let notional = size * entry_price;
        let position_margin = notional / leverage;

        let now = Utc::now();
        Ok(Self {
            id,
            symbol,
            side,
            size,
            entry_price,
            mark_price: entry_price,
            unrealized_pnl: dec!(0),
            leverage,
            position_margin,
            created_at: now,
            updated_at: now,
        })
    }

    /// Update mark price and unrealized PnL
    pub fn update_mark_price(&mut self, mark_price: Decimal) {
        self.mark_price = mark_price;
        let price_diff = mark_price - self.entry_price;
        self.unrealized_pnl = self.side * price_diff * self.size;
        self.updated_at = Utc::now();
    }

    /// Get notional value
    pub fn notional_value(&self) -> Decimal {
        self.size * self.mark_price
    }

    /// Get margin ratio for this position
    pub fn margin_ratio(&self) -> Decimal {
        let equity = self.position_margin + self.unrealized_pnl;
        if self.notional_value() == dec!(0) {
            return dec!(100);
        }
        (equity / self.notional_value()) * dec!(100)
    }
}

/// Portfolio margin requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMargin {
    /// Initial margin requirement
    pub initial_margin: Decimal,
    /// Maintenance margin requirement
    pub maintenance_margin: Decimal,
    /// Risk-adjusted margin (accounts for correlations)
    pub risk_adjusted_margin: Decimal,
    /// Margin utilization ratio (%)
    pub utilization: Decimal,
}

impl PortfolioMargin {
    /// Check if portfolio meets initial margin requirements
    pub fn meets_initial_margin(&self, available_balance: Decimal) -> bool {
        available_balance >= self.initial_margin
    }

    /// Check if portfolio meets maintenance margin requirements
    pub fn meets_maintenance_margin(&self, account_equity: Decimal) -> bool {
        account_equity >= self.maintenance_margin
    }

    /// Calculate excess margin
    pub fn excess_margin(&self, available_balance: Decimal) -> Decimal {
        (available_balance - self.initial_margin).max(dec!(0))
    }
}

/// Risk factor for correlation adjustments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// Base risk factor (1.0 = 100% margin)
    pub base: Decimal,
    /// Concentration risk multiplier
    pub concentration: Decimal,
    /// Correlation benefit (reduction factor)
    pub correlation_benefit: Decimal,
}

impl RiskFactor {
    /// Calculate effective risk factor
    pub fn effective(&self) -> Decimal {
        self.base * self.concentration * self.correlation_benefit
    }
}

impl Default for RiskFactor {
    fn default() -> Self {
        Self {
            base: dec!(1.0),
            concentration: dec!(1.0),
            correlation_benefit: dec!(1.0),
        }
    }
}

/// Cross-margin account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossMarginAccount {
    /// Account ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Total collateral (cash balance)
    pub collateral: Decimal,
    /// Positions by ID
    pub positions: HashMap<String, CrossMarginPosition>,
    /// Total unrealized PnL across all positions
    pub total_unrealized_pnl: Decimal,
    /// Account equity (collateral + unrealized PnL)
    pub equity: Decimal,
    /// Margin requirements
    pub margin: PortfolioMargin,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl CrossMarginAccount {
    /// Create a new cross-margin account
    pub fn new(id: String, user_id: String, initial_collateral: Decimal) -> Result<Self> {
        if initial_collateral < dec!(0) {
            return Err(CoreError::Validation(
                "Initial collateral cannot be negative".to_string(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id,
            user_id,
            collateral: initial_collateral,
            positions: HashMap::new(),
            total_unrealized_pnl: dec!(0),
            equity: initial_collateral,
            margin: PortfolioMargin {
                initial_margin: dec!(0),
                maintenance_margin: dec!(0),
                risk_adjusted_margin: dec!(0),
                utilization: dec!(0),
            },
            created_at: now,
            updated_at: now,
        })
    }

    /// Add position to account
    pub fn add_position(&mut self, position: CrossMarginPosition) -> Result<()> {
        if self.positions.contains_key(&position.id) {
            return Err(CoreError::Validation("Position already exists".to_string()));
        }

        self.positions.insert(position.id.clone(), position);
        self.recalculate_margin()?;
        Ok(())
    }

    /// Remove position from account
    pub fn remove_position(&mut self, position_id: &str) -> Result<CrossMarginPosition> {
        let position = self
            .positions
            .remove(position_id)
            .ok_or_else(|| CoreError::Validation("Position not found".to_string()))?;

        self.recalculate_margin()?;
        Ok(position)
    }

    /// Update mark prices for all positions
    pub fn update_mark_prices(&mut self, prices: &HashMap<String, Decimal>) -> Result<()> {
        for position in self.positions.values_mut() {
            if let Some(&mark_price) = prices.get(&position.symbol) {
                position.update_mark_price(mark_price);
            }
        }

        self.recalculate_margin()?;
        Ok(())
    }

    /// Recalculate margin requirements
    fn recalculate_margin(&mut self) -> Result<()> {
        // Calculate total unrealized PnL
        self.total_unrealized_pnl = self.positions.values().map(|p| p.unrealized_pnl).sum();

        // Calculate account equity
        self.equity = self.collateral + self.total_unrealized_pnl;

        // Calculate portfolio margin requirements
        let mut total_position_margin = dec!(0);
        let mut total_notional = dec!(0);

        for position in self.positions.values() {
            total_position_margin += position.position_margin;
            total_notional += position.notional_value();
        }

        // Calculate netting benefit (positions in opposite directions offset risk)
        let netting_benefit = self.calculate_netting_benefit();

        // Risk-adjusted margin accounts for correlations and netting
        self.margin.risk_adjusted_margin = total_position_margin * netting_benefit;

        // Initial margin is typically 100% of risk-adjusted
        self.margin.initial_margin = self.margin.risk_adjusted_margin;

        // Maintenance margin is typically 50-75% of initial margin
        self.margin.maintenance_margin = self.margin.initial_margin * dec!(0.5);

        // Calculate margin utilization
        if self.equity > dec!(0) {
            self.margin.utilization = (self.margin.initial_margin / self.equity) * dec!(100);
        } else {
            self.margin.utilization = dec!(999); // Very high utilization if negative equity
        }

        self.updated_at = Utc::now();
        Ok(())
    }

    /// Calculate netting benefit from offsetting positions
    fn calculate_netting_benefit(&self) -> Decimal {
        // Calculate gross exposure (sum of all absolute position notionals)
        let mut gross_exposure = dec!(0);
        let mut symbol_exposures: HashMap<String, Decimal> = HashMap::new();

        for position in self.positions.values() {
            let exposure = position.side * position.notional_value();
            gross_exposure += exposure.abs();
            *symbol_exposures
                .entry(position.symbol.clone())
                .or_insert(dec!(0)) += exposure;
        }

        // Calculate net exposure (sum of netted exposures by symbol)
        let net_exposure: Decimal = symbol_exposures.values().map(|e| e.abs()).sum();

        if gross_exposure == dec!(0) {
            return dec!(1.0);
        }

        // Netting benefit: ratio of net to gross exposure
        // Higher netting = lower benefit factor = lower margin requirement
        let netting_ratio = net_exposure / gross_exposure;

        // Benefit ranges from 0.5 (50% netting) to 1.0 (no netting)
        dec!(0.5) + (netting_ratio * dec!(0.5))
    }

    /// Check if account can open new position
    pub fn can_open_position(&self, required_margin: Decimal) -> bool {
        let available_margin = self.equity - self.margin.initial_margin;
        available_margin >= required_margin
    }

    /// Check if account is subject to liquidation
    pub fn is_liquidatable(&self) -> bool {
        self.equity < self.margin.maintenance_margin
    }

    /// Get available balance for new positions
    pub fn available_balance(&self) -> Decimal {
        (self.equity - self.margin.initial_margin).max(dec!(0))
    }

    /// Get margin call threshold
    pub fn margin_call_level(&self) -> Decimal {
        if self.margin.initial_margin == dec!(0) {
            return dec!(100);
        }

        (self.margin.maintenance_margin / self.margin.initial_margin) * dec!(100)
    }

    /// Add collateral to account
    pub fn add_collateral(&mut self, amount: Decimal) -> Result<()> {
        if amount <= dec!(0) {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        self.collateral += amount;
        self.recalculate_margin()?;
        Ok(())
    }

    /// Withdraw collateral from account
    pub fn withdraw_collateral(&mut self, amount: Decimal) -> Result<()> {
        if amount <= dec!(0) {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        // Check if withdrawal would violate margin requirements
        let new_collateral = self.collateral - amount;
        let new_equity = new_collateral + self.total_unrealized_pnl;

        if new_equity < self.margin.initial_margin {
            return Err(CoreError::Validation(
                "Withdrawal would violate margin requirements".to_string(),
            ));
        }

        self.collateral = new_collateral;
        self.recalculate_margin()?;
        Ok(())
    }

    /// Get portfolio summary
    pub fn get_summary(&self) -> PortfolioSummary {
        PortfolioSummary {
            account_id: self.id.clone(),
            collateral: self.collateral,
            equity: self.equity,
            unrealized_pnl: self.total_unrealized_pnl,
            initial_margin: self.margin.initial_margin,
            maintenance_margin: self.margin.maintenance_margin,
            available_balance: self.available_balance(),
            margin_utilization: self.margin.utilization,
            position_count: self.positions.len(),
            is_liquidatable: self.is_liquidatable(),
        }
    }
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    /// Unique identifier of the margin account.
    pub account_id: String,
    /// Total collateral deposited in the account.
    pub collateral: Decimal,
    /// Net equity (collateral + unrealized PnL).
    pub equity: Decimal,
    /// Total unrealized profit and loss across all positions.
    pub unrealized_pnl: Decimal,
    /// Total initial margin required for open positions.
    pub initial_margin: Decimal,
    /// Total maintenance margin required to avoid liquidation.
    pub maintenance_margin: Decimal,
    /// Balance available for new positions.
    pub available_balance: Decimal,
    /// Ratio of used margin to total equity.
    pub margin_utilization: Decimal,
    /// Number of open positions in the account.
    pub position_count: usize,
    /// Whether the account is eligible for liquidation.
    pub is_liquidatable: bool,
}

/// Cross-margin manager
#[derive(Debug)]
pub struct CrossMarginManager {
    /// Accounts by ID
    accounts: HashMap<String, CrossMarginAccount>,
    /// Accounts by user
    accounts_by_user: HashMap<String, Vec<String>>,
}

impl CrossMarginManager {
    /// Create a new cross-margin manager
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            accounts_by_user: HashMap::new(),
        }
    }

    /// Create a new account
    pub fn create_account(
        &mut self,
        account_id: String,
        user_id: String,
        initial_collateral: Decimal,
    ) -> Result<()> {
        if self.accounts.contains_key(&account_id) {
            return Err(CoreError::Validation("Account already exists".to_string()));
        }

        let account =
            CrossMarginAccount::new(account_id.clone(), user_id.clone(), initial_collateral)?;

        self.accounts.insert(account_id.clone(), account);
        self.accounts_by_user
            .entry(user_id)
            .or_default()
            .push(account_id);

        Ok(())
    }

    /// Get account
    pub fn get_account(&self, account_id: &str) -> Option<&CrossMarginAccount> {
        self.accounts.get(account_id)
    }

    /// Get mutable account
    pub fn get_account_mut(&mut self, account_id: &str) -> Option<&mut CrossMarginAccount> {
        self.accounts.get_mut(account_id)
    }

    /// Get user accounts
    pub fn get_user_accounts(&self, user_id: &str) -> Vec<&CrossMarginAccount> {
        self.accounts_by_user
            .get(user_id)
            .map(|ids| ids.iter().filter_map(|id| self.accounts.get(id)).collect())
            .unwrap_or_default()
    }

    /// Update mark prices for all accounts
    pub fn update_all_mark_prices(&mut self, prices: &HashMap<String, Decimal>) -> Result<()> {
        for account in self.accounts.values_mut() {
            account.update_mark_prices(prices)?;
        }
        Ok(())
    }

    /// Get all liquidatable accounts
    pub fn get_liquidatable_accounts(&self) -> Vec<&CrossMarginAccount> {
        self.accounts
            .values()
            .filter(|acc| acc.is_liquidatable())
            .collect()
    }
}

impl Default for CrossMarginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_margin_position() {
        let position = CrossMarginPosition::new(
            "pos1".to_string(),
            "BTC".to_string(),
            dec!(1), // Long
            dec!(1),
            dec!(50000),
            dec!(10),
        )
        .unwrap();

        assert_eq!(position.size, dec!(1));
        assert_eq!(position.entry_price, dec!(50000));
        assert_eq!(position.position_margin, dec!(5000)); // 50000 / 10
    }

    #[test]
    fn test_update_mark_price() {
        let mut position = CrossMarginPosition::new(
            "pos1".to_string(),
            "BTC".to_string(),
            dec!(1),
            dec!(1),
            dec!(50000),
            dec!(10),
        )
        .unwrap();

        position.update_mark_price(dec!(51000));
        assert_eq!(position.unrealized_pnl, dec!(1000)); // (51000 - 50000) * 1
    }

    #[test]
    fn test_cross_margin_account() {
        let account =
            CrossMarginAccount::new("acc1".to_string(), "user1".to_string(), dec!(10000)).unwrap();

        assert_eq!(account.collateral, dec!(10000));
        assert_eq!(account.equity, dec!(10000));
    }

    #[test]
    fn test_add_position() {
        let mut account =
            CrossMarginAccount::new("acc1".to_string(), "user1".to_string(), dec!(10000)).unwrap();

        let position = CrossMarginPosition::new(
            "pos1".to_string(),
            "BTC".to_string(),
            dec!(1),
            dec!(1),
            dec!(50000),
            dec!(10),
        )
        .unwrap();

        account.add_position(position).unwrap();
        assert_eq!(account.positions.len(), 1);
        assert!(account.margin.initial_margin > dec!(0));
    }

    #[test]
    fn test_netting_benefit() {
        let mut account =
            CrossMarginAccount::new("acc1".to_string(), "user1".to_string(), dec!(20000)).unwrap();

        // Add long position
        let pos1 = CrossMarginPosition::new(
            "pos1".to_string(),
            "BTC".to_string(),
            dec!(1), // Long
            dec!(1),
            dec!(50000),
            dec!(10),
        )
        .unwrap();

        // Add short position (opposite direction)
        let pos2 = CrossMarginPosition::new(
            "pos2".to_string(),
            "BTC".to_string(),
            dec!(-1), // Short
            dec!(0.5),
            dec!(50000),
            dec!(10),
        )
        .unwrap();

        account.add_position(pos1).unwrap();
        account.add_position(pos2).unwrap();

        // With netting, margin should be less than sum of individual positions
        let benefit = account.calculate_netting_benefit();
        assert!(benefit < dec!(1.0)); // Should have some netting benefit
    }

    #[test]
    fn test_collateral_operations() {
        let mut account =
            CrossMarginAccount::new("acc1".to_string(), "user1".to_string(), dec!(10000)).unwrap();

        account.add_collateral(dec!(5000)).unwrap();
        assert_eq!(account.collateral, dec!(15000));

        account.withdraw_collateral(dec!(3000)).unwrap();
        assert_eq!(account.collateral, dec!(12000));
    }

    #[test]
    fn test_cross_margin_manager() {
        let mut manager = CrossMarginManager::new();

        manager
            .create_account("acc1".to_string(), "user1".to_string(), dec!(10000))
            .unwrap();

        assert!(manager.get_account("acc1").is_some());

        let user_accounts = manager.get_user_accounts("user1");
        assert_eq!(user_accounts.len(), 1);
    }
}
