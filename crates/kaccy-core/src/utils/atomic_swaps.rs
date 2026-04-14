//! Atomic Swaps System
//!
//! Provides trustless cross-chain atomic swaps using Hash Time-Locked Contracts (HTLC):
//! - Hash time-locked contracts (HTLC)
//! - Cross-chain order matching
//! - Trustless settlement
//! - Refund mechanisms

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Blockchain network identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chain {
    /// Bitcoin
    Bitcoin,
    /// Ethereum
    Ethereum,
    /// Binance Smart Chain
    BinanceSmartChain,
    /// Polygon
    Polygon,
    /// Solana
    Solana,
    /// Cosmos
    Cosmos,
    /// Custom chain
    Custom,
}

impl Chain {
    /// Get chain name
    pub fn as_str(&self) -> &'static str {
        match self {
            Chain::Bitcoin => "bitcoin",
            Chain::Ethereum => "ethereum",
            Chain::BinanceSmartChain => "bsc",
            Chain::Polygon => "polygon",
            Chain::Solana => "solana",
            Chain::Cosmos => "cosmos",
            Chain::Custom => "custom",
        }
    }
}

/// Hash time-locked contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HTLC {
    /// Contract ID
    pub id: String,
    /// Sender address
    pub sender: String,
    /// Receiver address
    pub receiver: String,
    /// Amount locked
    pub amount: Decimal,
    /// Token identifier
    pub token: String,
    /// Hash lock (SHA256 of secret)
    pub hash_lock: String,
    /// Time lock (expiration)
    pub time_lock: DateTime<Utc>,
    /// Chain this HTLC is on
    pub chain: Chain,
    /// Current state
    pub state: HTLCState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// HTLC state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HTLCState {
    /// Contract created and funded
    Active,
    /// Successfully claimed by receiver
    Claimed,
    /// Refunded to sender (time lock expired)
    Refunded,
    /// Cancelled before funding
    Cancelled,
}

impl HTLC {
    /// Create a new HTLC
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        sender: String,
        receiver: String,
        amount: Decimal,
        token: String,
        hash_lock: String,
        time_lock: DateTime<Utc>,
        chain: Chain,
    ) -> Self {
        Self {
            id,
            sender,
            receiver,
            amount,
            token,
            hash_lock,
            time_lock,
            chain,
            state: HTLCState::Active,
            created_at: Utc::now(),
        }
    }

    /// Check if contract has expired
    pub fn is_expired(&self, current_time: DateTime<Utc>) -> bool {
        current_time >= self.time_lock
    }

    /// Verify secret against hash lock
    pub fn verify_secret(&self, secret: &str) -> bool {
        let hash = Sha256::digest(secret.as_bytes());
        let hash_hex = hex::encode(hash);
        hash_hex == self.hash_lock
    }

    /// Claim the HTLC with secret
    pub fn claim(&mut self, secret: &str, current_time: DateTime<Utc>) -> Result<(), HTLCError> {
        // Check state
        if self.state != HTLCState::Active {
            return Err(HTLCError::InvalidState);
        }

        // Check expiration
        if self.is_expired(current_time) {
            return Err(HTLCError::Expired);
        }

        // Verify secret
        if !self.verify_secret(secret) {
            return Err(HTLCError::InvalidSecret);
        }

        self.state = HTLCState::Claimed;
        Ok(())
    }

    /// Refund the HTLC (after expiration)
    pub fn refund(&mut self, current_time: DateTime<Utc>) -> Result<(), HTLCError> {
        // Check state
        if self.state != HTLCState::Active {
            return Err(HTLCError::InvalidState);
        }

        // Check if expired
        if !self.is_expired(current_time) {
            return Err(HTLCError::NotExpired);
        }

        self.state = HTLCState::Refunded;
        Ok(())
    }
}

/// HTLC error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HTLCError {
    /// The contract is not in a valid state for the requested operation
    InvalidState,
    /// The time lock has expired
    Expired,
    /// The time lock has not yet expired
    NotExpired,
    /// The provided preimage does not match the hash lock
    InvalidSecret,
    /// Insufficient funds to create the HTLC
    InsufficientFunds,
    /// The referenced order was not found
    OrderNotFound,
    /// The order has already been matched
    OrderAlreadyMatched,
    /// A swap with the given ID already exists
    SwapAlreadyExists,
}

