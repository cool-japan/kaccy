//! Trade model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// Executed trade record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trade {
    /// Unique identifier for this trade
    pub trade_id: Uuid,
    /// User who bought the tokens
    pub buyer_user_id: Uuid,
    /// User who sold the tokens (None for bonding-curve purchases)
    pub seller_user_id: Option<Uuid>,
    /// Token that was traded
    pub token_id: Uuid,
    /// Number of tokens exchanged
    pub amount: Decimal,
    /// Price per token in BTC
    pub price_btc: Decimal,
    /// Total BTC value of the trade
    pub total_btc: Decimal,
    /// Platform fee collected in BTC
    pub platform_fee_btc: Decimal,
    /// Royalty paid to the token issuer in BTC
    pub issuer_royalty_btc: Decimal,
    /// Timestamp when the trade was executed
    pub executed_at: DateTime<Utc>,
}

impl fmt::Display for Trade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let trade_type = if self.seller_user_id.is_some() {
            "P2P"
        } else {
            "BondingCurve"
        };
        write!(
            f,
            "Trade({}, {}, {} tokens @ {} BTC/token)",
            self.trade_id, trade_type, self.amount, self.price_btc
        )
    }
}

/// Summary of trades for a token
#[derive(Debug, Serialize)]
pub struct TradeSummary {
    /// Total number of trades executed
    pub total_trades: i64,
    /// Cumulative trade volume in BTC
    pub total_volume_btc: Decimal,
    /// Average execution price in BTC
    pub avg_price_btc: Decimal,
    /// Highest execution price in BTC
    pub high_price_btc: Decimal,
    /// Lowest execution price in BTC
    pub low_price_btc: Decimal,
}

/// Trade with additional context
#[derive(Debug, Serialize)]
pub struct TradeWithContext {
    /// Core trade record
    #[serde(flatten)]
    pub trade: Trade,
    /// Username of the buyer
    pub buyer_username: String,
    /// Username of the seller (None for bonding-curve purchases)
    pub seller_username: Option<String>,
    /// Symbol of the traded token
    pub token_symbol: String,
}
