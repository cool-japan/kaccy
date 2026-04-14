//! Advanced execution order types
//!
//! This module provides sophisticated order execution strategies:
//! - Pegged orders: Orders that track a reference price (primary peg, market peg, mid-point peg)
//! - Hidden/Iceberg orders: Display only visible portion, rest is hidden
//! - Discretionary orders: Orders with price improvement discretion
//! - Minimum quantity orders: Only execute if minimum quantity is available
//! - Fill-or-Kill (FOK): Execute entire order immediately or cancel
//! - Immediate-or-Cancel (IOC): Execute available quantity immediately, cancel rest

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide};

/// Peg type for pegged orders
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PegType {
    /// Peg to primary market (best bid/ask)
    Primary {
        /// Offset from primary price (can be positive or negative)
        offset: Decimal,
    },
    /// Peg to market (mid-point of bid/ask)
    Market {
        /// Offset from mid-point
        offset: Decimal,
    },
    /// Peg to mid-point of NBBO (National Best Bid and Offer)
    MidPoint {
        /// Offset from mid-point
        offset: Decimal,
    },
}

/// Advanced execution order type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdvancedExecutionType {
    /// Pegged order that tracks a reference price
    Pegged {
        /// Token being traded
        token_id: Uuid,
        /// Whether this is a buy or sell
        side: OrderSide,
        /// Total quantity to trade
        quantity: Decimal,
        /// Peg reference type and offset
        peg_type: PegType,
        /// Optional limit price (worst acceptable price)
        limit_price: Option<Decimal>,
    },
    /// Hidden/Iceberg order with visible and reserve quantity
    Hidden {
        /// The underlying order
        order: Box<LimitOrder>,
        /// Visible quantity shown in order book
        visible_quantity: Decimal,
        /// Reserve (hidden) quantity
        reserve_quantity: Decimal,
        /// Replenishment quantity (how much to show when visible is filled)
        replenish_quantity: Decimal,
    },
    /// Discretionary order with price improvement range
    Discretionary {
        /// The underlying limit order
        order: Box<LimitOrder>,
        /// Discretion amount (how much price can be improved)
        discretion_amount: Decimal,
        /// Direction of discretion (for buy: can pay up to price + discretion)
        discretion_direction: DiscretionDirection,
    },
    /// Minimum quantity order (only execute if min quantity available)
    MinimumQuantity {
        /// The underlying limit order
        order: Box<LimitOrder>,
        /// Minimum quantity that must be filled
        minimum_quantity: Decimal,
    },
    /// Fill-or-Kill: Execute entire order immediately or cancel
    FillOrKill {
        /// The underlying limit order
        order: Box<LimitOrder>,
    },
    /// Immediate-or-Cancel: Execute available quantity immediately, cancel rest
    ImmediateOrCancel {
        /// The underlying limit order
        order: Box<LimitOrder>,
    },
}

/// Direction of discretion for discretionary orders
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DiscretionDirection {
    /// For buy orders: can pay up to (price + discretion)
    /// For sell orders: can accept down to (price - discretion)
    Favorable,
    /// For buy orders: must be at or below (price - discretion)
    /// For sell orders: must be at or above (price + discretion)
    Unfavorable,
}

/// Status of an advanced execution order
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AdvancedExecutionStatus {
    /// Order is active
    Active,
    /// Order is partially filled
    PartiallyFilled,
    /// Order is fully filled
    Filled,
    /// Order was cancelled
    Cancelled,
    /// Order was rejected (e.g., FOK couldn't fill completely)
    Rejected,
    /// Order expired or was killed (e.g., IOC unfilled portion)
    Expired,
}

/// Advanced execution order with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedExecutionOrder {
    /// Unique identifier for this order
    pub id: Uuid,
    /// User who submitted the order
    pub user_id: Uuid,
    /// Specific advanced execution type and parameters
    pub order_type: AdvancedExecutionType,
    /// Current lifecycle status of the order
    pub status: AdvancedExecutionStatus,
    /// Unix timestamp (seconds) when the order was created
    pub created_at: i64,
    /// Unix timestamp (seconds) when the order was last updated
    pub updated_at: i64,
    /// Amount of the order that has been filled so far
    pub filled_quantity: Decimal,
    /// Arbitrary key-value metadata attached to the order
    pub metadata: HashMap<String, String>,
}

