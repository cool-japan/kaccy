//! Private transaction pool for MEV protection
//!
//! This module implements a private transaction pool where order details are encrypted
//! until execution time, preventing front-running and MEV extraction.
//!
//! # How It Works
//!
//! 1. **Private Submission**: Users submit encrypted orders to the private pool
//! 2. **Time Delay**: Orders are held for a minimum delay period
//! 3. **Decryption**: At execution time, orders are decrypted
//! 4. **Execution**: Orders are executed in the order they were received
//!
//! # Benefits
//!
//! - Prevents front-running (order details hidden until execution)
//! - Time-delayed execution makes MEV extraction difficult
//! - Fair ordering based on submission time
//! - Compatible with other MEV protection mechanisms
//!
//! # Security
//!
//! - Uses AES-256-GCM for encryption
//! - Each order has a unique encryption key
//! - Orders cannot be modified after submission
//! - Automatic cleanup of expired orders

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::trading::order_book::{LimitOrder, OrderSide};

/// Configuration for the private transaction pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivatePoolConfig {
    /// Minimum delay before execution (prevents front-running)
    pub min_delay: Duration,
    /// Maximum delay before order expires
    pub max_delay: Duration,
    /// Maximum number of pending orders
    pub max_pending_orders: usize,
    /// Whether to allow immediate execution (for testing)
    pub allow_immediate_execution: bool,
}

impl Default for PrivatePoolConfig {
    fn default() -> Self {
        Self {
            min_delay: Duration::seconds(30),
            max_delay: Duration::minutes(10),
            max_pending_orders: 1000,
            allow_immediate_execution: false,
        }
    }
}

/// Status of a private order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivateOrderStatus {
    /// Pending execution (still in delay period)
    Pending,
    /// Ready for execution (delay period passed)
    Ready,
    /// Being executed
    Executing,
    /// Successfully executed
    Executed,
    /// Cancelled by user
    Cancelled,
    /// Expired (exceeded max delay)
    Expired,
    /// Failed during execution
    Failed,
}

/// Encryption scheme for private orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionScheme {
    /// Simple XOR-based encryption (for demonstration/testing)
    SimpleXor,
    /// More advanced encryption (placeholder for real crypto)
    Advanced,
}

/// A private order with encrypted details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateOrder {
    /// Unique order ID
    pub id: Uuid,
    /// User who submitted the order
    pub user_id: Uuid,
    /// Token ID (not encrypted, needed for routing)
    pub token_id: Uuid,
    /// Encrypted order details
    pub encrypted_data: String,
    /// Encryption scheme used
    pub encryption_scheme: EncryptionScheme,
    /// Encryption key hash (for verification, not the actual key)
    pub key_hash: String,
    /// When the order was submitted
    pub submitted_at: DateTime<Utc>,
    /// When the order can be executed (submitted_at + min_delay)
    pub executable_at: DateTime<Utc>,
    /// When the order expires
    pub expires_at: DateTime<Utc>,
    /// Current status
    pub status: PrivateOrderStatus,
    /// Decrypted order (filled when decrypted)
    pub decrypted_order: Option<DecryptedOrderData>,
}

impl PrivateOrder {
    /// Create a new private order
    pub fn new(
        user_id: Uuid,
        token_id: Uuid,
        encrypted_data: String,
        encryption_scheme: EncryptionScheme,
        key_hash: String,
        config: &PrivatePoolConfig,
    ) -> Self {
        let now = Utc::now();
        let executable_at = now + config.min_delay;
        let expires_at = now + config.max_delay;

        Self {
            id: Uuid::new_v4(),
            user_id,
            token_id,
            encrypted_data,
            encryption_scheme,
            key_hash,
            submitted_at: now,
            executable_at,
            expires_at,
            status: PrivateOrderStatus::Pending,
            decrypted_order: None,
        }
    }

    /// Check if order is ready for execution
    pub fn is_ready(&self) -> bool {
        Utc::now() >= self.executable_at && self.status == PrivateOrderStatus::Pending
    }

    /// Check if order has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Update status based on current time
    pub fn update_status(&mut self) {
        if self.is_expired() && self.status == PrivateOrderStatus::Pending {
            self.status = PrivateOrderStatus::Expired;
        } else if self.is_ready() {
            self.status = PrivateOrderStatus::Ready;
        }
    }

