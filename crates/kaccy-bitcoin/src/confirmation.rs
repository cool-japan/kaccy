//! Confirmation tracking for Bitcoin transactions

use bitcoin::Amount;
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// Confirmation levels for payment processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ConfirmationLevel {
    /// Unconfirmed (in mempool)
    Unconfirmed,
    /// 1 confirmation - first block
    OneConfirmation,
    /// 3 confirmations - reasonably secure
    ThreeConfirmations,
    /// 6 confirmations - highly secure
    SixConfirmations,
}

impl ConfirmationLevel {
    /// Create from confirmation count
    pub fn from_count(confirmations: u32) -> Self {
        match confirmations {
            0 => Self::Unconfirmed,
            1..=2 => Self::OneConfirmation,
            3..=5 => Self::ThreeConfirmations,
            _ => Self::SixConfirmations,
        }
    }

    /// Get the minimum confirmations for this level
    pub fn min_confirmations(&self) -> u32 {
        match self {
            Self::Unconfirmed => 0,
            Self::OneConfirmation => 1,
            Self::ThreeConfirmations => 3,
            Self::SixConfirmations => 6,
        }
    }

    /// Check if this level meets a required threshold
    pub fn meets_threshold(&self, required: ConfirmationLevel) -> bool {
        *self >= required
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::Unconfirmed => "Unconfirmed (waiting for first block)",
            Self::OneConfirmation => "1 confirmation (initial confirmation)",
            Self::ThreeConfirmations => "3+ confirmations (reasonably secure)",
            Self::SixConfirmations => "6+ confirmations (highly secure)",
        }
    }
}

/// Tracked order with confirmation status
#[derive(Debug, Clone)]
pub struct TrackedOrder {
    /// Order ID
    pub order_id: Uuid,
    /// Payment address
    pub address: String,
    /// Expected amount
    pub expected_amount: Amount,
    /// Transaction ID (if known)
    pub txid: Option<String>,
    /// Current confirmation count
    pub confirmations: u32,
    /// Current confirmation level
    pub level: ConfirmationLevel,
    /// Previous level (for change detection)
    pub previous_level: ConfirmationLevel,
    /// Received amount
    pub received_amount: Option<Amount>,
}

impl TrackedOrder {
    /// Create a new tracked order
    pub fn new(order_id: Uuid, address: String, expected_amount: Amount) -> Self {
        Self {
            order_id,
            address,
            expected_amount,
            txid: None,
            confirmations: 0,
            level: ConfirmationLevel::Unconfirmed,
            previous_level: ConfirmationLevel::Unconfirmed,
            received_amount: None,
        }
    }

    /// Update confirmation count and detect level changes
    pub fn update_confirmations(&mut self, confirmations: u32) -> Option<ConfirmationEvent> {
        self.confirmations = confirmations;
        let new_level = ConfirmationLevel::from_count(confirmations);

        if new_level != self.level {
            let event = ConfirmationEvent {
                order_id: self.order_id,
                previous_level: self.level,
                new_level,
                confirmations,
                txid: self.txid.clone(),
            };
            self.previous_level = self.level;
            self.level = new_level;
            Some(event)
        } else {
            None
        }
    }

    /// Check if payment has reached the required confirmation level
    pub fn is_confirmed(&self, required: ConfirmationLevel) -> bool {
        self.level.meets_threshold(required)
    }
}

/// Event emitted when confirmation level changes
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmationEvent {
    /// Order ID
    pub order_id: Uuid,
    /// Previous confirmation level
    pub previous_level: ConfirmationLevel,
    /// New confirmation level
    pub new_level: ConfirmationLevel,
    /// Current confirmation count
    pub confirmations: u32,
    /// Transaction ID
    pub txid: Option<String>,
}

impl ConfirmationEvent {
    /// Get a notification message for this event
    pub fn notification_message(&self) -> String {
        match self.new_level {
            ConfirmationLevel::Unconfirmed => {
                format!("Payment detected in mempool for order {}", self.order_id)
            }
            ConfirmationLevel::OneConfirmation => {
                format!(
                    "Payment received first confirmation for order {} (tx: {})",
                    self.order_id,
                    self.txid.as_deref().unwrap_or("unknown")
                )
            }
            ConfirmationLevel::ThreeConfirmations => {
                format!(
                    "Payment reached 3 confirmations for order {} - reasonably secure",
                    self.order_id
                )
            }
            ConfirmationLevel::SixConfirmations => {
                format!(
                    "Payment fully confirmed for order {} - 6+ confirmations",
                    self.order_id
                )
            }
        }
    }

    /// Check if this is a significant event worth notifying
    pub fn is_significant(&self) -> bool {
        // All level changes are significant
        true
    }
}

