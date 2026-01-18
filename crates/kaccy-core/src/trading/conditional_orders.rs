//! Conditional order types for advanced trading strategies
//!
//! This module provides sophisticated order types that enable complex trading strategies:
//! - OCO (One-Cancels-Other): Two orders where if one fills, the other is cancelled
//! - OTO (One-Triggers-Other): First order triggers second order when filled
//! - Bracket orders: Entry order with stop-loss and take-profit
//! - If-Done orders: Conditional order execution based on parent order

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide};

/// Conditional order type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionalOrderType {
    /// One-Cancels-Other: two orders where filling one cancels the other
    OCO {
        /// First order in the pair
        order1: Box<LimitOrder>,
        /// Second order in the pair
        order2: Box<LimitOrder>,
    },
    /// One-Triggers-Other: first order triggers second order when filled
    OTO {
        /// Parent order that must fill first
        parent: Box<LimitOrder>,
        /// Child order triggered after parent fills
        child: Box<LimitOrder>,
    },
    /// Bracket order: entry with stop-loss and take-profit
    Bracket {
        /// Entry order
        entry: Box<LimitOrder>,
        /// Stop-loss order
        stop_loss: Box<LimitOrder>,
        /// Take-profit order
        take_profit: Box<LimitOrder>,
    },
    /// If-Done: child order only submitted if parent order fills
    IfDone {
        /// Parent order
        parent: Box<LimitOrder>,
        /// Child orders to submit if parent fills
        children: Vec<LimitOrder>,
    },
}

/// Status of a conditional order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ConditionalOrderStatus {
    /// Order is pending, waiting for trigger conditions
    Pending,
    /// Order is active in the market
    Active,
    /// Order has been partially filled
    PartiallyFilled,
    /// Order has been fully filled
    Filled,
    /// Order was cancelled
    Cancelled,
    /// Order was triggered (for OTO/Bracket)
    Triggered,
    /// Order expired
    Expired,
    /// Order was rejected
    Rejected,
}

/// Conditional order with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub order_type: ConditionalOrderType,
    pub status: ConditionalOrderStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub metadata: HashMap<String, String>,
}

impl ConditionalOrder {
    /// Create a new OCO (One-Cancels-Other) order
    pub fn new_oco(
        user_id: Uuid,
        order1: LimitOrder,
        order2: LimitOrder,
    ) -> Result<Self, &'static str> {
        // Validate that orders are for the same token
        if order1.token_id != order2.token_id {
            return Err("OCO orders must be for the same token");
        }

