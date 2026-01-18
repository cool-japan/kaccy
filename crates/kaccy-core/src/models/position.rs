//! Position management and tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// User's trading position for a token
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Position {
    pub position_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    /// Total tokens held
    pub quantity: Decimal,
    /// Average entry price
    pub average_entry_price: Decimal,
    /// Total cost basis
    pub cost_basis: Decimal,
    /// Realized profit/loss from closed trades
    pub realized_pnl: Decimal,
    /// Position status
    pub status: PositionStatus,
    /// First entry time
    pub opened_at: DateTime<Utc>,
    /// Last modification time
    pub updated_at: DateTime<Utc>,
    /// Close time (if closed)
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum PositionStatus {
    #[default]
    Open,
    Closed,
}

impl fmt::Display for PositionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PositionStatus::Open => write!(f, "open"),
            PositionStatus::Closed => write!(f, "closed"),
        }
    }
}

impl Position {
    /// Calculate unrealized PnL based on current price
    pub fn unrealized_pnl(&self, current_price: Decimal) -> Decimal {
        let current_value = self.quantity * current_price;
        current_value - self.cost_basis
    }

    /// Calculate total PnL (realized + unrealized)
    pub fn total_pnl(&self, current_price: Decimal) -> Decimal {
        self.realized_pnl + self.unrealized_pnl(current_price)
    }

    /// Calculate PnL percentage
    pub fn pnl_percentage(&self, current_price: Decimal) -> Decimal {
        if self.cost_basis == dec!(0) {
            return dec!(0);
        }
        (self.total_pnl(current_price) / self.cost_basis) * dec!(100)
    }

    /// Calculate return on investment (ROI)
    pub fn roi(&self, current_price: Decimal) -> Decimal {
        if self.cost_basis == dec!(0) {
            return dec!(0);
        }
        (self.unrealized_pnl(current_price) / self.cost_basis) * dec!(100)
    }

    /// Add to position (buy more)
    pub fn add(&mut self, quantity: Decimal, price: Decimal) {
        let additional_cost = quantity * price;
        let new_cost_basis = self.cost_basis + additional_cost;
        let new_quantity = self.quantity + quantity;

        self.average_entry_price = if new_quantity > dec!(0) {
            new_cost_basis / new_quantity
        } else {
            dec!(0)
        };

        self.quantity = new_quantity;
        self.cost_basis = new_cost_basis;
        self.updated_at = Utc::now();
    }

    /// Reduce position (sell some)
    pub fn reduce(&mut self, quantity: Decimal, price: Decimal) -> Decimal {
        if quantity > self.quantity {
            return dec!(0); // Can't reduce more than we have
        }

        // Calculate realized PnL from this sale
        let sale_proceeds = quantity * price;
        let proportional_cost = (quantity / self.quantity) * self.cost_basis;
        let realized_pnl = sale_proceeds - proportional_cost;

        self.quantity -= quantity;
        self.cost_basis -= proportional_cost;
        self.realized_pnl += realized_pnl;
        self.updated_at = Utc::now();

        // If position is now zero, mark as closed
        if self.quantity == dec!(0) {
            self.status = PositionStatus::Closed;
            self.closed_at = Some(Utc::now());
        }

        realized_pnl
    }

    /// Check if position is profitable at current price
    pub fn is_profitable(&self, current_price: Decimal) -> bool {
        self.total_pnl(current_price) > dec!(0)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Position({}, qty={}, avg_price={}, status={})",
            self.position_id, self.quantity, self.average_entry_price, self.status
        )
    }
}

/// Position limits and risk controls
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionLimits {
    pub user_id: Uuid,
    /// Maximum position size per token
    pub max_position_size: Option<Decimal>,
    /// Maximum total portfolio value
    pub max_portfolio_value: Option<Decimal>,
    /// Maximum leverage (if applicable)
    pub max_leverage: Option<Decimal>,
    /// Maximum number of open positions
    pub max_open_positions: Option<i32>,
    /// Daily loss limit
    pub daily_loss_limit: Option<Decimal>,
    /// Current daily loss
    pub current_daily_loss: Decimal,
    /// Last reset time for daily limits
    pub last_reset_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PositionLimits {
    /// Create default limits for a user
    pub fn default_for_user(user_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            max_position_size: None,
            max_portfolio_value: None,
            max_leverage: Some(dec!(1)), // No leverage by default
            max_open_positions: Some(10),
            daily_loss_limit: None,
            current_daily_loss: dec!(0),
            last_reset_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if user can open new position
    pub fn can_open_position(
        &self,
        current_positions: i32,
        position_size: Decimal,
        _portfolio_value: Decimal,
    ) -> Result<(), ValidationError> {
        // Check max open positions
        if let Some(max_positions) = self.max_open_positions {
            if current_positions >= max_positions {
                return Err(ValidationError(format!(
                    "Maximum open positions reached: {}",
                    max_positions
                )));
            }
        }

        // Check max position size
        if let Some(max_size) = self.max_position_size {
            if position_size > max_size {
                return Err(ValidationError(format!(
                    "Position size {} exceeds limit: {}",
                    position_size, max_size
                )));
            }
        }

        Ok(())
    }

    /// Check if daily loss limit is exceeded
    pub fn is_daily_loss_limit_exceeded(&mut self) -> bool {
        // Reset daily loss if it's a new day
        let now = Utc::now();
        if (now - self.last_reset_at).num_hours() >= 24 {
            self.current_daily_loss = dec!(0);
            self.last_reset_at = now;
        }

        if let Some(limit) = self.daily_loss_limit {
            self.current_daily_loss.abs() >= limit
        } else {
            false
        }
    }

    /// Record a loss
    pub fn record_loss(&mut self, loss: Decimal) {
        if loss < dec!(0) {
            self.current_daily_loss += loss.abs();
            self.updated_at = Utc::now();
        }
    }
}

/// Position history entry
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionHistory {
    pub history_id: Uuid,
    pub position_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    /// Action type (open, add, reduce, close)
    pub action: PositionAction,
    /// Quantity changed
    pub quantity: Decimal,
    /// Price at which action occurred
    pub price: Decimal,
    /// PnL realized from this action (if any)
    pub realized_pnl: Option<Decimal>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum PositionAction {
    #[default]
    Open,
    Add,
    Reduce,
    Close,
}

impl fmt::Display for PositionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PositionAction::Open => write!(f, "open"),
            PositionAction::Add => write!(f, "add"),
            PositionAction::Reduce => write!(f, "reduce"),
            PositionAction::Close => write!(f, "close"),
        }
    }
}

