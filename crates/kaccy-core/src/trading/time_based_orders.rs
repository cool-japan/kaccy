//! Time-based order types for advanced trading
//!
//! This module provides orders that have time-based activation or execution:
//! - MOC (Market-on-Close): Execute at market price at session close
//! - LOC (Limit-on-Close): Execute at limit price at session close
//! - GAT (Good-After-Time): Order becomes active after specified time
//! - Day orders: Orders that expire at end of trading day
//! - GTT (Good-Till-Time): Orders that expire at specific time

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide};

/// Time-based order type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimeBasedOrderType {
    /// Market-on-Close: execute at market price at session close
    MOC {
        token_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        /// Trading session close time (Unix timestamp)
        close_time: i64,
    },
    /// Limit-on-Close: execute at limit price at session close
    LOC {
        token_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
        /// Trading session close time (Unix timestamp)
        close_time: i64,
    },
    /// Good-After-Time: order becomes active after specified time
    GAT {
        order: Box<LimitOrder>,
        /// Activation time (Unix timestamp)
        activation_time: i64,
    },
    /// Good-Till-Time: order expires at specified time
    GTT {
        order: Box<LimitOrder>,
        /// Expiration time (Unix timestamp)
        expiration_time: i64,
    },
    /// Day order: expires at end of trading day
    Day {
        order: Box<LimitOrder>,
        /// End of day time (Unix timestamp)
        eod_time: i64,
    },
    /// Time slice order: execute only during specific time window
    TimeSlice {
        order: Box<LimitOrder>,
        /// Window start time (Unix timestamp)
        start_time: i64,
        /// Window end time (Unix timestamp)
        end_time: i64,
    },
}

/// Status of a time-based order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TimeBasedOrderStatus {
    /// Order is scheduled but not yet active
    Scheduled,
    /// Order is active and can be executed
    Active,
    /// Order has been partially filled
    PartiallyFilled,
    /// Order has been fully filled
    Filled,
    /// Order expired before execution
    Expired,
    /// Order was cancelled
    Cancelled,
    /// Order was rejected
    Rejected,
}

/// Time-based order with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub order_type: TimeBasedOrderType,
    pub status: TimeBasedOrderStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub filled_quantity: Decimal,
    pub metadata: HashMap<String, String>,
}

impl TimeBasedOrder {
    /// Create a new Market-on-Close order
    pub fn new_moc(
        user_id: Uuid,
        token_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        close_time: i64,
    ) -> Result<Self, &'static str> {
        if quantity <= Decimal::ZERO {
            return Err("Quantity must be positive");
        }

        let now = chrono::Utc::now().timestamp();
        if close_time <= now {
            return Err("Close time must be in the future");
        }

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::MOC {
                token_id,
                side,
                quantity,
                close_time,
            },
            status: TimeBasedOrderStatus::Scheduled,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Limit-on-Close order
    pub fn new_loc(
        user_id: Uuid,
        token_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
        close_time: i64,
    ) -> Result<Self, &'static str> {
        if quantity <= Decimal::ZERO {
            return Err("Quantity must be positive");
        }

        if price <= Decimal::ZERO {
            return Err("Price must be positive");
        }

        let now = chrono::Utc::now().timestamp();
        if close_time <= now {
            return Err("Close time must be in the future");
        }

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::LOC {
                token_id,
                side,
                quantity,
                price,
                close_time,
            },
            status: TimeBasedOrderStatus::Scheduled,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Good-After-Time order
    pub fn new_gat(
        user_id: Uuid,
        order: LimitOrder,
        activation_time: i64,
    ) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();
        if activation_time <= now {
            return Err("Activation time must be in the future");
        }

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::GAT {
                order: Box::new(order),
                activation_time,
            },
            status: TimeBasedOrderStatus::Scheduled,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Good-Till-Time order
    pub fn new_gtt(
        user_id: Uuid,
        order: LimitOrder,
        expiration_time: i64,
    ) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();
        if expiration_time <= now {
            return Err("Expiration time must be in the future");
        }

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::GTT {
                order: Box::new(order),
                expiration_time,
            },
            status: TimeBasedOrderStatus::Active, // GTT is active immediately
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Day order
    pub fn new_day(user_id: Uuid, order: LimitOrder, eod_time: i64) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();
        if eod_time <= now {
            return Err("End of day time must be in the future");
        }

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::Day {
                order: Box::new(order),
                eod_time,
            },
            status: TimeBasedOrderStatus::Active, // Day orders are active immediately
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Time Slice order
    pub fn new_time_slice(
        user_id: Uuid,
        order: LimitOrder,
        start_time: i64,
        end_time: i64,
    ) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();