        // Validate that orders are on opposite sides (or at different prices)
        if order1.side == order2.side && order1.price == order2.price {
            return Err("OCO orders must have different conditions");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: ConditionalOrderType::OCO {
                order1: Box::new(order1),
                order2: Box::new(order2),
            },
            status: ConditionalOrderStatus::Pending,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        })
    }

    /// Create a new OTO (One-Triggers-Other) order
    pub fn new_oto(
        user_id: Uuid,
        parent: LimitOrder,
        child: LimitOrder,
    ) -> Result<Self, &'static str> {
        // Validate that orders are for the same token
        if parent.token_id != child.token_id {
            return Err("OTO orders must be for the same token");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: ConditionalOrderType::OTO {
                parent: Box::new(parent),
                child: Box::new(child),
            },
            status: ConditionalOrderStatus::Pending,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Bracket order
    pub fn new_bracket(
        user_id: Uuid,
        entry: LimitOrder,
        stop_loss: LimitOrder,
        take_profit: LimitOrder,
    ) -> Result<Self, &'static str> {
        // Validate all orders are for the same token
        if entry.token_id != stop_loss.token_id || entry.token_id != take_profit.token_id {
            return Err("Bracket orders must be for the same token");
        }

        // Validate price relationships
        match entry.side {
            OrderSide::Buy => {
                // For a buy entry: stop loss < entry < take profit
                if stop_loss.price >= entry.price {
                    return Err("Stop loss must be below entry price for buy orders");
                }
                if take_profit.price <= entry.price {
                    return Err("Take profit must be above entry price for buy orders");
                }
            }
            OrderSide::Sell => {
                // For a sell entry: take profit < entry < stop loss
                if take_profit.price >= entry.price {
                    return Err("Take profit must be below entry price for sell orders");
                }
                if stop_loss.price <= entry.price {
                    return Err("Stop loss must be above entry price for sell orders");
                }
            }
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: ConditionalOrderType::Bracket {
                entry: Box::new(entry),
                stop_loss: Box::new(stop_loss),
                take_profit: Box::new(take_profit),
            },
            status: ConditionalOrderStatus::Pending,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        })
    }

    /// Create a new If-Done order
    pub fn new_if_done(
        user_id: Uuid,
        parent: LimitOrder,
        children: Vec<LimitOrder>,
    ) -> Result<Self, &'static str> {
        if children.is_empty() {
            return Err("If-Done order must have at least one child order");
        }

        // Validate all orders are for the same token
        let token_id = parent.token_id;
        if !children.iter().all(|c| c.token_id == token_id) {
            return Err("All If-Done orders must be for the same token");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: ConditionalOrderType::IfDone {
                parent: Box::new(parent),
                children,
            },
            status: ConditionalOrderStatus::Pending,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        })
    }

    /// Update order status
    pub fn update_status(&mut self, status: ConditionalOrderStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now().timestamp();
    }
}

/// Manager for conditional orders
pub struct ConditionalOrderManager {
    /// Active conditional orders
    orders: Arc<RwLock<HashMap<Uuid, ConditionalOrder>>>,
    /// Mapping of order ID to conditional order ID
    order_to_conditional: Arc<RwLock<HashMap<Uuid, Uuid>>>,
}

impl ConditionalOrderManager {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            order_to_conditional: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Submit a conditional order
    pub async fn submit(&self, order: ConditionalOrder) -> Result<Uuid, &'static str> {
        let order_id = order.id;

        // Track the relationship between component orders and the conditional order
        match &order.order_type {
            ConditionalOrderType::OCO { order1, order2 } => {
                let mut mapping = self.order_to_conditional.write().await;
                mapping.insert(order1.order_id, order_id);
                mapping.insert(order2.order_id, order_id);
            }
            ConditionalOrderType::OTO { parent, child } => {
                let mut mapping = self.order_to_conditional.write().await;
                mapping.insert(parent.order_id, order_id);
                mapping.insert(child.order_id, order_id);
            }
            ConditionalOrderType::Bracket {
                entry,
                stop_loss,
                take_profit,
            } => {
                let mut mapping = self.order_to_conditional.write().await;
                mapping.insert(entry.order_id, order_id);
                mapping.insert(stop_loss.order_id, order_id);
                mapping.insert(take_profit.order_id, order_id);
            }
            ConditionalOrderType::IfDone { parent, children } => {
                let mut mapping = self.order_to_conditional.write().await;
                mapping.insert(parent.order_id, order_id);
                for child in children {
                    mapping.insert(child.order_id, order_id);
                }
            }
        }

        // Store the conditional order
        self.orders.write().await.insert(order_id, order);

