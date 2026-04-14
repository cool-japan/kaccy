//! Advanced order types (stop-loss, take-profit, trailing stop, TWAP)

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Advanced order with conditional execution
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdvancedOrder {
    /// Unique identifier for this order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token being traded
    pub token_id: Uuid,
    /// Amount to trade
    pub amount: Decimal,
    /// Order type (stop-loss, take-profit, etc.)
    pub order_type: AdvancedOrderType,
    /// Order status
    pub status: AdvancedOrderStatus,
    /// Trigger price for stop/limit orders
    pub trigger_price: Option<Decimal>,
    /// Limit price (max buy / min sell)
    pub limit_price: Option<Decimal>,
    /// Trailing distance (for trailing stops)
    pub trailing_distance: Option<Decimal>,
    /// Current trailing stop price
    pub current_trailing_price: Option<Decimal>,
    /// Expiration time (GTT - Good Till Time)
    pub expires_at: Option<DateTime<Utc>>,
    /// For TWAP orders: start time
    pub twap_start_time: Option<DateTime<Utc>>,
    /// For TWAP orders: end time
    pub twap_end_time: Option<DateTime<Utc>>,
    /// For TWAP orders: interval in seconds
    pub twap_interval_seconds: Option<i64>,
    /// For TWAP orders: amount per interval
    pub twap_amount_per_interval: Option<Decimal>,
    /// For TWAP orders: executed intervals
    pub twap_executed_intervals: i32,
    /// Total amount filled
    pub filled_amount: Decimal,
    /// Average fill price
    pub average_fill_price: Option<Decimal>,
    /// Timestamp when this order was created
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent change to this order
    pub updated_at: DateTime<Utc>,
}

/// Type of advanced conditional order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum AdvancedOrderType {
    /// Stop-loss: sell when price falls below trigger
    StopLoss,
    /// Take-profit: sell when price rises above trigger
    TakeProfit,
    /// Trailing stop: stop-loss that follows price upward
    TrailingStop,
    /// TWAP: Time-Weighted Average Price order
    Twap,
    /// Good-Till-Time limit order
    #[default]
    GoodTillTime,
}

impl fmt::Display for AdvancedOrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdvancedOrderType::StopLoss => write!(f, "stop_loss"),
            AdvancedOrderType::TakeProfit => write!(f, "take_profit"),
            AdvancedOrderType::TrailingStop => write!(f, "trailing_stop"),
            AdvancedOrderType::Twap => write!(f, "twap"),
            AdvancedOrderType::GoodTillTime => write!(f, "good_till_time"),
        }
    }
}

/// Status of an advanced conditional order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum AdvancedOrderStatus {
    /// Order is active and monitoring
    #[default]
    Active,
    /// Order triggered and executing
    Triggered,
    /// Order partially filled
    PartiallyFilled,
    /// Order fully filled
    Filled,
    /// Order cancelled by user
    Cancelled,
    /// Order expired
    Expired,
    /// Order failed
    Failed,
}

impl fmt::Display for AdvancedOrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdvancedOrderStatus::Active => write!(f, "active"),
            AdvancedOrderStatus::Triggered => write!(f, "triggered"),
            AdvancedOrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            AdvancedOrderStatus::Filled => write!(f, "filled"),
            AdvancedOrderStatus::Cancelled => write!(f, "cancelled"),
            AdvancedOrderStatus::Expired => write!(f, "expired"),
            AdvancedOrderStatus::Failed => write!(f, "failed"),
        }
    }
}

impl AdvancedOrder {
    /// Check if order should be triggered based on current price
    pub fn should_trigger(&self, current_price: Decimal) -> bool {
        if self.status != AdvancedOrderStatus::Active {
            return false;
        }

        match self.order_type {
            AdvancedOrderType::StopLoss => {
                // Trigger when price falls below trigger price
                if let Some(trigger) = self.trigger_price {
                    current_price <= trigger
                } else {
                    false
                }
            }
            AdvancedOrderType::TakeProfit => {
                // Trigger when price rises above trigger price
                if let Some(trigger) = self.trigger_price {
                    current_price >= trigger
                } else {
                    false
                }
            }
            AdvancedOrderType::TrailingStop => {
                // Trigger when price falls below current trailing price
                if let Some(trailing) = self.current_trailing_price {
                    current_price <= trailing
                } else {
                    false
                }
            }
            AdvancedOrderType::Twap => {
                // TWAP triggers at specific intervals
                self.should_execute_twap_interval()
            }
            AdvancedOrderType::GoodTillTime => {
                // GTT orders trigger immediately if price is acceptable
                if let Some(limit) = self.limit_price {
                    current_price <= limit // For buy orders
                } else {
                    true
                }
            }
        }
    }

