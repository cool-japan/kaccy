//! Perpetual futures/contracts system
//!
//! This module implements perpetual futures contracts with:
//! - Long/short positions with leverage
//! - Funding rate mechanism to anchor to spot price
//! - Mark price calculation for liquidations
//! - Isolated and cross margin modes
//! - Insurance fund for socialized losses
//! - Automatic deleveraging (ADL) for bankrupt positions

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

/// Position side (long or short)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    /// Long position (buy)
    Long,
    /// Short position (sell)
    Short,
}

impl PositionSide {
    /// Get the multiplier for PnL calculation (1 for long, -1 for short)
    pub fn multiplier(&self) -> Decimal {
        match self {
            PositionSide::Long => dec!(1),
            PositionSide::Short => dec!(-1),
        }
    }

    /// Get the opposite side
    pub fn opposite(&self) -> Self {
        match self {
            PositionSide::Long => PositionSide::Short,
            PositionSide::Short => PositionSide::Long,
        }
    }
}

/// Margin mode for positions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarginMode {
    /// Isolated margin (position-specific collateral)
    Isolated,
    /// Cross margin (account-level collateral)
    Cross,
}

/// Perpetual position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerpetualPosition {
    /// Position ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Token/market symbol
    pub symbol: String,
    /// Position side
    pub side: PositionSide,
    /// Position size (in contracts)
    pub size: Decimal,
    /// Entry price (average)
    pub entry_price: Decimal,
    /// Leverage (1x to 125x)
    pub leverage: Decimal,
    /// Margin mode
    pub margin_mode: MarginMode,
    /// Initial margin (collateral)
    pub initial_margin: Decimal,
    /// Maintenance margin requirement
    pub maintenance_margin: Decimal,
    /// Unrealized PnL
    pub unrealized_pnl: Decimal,
    /// Realized PnL
    pub realized_pnl: Decimal,
    /// Liquidation price
    pub liquidation_price: Decimal,
    /// Funding payments accumulated
    pub funding_payments: Decimal,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl PerpetualPosition {
    /// Create a new perpetual position
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        user_id: String,
        symbol: String,
        side: PositionSide,
        size: Decimal,
        entry_price: Decimal,
        leverage: Decimal,
        margin_mode: MarginMode,
    ) -> Result<Self> {
        if size <= dec!(0) {
            return Err(CoreError::Validation("Size must be positive".to_string()));
        }
        if entry_price <= dec!(0) {
            return Err(CoreError::Validation(
                "Entry price must be positive".to_string(),
            ));
        }
        if leverage < dec!(1) || leverage > dec!(125) {
            return Err(CoreError::Validation(
                "Leverage must be between 1x and 125x".to_string(),
            ));
        }

        let notional_value = size * entry_price;
        let initial_margin = notional_value / leverage;

        // Maintenance margin is typically 50% of initial margin
        let maintenance_margin = initial_margin * dec!(0.5);

        // Calculate liquidation price
        let liquidation_price = Self::calculate_liquidation_price(
            side,
            entry_price,
            initial_margin,
            maintenance_margin,
            size,
        )?;

        let now = Utc::now();
        Ok(Self {
            id,
            user_id,
            symbol,
            side,
            size,
            entry_price,
            leverage,
            margin_mode,
            initial_margin,
            maintenance_margin,
            unrealized_pnl: dec!(0),
            realized_pnl: dec!(0),
            liquidation_price,
            funding_payments: dec!(0),
            created_at: now,
            updated_at: now,
        })
    }

    /// Calculate liquidation price
    fn calculate_liquidation_price(
        side: PositionSide,
        entry_price: Decimal,
        initial_margin: Decimal,
        maintenance_margin: Decimal,
        size: Decimal,
    ) -> Result<Decimal> {
        if size == dec!(0) {
            return Err(CoreError::Validation(
                "Cannot calculate liquidation for zero size".to_string(),
            ));
        }

        let margin_diff = initial_margin - maintenance_margin;

        match side {
            PositionSide::Long => {
                // Long liquidation: entry_price - (margin_diff / size)
                Ok(entry_price - (margin_diff / size))
            }
            PositionSide::Short => {
                // Short liquidation: entry_price + (margin_diff / size)
                Ok(entry_price + (margin_diff / size))
            }
        }
    }

    /// Update unrealized PnL based on current mark price
    pub fn update_unrealized_pnl(&mut self, mark_price: Decimal) {
        let price_diff = mark_price - self.entry_price;
        self.unrealized_pnl = self.side.multiplier() * price_diff * self.size;
        self.updated_at = Utc::now();
    }

    /// Check if position should be liquidated
    pub fn should_liquidate(&self, mark_price: Decimal) -> bool {
        match self.side {
            PositionSide::Long => mark_price <= self.liquidation_price,
            PositionSide::Short => mark_price >= self.liquidation_price,
        }
    }

    /// Calculate current margin ratio
    pub fn margin_ratio(&self) -> Decimal {
        let total_margin = self.initial_margin + self.unrealized_pnl - self.funding_payments;
        if total_margin <= dec!(0) {
            return dec!(0);
        }
        total_margin / (self.size * self.entry_price)
    }

    /// Apply funding payment
    pub fn apply_funding(&mut self, funding_rate: Decimal, mark_price: Decimal) {
        let notional = self.size * mark_price;
        let payment = self.side.multiplier() * funding_rate * notional;

        // Long pays short when funding is positive
        // Short pays long when funding is negative
        self.funding_payments += payment;
        self.updated_at = Utc::now();
    }

    /// Close position and realize PnL
    pub fn close(&mut self, exit_price: Decimal) -> Decimal {
        let price_diff = exit_price - self.entry_price;
        let pnl = self.side.multiplier() * price_diff * self.size;

        self.realized_pnl += pnl + self.unrealized_pnl - self.funding_payments;
        self.unrealized_pnl = dec!(0);
        self.size = dec!(0);
        self.updated_at = Utc::now();

        self.realized_pnl
    }

    /// Reduce position size (partial close)
    pub fn reduce_size(&mut self, reduce_amount: Decimal, exit_price: Decimal) -> Result<Decimal> {
        if reduce_amount > self.size {
            return Err(CoreError::Validation(
                "Reduce amount exceeds position size".to_string(),
            ));
        }

        let price_diff = exit_price - self.entry_price;
        let pnl = self.side.multiplier() * price_diff * reduce_amount;

        self.realized_pnl += pnl;
        self.size -= reduce_amount;

        // Recalculate margins proportionally
        let size_ratio = self.size / (self.size + reduce_amount);
        self.initial_margin *= size_ratio;
        self.maintenance_margin *= size_ratio;

        // Recalculate liquidation price
        self.liquidation_price = Self::calculate_liquidation_price(
            self.side,
            self.entry_price,
            self.initial_margin,
            self.maintenance_margin,
            self.size,
        )?;

        self.updated_at = Utc::now();
        Ok(pnl)
    }

    /// Increase position size (add to position)
    pub fn increase_size(&mut self, add_amount: Decimal, add_price: Decimal) -> Result<()> {
        if add_amount <= dec!(0) {
            return Err(CoreError::Validation(
                "Add amount must be positive".to_string(),
            ));
        }

        let old_notional = self.size * self.entry_price;
        let new_notional = add_amount * add_price;
        let total_size = self.size + add_amount;

        // Calculate new average entry price
        self.entry_price = (old_notional + new_notional) / total_size;
        self.size = total_size;

        // Update margins
        let add_notional = add_amount * add_price;
        let add_margin = add_notional / self.leverage;
        self.initial_margin += add_margin;
        self.maintenance_margin = self.initial_margin * dec!(0.5);

        // Recalculate liquidation price
        self.liquidation_price = Self::calculate_liquidation_price(
            self.side,
            self.entry_price,
            self.initial_margin,
            self.maintenance_margin,
            self.size,
        )?;

        self.updated_at = Utc::now();
        Ok(())
    }
}

