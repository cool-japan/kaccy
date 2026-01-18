//! Order model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub order_type: OrderType,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub status: OrderStatus,
    pub btc_address: Option<String>,
    pub btc_txid: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum OrderStatus {
    #[default]
    Pending,
    Completed,
    Cancelled,
    Expired,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Order({}, {:?}, {} tokens @ {} BTC)",
            self.order_id, self.order_type, self.amount, self.price_btc
        )
    }
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderType::Buy => write!(f, "buy"),
            OrderType::Sell => write!(f, "sell"),
        }
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Completed => write!(f, "completed"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Expired => write!(f, "expired"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBuyOrderRequest {
    pub token_id: Uuid,
    pub amount: Decimal,
    /// Maximum price user is willing to pay (slippage protection)
    pub max_price_btc: Option<Decimal>,
}

impl CreateBuyOrderRequest {
    /// Validate the buy order request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }
        if self.amount < dec!(0.001) {
            return Err(ValidationError(
                "Minimum order amount is 0.001 tokens".to_string(),
            ));
        }
        if let Some(max_price) = self.max_price_btc {
            if max_price <= dec!(0) {
                return Err(ValidationError("Max price must be positive".to_string()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSellOrderRequest {
    pub token_id: Uuid,
    pub amount: Decimal,
    /// Minimum price user is willing to accept (slippage protection)
    pub min_price_btc: Option<Decimal>,
}

impl CreateSellOrderRequest {
    /// Validate the sell order request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }
        if self.amount < dec!(0.001) {
            return Err(ValidationError(
                "Minimum order amount is 0.001 tokens".to_string(),
            ));
        }
        if let Some(min_price) = self.min_price_btc {
            if min_price < dec!(0) {
                return Err(ValidationError("Min price cannot be negative".to_string()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct BuyOrderResponse {
    pub order_id: Uuid,
    pub btc_address: String,
    pub total_btc: Decimal,
    pub price_per_token: Decimal,
    pub price_impact_percent: Decimal,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SellOrderResponse {
    pub order_id: Uuid,
    pub total_btc: Decimal,
    pub price_per_token: Decimal,
    pub price_impact_percent: Decimal,
}