        Ok(order_id)
    }

    /// Handle order fill event
    pub async fn on_order_filled(&self, order_id: Uuid) -> Option<Vec<OrderAction>> {
        // Find the conditional order
        let conditional_id = self
            .order_to_conditional
            .read()
            .await
            .get(&order_id)
            .copied()?;

        let mut orders = self.orders.write().await;
        let conditional_order = orders.get_mut(&conditional_id)?;

        // Determine actions and new status
        let (actions, new_status) = match &conditional_order.order_type {
            ConditionalOrderType::OCO { order1, order2 } => {
                // Cancel the other order
                if order1.order_id == order_id {
                    (
                        Some(vec![OrderAction::Cancel(order2.order_id)]),
                        Some(ConditionalOrderStatus::Filled),
                    )
                } else {
                    (
                        Some(vec![OrderAction::Cancel(order1.order_id)]),
                        Some(ConditionalOrderStatus::Filled),
                    )
                }
            }
            ConditionalOrderType::OTO { parent, child } => {
                if parent.order_id == order_id {
                    // Parent filled, submit child
                    (
                        Some(vec![OrderAction::Submit((**child).clone())]),
                        Some(ConditionalOrderStatus::Triggered),
                    )
                } else {
                    // Child filled
                    (None, Some(ConditionalOrderStatus::Filled))
                }
            }
            ConditionalOrderType::Bracket {
                entry,
                stop_loss,
                take_profit,
            } => {
                if entry.order_id == order_id {
                    // Entry filled, submit stop-loss and take-profit as OCO
                    (
                        Some(vec![
                            OrderAction::Submit((**stop_loss).clone()),
                            OrderAction::Submit((**take_profit).clone()),
                        ]),
                        Some(ConditionalOrderStatus::Triggered),
                    )
                } else if stop_loss.order_id == order_id {
                    // Stop-loss triggered, cancel take-profit
                    (
                        Some(vec![OrderAction::Cancel(take_profit.order_id)]),
                        Some(ConditionalOrderStatus::Filled),
                    )
                } else {
                    // Take-profit triggered, cancel stop-loss
                    (
                        Some(vec![OrderAction::Cancel(stop_loss.order_id)]),
                        Some(ConditionalOrderStatus::Filled),
                    )
                }
            }
            ConditionalOrderType::IfDone { parent, children } => {
                if parent.order_id == order_id {
                    // Parent filled, submit all children
                    (
                        Some(
                            children
                                .iter()
                                .map(|c| OrderAction::Submit(c.clone()))
                                .collect(),
                        ),
                        Some(ConditionalOrderStatus::Triggered),
                    )
                } else {
                    // One of the children filled
                    (None, Some(ConditionalOrderStatus::PartiallyFilled))
                }
            }
        };

        // Update status if needed
        if let Some(status) = new_status {
            conditional_order.update_status(status);
        }

        actions
    }

    /// Cancel a conditional order
    pub async fn cancel(&self, conditional_id: Uuid) -> Option<Vec<Uuid>> {
        let mut orders = self.orders.write().await;
        let order = orders.get_mut(&conditional_id)?;

        order.update_status(ConditionalOrderStatus::Cancelled);

        // Get all component order IDs
        let component_ids = match &order.order_type {
            ConditionalOrderType::OCO { order1, order2 } => {
                vec![order1.order_id, order2.order_id]
            }
            ConditionalOrderType::OTO { parent, child } => {
                vec![parent.order_id, child.order_id]
            }
            ConditionalOrderType::Bracket {
                entry,
                stop_loss,
                take_profit,
            } => {
                vec![entry.order_id, stop_loss.order_id, take_profit.order_id]
            }
            ConditionalOrderType::IfDone { parent, children } => {
                let mut ids = vec![parent.order_id];
                ids.extend(children.iter().map(|c| c.order_id));
                ids
            }
        };

        Some(component_ids)
    }

    /// Get a conditional order by ID
    pub async fn get(&self, id: Uuid) -> Option<ConditionalOrder> {
        self.orders.read().await.get(&id).cloned()
    }

    /// Get all conditional orders for a user
    pub async fn get_by_user(&self, user_id: Uuid) -> Vec<ConditionalOrder> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Get active conditional orders for a user
    pub async fn get_active_by_user(&self, user_id: Uuid) -> Vec<ConditionalOrder> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| {
                o.user_id == user_id
                    && matches!(
                        o.status,
                        ConditionalOrderStatus::Pending
                            | ConditionalOrderStatus::Active
                            | ConditionalOrderStatus::PartiallyFilled
                    )
            })
            .cloned()
            .collect()
    }
}