        if start_time >= end_time {
            return Err("Start time must be before end time");
        }

        if end_time <= now {
            return Err("End time must be in the future");
        }

        let status = if start_time > now {
            TimeBasedOrderStatus::Scheduled
        } else {
            TimeBasedOrderStatus::Active
        };

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: TimeBasedOrderType::TimeSlice {
                order: Box::new(order),
                start_time,
                end_time,
            },
            status,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Update order status
    pub fn update_status(&mut self, status: TimeBasedOrderStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Check if order should be activated at current time
    pub fn should_activate(&self, current_time: i64) -> bool {
        match &self.order_type {
            TimeBasedOrderType::GAT {
                activation_time, ..
            } => current_time >= *activation_time && self.status == TimeBasedOrderStatus::Scheduled,
            TimeBasedOrderType::TimeSlice { start_time, .. } => {
                current_time >= *start_time && self.status == TimeBasedOrderStatus::Scheduled
            }
            _ => false,
        }
    }

    /// Check if order has expired at current time
    pub fn is_expired(&self, current_time: i64) -> bool {
        match &self.order_type {
            TimeBasedOrderType::MOC { close_time, .. } => current_time > *close_time,
            TimeBasedOrderType::LOC { close_time, .. } => current_time > *close_time,
            TimeBasedOrderType::GTT {
                expiration_time, ..
            } => current_time > *expiration_time,
            TimeBasedOrderType::Day { eod_time, .. } => current_time > *eod_time,
            TimeBasedOrderType::TimeSlice { end_time, .. } => current_time > *end_time,
            _ => false,
        }
    }

    /// Check if order should execute on close
    pub fn should_execute_on_close(&self, current_time: i64) -> bool {
        match &self.order_type {
            TimeBasedOrderType::MOC { close_time, .. } => {
                current_time >= *close_time && self.status != TimeBasedOrderStatus::Filled
            }
            TimeBasedOrderType::LOC { close_time, .. } => {
                current_time >= *close_time && self.status != TimeBasedOrderStatus::Filled
            }
            _ => false,
        }
    }
}

/// Manager for time-based orders
pub struct TimeBasedOrderManager {
    /// Active time-based orders
    orders: Arc<RwLock<HashMap<Uuid, TimeBasedOrder>>>,
    /// Scheduled activations (activation_time -> Vec<order_id>)
    scheduled_activations: Arc<RwLock<HashMap<i64, Vec<Uuid>>>>,
    /// Scheduled expirations (expiration_time -> Vec<order_id>)
    scheduled_expirations: Arc<RwLock<HashMap<i64, Vec<Uuid>>>>,
}

impl TimeBasedOrderManager {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            scheduled_activations: Arc::new(RwLock::new(HashMap::new())),
            scheduled_expirations: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Submit a time-based order
    pub async fn submit(&self, order: TimeBasedOrder) -> Result<Uuid, &'static str> {
        let order_id = order.id;

        // Schedule activation if needed
        match &order.order_type {
            TimeBasedOrderType::GAT {
                activation_time, ..
            } => {
                self.scheduled_activations
                    .write()
                    .await
                    .entry(*activation_time)
                    .or_insert_with(Vec::new)
                    .push(order_id);
            }
            TimeBasedOrderType::TimeSlice { start_time, .. } => {
                if order.status == TimeBasedOrderStatus::Scheduled {
                    self.scheduled_activations
                        .write()
                        .await
                        .entry(*start_time)
                        .or_insert_with(Vec::new)
                        .push(order_id);
                }
            }
            _ => {}
        }