/// Trading portfolio summary
#[derive(Debug, Serialize)]
pub struct TradingPortfolio {
    pub user_id: Uuid,
    pub total_positions: i32,
    pub open_positions: i32,
    pub total_cost_basis: Decimal,
    pub total_market_value: Decimal,
    pub total_unrealized_pnl: Decimal,
    pub total_realized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub portfolio_roi: Decimal,
    pub positions: Vec<PositionWithPrice>,
}

/// Position with current market data
#[derive(Debug, Serialize)]
pub struct PositionWithPrice {
    #[serde(flatten)]
    pub position: Position,
    pub current_price: Decimal,
    pub market_value: Decimal,
    pub unrealized_pnl: Decimal,
    pub unrealized_pnl_percentage: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_add() {
        let mut position = Position {
            position_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            quantity: dec!(100),
            average_entry_price: dec!(10),
            cost_basis: dec!(1000),
            realized_pnl: dec!(0),
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
        };

        // Add 100 more at $15
        position.add(dec!(100), dec!(15));

        assert_eq!(position.quantity, dec!(200));
        assert_eq!(position.cost_basis, dec!(2500)); // 1000 + 1500
        assert_eq!(position.average_entry_price, dec!(12.5)); // 2500 / 200
    }

    #[test]
    fn test_position_reduce() {
        let mut position = Position {
            position_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            quantity: dec!(100),
            average_entry_price: dec!(10),
            cost_basis: dec!(1000),
            realized_pnl: dec!(0),
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
        };

        // Sell 50 at $15
        let realized_pnl = position.reduce(dec!(50), dec!(15));

        assert_eq!(position.quantity, dec!(50));
        assert_eq!(position.cost_basis, dec!(500)); // Half of original
        assert_eq!(realized_pnl, dec!(250)); // (50 * 15) - 500
        assert_eq!(position.realized_pnl, dec!(250));
    }

    #[test]
    fn test_position_close() {
        let mut position = Position {
            position_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            quantity: dec!(100),
            average_entry_price: dec!(10),
            cost_basis: dec!(1000),
            realized_pnl: dec!(0),
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
        };

        // Sell all at $12
        position.reduce(dec!(100), dec!(12));

        assert_eq!(position.quantity, dec!(0));
        assert_eq!(position.status, PositionStatus::Closed);
        assert!(position.closed_at.is_some());
        assert_eq!(position.realized_pnl, dec!(200)); // (100 * 12) - 1000
    }

    #[test]
    fn test_unrealized_pnl() {
        let position = Position {
            position_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            quantity: dec!(100),
            average_entry_price: dec!(10),
            cost_basis: dec!(1000),
            realized_pnl: dec!(0),
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            closed_at: None,
        };

        // Current price $12
        let unrealized = position.unrealized_pnl(dec!(12));
        assert_eq!(unrealized, dec!(200)); // (100 * 12) - 1000

        // Current price $8
        let unrealized = position.unrealized_pnl(dec!(8));
        assert_eq!(unrealized, dec!(-200)); // (100 * 8) - 1000
    }

    #[test]
    fn test_position_limits() {
        let limits = PositionLimits {
            user_id: Uuid::new_v4(),
            max_position_size: Some(dec!(1000)),
            max_portfolio_value: Some(dec!(10000)),
            max_leverage: Some(dec!(2)),
            max_open_positions: Some(5),
            daily_loss_limit: Some(dec!(500)),
            current_daily_loss: dec!(0),
            last_reset_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Should allow position within limits
        assert!(limits.can_open_position(3, dec!(500), dec!(5000)).is_ok());

        // Should reject position exceeding size limit
        assert!(limits.can_open_position(3, dec!(1500), dec!(5000)).is_err());

        // Should reject when max positions reached
        assert!(limits.can_open_position(5, dec!(500), dec!(5000)).is_err());
    }

    #[test]
    fn test_daily_loss_limit() {
        let mut limits = PositionLimits {
            user_id: Uuid::new_v4(),
            max_position_size: None,
            max_portfolio_value: None,
            max_leverage: None,
            max_open_positions: None,
            daily_loss_limit: Some(dec!(100)),
            current_daily_loss: dec!(0),
            last_reset_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Record losses
        limits.record_loss(dec!(-50));
        assert!(!limits.is_daily_loss_limit_exceeded());

        limits.record_loss(dec!(-60));
        assert!(limits.is_daily_loss_limit_exceeded());
    }
}
