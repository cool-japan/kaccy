//! Order model

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// A buy or sell order placed by a user
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    /// Unique identifier for this order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token being bought or sold
    pub token_id: Uuid,
    /// Whether this is a buy or sell order
    pub order_type: OrderType,
    /// Token quantity
    pub amount: Decimal,
    /// Price per token in BTC
    pub price_btc: Decimal,
    /// Total BTC value of the order (amount × price_btc)
    pub total_btc: Decimal,
    /// Current lifecycle state of the order
    pub status: OrderStatus,
    /// BTC address provided by the buyer for payment
    pub btc_address: Option<String>,
    /// Bitcoin transaction ID once settlement is broadcast
    pub btc_txid: Option<String>,
    /// Timestamp when the order was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the order reached a terminal state
    pub completed_at: Option<DateTime<Utc>>,
}

/// Direction of an order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum OrderType {
    /// User intends to purchase tokens
    Buy,
    /// User intends to sell tokens
    Sell,
}

/// Lifecycle state of an order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum OrderStatus {
    /// Order is awaiting fulfilment
    #[default]
    Pending,
    /// Order was fully executed
    Completed,
    /// Order was cancelled by the user or system
    Cancelled,
    /// Order timed out before it could be filled
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

/// Request body for creating a buy order
#[derive(Debug, Deserialize)]
pub struct CreateBuyOrderRequest {
    /// Token the user wishes to buy
    pub token_id: Uuid,
    /// Number of tokens to buy
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

/// Request body for creating a sell order
#[derive(Debug, Deserialize)]
pub struct CreateSellOrderRequest {
    /// Token the user wishes to sell
    pub token_id: Uuid,
    /// Number of tokens to sell
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

/// Response returned after a buy order is created
#[derive(Debug, Serialize)]
pub struct BuyOrderResponse {
    /// ID of the newly created order
    pub order_id: Uuid,
    /// BTC address the buyer should send payment to
    pub btc_address: String,
    /// Total BTC required to complete this order
    pub total_btc: Decimal,
    /// BTC price per token at the time of order creation
    pub price_per_token: Decimal,
    /// Estimated price impact as a percentage
    pub price_impact_percent: Decimal,
    /// Timestamp after which this order will expire if unpaid
    pub expires_at: DateTime<Utc>,
}

/// Response returned after a sell order is created
#[derive(Debug, Serialize)]
pub struct SellOrderResponse {
    /// ID of the newly created order
    pub order_id: Uuid,
    /// Total BTC the seller will receive
    pub total_btc: Decimal,
    /// BTC price per token at the time of order creation
    pub price_per_token: Decimal,
    /// Estimated price impact as a percentage
    pub price_impact_percent: Decimal,
}
