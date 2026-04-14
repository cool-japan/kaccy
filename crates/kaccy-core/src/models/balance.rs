//! Balance model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// Token balance held by a user
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Balance {
    /// Unique identifier for this balance record
    pub balance_id: Uuid,
    /// User who holds this balance
    pub user_id: Uuid,
    /// Token this balance refers to
    pub token_id: Uuid,
    /// Total amount held (including locked)
    pub amount: Decimal,
    /// Amount currently locked (e.g. in open orders)
    pub locked_amount: Decimal,
    /// Timestamp when this record was last updated
    pub updated_at: DateTime<Utc>,
}

impl Balance {
    /// Get available (unlocked) balance
    pub fn available(&self) -> Decimal {
        self.amount - self.locked_amount
    }

    /// Check if balance is empty
    pub fn is_empty(&self) -> bool {
        self.amount == dec!(0)
    }

    /// Check if there's enough available balance
    pub fn has_available(&self, required: Decimal) -> bool {
        self.available() >= required
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Balance(user={}, token={}, amount={}, locked={})",
            self.user_id, self.token_id, self.amount, self.locked_amount
        )
    }
}

/// Balance enriched with token metadata and current value
#[derive(Debug, Serialize)]
pub struct BalanceWithToken {
    /// Underlying balance record
    #[serde(flatten)]
    pub balance: Balance,
    /// Token ticker symbol
    pub token_symbol: String,
    /// Token display name
    pub token_name: String,
    /// Current market price in BTC
    pub current_price_btc: Decimal,
    /// Current total value in BTC (amount × price)
    pub current_value_btc: Decimal,
}

/// User's token balance portfolio
#[derive(Debug, Serialize)]
pub struct TokenPortfolio {
    /// Number of distinct tokens held
    pub total_tokens_held: u32,
    /// Total portfolio value in BTC
    pub total_value_btc: Decimal,
    /// Individual token balances with metadata
    pub balances: Vec<BalanceWithToken>,
}
