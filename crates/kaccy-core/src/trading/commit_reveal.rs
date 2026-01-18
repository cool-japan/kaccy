//! Commit-reveal scheme for order submission
//!
//! This module implements a commit-reveal mechanism to prevent front-running:
//! 1. **Commit Phase**: User submits a hash of their order + secret
//! 2. **Reveal Phase**: User reveals the actual order and secret
//! 3. **Verification**: System verifies the reveal matches the commitment
//!
//! # Benefits
//!
//! - Prevents front-running (order details hidden during commit phase)
//! - Enables fair ordering (all commits processed before reveals)
//! - Reduces MEV extraction opportunities
//! - Compatible with batch auctions
//!
//! # Example Flow
//!
//! ```text
//! 1. User computes: commitment = Hash(order_params || secret)
//! 2. User submits commitment to blockchain/system
//! 3. Commit period ends
//! 4. User reveals: order_params, secret
//! 5. System verifies: Hash(order_params || secret) == commitment
//! 6. If valid, order is executed
//! ```

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::trading::order_book::{LimitOrder, OrderSide};

/// Configuration for commit-reveal mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRevealConfig {
    /// Duration of the commit phase
    pub commit_duration: Duration,
    /// Duration of the reveal phase
    pub reveal_duration: Duration,
    /// Penalty for not revealing (as percentage of order value)
    pub non_reveal_penalty_pct: Decimal,
    /// Whether to allow early reveals
    pub allow_early_reveal: bool,
}

impl Default for CommitRevealConfig {
    fn default() -> Self {
        Self {
            commit_duration: Duration::seconds(30),
            reveal_duration: Duration::seconds(60),
            non_reveal_penalty_pct: dec!(1.0), // 1% penalty
            allow_early_reveal: false,
        }
    }
}

/// Status of a commit-reveal cycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommitRevealPhase {
    /// Accepting commitments
    Commit,
    /// Accepting reveals
    Reveal,
    /// Processing completed reveals
    Processing,
    /// Cycle completed
    Completed,
    /// Cycle expired
    Expired,
}

/// A commitment to an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCommitment {
    /// Unique commitment ID
    pub id: Uuid,
    /// User who made the commitment
    pub user_id: Uuid,
    /// Token ID for the order
    pub token_id: Uuid,
    /// Hash of the order + secret
    pub commitment_hash: String,
    /// When the commitment was made
    pub committed_at: DateTime<Utc>,
    /// When the commitment expires
    pub expires_at: DateTime<Utc>,
    /// Whether this commitment has been revealed
    pub is_revealed: bool,
    /// The revealed order (if revealed)
    pub revealed_order: Option<RevealedOrder>,
}

impl OrderCommitment {
    /// Create a new commitment
    pub fn new(
        user_id: Uuid,
        token_id: Uuid,
        commitment_hash: String,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            token_id,
            commitment_hash,
            committed_at: Utc::now(),
            expires_at,
            is_revealed: false,
            revealed_order: None,
        }
    }

    /// Check if commitment has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if commitment can be revealed
    pub fn can_reveal(&self, allow_early: bool, reveal_start: DateTime<Utc>) -> bool {
        if self.is_revealed || self.is_expired() {
            return false;
        }
        allow_early || Utc::now() >= reveal_start
    }
}

/// A revealed order with its secret
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevealedOrder {
    /// The actual order parameters
    pub order: OrderParams,
    /// The secret used in the commitment
    pub secret: String,
    /// When the order was revealed
    pub revealed_at: DateTime<Utc>,
}

/// Order parameters for commit-reveal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderParams {
    /// Order side (buy/sell)
    pub side: OrderSide,
    /// Price
    pub price: Decimal,
    /// Amount
    pub amount: Decimal,
}

impl OrderParams {
    /// Compute commitment hash
    pub fn compute_commitment(&self, secret: &str) -> String {
        let data = format!("{:?}|{}|{}|{}", self.side, self.price, self.amount, secret);
        let hash = Sha256::digest(data.as_bytes());
        hex::encode(hash)
    }