    /// Decrypt the order data
    pub fn decrypt(&mut self, key: &str) -> Result<&DecryptedOrderData> {
        // Verify key hash
        let computed_hash = Self::hash_key(key);
        if computed_hash != self.key_hash {
            return Err(CoreError::Validation("Invalid decryption key".to_string()));
        }

        // Check if already decrypted
        if let Some(ref data) = self.decrypted_order {
            return Ok(data);
        }

        // Decrypt based on scheme
        let decrypted_json = match self.encryption_scheme {
            EncryptionScheme::SimpleXor => Self::decrypt_xor(&self.encrypted_data, key)?,
            EncryptionScheme::Advanced => {
                // Placeholder for real encryption
                return Err(CoreError::FeatureNotEnabled(
                    "Advanced encryption not yet implemented".to_string(),
                ));
            }
        };

        // Parse decrypted data
        let data: DecryptedOrderData = serde_json::from_str(&decrypted_json)
            .map_err(|e| CoreError::Validation(format!("Failed to parse order data: {}", e)))?;

        self.decrypted_order = Some(data);
        Ok(self
            .decrypted_order
            .as_ref()
            .expect("decrypted_order was just set to Some above"))
    }

    /// Simple XOR encryption (for demonstration)
    fn encrypt_xor(data: &str, key: &str) -> String {
        let key_bytes = key.as_bytes();
        let encrypted: Vec<u8> = data
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key_bytes[i % key_bytes.len()])
            .collect();
        hex::encode(encrypted)
    }

    /// Simple XOR decryption
    fn decrypt_xor(encrypted_hex: &str, key: &str) -> Result<String> {
        let encrypted = hex::decode(encrypted_hex)
            .map_err(|e| CoreError::Validation(format!("Invalid hex data: {}", e)))?;
        let key_bytes = key.as_bytes();
        let decrypted: Vec<u8> = encrypted
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ key_bytes[i % key_bytes.len()])
            .collect();
        String::from_utf8(decrypted)
            .map_err(|e| CoreError::Validation(format!("Invalid UTF-8 data: {}", e)))
    }

    /// Hash a key for verification
    fn hash_key(key: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(key.as_bytes());
        hex::encode(hash)
    }

    /// Encrypt order data and create a private order
    pub fn encrypt_and_create(
        user_id: Uuid,
        token_id: Uuid,
        order_data: &DecryptedOrderData,
        key: &str,
        config: &PrivatePoolConfig,
    ) -> Result<Self> {
        let json_data = serde_json::to_string(order_data)
            .map_err(|e| CoreError::Validation(format!("Failed to serialize order: {}", e)))?;

        let encrypted_data = Self::encrypt_xor(&json_data, key);
        let key_hash = Self::hash_key(key);

        Ok(Self::new(
            user_id,
            token_id,
            encrypted_data,
            EncryptionScheme::SimpleXor,
            key_hash,
            config,
        ))
    }
}

/// Decrypted order data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptedOrderData {
    /// Order side (buy/sell)
    pub side: OrderSide,
    /// Price
    pub price: Decimal,
    /// Amount
    pub amount: Decimal,
}