impl AdvancedExecutionOrder {
    /// Create a new pegged order
    pub fn new_pegged(
        user_id: Uuid,
        token_id: Uuid,
        side: OrderSide,
        quantity: Decimal,
        peg_type: PegType,
        limit_price: Option<Decimal>,
    ) -> Result<Self, &'static str> {
        if quantity <= Decimal::ZERO {
            return Err("Quantity must be positive");
        }

        if let Some(limit) = limit_price {
            if limit <= Decimal::ZERO {
                return Err("Limit price must be positive");
            }
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::Pegged {
                token_id,
                side,
                quantity,
                peg_type,
                limit_price,
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new hidden/iceberg order
    pub fn new_hidden(
        user_id: Uuid,
        order: LimitOrder,
        visible_quantity: Decimal,
        reserve_quantity: Decimal,
        replenish_quantity: Decimal,
    ) -> Result<Self, &'static str> {
        if visible_quantity <= Decimal::ZERO {
            return Err("Visible quantity must be positive");
        }

        if reserve_quantity < Decimal::ZERO {
            return Err("Reserve quantity cannot be negative");
        }

        if visible_quantity + reserve_quantity != order.amount {
            return Err("Visible + reserve must equal total order quantity");
        }

        if replenish_quantity > reserve_quantity {
            return Err("Replenish quantity cannot exceed reserve");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::Hidden {
                order: Box::new(order),
                visible_quantity,
                reserve_quantity,
                replenish_quantity,
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new discretionary order
    pub fn new_discretionary(
        user_id: Uuid,
        order: LimitOrder,
        discretion_amount: Decimal,
        discretion_direction: DiscretionDirection,
    ) -> Result<Self, &'static str> {
        if discretion_amount < Decimal::ZERO {
            return Err("Discretion amount cannot be negative");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::Discretionary {
                order: Box::new(order),
                discretion_amount,
                discretion_direction,
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new minimum quantity order
    pub fn new_minimum_quantity(
        user_id: Uuid,
        order: LimitOrder,
        minimum_quantity: Decimal,
    ) -> Result<Self, &'static str> {
        if minimum_quantity <= Decimal::ZERO {
            return Err("Minimum quantity must be positive");
        }

        if minimum_quantity > order.amount {
            return Err("Minimum quantity cannot exceed total quantity");
        }

        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::MinimumQuantity {
                order: Box::new(order),
                minimum_quantity,
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Fill-or-Kill order
    pub fn new_fok(user_id: Uuid, order: LimitOrder) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::FillOrKill {
                order: Box::new(order),
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Immediate-or-Cancel order
    pub fn new_ioc(user_id: Uuid, order: LimitOrder) -> Result<Self, &'static str> {
        let now = chrono::Utc::now().timestamp();
        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            order_type: AdvancedExecutionType::ImmediateOrCancel {
                order: Box::new(order),
            },
            status: AdvancedExecutionStatus::Active,
            created_at: now,
            updated_at: now,
            filled_quantity: Decimal::ZERO,
            metadata: HashMap::new(),
        })
    }

    /// Update order status
    pub fn update_status(&mut self, status: AdvancedExecutionStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Calculate pegged price based on market data
    pub fn calculate_pegged_price(&self, best_bid: Decimal, best_ask: Decimal) -> Option<Decimal> {
        match &self.order_type {
            AdvancedExecutionType::Pegged {
                side,
                peg_type,
                limit_price,
                ..
            } => {
                let pegged_price = match peg_type {
                    PegType::Primary { offset } => match side {
                        OrderSide::Buy => best_bid + offset,
                        OrderSide::Sell => best_ask + offset,
                    },
                    PegType::Market { offset } => {
                        let mid = (best_bid + best_ask) / dec!(2);
                        mid + offset
                    }
                    PegType::MidPoint { offset } => {
                        let mid = (best_bid + best_ask) / dec!(2);
                        mid + offset
                    }
                };

                // Apply limit price if specified
                if let Some(limit) = limit_price {
                    match side {
                        OrderSide::Buy => {
                            if pegged_price > *limit {
                                return None; // Price too high
                            }
                        }
                        OrderSide::Sell => {
                            if pegged_price < *limit {
                                return None; // Price too low
                            }
                        }
                    }
                }

                Some(pegged_price)
            }
            _ => None,
        }
    }

    /// Get acceptable price range for discretionary order
    pub fn get_discretionary_price_range(&self) -> Option<(Decimal, Decimal)> {
        match &self.order_type {
            AdvancedExecutionType::Discretionary {
                order,
                discretion_amount,
                discretion_direction,
            } => {
                let price = order.price;
                match (order.side, discretion_direction) {
                    (OrderSide::Buy, DiscretionDirection::Favorable) => {
                        // Can pay up to price + discretion
                        Some((price, price + *discretion_amount))
                    }
                    (OrderSide::Buy, DiscretionDirection::Unfavorable) => {
                        // Must be at or below price - discretion
                        Some((price - *discretion_amount, price))
                    }
                    (OrderSide::Sell, DiscretionDirection::Favorable) => {
                        // Can accept down to price - discretion
                        Some((price - *discretion_amount, price))
                    }
                    (OrderSide::Sell, DiscretionDirection::Unfavorable) => {
                        // Must be at or above price + discretion
                        Some((price, price + *discretion_amount))
                    }
                }
            }
            _ => None,
        }
    }
}

/// Manager for advanced execution orders
pub struct AdvancedExecutionManager {
    /// Active advanced execution orders
    orders: Arc<RwLock<HashMap<Uuid, AdvancedExecutionOrder>>>,
    /// Pegged orders that need price updates
    pegged_orders: Arc<RwLock<HashMap<Uuid, Uuid>>>, // token_id -> order_id
}

impl AdvancedExecutionManager {
    /// Create a new advanced execution order manager
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            pegged_orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Submit an advanced execution order
    pub async fn submit(&self, order: AdvancedExecutionOrder) -> Result<Uuid, &'static str> {
        let order_id = order.id;

        // Track pegged orders separately
        if let AdvancedExecutionType::Pegged { token_id, .. } = &order.order_type {
            self.pegged_orders.write().await.insert(*token_id, order_id);
        }

        self.orders.write().await.insert(order_id, order);
        Ok(order_id)
    }

    /// Update pegged order prices based on current market
    pub async fn update_pegged_prices(
        &self,
        token_id: Uuid,
        best_bid: Decimal,
        best_ask: Decimal,
    ) -> Vec<(Uuid, Option<Decimal>)> {
        let mut updates = Vec::new();

        // Find all pegged orders for this token
        let orders = self.orders.read().await;
        for (order_id, order) in orders.iter() {
            if let AdvancedExecutionType::Pegged { token_id: tid, .. } = &order.order_type {
                if *tid == token_id {
                    let new_price = order.calculate_pegged_price(best_bid, best_ask);
                    updates.push((*order_id, new_price));
                }
            }
        }

        updates
    }

    /// Handle replenishment for hidden orders
    pub async fn replenish_hidden_order(&self, order_id: Uuid) -> Option<Decimal> {
        let mut orders = self.orders.write().await;
        if let Some(order) = orders.get_mut(&order_id) {
            if let AdvancedExecutionType::Hidden {
                visible_quantity,
                reserve_quantity,
                replenish_quantity,
                ..
            } = &mut order.order_type
            {
                if *reserve_quantity > Decimal::ZERO {
                    let to_replenish = (*replenish_quantity).min(*reserve_quantity);
                    *visible_quantity += to_replenish;
                    *reserve_quantity -= to_replenish;
                    return Some(to_replenish);
                }
            }
        }
        None
    }

    /// Cancel an advanced execution order
    pub async fn cancel(&self, order_id: Uuid) -> Option<AdvancedExecutionOrder> {
        if let Some(mut order) = self.orders.write().await.remove(&order_id) {
            order.update_status(AdvancedExecutionStatus::Cancelled);

            // Remove from pegged tracking if applicable
            if let AdvancedExecutionType::Pegged { token_id, .. } = &order.order_type {
                self.pegged_orders.write().await.remove(token_id);
            }

            Some(order)
        } else {
            None
        }
    }

    /// Get an order by ID
    pub async fn get(&self, id: Uuid) -> Option<AdvancedExecutionOrder> {
        self.orders.read().await.get(&id).cloned()
    }

    /// Get all orders for a user
    pub async fn get_by_user(&self, user_id: Uuid) -> Vec<AdvancedExecutionOrder> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.user_id == user_id)
            .cloned()
            .collect()
    }

    /// Update fill status
    pub async fn update_fill(&self, order_id: Uuid, filled_quantity: Decimal, is_complete: bool) {
        if let Some(order) = self.orders.write().await.get_mut(&order_id) {
            order.filled_quantity = filled_quantity;
            if is_complete {
                order.update_status(AdvancedExecutionStatus::Filled);
            } else {
                order.update_status(AdvancedExecutionStatus::PartiallyFilled);
            }
        }
    }
}

impl Default for AdvancedExecutionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_pegged_order_primary() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = AdvancedExecutionOrder::new_pegged(
            user_id,
            token_id,
            OrderSide::Buy,
            dec!(100),
            PegType::Primary { offset: dec!(0.01) },
            Some(dec!(100)),
        );
        assert!(order.is_ok());
    }

    #[test]
    fn test_pegged_price_calculation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = AdvancedExecutionOrder::new_pegged(
            user_id,
            token_id,
            OrderSide::Buy,
            dec!(100),
            PegType::Primary { offset: dec!(0.01) },
            None,
        )
        .unwrap();

        let best_bid = dec!(99.50);
        let best_ask = dec!(100.00);

        let pegged_price = order.calculate_pegged_price(best_bid, best_ask);
        assert_eq!(pegged_price, Some(dec!(99.51)));
    }