    /// Verify a commitment matches these parameters and secret
    pub fn verify_commitment(&self, secret: &str, expected_hash: &str) -> bool {
        let computed_hash = self.compute_commitment(secret);
        computed_hash == expected_hash
    }

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

/// Commit-reveal cycle manager
pub struct CommitRevealCycle {
    /// Cycle ID
    pub id: Uuid,
    /// Token ID for this cycle
    pub token_id: Uuid,
    /// Configuration
    pub config: CommitRevealConfig,
    /// Current phase
    pub phase: CommitRevealPhase,
    /// Commit phase start time
    pub commit_start: DateTime<Utc>,
    /// Reveal phase start time
    pub reveal_start: DateTime<Utc>,
    /// Cycle end time
    pub end_time: DateTime<Utc>,
    /// All commitments in this cycle
    pub commitments: HashMap<Uuid, OrderCommitment>,
    /// Successfully revealed orders
    pub revealed_orders: Vec<LimitOrder>,
}

impl CommitRevealCycle {
    /// Create a new cycle
    pub fn new(token_id: Uuid, config: CommitRevealConfig) -> Self {
        let commit_start = Utc::now();
        let reveal_start = commit_start + config.commit_duration;
        let end_time = reveal_start + config.reveal_duration;

        Self {
            id: Uuid::new_v4(),
            token_id,
            config,
            phase: CommitRevealPhase::Commit,
            commit_start,
            reveal_start,
            end_time,
            commitments: HashMap::new(),
            revealed_orders: Vec::new(),
        }
    }

    /// Update the cycle phase based on current time
    pub fn update_phase(&mut self) {
        let now = Utc::now();

        if now > self.end_time {
            self.phase = CommitRevealPhase::Expired;
        } else if now >= self.reveal_start && self.phase == CommitRevealPhase::Commit {
            self.phase = CommitRevealPhase::Reveal;
        }
    }

    /// Add a commitment to the cycle
    pub fn add_commitment(
        &mut self,
        user_id: Uuid,
        token_id: Uuid,
        commitment_hash: String,
    ) -> Result<Uuid> {
        self.update_phase();

        // Check phase
        if self.phase != CommitRevealPhase::Commit {
            return Err(CoreError::InvalidState(format!(
                "Not in commit phase (current: {:?})",
                self.phase
            )));
        }

        // Check token matches
        if token_id != self.token_id {
            return Err(CoreError::Validation(format!(
                "Token {} does not match cycle token {}",
                token_id, self.token_id
            )));
        }

        let commitment = OrderCommitment::new(user_id, token_id, commitment_hash, self.end_time);
        let commitment_id = commitment.id;

        self.commitments.insert(commitment_id, commitment);

        Ok(commitment_id)
    }

    /// Reveal an order
    pub fn reveal_order(
        &mut self,
        commitment_id: Uuid,
        order_params: OrderParams,
        secret: String,
    ) -> Result<LimitOrder> {
        self.update_phase();

        // Get commitment
        let commitment = self.commitments.get_mut(&commitment_id).ok_or_else(|| {
            CoreError::NotFound(format!("Commitment {} not found", commitment_id))
        })?;

        // Check if can reveal
        if !commitment.can_reveal(self.config.allow_early_reveal, self.reveal_start) {
            return Err(CoreError::InvalidState(
                "Cannot reveal at this time".to_string(),
            ));
        }

        // Verify commitment
        if !order_params.verify_commitment(&secret, &commitment.commitment_hash) {
            return Err(CoreError::Validation(
                "Commitment verification failed".to_string(),
            ));
        }

        // Mark as revealed
        commitment.is_revealed = true;
        commitment.revealed_order = Some(RevealedOrder {
            order: order_params.clone(),
            secret: secret.clone(),
            revealed_at: Utc::now(),
        });

        // Create limit order
        let limit_order = order_params.to_limit_order(commitment.user_id, commitment.token_id);
        self.revealed_orders.push(limit_order.clone());

        Ok(limit_order)
    }

    /// Get statistics about the cycle
    pub fn stats(&self) -> CommitRevealStats {
        let total_commitments = self.commitments.len();
        let revealed_count = self.commitments.values().filter(|c| c.is_revealed).count();
        let unrevealed_count = total_commitments - revealed_count;

        CommitRevealStats {
            cycle_id: self.id,
            token_id: self.token_id,
            phase: self.phase,
            total_commitments,
            revealed_count,
            unrevealed_count,
            reveal_rate_pct: if total_commitments > 0 {
                (Decimal::from(revealed_count) / Decimal::from(total_commitments)) * dec!(100)
            } else {
                dec!(0)
            },
            commit_start: self.commit_start,
            reveal_start: self.reveal_start,
            end_time: self.end_time,
        }
    }

