//! Trade model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trade {
    pub trade_id: Uuid,
    pub buyer_user_id: Uuid,
    pub seller_user_id: Option<Uuid>,
    pub token_id: Uuid,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub platform_fee_btc: Decimal,
    pub issuer_royalty_btc: Decimal,
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
    pub total_trades: i64,
    pub total_volume_btc: Decimal,
    pub avg_price_btc: Decimal,
    pub high_price_btc: Decimal,
    pub low_price_btc: Decimal,
}

/// Trade with additional context
#[derive(Debug, Serialize)]
pub struct TradeWithContext {
    #[serde(flatten)]
    pub trade: Trade,
    pub buyer_username: String,
    pub seller_username: Option<String>,
    pub token_symbol: String,
}