/// Cross-chain swap order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapOrder {
    /// Order ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Offering chain
    pub offer_chain: Chain,
    /// Offering token
    pub offer_token: String,
    /// Offering amount
    pub offer_amount: Decimal,
    /// Requesting chain
    pub request_chain: Chain,
    /// Requesting token
    pub request_token: String,
    /// Requesting amount
    pub request_amount: Decimal,
    /// Order creation time
    pub created_at: DateTime<Utc>,
    /// Order expiration time
    pub expires_at: DateTime<Utc>,
    /// Order state
    pub state: SwapOrderState,
}

/// Swap order state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapOrderState {
    /// Order is open and waiting for match
    Open,
    /// Order has been matched
    Matched,
    /// Swap is in progress
    InProgress,
    /// Swap completed successfully
    Completed,
    /// Swap failed or cancelled
    Cancelled,
    /// Order expired
    Expired,
}

/// Atomic swap (pair of HTLCs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicSwap {
    /// Swap ID
    pub id: String,
    /// Order A (initiator)
    pub order_a: String,
    /// Order B (counterparty)
    pub order_b: String,
    /// HTLC on chain A
    pub htlc_a: HTLC,
    /// HTLC on chain B
    pub htlc_b: HTLC,
    /// Preimage secret — known only to the initiator until step 1 executes
    secret: Option<String>,
    /// Hash lock (SHA256 of secret)
    pub hash_lock: String,
    /// Swap state
    pub state: SwapState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Atomic swap state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapState {
    /// Waiting for HTLCs to be created
    Pending,
    /// Both HTLCs created and funded
    Locked,
    /// Swap completed successfully
    Completed,
    /// Swap failed (refunded)
    Failed,
}

impl AtomicSwap {
    /// Create a new atomic swap
    pub fn new(
        id: String,
        order_a_id: String,
        order_b_id: String,
        order_a: &SwapOrder,
        order_b: &SwapOrder,
        time_lock_duration: Duration,
    ) -> Self {
        // Generate secret
        let secret = Self::generate_secret();
        let hash_lock = Self::hash_secret(&secret);

        let now = Utc::now();
        let time_lock_a = now + time_lock_duration;
        let time_lock_b = now + time_lock_duration / 2; // Chain B lock expires first

        // Create HTLCs
        let htlc_a = HTLC::new(
            format!("{}_htlc_a", id),
            order_a.user_id.clone(),
            order_b.user_id.clone(),
            order_a.offer_amount,
            order_a.offer_token.clone(),
            hash_lock.clone(),
            time_lock_a,
            order_a.offer_chain,
        );

        let htlc_b = HTLC::new(
            format!("{}_htlc_b", id),
            order_b.user_id.clone(),
            order_a.user_id.clone(),
            order_b.offer_amount,
            order_b.offer_token.clone(),
            hash_lock.clone(),
            time_lock_b,
            order_b.offer_chain,
        );

        Self {
            id,
            order_a: order_a_id,
            order_b: order_b_id,
            htlc_a,
            htlc_b,
            secret: Some(secret),
            hash_lock,
            state: SwapState::Pending,
            created_at: now,
        }
    }

    /// Generate a random secret
    fn generate_secret() -> String {
        use rand::RngExt;
        let mut rng = rand::rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
        hex::encode(bytes)
    }

    /// Hash a secret using SHA256
    fn hash_secret(secret: &str) -> String {
        let hash = Sha256::digest(secret.as_bytes());
        hex::encode(hash)
    }

    /// Lock both HTLCs (mark as funded)
    pub fn lock(&mut self) -> Result<(), HTLCError> {
        if self.state != SwapState::Pending {
            return Err(HTLCError::InvalidState);
        }
        self.state = SwapState::Locked;
        Ok(())
    }

    /// Execute swap (initiator claims on chain B, revealing secret)
    pub fn execute_step1(&mut self, current_time: DateTime<Utc>) -> Result<String, HTLCError> {
        if self.state != SwapState::Locked {
            return Err(HTLCError::InvalidState);
        }

        // Initiator claims HTLC B using secret
        let secret = self.secret.as_ref().ok_or(HTLCError::InvalidSecret)?;
        self.htlc_b.claim(secret, current_time)?;

        Ok(secret.clone())
    }

    /// Complete swap (counterparty claims on chain A using revealed secret)
    pub fn execute_step2(
        &mut self,
        secret: &str,
        current_time: DateTime<Utc>,
    ) -> Result<(), HTLCError> {
        // Counterparty claims HTLC A using the secret learned from chain B
        self.htlc_a.claim(secret, current_time)?;
        self.state = SwapState::Completed;
        Ok(())
    }