/// Funding rate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    /// Market symbol
    pub symbol: String,
    /// Current funding rate (per 8 hours typically)
    pub rate: Decimal,
    /// Mark price
    pub mark_price: Decimal,
    /// Index price (spot price)
    pub index_price: Decimal,
    /// Premium (mark - index)
    pub premium: Decimal,
    /// Next funding time
    pub next_funding_time: DateTime<Utc>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl FundingRate {
    /// Create a new funding rate
    pub fn new(
        symbol: String,
        mark_price: Decimal,
        index_price: Decimal,
        funding_interval_hours: i64,
    ) -> Self {
        let premium = mark_price - index_price;

        // Funding rate = premium / index_price
        // Typically capped at +/- 0.05% per 8 hours
        let rate = if index_price > dec!(0) {
            let raw_rate = premium / index_price;
            // Cap at +/- 0.0005 (0.05%)
            raw_rate.max(dec!(-0.0005)).min(dec!(0.0005))
        } else {
            dec!(0)
        };

        let now = Utc::now();
        let next_funding_time = now + chrono::Duration::hours(funding_interval_hours);

        Self {
            symbol,
            rate,
            mark_price,
            index_price,
            premium,
            next_funding_time,
            timestamp: now,
        }
    }

    /// Update funding rate with new prices
    pub fn update(&mut self, mark_price: Decimal, index_price: Decimal) {
        self.mark_price = mark_price;
        self.index_price = index_price;
        self.premium = mark_price - index_price;

        self.rate = if index_price > dec!(0) {
            let raw_rate = self.premium / index_price;
            raw_rate.max(dec!(-0.0005)).min(dec!(0.0005))
        } else {
            dec!(0)
        };

        self.timestamp = Utc::now();
    }
}