    /// Process the cycle (finalize and return orders)
    pub fn process(&mut self) -> Result<Vec<LimitOrder>> {
        self.update_phase();

        if self.phase != CommitRevealPhase::Reveal && self.phase != CommitRevealPhase::Expired {
            return Err(CoreError::InvalidState(
                "Cycle not ready for processing".to_string(),
            ));
        }

        self.phase = CommitRevealPhase::Processing;

        // Return all revealed orders
        let orders = self.revealed_orders.clone();

        self.phase = CommitRevealPhase::Completed;

        Ok(orders)
    }
}

/// Statistics about a commit-reveal cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRevealStats {
    /// Cycle ID
    pub cycle_id: Uuid,
    /// Token ID
    pub token_id: Uuid,
    /// Current phase
    pub phase: CommitRevealPhase,
    /// Total commitments submitted
    pub total_commitments: usize,
    /// Number of revealed orders
    pub revealed_count: usize,
    /// Number of unrevealed orders
    pub unrevealed_count: usize,
    /// Reveal rate as percentage
    pub reveal_rate_pct: Decimal,
    /// Commit phase start
    pub commit_start: DateTime<Utc>,
    /// Reveal phase start
    pub reveal_start: DateTime<Utc>,
    /// Cycle end time
    pub end_time: DateTime<Utc>,
}

/// Manager for commit-reveal cycles
pub struct CommitRevealManager {
    /// Active cycles by token
    active_cycles: HashMap<Uuid, CommitRevealCycle>,
    /// Completed cycles (for history)
    completed_cycles: Vec<CommitRevealCycle>,
    /// Default configuration
    default_config: CommitRevealConfig,
}

impl CommitRevealManager {
    /// Create a new manager
    pub fn new(config: CommitRevealConfig) -> Self {
        Self {
            active_cycles: HashMap::new(),
            completed_cycles: Vec::new(),
            default_config: config,
        }
    }

    /// Start a new cycle for a token
    pub fn start_cycle(&mut self, token_id: Uuid) -> Result<Uuid> {
        if self.active_cycles.contains_key(&token_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Active cycle already exists for token {}",
                token_id
            )));
        }

        let cycle = CommitRevealCycle::new(token_id, self.default_config.clone());
        let cycle_id = cycle.id;

        self.active_cycles.insert(token_id, cycle);

        Ok(cycle_id)
    }

    /// Add a commitment to the active cycle
    pub fn add_commitment(
        &mut self,
        token_id: Uuid,
        user_id: Uuid,
        commitment_hash: String,
    ) -> Result<Uuid> {
        let cycle = self.active_cycles.get_mut(&token_id).ok_or_else(|| {
            CoreError::NotFound(format!("No active cycle for token {}", token_id))
        })?;

        cycle.add_commitment(user_id, token_id, commitment_hash)
    }

    /// Reveal an order
    pub fn reveal_order(
        &mut self,
        token_id: Uuid,
        commitment_id: Uuid,
        order_params: OrderParams,
        secret: String,
    ) -> Result<LimitOrder> {
        let cycle = self.active_cycles.get_mut(&token_id).ok_or_else(|| {
            CoreError::NotFound(format!("No active cycle for token {}", token_id))
        })?;

        cycle.reveal_order(commitment_id, order_params, secret)
    }

    /// Process and complete a cycle
    pub fn process_cycle(&mut self, token_id: Uuid) -> Result<Vec<LimitOrder>> {
        let mut cycle = self.active_cycles.remove(&token_id).ok_or_else(|| {
            CoreError::NotFound(format!("No active cycle for token {}", token_id))
        })?;

        let orders = cycle.process()?;

        self.completed_cycles.push(cycle);

        Ok(orders)
    }

    /// Get active cycle for a token
    pub fn get_active_cycle(&self, token_id: &Uuid) -> Option<&CommitRevealCycle> {
        self.active_cycles.get(token_id)
    }

    /// Get statistics for all completed cycles
    pub fn get_completed_stats(&self) -> Vec<CommitRevealStats> {
        self.completed_cycles.iter().map(|c| c.stats()).collect()
    }

    /// Clean up old completed cycles
    pub fn cleanup_old_cycles(&mut self, keep_last: usize) {
        if self.completed_cycles.len() > keep_last {
            self.completed_cycles
                .drain(0..self.completed_cycles.len() - keep_last);
        }
    }
}