    /// Update trailing stop price if price has moved favorably
    pub fn update_trailing_stop(&mut self, current_price: Decimal) -> bool {
        if self.order_type != AdvancedOrderType::TrailingStop {
            return false;
        }

        let trailing_distance = match self.trailing_distance {
            Some(d) => d,
            None => return false,
        };

        // Calculate new trailing stop price
        let new_trailing_price = current_price - trailing_distance;

        // Update if price has moved up (for sell orders)
        if let Some(current_trailing) = self.current_trailing_price {
            if new_trailing_price > current_trailing {
                self.current_trailing_price = Some(new_trailing_price);
                self.updated_at = Utc::now();
                return true;
            }
        } else {
            // First time setting trailing price
            self.current_trailing_price = Some(new_trailing_price);
            self.updated_at = Utc::now();
            return true;
        }

        false
    }

    /// Check if order has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if TWAP interval should execute
    fn should_execute_twap_interval(&self) -> bool {
        if self.order_type != AdvancedOrderType::Twap {
            return false;
        }

        let now = Utc::now();

        // Check if within TWAP time window
        if let (Some(start), Some(end)) = (self.twap_start_time, self.twap_end_time) {
            if now < start || now > end {
                return false;
            }

            // Check if it's time for next interval
            if let Some(interval_seconds) = self.twap_interval_seconds {
                let elapsed = (now - start).num_seconds();
                let expected_intervals = (elapsed / interval_seconds) as i32;
                return expected_intervals > self.twap_executed_intervals;
            }
        }

        false
    }

    /// Get remaining TWAP amount
    pub fn remaining_twap_amount(&self) -> Decimal {
        self.amount - self.filled_amount
    }

    /// Calculate next TWAP execution amount
    pub fn next_twap_execution_amount(&self) -> Option<Decimal> {
        if self.order_type != AdvancedOrderType::Twap {
            return None;
        }

        let per_interval = self.twap_amount_per_interval?;
        let remaining = self.remaining_twap_amount();

        Some(per_interval.min(remaining))
    }