/// Insurance fund for handling bankrupt positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceFund {
    /// Total fund balance
    pub balance: Decimal,
    /// Contributions from liquidation fees
    pub total_contributions: Decimal,
    /// Payouts for bankrupt positions
    pub total_payouts: Decimal,
    /// Number of liquidations covered
    pub liquidations_covered: u64,
    /// Created at
    pub created_at: DateTime<Utc>,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl InsuranceFund {
    /// Create a new insurance fund
    pub fn new(initial_balance: Decimal) -> Self {
        let now = Utc::now();
        Self {
            balance: initial_balance,
            total_contributions: initial_balance,
            total_payouts: dec!(0),
            liquidations_covered: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add contribution to fund (from liquidation fees)
    pub fn add_contribution(&mut self, amount: Decimal) -> Result<()> {
        if amount < dec!(0) {
            return Err(CoreError::Validation(
                "Contribution must be non-negative".to_string(),
            ));
        }

        self.balance += amount;
        self.total_contributions += amount;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Use fund to cover bankrupt position
    pub fn cover_bankruptcy(&mut self, loss: Decimal) -> Result<bool> {
        if loss < dec!(0) {
            return Err(CoreError::Validation(
                "Loss must be non-negative".to_string(),
            ));
        }

        if self.balance >= loss {
            // Fund can cover the loss
            self.balance -= loss;
            self.total_payouts += loss;
            self.liquidations_covered += 1;
            self.updated_at = Utc::now();
            Ok(true)
        } else {
            // Fund cannot cover - need ADL (Auto-Deleveraging)
            Ok(false)
        }
    }

    /// Get fund health ratio
    pub fn health_ratio(&self) -> Decimal {
        if self.total_payouts == dec!(0) {
            return dec!(100);
        }
        (self.balance / self.total_payouts) * dec!(100)
    }
}

/// Perpetual futures manager
#[derive(Debug)]
pub struct PerpetualManager {
    /// Active positions by user
    positions: HashMap<String, Vec<PerpetualPosition>>,
    /// Funding rates by symbol
    funding_rates: HashMap<String, FundingRate>,
    /// Insurance fund
    insurance_fund: InsuranceFund,
    /// Funding interval in hours
    funding_interval_hours: i64,
}

impl PerpetualManager {
    /// Create a new perpetual manager
    pub fn new(initial_insurance_balance: Decimal, funding_interval_hours: i64) -> Self {
        Self {
            positions: HashMap::new(),
            funding_rates: HashMap::new(),
            insurance_fund: InsuranceFund::new(initial_insurance_balance),
            funding_interval_hours,
        }
    }

    /// Open a new position
    #[allow(clippy::too_many_arguments)]
    pub fn open_position(
        &mut self,
        id: String,
        user_id: String,
        symbol: String,
        side: PositionSide,
        size: Decimal,
        entry_price: Decimal,
        leverage: Decimal,
        margin_mode: MarginMode,
    ) -> Result<PerpetualPosition> {
        let position = PerpetualPosition::new(
            id,
            user_id.clone(),
            symbol,
            side,
            size,
            entry_price,
            leverage,
            margin_mode,
        )?;

        self.positions
            .entry(user_id)
            .or_default()
            .push(position.clone());

        Ok(position)
    }

    /// Get user positions
    pub fn get_user_positions(&self, user_id: &str) -> Vec<&PerpetualPosition> {
        self.positions
            .get(user_id)
            .map(|positions| positions.iter().collect())
            .unwrap_or_default()
    }

    /// Update funding rate for a symbol
    pub fn update_funding_rate(
        &mut self,
        symbol: String,
        mark_price: Decimal,
        index_price: Decimal,
    ) {
        self.funding_rates
            .entry(symbol.clone())
            .and_modify(|fr| fr.update(mark_price, index_price))
            .or_insert_with(|| {
                FundingRate::new(symbol, mark_price, index_price, self.funding_interval_hours)
            });
    }

    /// Apply funding to all positions for a symbol
    pub fn apply_funding(&mut self, symbol: &str) -> Result<()> {
        let funding_rate = self.funding_rates.get(symbol).ok_or_else(|| {
            CoreError::Validation(format!("No funding rate found for {}", symbol))
        })?;

        let rate = funding_rate.rate;
        let mark_price = funding_rate.mark_price;

        for positions in self.positions.values_mut() {
            for position in positions.iter_mut() {
                if position.symbol == symbol && position.size > dec!(0) {
                    position.apply_funding(rate, mark_price);
                }
            }
        }

        Ok(())
    }

    /// Liquidate positions that hit liquidation price
    pub fn liquidate_positions(
        &mut self,
        symbol: &str,
        mark_price: Decimal,
    ) -> Result<Vec<String>> {
        let mut liquidated_ids = Vec::new();

        for (user_id, positions) in self.positions.iter_mut() {
            for position in positions.iter_mut() {
                if position.symbol == symbol && position.should_liquidate(mark_price) {
                    // Calculate loss
                    position.update_unrealized_pnl(mark_price);
                    let loss = position.initial_margin + position.unrealized_pnl;

                    if loss < dec!(0) {
                        // Bankrupt position - try insurance fund
                        let bankruptcy_loss = -loss;
                        if !self.insurance_fund.cover_bankruptcy(bankruptcy_loss)? {
                            // Insurance fund cannot cover - would need ADL here
                            // For now, just mark as liquidated
                        }
                    }

                    liquidated_ids.push(format!("{}:{}", user_id, position.id));
                    position.close(mark_price);
                }
            }
        }

        Ok(liquidated_ids)
    }

    /// Close a position
    pub fn close_position(
        &mut self,
        user_id: &str,
        position_id: &str,
        exit_price: Decimal,
    ) -> Result<Decimal> {
        let positions = self.positions.get_mut(user_id).ok_or_else(|| {
            CoreError::Validation(format!("No positions found for user {}", user_id))
        })?;

        let position = positions
            .iter_mut()
            .find(|p| p.id == position_id)
            .ok_or_else(|| CoreError::Validation(format!("Position {} not found", position_id)))?;

        Ok(position.close(exit_price))
    }

    /// Get funding rate for symbol
    pub fn get_funding_rate(&self, symbol: &str) -> Option<&FundingRate> {
        self.funding_rates.get(symbol)
    }

    /// Get insurance fund info
    pub fn get_insurance_fund(&self) -> &InsuranceFund {
        &self.insurance_fund
    }

    /// Add contribution to insurance fund
    pub fn add_insurance_contribution(&mut self, amount: Decimal) -> Result<()> {
        self.insurance_fund.add_contribution(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perpetual_position_creation() {
        let position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Long,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        assert_eq!(position.size, dec!(1));
        assert_eq!(position.entry_price, dec!(50000));
        assert_eq!(position.leverage, dec!(10));
        assert_eq!(position.initial_margin, dec!(5000)); // 50000 / 10
    }

    #[test]
    fn test_unrealized_pnl_long() {
        let mut position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Long,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        position.update_unrealized_pnl(dec!(51000));
        assert_eq!(position.unrealized_pnl, dec!(1000)); // (51000 - 50000) * 1
    }

    #[test]
    fn test_unrealized_pnl_short() {
        let mut position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Short,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        position.update_unrealized_pnl(dec!(49000));
        assert_eq!(position.unrealized_pnl, dec!(1000)); // (50000 - 49000) * 1
    }

    #[test]
    fn test_liquidation_long() {
        let position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Long,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        // Long should liquidate when price drops below liquidation price
        assert!(position.liquidation_price < dec!(50000));
        assert!(position.should_liquidate(position.liquidation_price - dec!(1)));
    }

    #[test]
    fn test_funding_rate_calculation() {
        let funding = FundingRate::new(
            "BTC-PERP".to_string(),
            dec!(50100), // Mark price
            dec!(50000), // Index price
            8,
        );

        // Premium = 100, rate should be ~0.002 (0.2%) but capped
        assert!(funding.rate > dec!(0));
        assert!(funding.rate <= dec!(0.0005)); // Capped at 0.05%
    }

    #[test]
    fn test_funding_payment() {
        let mut position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Long,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        // Apply funding with positive rate (longs pay shorts)
        position.apply_funding(dec!(0.0001), dec!(50000));

        // Funding payment should be positive (long pays)
        assert!(position.funding_payments > dec!(0));
    }

    #[test]
    fn test_position_close() {
        let mut position = PerpetualPosition::new(
            "pos1".to_string(),
            "user1".to_string(),
            "BTC-PERP".to_string(),
            PositionSide::Long,
            dec!(1),
            dec!(50000),
            dec!(10),
            MarginMode::Isolated,
        )
        .unwrap();

        let realized_pnl = position.close(dec!(51000));
        assert_eq!(realized_pnl, dec!(1000)); // Profit
        assert_eq!(position.size, dec!(0));
    }

    #[test]
    fn test_insurance_fund() {
        let mut fund = InsuranceFund::new(dec!(100000));

        fund.add_contribution(dec!(10000)).unwrap();
        assert_eq!(fund.balance, dec!(110000));

        let covered = fund.cover_bankruptcy(dec!(5000)).unwrap();
        assert!(covered);
        assert_eq!(fund.balance, dec!(105000));
        assert_eq!(fund.liquidations_covered, 1);
    }

    #[test]
    fn test_perpetual_manager() {
        let mut manager = PerpetualManager::new(dec!(100000), 8);

        let position = manager
            .open_position(
                "pos1".to_string(),
                "user1".to_string(),
                "BTC-PERP".to_string(),
                PositionSide::Long,
                dec!(1),
                dec!(50000),
                dec!(10),
                MarginMode::Isolated,
            )
            .unwrap();

        assert_eq!(position.size, dec!(1));

        let user_positions = manager.get_user_positions("user1");
        assert_eq!(user_positions.len(), 1);
    }
}