impl Default for CommitRevealManager {
    fn default() -> Self {
        Self::new(CommitRevealConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_hash() {
        let params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let secret = "my_secret_123";
        let hash1 = params.compute_commitment(secret);
        let hash2 = params.compute_commitment(secret);

        // Same params + secret should produce same hash
        assert_eq!(hash1, hash2);

        // Different secret should produce different hash
        let hash3 = params.compute_commitment("different_secret");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_commitment_verification() {
        let params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };

        let secret = "my_secret";
        let hash = params.compute_commitment(secret);

        // Correct verification
        assert!(params.verify_commitment(secret, &hash));

        // Wrong secret
        assert!(!params.verify_commitment("wrong_secret", &hash));

        // Wrong hash
        assert!(!params.verify_commitment(secret, "wrong_hash"));
    }

    #[test]
    fn test_commit_reveal_cycle() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let config = CommitRevealConfig {
            allow_early_reveal: true, // Allow immediate reveal for testing
            ..Default::default()
        };

        let mut cycle = CommitRevealCycle::new(token_id, config);

        // Add commitment
        let params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };
        let secret = "test_secret";
        let commitment_hash = params.compute_commitment(secret);

        let commitment_id = cycle
            .add_commitment(user_id, token_id, commitment_hash)
            .unwrap();

        // Reveal order
        let order = cycle
            .reveal_order(commitment_id, params.clone(), secret.to_string())
            .unwrap();

        assert_eq!(order.price, dec!(100));
        assert_eq!(order.amount, dec!(10));
        assert_eq!(order.side, OrderSide::Buy);
    }

    #[test]
    fn test_invalid_reveal() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let config = CommitRevealConfig {
            allow_early_reveal: true,
            ..Default::default()
        };

        let mut cycle = CommitRevealCycle::new(token_id, config);

        // Add commitment
        let params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };
        let secret = "test_secret";
        let commitment_hash = params.compute_commitment(secret);

        let commitment_id = cycle
            .add_commitment(user_id, token_id, commitment_hash)
            .unwrap();

        // Try to reveal with wrong secret
        let wrong_params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };
        let result = cycle.reveal_order(commitment_id, wrong_params, "wrong_secret".to_string());

        assert!(result.is_err());
    }

    #[test]
    fn test_commit_reveal_manager() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let mut manager = CommitRevealManager::default();

        // Start cycle
        let cycle_id = manager.start_cycle(token_id).unwrap();
        assert!(cycle_id != Uuid::nil());

        // Add commitment
        let params = OrderParams {
            side: OrderSide::Buy,
            price: dec!(100),
            amount: dec!(10),
        };
        let secret = "secret";
        let hash = params.compute_commitment(secret);

        let commitment_id = manager.add_commitment(token_id, user_id, hash).unwrap();
        assert!(commitment_id != Uuid::nil());
    }

    #[test]
    fn test_cycle_stats() {
        let token_id = Uuid::new_v4();
        let config = CommitRevealConfig {
            allow_early_reveal: true,
            ..Default::default()
        };

        let mut cycle = CommitRevealCycle::new(token_id, config);

        // Add commitments
        for i in 0..5 {
            let params = OrderParams {
                side: OrderSide::Buy,
                price: dec!(100),
                amount: Decimal::from(i),
            };
            let hash = params.compute_commitment(&format!("secret_{}", i));
            cycle
                .add_commitment(Uuid::new_v4(), token_id, hash)
                .unwrap();
        }

        let stats = cycle.stats();
        assert_eq!(stats.total_commitments, 5);
        assert_eq!(stats.revealed_count, 0);
        assert_eq!(stats.unrevealed_count, 5);
    }
}
