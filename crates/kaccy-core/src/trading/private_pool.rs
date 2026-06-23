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
//! - Uses real authenticated encryption (AEAD): ChaCha20-Poly1305 or AES-256-GCM
//! - A fresh random 96-bit nonce is generated per encryption and stored with the ciphertext
//! - The Poly1305 / GCM authentication tag is verified on decryption, so any tampering with
//!   the ciphertext (or use of a wrong key) fails loudly instead of returning garbage
//! - The 256-bit cipher key is derived from the caller-supplied secret via SHA-256
//! - Orders cannot be modified after submission
//! - Automatic cleanup of expired orders

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use chacha20poly1305::ChaCha20Poly1305;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::trading::order_book::{LimitOrder, OrderSide};

/// Length in bytes of the AEAD nonce used by both supported schemes (96 bits).
const AEAD_NONCE_LEN: usize = 12;

/// Length in bytes of the symmetric AEAD key derived from the caller secret (256 bits).
const AEAD_KEY_LEN: usize = 32;

/// Domain-separation label mixed into the AEAD key derivation.
///
/// The public [`PrivateOrder::key_hash`] field stores `SHA-256(secret)` for cheap key
/// verification. If the AEAD key were *also* `SHA-256(secret)`, that public field would equal
/// the encryption key and leak it. Prefixing the key-derivation input with this label makes
/// the derived key `SHA-256(label || secret)`, which (by SHA-256 preimage/second-preimage
/// resistance) cannot be recovered from the published `SHA-256(secret)`.
const AEAD_KEY_DERIVATION_LABEL: &[u8] = b"kaccy-private-pool/aead-key/v1";

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

/// Authenticated encryption scheme used to protect private orders.
///
/// Both variants are real, authenticated (AEAD) ciphers that provide
/// confidentiality *and* integrity: decryption verifies an authentication tag and
/// fails on any tampering or wrong key. There is no insecure / placeholder option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionScheme {
    /// ChaCha20-Poly1305 AEAD (RFC 8439) with a 256-bit key and a random 96-bit nonce.
    ChaCha20Poly1305,
    /// AES-256-GCM AEAD with a 256-bit key and a random 96-bit nonce.
    Aes256Gcm,
}

impl EncryptionScheme {
    /// Default scheme used when a caller does not specify one.
    pub const DEFAULT: Self = Self::ChaCha20Poly1305;
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

        // Decrypt with the real AEAD scheme. The authentication tag is verified here, so a
        // tampered ciphertext or a wrong key produces a hard error rather than garbage bytes.
        let decrypted_json = Self::decrypt_aead(self.encryption_scheme, &self.encrypted_data, key)?;

        // Parse decrypted data
        let data: DecryptedOrderData = serde_json::from_str(&decrypted_json)
            .map_err(|e| CoreError::Validation(format!("Failed to parse order data: {e}")))?;