/// Notification types for confirmation events
#[derive(Debug, Clone, Serialize)]
pub enum NotificationType {
    /// Payment detected in mempool
    PaymentDetected,
    /// First confirmation received
    FirstConfirmation,
    /// Reached reasonably secure level (3 confirmations)
    ReasonablySecure,
    /// Fully confirmed (6+ confirmations)
    FullyConfirmed,
}

impl From<ConfirmationLevel> for NotificationType {
    fn from(level: ConfirmationLevel) -> Self {
        match level {
            ConfirmationLevel::Unconfirmed => Self::PaymentDetected,
            ConfirmationLevel::OneConfirmation => Self::FirstConfirmation,
            ConfirmationLevel::ThreeConfirmations => Self::ReasonablySecure,
            ConfirmationLevel::SixConfirmations => Self::FullyConfirmed,
        }
    }
}

/// Confirmation tracker that monitors multiple orders
pub struct ConfirmationTracker {
    /// Tracked orders by order ID
    orders: HashMap<Uuid, TrackedOrder>,
    /// Required confirmation level for "confirmed" status
    required_level: ConfirmationLevel,
}

impl ConfirmationTracker {
    /// Create a new confirmation tracker
    pub fn new(required_level: ConfirmationLevel) -> Self {
        Self {
            orders: HashMap::new(),
            required_level,
        }
    }

    /// Create with default settings (3 confirmations required)
    pub fn with_defaults() -> Self {
        Self::new(ConfirmationLevel::ThreeConfirmations)
    }

    /// Start tracking an order
    pub fn track(&mut self, order_id: Uuid, address: String, expected_amount: Amount) {
        let order = TrackedOrder::new(order_id, address, expected_amount);
        self.orders.insert(order_id, order);
    }

    /// Stop tracking an order
    pub fn untrack(&mut self, order_id: Uuid) -> Option<TrackedOrder> {
        self.orders.remove(&order_id)
    }

    /// Update confirmation count for an order
    pub fn update(
        &mut self,
        order_id: Uuid,
        confirmations: u32,
        txid: Option<String>,
        received_amount: Option<Amount>,
    ) -> Option<ConfirmationEvent> {
        if let Some(order) = self.orders.get_mut(&order_id) {
            if let Some(txid) = txid {
                order.txid = Some(txid);
            }
            if let Some(amount) = received_amount {
                order.received_amount = Some(amount);
            }
            order.update_confirmations(confirmations)
        } else {
            None
        }
    }

    /// Get order status
    pub fn get(&self, order_id: Uuid) -> Option<&TrackedOrder> {
        self.orders.get(&order_id)
    }

    /// Check if an order is confirmed
    pub fn is_confirmed(&self, order_id: Uuid) -> bool {
        self.orders
            .get(&order_id)
            .map(|o| o.is_confirmed(self.required_level))
            .unwrap_or(false)
    }

    /// Get all orders at a specific confirmation level
    pub fn orders_at_level(&self, level: ConfirmationLevel) -> Vec<&TrackedOrder> {
        self.orders.values().filter(|o| o.level == level).collect()
    }

    /// Get all confirmed orders
    pub fn confirmed_orders(&self) -> Vec<&TrackedOrder> {
        self.orders
            .values()
            .filter(|o| o.is_confirmed(self.required_level))
            .collect()
    }

    /// Get all pending orders (not yet confirmed)
    pub fn pending_orders(&self) -> Vec<&TrackedOrder> {
        self.orders
            .values()
            .filter(|o| !o.is_confirmed(self.required_level))
            .collect()
    }

    /// Get statistics
    pub fn stats(&self) -> ConfirmationStats {
        let mut stats = ConfirmationStats::default();
        for order in self.orders.values() {
            match order.level {
                ConfirmationLevel::Unconfirmed => stats.unconfirmed += 1,
                ConfirmationLevel::OneConfirmation => stats.one_confirmation += 1,
                ConfirmationLevel::ThreeConfirmations => stats.three_confirmations += 1,
                ConfirmationLevel::SixConfirmations => stats.six_confirmations += 1,
            }
        }
        stats.total = self.orders.len();
        stats
    }
}

/// Statistics about tracked confirmations
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConfirmationStats {
    /// Total tracked orders
    pub total: usize,
    /// Orders with 0 confirmations
    pub unconfirmed: usize,
    /// Orders with 1-2 confirmations
    pub one_confirmation: usize,
    /// Orders with 3-5 confirmations
    pub three_confirmations: usize,
    /// Orders with 6+ confirmations
    pub six_confirmations: usize,
}

impl Default for ConfirmationTracker {
    fn default() -> Self {
        Self::with_defaults()
    }
}