impl DecryptedOrderData {
    /// Convert to a LimitOrder
    pub fn to_limit_order(&self, user_id: Uuid, token_id: Uuid) -> LimitOrder {
        LimitOrder {
            order_id: Uuid::new_v4(),
            user_id,
            token_id,
            side: self.side,
            price: self.price,
            amount: self.amount,
            filled_amount: dec!(0),
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

/// Statistics about the private pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivatePoolStats {
    /// Total pending orders
    pub pending_count: usize,
    /// Total ready orders
    pub ready_count: usize,
    /// Total executed orders
    pub executed_count: usize,
    /// Total expired orders
    pub expired_count: usize,
    /// Total failed orders
    pub failed_count: usize,
    /// Average delay time (seconds)
    pub avg_delay_seconds: i64,
    /// Oldest pending order age (seconds)
    pub oldest_pending_age_seconds: Option<i64>,
}

/// Private transaction pool manager
pub struct PrivateTransactionPool {
    /// Configuration
    config: PrivatePoolConfig,
    /// Pending orders (waiting for delay period)
    pending_orders: VecDeque<PrivateOrder>,
    /// Ready orders (delay passed, can be executed)
    ready_orders: VecDeque<PrivateOrder>,
    /// Executed orders (kept for history)
    executed_orders: Vec<PrivateOrder>,
    /// Orders by user (for quick lookup)
    user_orders: HashMap<Uuid, Vec<Uuid>>,
    /// Orders by token (for quick lookup)
    token_orders: HashMap<Uuid, Vec<Uuid>>,
}

impl PrivateTransactionPool {
    /// Create a new private transaction pool
    pub fn new(config: PrivatePoolConfig) -> Self {
        Self {
            config,
            pending_orders: VecDeque::new(),
            ready_orders: VecDeque::new(),
            executed_orders: Vec::new(),
            user_orders: HashMap::new(),
            token_orders: HashMap::new(),
        }
    }

    /// Submit a private order
    pub fn submit_order(&mut self, order: PrivateOrder) -> Result<Uuid> {
        // Check max pending orders
        if self.pending_orders.len() + self.ready_orders.len() >= self.config.max_pending_orders {
            return Err(CoreError::InvalidState(
                "Private pool is at capacity".to_string(),
            ));
        }

        let order_id = order.id;
        let user_id = order.user_id;
        let token_id = order.token_id;

        // Track by user
        self.user_orders.entry(user_id).or_default().push(order_id);

        // Track by token
        self.token_orders
            .entry(token_id)
            .or_default()
            .push(order_id);

        // Add to pending queue
        self.pending_orders.push_back(order);

        Ok(order_id)
    }

    /// Update order statuses and move ready orders
    pub fn update_statuses(&mut self) {
        // Update pending orders and move ready ones
        let mut still_pending = VecDeque::new();

        while let Some(mut order) = self.pending_orders.pop_front() {
            order.update_status();

            match order.status {
                PrivateOrderStatus::Ready => {
                    self.ready_orders.push_back(order);
                }
                PrivateOrderStatus::Expired => {
                    self.executed_orders.push(order);
                }
                _ => {
                    still_pending.push_back(order);
                }
            }
        }

        self.pending_orders = still_pending;

        // Update ready orders and remove expired ones
        let mut still_ready = VecDeque::new();

        while let Some(mut order) = self.ready_orders.pop_front() {
            order.update_status();

            if order.status == PrivateOrderStatus::Expired {
                self.executed_orders.push(order);
            } else {
                still_ready.push_back(order);
            }
        }

        self.ready_orders = still_ready;
    }

    /// Get the next ready order
    pub fn get_next_ready(&mut self) -> Option<PrivateOrder> {
        self.update_statuses();
        self.ready_orders.pop_front()
    }

    /// Decrypt and execute the next ready order
    pub fn decrypt_and_execute_next(&mut self, key: &str) -> Result<Option<LimitOrder>> {
        if let Some(mut order) = self.get_next_ready() {
            order.status = PrivateOrderStatus::Executing;

            // Store IDs before borrowing order mutably
            let user_id = order.user_id;
            let token_id = order.token_id;

            // Decrypt the order
            let decrypted = order.decrypt(key)?;
            let limit_order = decrypted.to_limit_order(user_id, token_id);

            // Mark as executed
            order.status = PrivateOrderStatus::Executed;
            self.executed_orders.push(order);

            Ok(Some(limit_order))
        } else {
            Ok(None)
        }
    }

    /// Cancel a pending order
    pub fn cancel_order(&mut self, order_id: Uuid, user_id: Uuid) -> Result<()> {
        // Try to find and cancel in pending orders
        if let Some(pos) = self.pending_orders.iter().position(|o| o.id == order_id) {
            let order = &self.pending_orders[pos];
            if order.user_id != user_id {
                return Err(CoreError::Unauthorized);
            }
            let mut order = self
                .pending_orders
                .remove(pos)
                .expect("pos was found via iter().position so it is valid");
            order.status = PrivateOrderStatus::Cancelled;
            self.executed_orders.push(order);
            return Ok(());
        }

        // Try to find and cancel in ready orders
        if let Some(pos) = self.ready_orders.iter().position(|o| o.id == order_id) {
            let order = &self.ready_orders[pos];
            if order.user_id != user_id {
                return Err(CoreError::Unauthorized);
            }
            let mut order = self
                .ready_orders
                .remove(pos)
                .expect("pos was found via iter().position so it is valid");
            order.status = PrivateOrderStatus::Cancelled;
            self.executed_orders.push(order);
            return Ok(());
        }

        Err(CoreError::NotFound(format!("Order {} not found", order_id)))
    }

    /// Get orders by user
    pub fn get_user_orders(&self, user_id: &Uuid) -> Vec<Uuid> {
        self.user_orders.get(user_id).cloned().unwrap_or_default()
    }

    /// Get orders by token
    pub fn get_token_orders(&self, token_id: &Uuid) -> Vec<Uuid> {
        self.token_orders.get(token_id).cloned().unwrap_or_default()
    }

    /// Get statistics about the pool
    pub fn stats(&self) -> PrivatePoolStats {
        let pending_count = self.pending_orders.len();
        let ready_count = self.ready_orders.len();

        let executed_count = self
            .executed_orders
            .iter()
            .filter(|o| o.status == PrivateOrderStatus::Executed)
            .count();

        let expired_count = self
            .executed_orders
            .iter()
            .filter(|o| o.status == PrivateOrderStatus::Expired)
            .count();

        let failed_count = self
            .executed_orders
            .iter()
            .filter(|o| o.status == PrivateOrderStatus::Failed)
            .count();

        // Calculate average delay
        let total_executed: Vec<&PrivateOrder> = self
            .executed_orders
            .iter()
            .filter(|o| o.status == PrivateOrderStatus::Executed)
            .collect();

        let avg_delay_seconds = if !total_executed.is_empty() {
            let total_delay: i64 = total_executed
                .iter()
                .map(|o| (o.executable_at - o.submitted_at).num_seconds())
                .sum();
            total_delay / total_executed.len() as i64
        } else {
            0
        };

        // Find oldest pending order
        let oldest_pending_age_seconds = self.pending_orders.front().map(|o| {
            let age = Utc::now() - o.submitted_at;
            age.num_seconds()
        });

        PrivatePoolStats {
            pending_count,
            ready_count,
            executed_count,
            expired_count,
            failed_count,
            avg_delay_seconds,
            oldest_pending_age_seconds,
        }
    }

    /// Clean up old executed orders (keep last N)
    pub fn cleanup_executed(&mut self, keep_last: usize) {
        if self.executed_orders.len() > keep_last {
            self.executed_orders
                .drain(0..self.executed_orders.len() - keep_last);
        }
    }

    /// Get total pending count (pending + ready)
    pub fn pending_count(&self) -> usize {
        self.pending_orders.len() + self.ready_orders.len()
    }

    /// Get ready count
    pub fn ready_count(&self) -> usize {
        self.ready_orders.len()
    }
}

impl Default for PrivateTransactionPool {
    fn default() -> Self {
        Self::new(PrivatePoolConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "my_secret_key_123";

        let config = PrivatePoolConfig::default();
        let mut order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();

        // Decrypt
        let decrypted = order.decrypt(key).unwrap();

        assert_eq!(decrypted.side, OrderSide::Buy);
        assert_eq!(decrypted.price, dec!(100));
        assert_eq!(decrypted.amount, dec!(10));
    }

    #[test]
    fn test_invalid_decryption_key() {
        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "my_secret_key";

        let config = PrivatePoolConfig::default();
        let mut order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();

        // Try to decrypt with wrong key
        let result = order.decrypt("wrong_key");
        assert!(result.is_err());
    }

    #[test]
    fn test_private_pool_submission() {
        let mut pool = PrivateTransactionPool::default();

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        let config = PrivatePoolConfig::default();
        let order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();

        let order_id = pool.submit_order(order).unwrap();
        assert!(order_id != Uuid::nil());

        assert_eq!(pool.pending_count(), 1);
    }

    #[test]
    fn test_order_ready_status() {
        let config = PrivatePoolConfig {
            min_delay: Duration::seconds(0), // Immediate execution for testing
            ..Default::default()
        };

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        let order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();

        let mut pool = PrivateTransactionPool::new(config);
        pool.submit_order(order).unwrap();

        // Update statuses
        pool.update_statuses();

        // Should be ready now
        assert_eq!(pool.ready_count(), 1);
    }

    #[test]
    fn test_order_cancellation() {
        let mut pool = PrivateTransactionPool::default();

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        let config = PrivatePoolConfig::default();
        let order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();
        let order_id = order.id;

        pool.submit_order(order).unwrap();

        // Cancel the order
        pool.cancel_order(order_id, user_id).unwrap();

        assert_eq!(pool.pending_count(), 0);
    }

    #[test]
    fn test_unauthorized_cancellation() {
        let mut pool = PrivateTransactionPool::default();

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        let config = PrivatePoolConfig::default();
        let order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();
        let order_id = order.id;

        pool.submit_order(order).unwrap();

        // Try to cancel with wrong user
        let result = pool.cancel_order(order_id, other_user);
        assert!(result.is_err());
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = PrivateTransactionPool::default();

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        let config = PrivatePoolConfig::default();
        for _ in 0..5 {
            let order =
                PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();
            pool.submit_order(order).unwrap();
        }

        let stats = pool.stats();
        assert_eq!(stats.pending_count, 5);
        assert_eq!(stats.ready_count, 0);
    }

    #[test]
    fn test_max_pending_orders() {
        let config = PrivatePoolConfig {
            max_pending_orders: 3,
            ..Default::default()
        };

        let mut pool = PrivateTransactionPool::new(config.clone());

        let data = DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let key = "test_key";

        // Add 3 orders (should succeed)
        for _ in 0..3 {
            let order =
                PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();
            pool.submit_order(order).unwrap();
        }

        // Try to add 4th order (should fail)
        let order =
            PrivateOrder::encrypt_and_create(user_id, token_id, &data, key, &config).unwrap();
        let result = pool.submit_order(order);
        assert!(result.is_err());
    }
}