        // Schedule expiration
        let expiration_time = match &order.order_type {
            TimeBasedOrderType::MOC { close_time, .. } => Some(*close_time),
            TimeBasedOrderType::LOC { close_time, .. } => Some(*close_time),
            TimeBasedOrderType::GTT {
                expiration_time, ..
            } => Some(*expiration_time),
            TimeBasedOrderType::Day { eod_time, .. } => Some(*eod_time),
            TimeBasedOrderType::TimeSlice { end_time, .. } => Some(*end_time),
            _ => None,
        };

        if let Some(exp_time) = expiration_time {
            self.scheduled_expirations
                .write()
                .await
                .entry(exp_time)
                .or_insert_with(Vec::new)
                .push(order_id);
        }

        // Store the order
        self.orders.write().await.insert(order_id, order);

        Ok(order_id)
    }

    /// Process time events (activations and expirations)
    pub async fn process_time_events(&self, current_time: i64) -> TimeEventResult {
        let mut activated = Vec::new();
        let mut expired = Vec::new();
        let mut to_execute_on_close = Vec::new();

        // Check for activations
        let activation_times: Vec<i64> = self
            .scheduled_activations
            .read()
            .await
            .keys()
            .filter(|&&t| t <= current_time)
            .copied()
            .collect();

        for time in activation_times {
            if let Some(order_ids) = self.scheduled_activations.write().await.remove(&time) {
                for order_id in order_ids {
                    if let Some(order) = self.orders.write().await.get_mut(&order_id) {
                        if order.should_activate(current_time) {
                            order.update_status(TimeBasedOrderStatus::Active);
                            activated.push(order_id);
                        }
                    }
                }
            }
        }

        // Check for expirations
        let expiration_times: Vec<i64> = self
            .scheduled_expirations
            .read()
            .await
            .keys()
            .filter(|&&t| t <= current_time)
            .copied()
            .collect();

        for time in expiration_times {
            if let Some(order_ids) = self.scheduled_expirations.write().await.remove(&time) {
                for order_id in order_ids {
                    if let Some(order) = self.orders.write().await.get_mut(&order_id) {
                        if order.should_execute_on_close(current_time) {
                            to_execute_on_close.push(order_id);
                        } else if order.is_expired(current_time)
                            && !matches!(order.status, TimeBasedOrderStatus::Filled)
                        {
                            order.update_status(TimeBasedOrderStatus::Expired);
                            expired.push(order_id);
                        }
                    }
                }
            }
        }

        TimeEventResult {
            activated,
            expired,
            to_execute_on_close,
        }
    }

    /// Cancel a time-based order
    pub async fn cancel(&self, order_id: Uuid) -> Option<TimeBasedOrder> {
        if let Some(mut order) = self.orders.write().await.remove(&order_id) {
            order.update_status(TimeBasedOrderStatus::Cancelled);
            Some(order)
        } else {
            None
        }
    }

    /// Get a time-based order by ID
    pub async fn get(&self, id: Uuid) -> Option<TimeBasedOrder> {
        self.orders.read().await.get(&id).cloned()
    }

    /// Get all time-based orders for a user
    pub async fn get_by_user(&self, user_id: Uuid) -> Vec<TimeBasedOrder> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Get active orders for a user
    pub async fn get_active_by_user(&self, user_id: Uuid) -> Vec<TimeBasedOrder> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| {
                o.user_id == user_id
                    && matches!(
                        o.status,
                        TimeBasedOrderStatus::Active
                            | TimeBasedOrderStatus::Scheduled
                            | TimeBasedOrderStatus::PartiallyFilled
                    )
            })
            .cloned()
            .collect()
    }

    /// Update fill status for an order
    pub async fn update_fill(&self, order_id: Uuid, filled_quantity: Decimal, is_complete: bool) {
        if let Some(order) = self.orders.write().await.get_mut(&order_id) {
            order.filled_quantity = filled_quantity;
            if is_complete {
                order.update_status(TimeBasedOrderStatus::Filled);
            } else {
                order.update_status(TimeBasedOrderStatus::PartiallyFilled);
            }
        }
    }
}

