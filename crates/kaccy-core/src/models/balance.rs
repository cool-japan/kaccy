//! Balance model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Balance {
    pub balance_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub locked_amount: Decimal,
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

#[derive(Debug, Serialize)]
pub struct BalanceWithToken {
    #[serde(flatten)]
    pub balance: Balance,
    pub token_symbol: String,
    pub token_name: String,
    pub current_price_btc: Decimal,
    pub current_value_btc: Decimal,
}

/// User's token balance portfolio
#[derive(Debug, Serialize)]
pub struct TokenPortfolio {
    pub total_tokens_held: u32,
    pub total_value_btc: Decimal,
    pub balances: Vec<BalanceWithToken>,
}