impl Default for ConditionalOrderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions to take when a conditional order is triggered
#[derive(Debug, Clone)]
pub enum OrderAction {
    /// Submit a new order
    Submit(LimitOrder),
    /// Cancel an existing order
    Cancel(Uuid),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn create_test_order(
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
        token_id: Uuid,
    ) -> LimitOrder {
        LimitOrder {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id,
            side,
            price,
            amount: quantity,
            filled_amount: Decimal::ZERO,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_oco_order_creation() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let order1 = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let order2 = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);

        let oco = ConditionalOrder::new_oco(user_id, order1, order2);
        assert!(oco.is_ok());
        assert_eq!(oco.unwrap().status, ConditionalOrderStatus::Pending);
    }

    #[test]
    fn test_oco_order_different_tokens() {
        let user_id = Uuid::new_v4();

        let order1 = create_test_order(OrderSide::Buy, dec!(100), dec!(10), Uuid::new_v4());
        let order2 = create_test_order(OrderSide::Sell, dec!(105), dec!(10), Uuid::new_v4());

        let oco = ConditionalOrder::new_oco(user_id, order1, order2);
        assert!(oco.is_err());
    }

    #[test]
    fn test_oto_order_creation() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let parent = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let child = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);

        let oto = ConditionalOrder::new_oto(user_id, parent, child);
        assert!(oto.is_ok());
    }

    #[test]
    fn test_bracket_order_buy() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let stop_loss = create_test_order(OrderSide::Sell, dec!(95), dec!(10), token_id);
        let take_profit = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);

        let bracket = ConditionalOrder::new_bracket(user_id, entry, stop_loss, take_profit);
        assert!(bracket.is_ok());
    }

    #[test]
    fn test_bracket_order_invalid_prices() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        // Invalid: stop loss above entry for buy
        let stop_loss = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);
        let take_profit = create_test_order(OrderSide::Sell, dec!(110), dec!(10), token_id);

        let bracket = ConditionalOrder::new_bracket(user_id, entry, stop_loss, take_profit);
        assert!(bracket.is_err());
    }

    #[test]
    fn test_if_done_order() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let parent = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let child1 = create_test_order(OrderSide::Sell, dec!(105), dec!(5), token_id);
        let child2 = create_test_order(OrderSide::Sell, dec!(110), dec!(5), token_id);

        let if_done = ConditionalOrder::new_if_done(user_id, parent, vec![child1, child2]);
        assert!(if_done.is_ok());
    }

    #[tokio::test]
    async fn test_conditional_order_manager() {
        let manager = ConditionalOrderManager::new();
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let order1 = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let order2 = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);
        let order1_id = order1.order_id;

        let oco = ConditionalOrder::new_oco(user_id, order1, order2).unwrap();
        let _oco_id = manager.submit(oco).await.unwrap();

        // Test on_order_filled
        let actions = manager.on_order_filled(order1_id).await;
        assert!(actions.is_some());

        let actions = actions.unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], OrderAction::Cancel(_)));
    }

    #[tokio::test]
    async fn test_bracket_order_trigger() {
        let manager = ConditionalOrderManager::new();
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let entry = create_test_order(OrderSide::Buy, dec!(100), dec!(10), token_id);
        let entry_id = entry.order_id;
        let stop_loss = create_test_order(OrderSide::Sell, dec!(95), dec!(10), token_id);
        let take_profit = create_test_order(OrderSide::Sell, dec!(105), dec!(10), token_id);

        let bracket =
            ConditionalOrder::new_bracket(user_id, entry, stop_loss, take_profit).unwrap();
        manager.submit(bracket).await.unwrap();

        // Fill entry order
        let actions = manager.on_order_filled(entry_id).await;
        assert!(actions.is_some());

        let actions = actions.unwrap();
        assert_eq!(actions.len(), 2); // Submit stop-loss and take-profit
    }
}