    #[test]
    fn test_hidden_order_creation() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let hidden = AdvancedExecutionOrder::new_hidden(
            user_id,
            order,
            dec!(20), // visible
            dec!(80), // reserve
            dec!(20), // replenish
        );
        assert!(hidden.is_ok());
    }

    #[test]
    fn test_hidden_order_invalid_quantities() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let hidden = AdvancedExecutionOrder::new_hidden(
            user_id,
            order,
            dec!(30), // visible
            dec!(80), // reserve (30 + 80 != 100)
            dec!(20),
        );
        assert!(hidden.is_err());
    }

    #[test]
    fn test_discretionary_order() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let discretionary = AdvancedExecutionOrder::new_discretionary(
            user_id,
            order,
            dec!(1.0),
            DiscretionDirection::Favorable,
        );
        assert!(discretionary.is_ok());
    }

    #[test]
    fn test_discretionary_price_range() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let discretionary = AdvancedExecutionOrder::new_discretionary(
            user_id,
            order,
            dec!(1.0),
            DiscretionDirection::Favorable,
        )
        .unwrap();

        let range = discretionary.get_discretionary_price_range();
        assert_eq!(range, Some((dec!(100), dec!(101))));
    }

    #[test]
    fn test_minimum_quantity_order() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let min_qty = AdvancedExecutionOrder::new_minimum_quantity(user_id, order, dec!(50));
        assert!(min_qty.is_ok());
    }

    #[test]
    fn test_fok_order() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let fok = AdvancedExecutionOrder::new_fok(user_id, order);
        assert!(fok.is_ok());
    }

    #[test]
    fn test_ioc_order() {
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let ioc = AdvancedExecutionOrder::new_ioc(user_id, order);
        assert!(ioc.is_ok());
    }

    #[tokio::test]
    async fn test_advanced_execution_manager() {
        let manager = AdvancedExecutionManager::new();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = AdvancedExecutionOrder::new_pegged(
            user_id,
            token_id,
            OrderSide::Buy,
            dec!(100),
            PegType::Primary { offset: dec!(0.01) },
            None,
        )
        .unwrap();

        let order_id = manager.submit(order).await.unwrap();

        // Update pegged prices
        let updates = manager
            .update_pegged_prices(token_id, dec!(99.50), dec!(100.00))
            .await;
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, order_id);
        assert_eq!(updates[0].1, Some(dec!(99.51)));
    }

    #[tokio::test]
    async fn test_hidden_order_replenishment() {
        let manager = AdvancedExecutionManager::new();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let order = create_test_order(token_id, OrderSide::Buy, dec!(100));
        let hidden =
            AdvancedExecutionOrder::new_hidden(user_id, order, dec!(20), dec!(80), dec!(20))
                .unwrap();

        let order_id = manager.submit(hidden).await.unwrap();

        // Replenish
        let replenished = manager.replenish_hidden_order(order_id).await;
        assert_eq!(replenished, Some(dec!(20)));
    }
}