        self.decrypted_order = Some(data);
        Ok(self
            .decrypted_order
            .as_ref()
            .expect("decrypted_order was just set to Some above"))
    }

    /// Derive a 256-bit AEAD key from the caller-supplied secret.
    ///
    /// The public API accepts an arbitrary-length secret (`&str`), but the AEAD ciphers
    /// require an exactly 32-byte key. We deterministically derive one as
    /// `SHA-256(AEAD_KEY_DERIVATION_LABEL || secret)`. The domain-separation label ensures the
    /// derived key differs from the publicly-stored `SHA-256(secret)` verification hash, so the
    /// stored `key_hash` cannot be used to recover the encryption key.
    fn derive_key(secret: &str) -> [u8; AEAD_KEY_LEN] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(AEAD_KEY_DERIVATION_LABEL);
        hasher.update(secret.as_bytes());
        let digest = hasher.finalize();
        let mut key = [0u8; AEAD_KEY_LEN];
        key.copy_from_slice(&digest);
        key
    }

    /// Generate a fresh random 96-bit nonce from the operating-system CSPRNG.
    fn random_nonce() -> [u8; AEAD_NONCE_LEN] {
        let mut nonce = [0u8; AEAD_NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    /// Encrypt `plaintext` with the requested AEAD scheme.
    ///
    /// A fresh random nonce is generated and prepended to the ciphertext (which already
    /// carries the authentication tag); the whole `nonce || ciphertext` blob is hex-encoded
    /// so it can be stored in the `encrypted_data` string field.
    fn encrypt_aead(scheme: EncryptionScheme, plaintext: &str, secret: &str) -> Result<String> {
        let key = Self::derive_key(secret);
        let nonce_bytes = Self::random_nonce();
        let plaintext_bytes = plaintext.as_bytes();

        let ciphertext = match scheme {
            EncryptionScheme::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| {
                    CoreError::Validation(format!("Failed to initialize ChaCha20-Poly1305: {e}"))
                })?;
                cipher
                    .encrypt(nonce_bytes.as_slice().into(), plaintext_bytes)
                    .map_err(|_| {
                        CoreError::Validation("ChaCha20-Poly1305 encryption failed".to_string())
                    })?
            }
            EncryptionScheme::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| {
                    CoreError::Validation(format!("Failed to initialize AES-256-GCM: {e}"))
                })?;
                cipher
                    .encrypt(nonce_bytes.as_slice().into(), plaintext_bytes)
                    .map_err(|_| {
                        CoreError::Validation("AES-256-GCM encryption failed".to_string())
                    })?
            }
        };

        let mut blob = Vec::with_capacity(AEAD_NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(hex::encode(blob))
    }

    /// Decrypt a hex-encoded `nonce || ciphertext` blob produced by [`Self::encrypt_aead`].
    ///
    /// The AEAD authentication tag is verified during decryption: a tampered ciphertext or a
    /// wrong key yields a [`CoreError::Validation`] error instead of corrupted plaintext.
    fn decrypt_aead(
        scheme: EncryptionScheme,
        encrypted_hex: &str,
        secret: &str,
    ) -> Result<String> {
        let blob = hex::decode(encrypted_hex)
            .map_err(|e| CoreError::Validation(format!("Invalid hex data: {e}")))?;
        if blob.len() <= AEAD_NONCE_LEN {
            return Err(CoreError::Validation(
                "Ciphertext too short to contain nonce and authentication tag".to_string(),
            ));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(AEAD_NONCE_LEN);

        let key = Self::derive_key(secret);
        let plaintext = match scheme {
            EncryptionScheme::ChaCha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| {
                    CoreError::Validation(format!("Failed to initialize ChaCha20-Poly1305: {e}"))
                })?;
                cipher
                    .decrypt(nonce_bytes.into(), ciphertext)
                    .map_err(|_| {
                        CoreError::Validation(
                            "ChaCha20-Poly1305 authentication failed (tampered data or wrong key)"
                                .to_string(),
                        )
                    })?
            }
            EncryptionScheme::Aes256Gcm => {
                let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| {
                    CoreError::Validation(format!("Failed to initialize AES-256-GCM: {e}"))
                })?;
                cipher.decrypt(nonce_bytes.into(), ciphertext).map_err(|_| {
                    CoreError::Validation(
                        "AES-256-GCM authentication failed (tampered data or wrong key)"
                            .to_string(),
                    )
                })?
            }
        };

        String::from_utf8(plaintext)
            .map_err(|e| CoreError::Validation(format!("Invalid UTF-8 data: {e}")))
    }

    /// Hash a key for verification
    fn hash_key(key: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(key.as_bytes());
        hex::encode(hash)
    }

    /// Encrypt order data and create a private order using the default AEAD scheme
    /// ([`EncryptionScheme::DEFAULT`], ChaCha20-Poly1305).
    pub fn encrypt_and_create(
        user_id: Uuid,
        token_id: Uuid,
        order_data: &DecryptedOrderData,
        key: &str,
        config: &PrivatePoolConfig,
    ) -> Result<Self> {
        Self::encrypt_and_create_with_scheme(
            user_id,
            token_id,
            order_data,
            key,
            EncryptionScheme::DEFAULT,
            config,
        )
    }

    /// Encrypt order data and create a private order using the requested AEAD scheme.
    pub fn encrypt_and_create_with_scheme(
        user_id: Uuid,
        token_id: Uuid,
        order_data: &DecryptedOrderData,
        key: &str,
        scheme: EncryptionScheme,
        config: &PrivatePoolConfig,
    ) -> Result<Self> {
        let json_data = serde_json::to_string(order_data)
            .map_err(|e| CoreError::Validation(format!("Failed to serialize order: {e}")))?;

        let encrypted_data = Self::encrypt_aead(scheme, &json_data, key)?;
        let key_hash = Self::hash_key(key);

        Ok(Self::new(
            user_id, token_id, encrypted_data, scheme, key_hash, config,
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

    /// Both real AEAD schemes are exercised by the round-trip / tamper / wrong-key tests.
    const ALL_SCHEMES: [EncryptionScheme; 2] = [
        EncryptionScheme::ChaCha20Poly1305,
        EncryptionScheme::Aes256Gcm,
    ];

    fn sample_order() -> DecryptedOrderData {
        DecryptedOrderData {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        }
    }

    #[test]
    fn test_encryption_decryption() {
        // Round-trip encrypt -> decrypt must recover the exact order for every real scheme.
        for scheme in ALL_SCHEMES {
            let data = sample_order();
            let user_id = Uuid::new_v4();
            let token_id = Uuid::new_v4();
            let key = "my_secret_key_123";

            let config = PrivatePoolConfig::default();
            let mut order = PrivateOrder::encrypt_and_create_with_scheme(
                user_id, token_id, &data, key, scheme, &config,
            )
            .expect("encryption should succeed");

            assert_eq!(order.encryption_scheme, scheme);

            let decrypted = order.decrypt(key).expect("decryption should succeed");
            assert_eq!(decrypted.side, OrderSide::Buy, "scheme {scheme:?}");
            assert_eq!(decrypted.price, dec!(100), "scheme {scheme:?}");
            assert_eq!(decrypted.amount, dec!(10), "scheme {scheme:?}");
        }
    }

    #[test]
    fn test_default_scheme_is_real_aead() {
        // The convenience constructor must pick a genuine AEAD scheme, never a placeholder.
        let data = sample_order();
        let config = PrivatePoolConfig::default();
        let order = PrivateOrder::encrypt_and_create(
            Uuid::new_v4(),
            Uuid::new_v4(),
            &data,
            "secret",
            &config,
        )
        .expect("encryption should succeed");
        assert_eq!(order.encryption_scheme, EncryptionScheme::DEFAULT);
        assert_eq!(order.encryption_scheme, EncryptionScheme::ChaCha20Poly1305);
    }

    #[test]
    fn test_ciphertext_is_not_plaintext() {
        // Sanity check that the stored blob is real ciphertext, not a reversible XOR of the
        // serialized order, and that it carries a fresh nonce plus a 16-byte auth tag.
        for scheme in ALL_SCHEMES {
            let data = sample_order();
            let config = PrivatePoolConfig::default();
            let order = PrivateOrder::encrypt_and_create_with_scheme(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &data,
                "secret",
                scheme,
                &config,
            )
            .expect("encryption should succeed");

            let json = serde_json::to_string(&data).expect("serialize");
            let blob = hex::decode(&order.encrypted_data).expect("ciphertext must be valid hex");
            // nonce (12) + ciphertext (== plaintext len for a stream/CTR AEAD) + tag (16).
            assert_eq!(
                blob.len(),
                AEAD_NONCE_LEN + json.len() + 16,
                "unexpected blob layout for {scheme:?}"
            );
            // The plaintext JSON must not appear verbatim inside the ciphertext blob.
            assert!(
                !order.encrypted_data.contains(&hex::encode(json.as_bytes())),
                "plaintext leaked into ciphertext for {scheme:?}"
            );
        }
    }

    #[test]
    fn test_nonce_is_randomized_per_encryption() {
        // Two encryptions of identical plaintext under the same key must differ (random nonce),
        // which is what stops an attacker from correlating equal orders.
        for scheme in ALL_SCHEMES {
            let data = sample_order();
            let config = PrivatePoolConfig::default();
            let key = "same_key";
            let first = PrivateOrder::encrypt_and_create_with_scheme(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &data,
                key,
                scheme,
                &config,
            )
            .expect("encryption should succeed");
            let second = PrivateOrder::encrypt_and_create_with_scheme(
                Uuid::new_v4(),
                Uuid::new_v4(),
                &data,
                key,
                scheme,
                &config,
            )
            .expect("encryption should succeed");
            assert_ne!(
                first.encrypted_data, second.encrypted_data,
                "ciphertext must vary per encryption for {scheme:?}"
            );
        }
    }

    #[test]
    fn test_invalid_decryption_key() {
        // A wrong key must fail. The SHA-256 key-hash gate catches it first; even bypassing
        // that gate (see `test_wrong_key_fails_aead`) the AEAD tag verification rejects it.
        for scheme in ALL_SCHEMES {
            let data = sample_order();
            let user_id = Uuid::new_v4();
            let token_id = Uuid::new_v4();
            let key = "my_secret_key";

            let config = PrivatePoolConfig::default();
            let mut order = PrivateOrder::encrypt_and_create_with_scheme(
                user_id, token_id, &data, key, scheme, &config,
            )
            .expect("encryption should succeed");

            let result = order.decrypt("wrong_key");
            assert!(result.is_err(), "wrong key must fail for {scheme:?}");
        }
    }

    #[test]
    fn test_wrong_key_fails_aead() {
        // Directly exercise the AEAD layer with a wrong key (bypassing the key-hash check) to
        // prove the authentication tag itself rejects decryption under the wrong key.
        for scheme in ALL_SCHEMES {
            let plaintext = "{\"side\":\"Buy\",\"price\":\"100\",\"amount\":\"10\"}";
            let blob = PrivateOrder::encrypt_aead(scheme, plaintext, "correct_secret")
                .expect("encryption should succeed");
            let result = PrivateOrder::decrypt_aead(scheme, &blob, "incorrect_secret");
            assert!(result.is_err(), "AEAD must reject wrong key for {scheme:?}");
        }
    }

    #[test]
    fn test_tamper_detection() {
        // Flipping a single bit of the ciphertext (here, the last tag byte) must be detected
        // by AEAD authentication and cause decryption to fail.
        for scheme in ALL_SCHEMES {
            let data = sample_order();
            let user_id = Uuid::new_v4();
            let token_id = Uuid::new_v4();
            let key = "tamper_test_key";

            let config = PrivatePoolConfig::default();
            let mut order = PrivateOrder::encrypt_and_create_with_scheme(
                user_id, token_id, &data, key, scheme, &config,
            )
            .expect("encryption should succeed");

            // Corrupt the last byte of the hex-encoded blob (part of the auth tag).
            let mut blob =
                hex::decode(&order.encrypted_data).expect("ciphertext must be valid hex");
            let last = blob.len() - 1;
            blob[last] ^= 0x01;
            order.encrypted_data = hex::encode(&blob);

            let result = order.decrypt(key);
            assert!(
                result.is_err(),
                "tampered ciphertext must be rejected for {scheme:?}"
            );
        }
    }

    #[test]
    fn test_truncated_ciphertext_rejected() {
        // A blob too short to even hold a nonce + tag must produce an honest error.
        for scheme in ALL_SCHEMES {
            let result = PrivateOrder::decrypt_aead(scheme, "00112233", "secret");
            assert!(result.is_err(), "truncated blob must fail for {scheme:?}");
        }
    }

    #[test]
    fn test_key_hash_does_not_leak_aead_key() {
        // The public `key_hash` (SHA-256 of the secret) must NOT equal the derived AEAD key,
        // otherwise the serialized order would expose its own decryption key. Domain
        // separation in `derive_key` guarantees the two differ.
        let secret = "leak_check_secret";
        let key_hash = PrivateOrder::hash_key(secret);
        let derived_key_hex = hex::encode(PrivateOrder::derive_key(secret));
        assert_ne!(
            key_hash, derived_key_hex,
            "public key_hash must not reveal the AEAD key"
        );

        // And the published hash must not appear inside the stored ciphertext blob either.
        let data = sample_order();
        let config = PrivatePoolConfig::default();
        let order = PrivateOrder::encrypt_and_create(
            Uuid::new_v4(),
            Uuid::new_v4(),
            &data,
            secret,
            &config,
        )
        .expect("encryption should succeed");
        assert!(
            !order.encrypted_data.contains(&derived_key_hex),
            "derived key must not appear in ciphertext"
        );
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