    /// Refund swap (both parties refund if swap fails)
    pub fn refund(&mut self, current_time: DateTime<Utc>) -> Result<(), HTLCError> {
        // Attempt to refund both HTLCs
        let _ = self.htlc_a.refund(current_time);
        let _ = self.htlc_b.refund(current_time);

        self.state = SwapState::Failed;
        Ok(())
    }

    /// Check if swap has expired
    pub fn is_expired(&self, current_time: DateTime<Utc>) -> bool {
        self.htlc_a.is_expired(current_time) || self.htlc_b.is_expired(current_time)
    }
}

/// Cross-chain order matcher
pub struct OrderMatcher {
    /// Open swap orders indexed by order ID
    orders: HashMap<String, SwapOrder>,
    /// Active atomic swaps indexed by swap ID
    swaps: HashMap<String, AtomicSwap>,
}

impl OrderMatcher {
    /// Create a new order matcher
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            swaps: HashMap::new(),
        }
    }

    /// Submit a swap order
    pub fn submit_order(&mut self, order: SwapOrder) -> Result<(), HTLCError> {
        self.orders.insert(order.id.clone(), order);
        Ok(())
    }

    /// Find matching orders
    pub fn find_matches(&self, order_id: &str) -> Vec<String> {
        let order = match self.orders.get(order_id) {
            Some(o) => o,
            None => return vec![],
        };

        if order.state != SwapOrderState::Open {
            return vec![];
        }

        self.orders
            .iter()
            .filter(|(id, other)| {
                // Don't match with self
                if *id == order_id {
                    return false;
                }

                // Only match open orders
                if other.state != SwapOrderState::Open {
                    return false;
                }

                // Check if orders are compatible
                order.offer_chain == other.request_chain
                    && order.offer_token == other.request_token
                    && order.request_chain == other.offer_chain
                    && order.request_token == other.offer_token
                    && order.offer_amount >= other.request_amount
                    && order.request_amount <= other.offer_amount
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Create atomic swap from matched orders
    pub fn create_swap(
        &mut self,
        order_a_id: &str,
        order_b_id: &str,
        time_lock_duration: Duration,
    ) -> Result<String, HTLCError> {
        // Get orders
        let order_a = self
            .orders
            .get(order_a_id)
            .ok_or(HTLCError::OrderNotFound)?;
        let order_b = self
            .orders
            .get(order_b_id)
            .ok_or(HTLCError::OrderNotFound)?;

        // Check states
        if order_a.state != SwapOrderState::Open || order_b.state != SwapOrderState::Open {
            return Err(HTLCError::OrderAlreadyMatched);
        }

        // Create swap
        let swap_id = format!("swap_{}_{}", order_a_id, order_b_id);
        let swap = AtomicSwap::new(
            swap_id.clone(),
            order_a_id.to_string(),
            order_b_id.to_string(),
            order_a,
            order_b,
            time_lock_duration,
        );

        // Update order states
        if let Some(order) = self.orders.get_mut(order_a_id) {
            order.state = SwapOrderState::Matched;
        }
        if let Some(order) = self.orders.get_mut(order_b_id) {
            order.state = SwapOrderState::Matched;
        }

        self.swaps.insert(swap_id.clone(), swap);
        Ok(swap_id)
    }

    /// Get swap
    pub fn get_swap(&self, swap_id: &str) -> Option<&AtomicSwap> {
        self.swaps.get(swap_id)
    }

    /// Get mutable swap
    pub fn get_swap_mut(&mut self, swap_id: &str) -> Option<&mut AtomicSwap> {
        self.swaps.get_mut(swap_id)
    }

    /// Cancel an order
    pub fn cancel_order(&mut self, order_id: &str) -> Result<(), HTLCError> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or(HTLCError::OrderNotFound)?;

        if order.state != SwapOrderState::Open {
            return Err(HTLCError::OrderAlreadyMatched);
        }

        order.state = SwapOrderState::Cancelled;
        Ok(())
    }

    /// Clean up expired orders
    pub fn cleanup_expired(&mut self, current_time: DateTime<Utc>) {
        for order in self.orders.values_mut() {
            if order.state == SwapOrderState::Open && current_time >= order.expires_at {
                order.state = SwapOrderState::Expired;
            }
        }
    }

    /// Get order statistics
    pub fn get_stats(&self) -> OrderMatcherStats {
        let mut stats = OrderMatcherStats {
            total_orders: self.orders.len(),
            open_orders: 0,
            matched_orders: 0,
            completed_swaps: 0,
            failed_swaps: 0,
            active_swaps: 0,
        };

        for order in self.orders.values() {
            match order.state {
                SwapOrderState::Open => stats.open_orders += 1,
                SwapOrderState::Matched | SwapOrderState::InProgress => stats.matched_orders += 1,
                _ => {}
            }
        }

        for swap in self.swaps.values() {
            match swap.state {
                SwapState::Pending | SwapState::Locked => stats.active_swaps += 1,
                SwapState::Completed => stats.completed_swaps += 1,
                SwapState::Failed => stats.failed_swaps += 1,
            }
        }

        stats
    }
}

impl Default for OrderMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Order matcher statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMatcherStats {
    /// Total number of orders ever submitted
    pub total_orders: usize,
    /// Number of orders currently open and awaiting a match
    pub open_orders: usize,
    /// Number of orders that have been matched
    pub matched_orders: usize,
    /// Number of swaps that completed successfully
    pub completed_swaps: usize,
    /// Number of swaps that failed or were refunded
    pub failed_swaps: usize,
    /// Number of swaps currently pending or locked
    pub active_swaps: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htlc_creation() {
        let secret = "my_secret_123";
        let hash = Sha256::digest(secret.as_bytes());
        let hash_hex = hex::encode(hash);

        let htlc = HTLC::new(
            "htlc1".to_string(),
            "alice".to_string(),
            "bob".to_string(),
            Decimal::from(100),
            "BTC".to_string(),
            hash_hex.clone(),
            Utc::now() + Duration::hours(24),
            Chain::Bitcoin,
        );

        assert_eq!(htlc.state, HTLCState::Active);
        assert!(htlc.verify_secret(secret));
    }

    #[test]
    fn test_htlc_claim() {
        let secret = "my_secret_123";
        let hash = Sha256::digest(secret.as_bytes());
        let hash_hex = hex::encode(hash);

        let mut htlc = HTLC::new(
            "htlc1".to_string(),
            "alice".to_string(),
            "bob".to_string(),
            Decimal::from(100),
            "BTC".to_string(),
            hash_hex,
            Utc::now() + Duration::hours(24),
            Chain::Bitcoin,
        );

        let result = htlc.claim(secret, Utc::now());
        assert!(result.is_ok());
        assert_eq!(htlc.state, HTLCState::Claimed);
    }

    #[test]
    fn test_htlc_refund() {
        let secret = "my_secret_123";
        let hash = Sha256::digest(secret.as_bytes());
        let hash_hex = hex::encode(hash);

        let expiration = Utc::now() + Duration::hours(1);
        let mut htlc = HTLC::new(
            "htlc1".to_string(),
            "alice".to_string(),
            "bob".to_string(),
            Decimal::from(100),
            "BTC".to_string(),
            hash_hex,
            expiration,
            Chain::Bitcoin,
        );

        // Try to refund before expiration
        let result = htlc.refund(Utc::now());
        assert_eq!(result, Err(HTLCError::NotExpired));

        // Refund after expiration
        let result = htlc.refund(expiration + Duration::hours(1));
        assert!(result.is_ok());
        assert_eq!(htlc.state, HTLCState::Refunded);
    }

    #[test]
    fn test_atomic_swap_creation() {
        let order_a = SwapOrder {
            id: "order_a".to_string(),
            user_id: "alice".to_string(),
            offer_chain: Chain::Bitcoin,
            offer_token: "BTC".to_string(),
            offer_amount: Decimal::from(1),
            request_chain: Chain::Ethereum,
            request_token: "ETH".to_string(),
            request_amount: Decimal::from(15),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let order_b = SwapOrder {
            id: "order_b".to_string(),
            user_id: "bob".to_string(),
            offer_chain: Chain::Ethereum,
            offer_token: "ETH".to_string(),
            offer_amount: Decimal::from(15),
            request_chain: Chain::Bitcoin,
            request_token: "BTC".to_string(),
            request_amount: Decimal::from(1),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let swap = AtomicSwap::new(
            "swap1".to_string(),
            "order_a".to_string(),
            "order_b".to_string(),
            &order_a,
            &order_b,
            Duration::hours(24),
        );

        assert_eq!(swap.state, SwapState::Pending);
        assert_eq!(swap.htlc_a.amount, Decimal::from(1));
        assert_eq!(swap.htlc_b.amount, Decimal::from(15));
    }

    #[test]
    fn test_atomic_swap_execution() {
        let order_a = SwapOrder {
            id: "order_a".to_string(),
            user_id: "alice".to_string(),
            offer_chain: Chain::Bitcoin,
            offer_token: "BTC".to_string(),
            offer_amount: Decimal::from(1),
            request_chain: Chain::Ethereum,
            request_token: "ETH".to_string(),
            request_amount: Decimal::from(15),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let order_b = SwapOrder {
            id: "order_b".to_string(),
            user_id: "bob".to_string(),
            offer_chain: Chain::Ethereum,
            offer_token: "ETH".to_string(),
            offer_amount: Decimal::from(15),
            request_chain: Chain::Bitcoin,
            request_token: "BTC".to_string(),
            request_amount: Decimal::from(1),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let mut swap = AtomicSwap::new(
            "swap1".to_string(),
            "order_a".to_string(),
            "order_b".to_string(),
            &order_a,
            &order_b,
            Duration::hours(24),
        );

        // Lock HTLCs
        swap.lock().unwrap();
        assert_eq!(swap.state, SwapState::Locked);

        // Step 1: Initiator claims HTLC B
        let secret = swap.execute_step1(Utc::now()).unwrap();
        assert_eq!(swap.htlc_b.state, HTLCState::Claimed);

        // Step 2: Counterparty claims HTLC A using revealed secret
        swap.execute_step2(&secret, Utc::now()).unwrap();
        assert_eq!(swap.htlc_a.state, HTLCState::Claimed);
        assert_eq!(swap.state, SwapState::Completed);
    }

    #[test]
    fn test_order_matching() {
        let mut matcher = OrderMatcher::new();

        let order_a = SwapOrder {
            id: "order_a".to_string(),
            user_id: "alice".to_string(),
            offer_chain: Chain::Bitcoin,
            offer_token: "BTC".to_string(),
            offer_amount: Decimal::from(1),
            request_chain: Chain::Ethereum,
            request_token: "ETH".to_string(),
            request_amount: Decimal::from(15),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let order_b = SwapOrder {
            id: "order_b".to_string(),
            user_id: "bob".to_string(),
            offer_chain: Chain::Ethereum,
            offer_token: "ETH".to_string(),
            offer_amount: Decimal::from(15),
            request_chain: Chain::Bitcoin,
            request_token: "BTC".to_string(),
            request_amount: Decimal::from(1),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        matcher.submit_order(order_a).unwrap();
        matcher.submit_order(order_b).unwrap();

        let matches = matcher.find_matches("order_a");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "order_b");
    }

    #[test]
    fn test_create_swap_from_orders() {
        let mut matcher = OrderMatcher::new();

        let order_a = SwapOrder {
            id: "order_a".to_string(),
            user_id: "alice".to_string(),
            offer_chain: Chain::Bitcoin,
            offer_token: "BTC".to_string(),
            offer_amount: Decimal::from(1),
            request_chain: Chain::Ethereum,
            request_token: "ETH".to_string(),
            request_amount: Decimal::from(15),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        let order_b = SwapOrder {
            id: "order_b".to_string(),
            user_id: "bob".to_string(),
            offer_chain: Chain::Ethereum,
            offer_token: "ETH".to_string(),
            offer_amount: Decimal::from(15),
            request_chain: Chain::Bitcoin,
            request_token: "BTC".to_string(),
            request_amount: Decimal::from(1),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        matcher.submit_order(order_a).unwrap();
        matcher.submit_order(order_b).unwrap();

        let swap_id = matcher
            .create_swap("order_a", "order_b", Duration::hours(24))
            .unwrap();
        let swap = matcher.get_swap(&swap_id).unwrap();

        assert_eq!(swap.state, SwapState::Pending);
    }

    #[test]
    fn test_order_stats() {
        let mut matcher = OrderMatcher::new();

        let order = SwapOrder {
            id: "order_a".to_string(),
            user_id: "alice".to_string(),
            offer_chain: Chain::Bitcoin,
            offer_token: "BTC".to_string(),
            offer_amount: Decimal::from(1),
            request_chain: Chain::Ethereum,
            request_token: "ETH".to_string(),
            request_amount: Decimal::from(15),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            state: SwapOrderState::Open,
        };

        matcher.submit_order(order).unwrap();

        let stats = matcher.get_stats();
        assert_eq!(stats.total_orders, 1);
        assert_eq!(stats.open_orders, 1);
    }
}