impl Default for TimeBasedOrderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing time events
#[derive(Debug, Clone)]
pub struct TimeEventResult {
    /// Orders that were activated
    pub activated: Vec<Uuid>,
    /// Orders that expired
    pub expired: Vec<Uuid>,
    /// Orders to execute at close
    pub to_execute_on_close: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_order(token_id: Uuid, side: OrderSide, price: Decimal) -> LimitOrder {
        LimitOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id,
            side,
            price,
            amount: dec!(100),
            filled_amount: Decimal::ZERO,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_moc_order_creation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let close_time = chrono::Utc::now().timestamp() + 3600; // 1 hour from now

        let moc = TimeBasedOrder::new_moc(user_id, token_id, OrderSide::Buy, dec!(100), close_time);
        assert!(moc.is_ok());
        assert_eq!(moc.unwrap().status, TimeBasedOrderStatus::Scheduled);
    }

    #[test]
    fn test_moc_order_past_close_time() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let close_time = chrono::Utc::now().timestamp() - 3600; // 1 hour ago

        let moc = TimeBasedOrder::new_moc(user_id, token_id, OrderSide::Buy, dec!(100), close_time);
        assert!(moc.is_err());
    }

    #[test]
    fn test_loc_order_creation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let close_time = chrono::Utc::now().timestamp() + 3600;

        let loc = TimeBasedOrder::new_loc(
            user_id,
            token_id,
            OrderSide::Buy,
            dec!(100),
            dec!(50),
            close_time,
        );
        assert!(loc.is_ok());
    }

    #[test]
    fn test_gat_order_creation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let activation_time = chrono::Utc::now().timestamp() + 3600;

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let gat = TimeBasedOrder::new_gat(user_id, order, activation_time);
        assert!(gat.is_ok());
        assert_eq!(gat.unwrap().status, TimeBasedOrderStatus::Scheduled);
    }

    #[test]
    fn test_gtt_order_creation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let expiration_time = chrono::Utc::now().timestamp() + 3600;

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let gtt = TimeBasedOrder::new_gtt(user_id, order, expiration_time);
        assert!(gtt.is_ok());
        assert_eq!(gtt.unwrap().status, TimeBasedOrderStatus::Active);
    }

    #[test]
    fn test_time_slice_order() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();
        let start_time = now + 1800; // 30 minutes from now
        let end_time = now + 3600; // 1 hour from now

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let slice = TimeBasedOrder::new_time_slice(user_id, order, start_time, end_time);
        assert!(slice.is_ok());
        assert_eq!(slice.unwrap().status, TimeBasedOrderStatus::Scheduled);
    }

    #[test]
    fn test_should_activate() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let activation_time = chrono::Utc::now().timestamp() + 100;

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let gat = TimeBasedOrder::new_gat(user_id, order, activation_time).unwrap();

        assert!(!gat.should_activate(activation_time - 50));
        assert!(gat.should_activate(activation_time + 50));
    }

    #[test]
    fn test_is_expired() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let expiration_time = chrono::Utc::now().timestamp() + 100;

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let gtt = TimeBasedOrder::new_gtt(user_id, order, expiration_time).unwrap();

        assert!(!gtt.is_expired(expiration_time - 50));
        assert!(gtt.is_expired(expiration_time + 50));
    }

    #[tokio::test]
    async fn test_time_based_order_manager() {
        let manager = TimeBasedOrderManager::new();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let activation_time = chrono::Utc::now().timestamp() + 100;

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let gat = TimeBasedOrder::new_gat(user_id, order, activation_time).unwrap();
        let order_id = manager.submit(gat).await.unwrap();

        // Process before activation time
        let result = manager.process_time_events(activation_time - 50).await;
        assert_eq!(result.activated.len(), 0);

        // Process after activation time
        let result = manager.process_time_events(activation_time + 50).await;
        assert_eq!(result.activated.len(), 1);
        assert_eq!(result.activated[0], order_id);
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let manager = TimeBasedOrderManager::new();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let close_time = chrono::Utc::now().timestamp() + 3600;

        let moc = TimeBasedOrder::new_moc(user_id, token_id, OrderSide::Buy, dec!(100), close_time)
            .unwrap();
        let order_id = manager.submit(moc).await.unwrap();

        let cancelled = manager.cancel(order_id).await;
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().status, TimeBasedOrderStatus::Cancelled);
    }
}