    /// Validate the advanced order
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }

        match self.order_type {
            AdvancedOrderType::StopLoss | AdvancedOrderType::TakeProfit => {
                if self.trigger_price.is_none() {
                    return Err(ValidationError(
                        "Trigger price required for stop/take-profit orders".to_string(),
                    ));
                }
                if let Some(trigger) = self.trigger_price {
                    if trigger <= dec!(0) {
                        return Err(ValidationError(
                            "Trigger price must be positive".to_string(),
                        ));
                    }
                }
            }
            AdvancedOrderType::TrailingStop => {
                if self.trailing_distance.is_none() {
                    return Err(ValidationError(
                        "Trailing distance required for trailing stop".to_string(),
                    ));
                }
                if let Some(distance) = self.trailing_distance {
                    if distance <= dec!(0) {
                        return Err(ValidationError(
                            "Trailing distance must be positive".to_string(),
                        ));
                    }
                }
            }
            AdvancedOrderType::Twap => {
                if self.twap_start_time.is_none()
                    || self.twap_end_time.is_none()
                    || self.twap_interval_seconds.is_none()
                {
                    return Err(ValidationError(
                        "TWAP orders require start time, end time, and interval".to_string(),
                    ));
                }
                if let (Some(start), Some(end)) = (self.twap_start_time, self.twap_end_time) {
                    if end <= start {
                        return Err(ValidationError(
                            "TWAP end time must be after start time".to_string(),
                        ));
                    }
                }
                if let Some(interval) = self.twap_interval_seconds {
                    if interval <= 0 {
                        return Err(ValidationError(
                            "TWAP interval must be positive".to_string(),
                        ));
                    }
                }
            }
            AdvancedOrderType::GoodTillTime => {
                if self.expires_at.is_none() {
                    return Err(ValidationError(
                        "GTT orders require expiration time".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for AdvancedOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AdvancedOrder({}, type={}, amount={}, status={})",
            self.order_id, self.order_type, self.amount, self.status
        )
    }
}

/// Request to create a stop-loss order
#[derive(Debug, Deserialize)]
pub struct CreateStopLossRequest {
    /// Token to trade
    pub token_id: Uuid,
    /// Quantity to sell when triggered
    pub amount: Decimal,
    /// Price below which the order triggers
    pub trigger_price: Decimal,
    /// Optional limit price for the resulting sell order
    pub limit_price: Option<Decimal>,
}

impl CreateStopLossRequest {
    /// Validate the stop-loss request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }
        if self.trigger_price <= dec!(0) {
            return Err(ValidationError(
                "Trigger price must be positive".to_string(),
            ));
        }
        if let Some(limit) = self.limit_price {
            if limit <= dec!(0) {
                return Err(ValidationError("Limit price must be positive".to_string()));
            }
            if limit > self.trigger_price {
                return Err(ValidationError(
                    "Limit price should be below trigger for stop-loss".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Request to create a take-profit order
#[derive(Debug, Deserialize)]
pub struct CreateTakeProfitRequest {
    /// Token to trade
    pub token_id: Uuid,
    /// Quantity to sell when triggered
    pub amount: Decimal,
    /// Price above which the order triggers
    pub trigger_price: Decimal,
    /// Optional limit price for the resulting sell order
    pub limit_price: Option<Decimal>,
}

impl CreateTakeProfitRequest {
    /// Validate the take-profit request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }
        if self.trigger_price <= dec!(0) {
            return Err(ValidationError(
                "Trigger price must be positive".to_string(),
            ));
        }
        if let Some(limit) = self.limit_price {
            if limit <= dec!(0) {
                return Err(ValidationError("Limit price must be positive".to_string()));
            }
            if limit < self.trigger_price {
                return Err(ValidationError(
                    "Limit price should be above trigger for take-profit".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Request to create a trailing stop order
#[derive(Debug, Deserialize)]
pub struct CreateTrailingStopRequest {
    /// Token to trade
    pub token_id: Uuid,
    /// Quantity to sell when triggered
    pub amount: Decimal,
    /// Distance (in price units) that the stop trails below the high-water mark
    pub trailing_distance: Decimal,
    /// Current market price used to set the initial stop level
    pub initial_price: Decimal,
}

impl CreateTrailingStopRequest {
    /// Validate the trailing stop request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.amount <= dec!(0) {
            return Err(ValidationError("Amount must be positive".to_string()));
        }
        if self.trailing_distance <= dec!(0) {
            return Err(ValidationError(
                "Trailing distance must be positive".to_string(),
            ));
        }
        if self.initial_price <= dec!(0) {
            return Err(ValidationError(
                "Initial price must be positive".to_string(),
            ));
        }
        if self.trailing_distance >= self.initial_price {
            return Err(ValidationError(
                "Trailing distance must be less than initial price".to_string(),
            ));
        }
        Ok(())
    }
}

/// Request to create a TWAP (Time-Weighted Average Price) order
#[derive(Debug, Deserialize)]
pub struct CreateTwapOrderRequest {
    /// Token to trade
    pub token_id: Uuid,
    /// Total quantity to execute over the TWAP window
    pub total_amount: Decimal,
    /// When to begin executing the TWAP order
    pub start_time: DateTime<Utc>,
    /// When to stop executing the TWAP order
    pub end_time: DateTime<Utc>,
    /// Time between individual sub-order executions (seconds)
    pub interval_seconds: i64,
}

impl CreateTwapOrderRequest {
    /// Validate the TWAP order request
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.total_amount <= dec!(0) {
            return Err(ValidationError("Total amount must be positive".to_string()));
        }
        if self.end_time <= self.start_time {
            return Err(ValidationError(
                "End time must be after start time".to_string(),
            ));
        }
        if self.interval_seconds <= 0 {
            return Err(ValidationError("Interval must be positive".to_string()));
        }

        let duration_seconds = (self.end_time - self.start_time).num_seconds();
        if self.interval_seconds > duration_seconds {
            return Err(ValidationError(
                "Interval cannot be longer than total duration".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate amount per interval
    pub fn amount_per_interval(&self) -> Decimal {
        let duration_seconds = (self.end_time - self.start_time).num_seconds();
        let num_intervals = duration_seconds / self.interval_seconds;

        if num_intervals > 0 {
            self.total_amount / Decimal::from(num_intervals)
        } else {
            self.total_amount
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_loss_trigger() {
        let order = AdvancedOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: dec!(100),
            order_type: AdvancedOrderType::StopLoss,
            status: AdvancedOrderStatus::Active,
            trigger_price: Some(dec!(50)),
            limit_price: None,
            trailing_distance: None,
            current_trailing_price: None,
            expires_at: None,
            twap_start_time: None,
            twap_end_time: None,
            twap_interval_seconds: None,
            twap_amount_per_interval: None,
            twap_executed_intervals: 0,
            filled_amount: dec!(0),
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Should trigger when price falls below trigger
        assert!(order.should_trigger(dec!(49)));
        assert!(order.should_trigger(dec!(50)));

        // Should not trigger when price is above trigger
        assert!(!order.should_trigger(dec!(51)));
    }

    #[test]
    fn test_take_profit_trigger() {
        let order = AdvancedOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: dec!(100),
            order_type: AdvancedOrderType::TakeProfit,
            status: AdvancedOrderStatus::Active,
            trigger_price: Some(dec!(100)),
            limit_price: None,
            trailing_distance: None,
            current_trailing_price: None,
            expires_at: None,
            twap_start_time: None,
            twap_end_time: None,
            twap_interval_seconds: None,
            twap_amount_per_interval: None,
            twap_executed_intervals: 0,
            filled_amount: dec!(0),
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Should trigger when price rises above trigger
        assert!(order.should_trigger(dec!(101)));
        assert!(order.should_trigger(dec!(100)));

        // Should not trigger when price is below trigger
        assert!(!order.should_trigger(dec!(99)));
    }

    #[test]
    fn test_trailing_stop_update() {
        let mut order = AdvancedOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: dec!(100),
            order_type: AdvancedOrderType::TrailingStop,
            status: AdvancedOrderStatus::Active,
            trigger_price: None,
            limit_price: None,
            trailing_distance: Some(dec!(5)),
            current_trailing_price: Some(dec!(95)),
            expires_at: None,
            twap_start_time: None,
            twap_end_time: None,
            twap_interval_seconds: None,
            twap_amount_per_interval: None,
            twap_executed_intervals: 0,
            filled_amount: dec!(0),
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Price moves up - trailing stop should update
        assert!(order.update_trailing_stop(dec!(105)));
        assert_eq!(order.current_trailing_price, Some(dec!(100)));

        // Price moves down - trailing stop should not update
        assert!(!order.update_trailing_stop(dec!(103)));
        assert_eq!(order.current_trailing_price, Some(dec!(100)));
    }

    #[test]
    fn test_order_expiration() {
        let past_time = Utc::now() - chrono::Duration::hours(1);
        let future_time = Utc::now() + chrono::Duration::hours(1);

        let expired_order = AdvancedOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            amount: dec!(100),
            order_type: AdvancedOrderType::GoodTillTime,
            status: AdvancedOrderStatus::Active,
            trigger_price: None,
            limit_price: Some(dec!(50)),
            trailing_distance: None,
            current_trailing_price: None,
            expires_at: Some(past_time),
            twap_start_time: None,
            twap_end_time: None,
            twap_interval_seconds: None,
            twap_amount_per_interval: None,
            twap_executed_intervals: 0,
            filled_amount: dec!(0),
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(expired_order.is_expired());

        let active_order = AdvancedOrder {
            expires_at: Some(future_time),
            ..expired_order
        };

        assert!(!active_order.is_expired());
    }

    #[test]
    fn test_twap_amount_calculation() {
        let now = Utc::now();
        let request = CreateTwapOrderRequest {
            token_id: Uuid::new_v4(),
            total_amount: dec!(1000),
            start_time: now,
            end_time: now + chrono::Duration::hours(10),
            interval_seconds: 3600, // 1 hour
        };

        // 10 intervals over 10 hours = 100 per interval
        let amount_per_interval = request.amount_per_interval();
        assert_eq!(amount_per_interval, dec!(100));
    }
}
