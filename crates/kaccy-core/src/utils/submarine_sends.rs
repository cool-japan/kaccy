//! Submarine Sends
//!
//! Provides submarine send functionality for privacy-preserving transfers:
//! - Commit-reveal with cryptographic commitments
//! - Front-running protection via delayed reveal
//! - Privacy-preserving transfers
//! - Delayed execution after reveal

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

/// Submarine send state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmarineSendState {
    /// Commitment has been submitted
    Committed,
    /// Reveal period (can be revealed)
    Revealable,
    /// Transaction has been revealed
    Revealed,
    /// Transaction has been executed
    Executed,
    /// Transaction expired without reveal
    Expired,
    /// Transaction was cancelled
    Cancelled,
}

/// Cryptographic commitment for a submarine send
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineCommitment {
    /// Commitment ID
    pub id: String,
    /// Commitment hash (hash of transaction data + nonce)
    pub commitment_hash: Vec<u8>,
    /// Commit timestamp
    pub committed_at: DateTime<Utc>,
    /// Earliest reveal time
    pub reveal_after: DateTime<Utc>,
    /// Latest reveal time (expiration)
    pub reveal_before: DateTime<Utc>,
    /// Committer address
    pub committer: String,
    /// Optional bond/stake (to prevent spam)
    pub bond_amount: Option<Decimal>,
    /// State
    pub state: SubmarineSendState,
}

impl SubmarineCommitment {
    /// Create a new submarine commitment
    pub fn new(
        id: String,
        commitment_hash: Vec<u8>,
        committer: String,
        commit_delay_seconds: i64,
        reveal_window_seconds: i64,
        bond_amount: Option<Decimal>,
    ) -> Self {
        let now = Utc::now();
        let reveal_after = now + chrono::Duration::seconds(commit_delay_seconds);
        let reveal_before = reveal_after + chrono::Duration::seconds(reveal_window_seconds);

        Self {
            id,
            commitment_hash,
            committed_at: now,
            reveal_after,
            reveal_before,
            committer,
            bond_amount,
            state: SubmarineSendState::Committed,
        }
    }

    /// Check if the commitment can be revealed
    pub fn can_reveal(&self, current_time: DateTime<Utc>) -> bool {
        (self.state == SubmarineSendState::Committed
            || self.state == SubmarineSendState::Revealable)
            && current_time >= self.reveal_after
            && current_time <= self.reveal_before
    }

    /// Check if the commitment has expired
    pub fn is_expired(&self, current_time: DateTime<Utc>) -> bool {
        self.state == SubmarineSendState::Committed && current_time > self.reveal_before
    }

    /// Mark as revealable
    pub fn mark_revealable(&mut self) {
        if self.state == SubmarineSendState::Committed {
            self.state = SubmarineSendState::Revealable;
        }
    }

    /// Mark as revealed
    pub fn mark_revealed(&mut self) {
        if self.state == SubmarineSendState::Committed
            || self.state == SubmarineSendState::Revealable
        {
            self.state = SubmarineSendState::Revealed;
        }
    }

    /// Mark as executed
    pub fn mark_executed(&mut self) {
        if self.state == SubmarineSendState::Revealed {
            self.state = SubmarineSendState::Executed;
        }
    }

    /// Mark as expired
    pub fn mark_expired(&mut self) {
        if self.state == SubmarineSendState::Committed
            || self.state == SubmarineSendState::Revealable
        {
            self.state = SubmarineSendState::Expired;
        }
    }

    /// Mark as cancelled
    pub fn mark_cancelled(&mut self) {
        if self.state == SubmarineSendState::Committed
            || self.state == SubmarineSendState::Revealable
        {
            self.state = SubmarineSendState::Cancelled;
        }
    }
}

/// Submarine send transaction (revealed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineSend {
    /// Transaction ID
    pub id: String,
    /// Commitment ID this was revealed from
    pub commitment_id: String,
    /// From address
    pub from: String,
    /// To address
    pub to: String,
    /// Token ID
    pub token_id: String,
    /// Amount
    pub amount: Decimal,
    /// Nonce (for commitment)
    pub nonce: Vec<u8>,
    /// Additional data (encrypted or not)
    pub data: Vec<u8>,
    /// Reveal timestamp
    pub revealed_at: DateTime<Utc>,
    /// Execution timestamp
    pub executed_at: Option<DateTime<Utc>>,
}

impl SubmarineSend {
    /// Create a new submarine send
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        commitment_id: String,
        from: String,
        to: String,
        token_id: String,
        amount: Decimal,
        nonce: Vec<u8>,
        data: Vec<u8>,
    ) -> Self {
        Self {
            id,
            commitment_id,
            from,
            to,
            token_id,
            amount,
            nonce,
            data,
            revealed_at: Utc::now(),
            executed_at: None,
        }
    }

    /// Compute commitment hash for verification
    pub fn compute_commitment_hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.from.as_bytes());
        hasher.update(self.to.as_bytes());
        hasher.update(self.token_id.as_bytes());
        hasher.update(self.amount.to_string().as_bytes());
        hasher.update(&self.nonce);
        hasher.update(&self.data);
        hasher.finalize().to_vec()
    }

    /// Verify that this transaction matches the commitment
    pub fn verify_commitment(&self, commitment_hash: &[u8]) -> bool {
        self.compute_commitment_hash() == commitment_hash
    }

    /// Mark as executed
    pub fn mark_executed(&mut self) {
        self.executed_at = Some(Utc::now());
    }

    /// Check if executed
    pub fn is_executed(&self) -> bool {
        self.executed_at.is_some()
    }
}

/// Submarine send helper for creating commitments
pub struct SubmarineSendBuilder {
    /// From address
    pub from: String,
    /// To address
    pub to: String,
    /// Token ID
    pub token_id: String,
    /// Amount
    pub amount: Decimal,
    /// Additional data
    pub data: Vec<u8>,
}

impl SubmarineSendBuilder {
    /// Create a new builder
    pub fn new(from: String, to: String, token_id: String, amount: Decimal) -> Self {
        Self {
            from,
            to,
            token_id,
            amount,
            data: Vec::new(),
        }
    }

    /// Add additional data
    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Generate a random nonce
    fn generate_nonce() -> Vec<u8> {
        // In real implementation, use a CSPRNG
        let mut hasher = Sha256::new();
        hasher.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes());
        hasher.update(rand::random::<[u8; 32]>());
        hasher.finalize()[0..32].to_vec()
    }

    /// Build the commitment and transaction
    pub fn build(
        self,
        commitment_id: String,
        tx_id: String,
        commit_delay_seconds: i64,
        reveal_window_seconds: i64,
        bond_amount: Option<Decimal>,
    ) -> (SubmarineCommitment, SubmarineSend) {
        let nonce = Self::generate_nonce();

        // Create the transaction
        let tx = SubmarineSend::new(
            tx_id,
            commitment_id.clone(),
            self.from.clone(),
            self.to,
            self.token_id,
            self.amount,
            nonce,
            self.data,
        );

        // Compute commitment hash
        let commitment_hash = tx.compute_commitment_hash();

        // Create the commitment
        let commitment = SubmarineCommitment::new(
            commitment_id,
            commitment_hash,
            self.from,
            commit_delay_seconds,
            reveal_window_seconds,
            bond_amount,
        );

        (commitment, tx)
    }
}

/// Submarine send manager
#[derive(Debug)]
pub struct SubmarineSendManager {
    /// Active commitments (commitment_id -> commitment)
    pub commitments: HashMap<String, SubmarineCommitment>,
    /// Revealed transactions (commitment_id -> transaction)
    pub revealed_txs: HashMap<String, SubmarineSend>,
    /// Executed transactions
    pub executed_txs: Vec<SubmarineSend>,
    /// Default commit delay (seconds)
    pub default_commit_delay: i64,
    /// Default reveal window (seconds)
    pub default_reveal_window: i64,
    /// Require bond for commitments
    pub require_bond: bool,
    /// Minimum bond amount (if required)
    pub min_bond_amount: Decimal,
}

impl SubmarineSendManager {
    /// Create a new submarine send manager
    pub fn new(
        default_commit_delay: i64,
        default_reveal_window: i64,
        require_bond: bool,
        min_bond_amount: Decimal,
    ) -> Self {
        Self {
            commitments: HashMap::new(),
            revealed_txs: HashMap::new(),
            executed_txs: Vec::new(),
            default_commit_delay,
            default_reveal_window,
            require_bond,
            min_bond_amount,
        }
    }

    /// Submit a commitment
    pub fn submit_commitment(&mut self, commitment: SubmarineCommitment) -> Result<()> {
        // Check if commitment already exists
        if self.commitments.contains_key(&commitment.id) {
            return Err(CoreError::Validation(format!(
                "Commitment {} already exists",
                commitment.id
            )));
        }

        // Verify bond if required
        if self.require_bond {
            match commitment.bond_amount {
                Some(amount) if amount >= self.min_bond_amount => {}
                _ => {
                    return Err(CoreError::Validation(format!(
                        "Bond amount must be at least {}",
                        self.min_bond_amount
                    )));
                }
            }
        }

        self.commitments.insert(commitment.id.clone(), commitment);
        Ok(())
    }

    /// Reveal a transaction
    pub fn reveal_transaction(
        &mut self,
        tx: SubmarineSend,
        current_time: DateTime<Utc>,
    ) -> Result<()> {
        // Get the commitment
        let commitment = self.commitments.get_mut(&tx.commitment_id).ok_or_else(|| {
            CoreError::Validation(format!("Commitment {} not found", tx.commitment_id))
        })?;

        // Check if can reveal
        if !commitment.can_reveal(current_time) {
            return Err(CoreError::Validation(format!(
                "Cannot reveal commitment {} at this time",
                tx.commitment_id
            )));
        }

        // Verify commitment matches
        if !tx.verify_commitment(&commitment.commitment_hash) {
            return Err(CoreError::Validation(
                "Transaction does not match commitment".to_string(),
            ));
        }

        // Mark commitment as revealed
        commitment.mark_revealed();

        // Store revealed transaction
        self.revealed_txs.insert(tx.commitment_id.clone(), tx);

        Ok(())
    }

    /// Execute a revealed transaction
    pub fn execute_transaction(&mut self, commitment_id: &str) -> Result<SubmarineSend> {
        // Get the revealed transaction
        let mut tx = self.revealed_txs.remove(commitment_id).ok_or_else(|| {
            CoreError::Validation(format!(
                "No revealed transaction for commitment {}",
                commitment_id
            ))
        })?;

        // Get the commitment
        let commitment = self.commitments.get_mut(commitment_id).ok_or_else(|| {
            CoreError::Validation(format!("Commitment {} not found", commitment_id))
        })?;

        // Verify state
        if commitment.state != SubmarineSendState::Revealed {
            return Err(CoreError::Validation(format!(
                "Commitment {} is not in revealed state",
                commitment_id
            )));
        }

        // Mark as executed
        tx.mark_executed();
        commitment.mark_executed();

        // Store executed transaction
        self.executed_txs.push(tx.clone());

        Ok(tx)
    }

    /// Cancel a commitment (before reveal)
    pub fn cancel_commitment(&mut self, commitment_id: &str, requester: &str) -> Result<()> {
        let commitment = self.commitments.get_mut(commitment_id).ok_or_else(|| {
            CoreError::Validation(format!("Commitment {} not found", commitment_id))
        })?;

        // Verify requester is the committer
        if commitment.committer != requester {
            return Err(CoreError::Validation(
                "Only committer can cancel".to_string(),
            ));
        }

        // Can only cancel if not yet revealed
        if commitment.state != SubmarineSendState::Committed
            && commitment.state != SubmarineSendState::Revealable
        {
            return Err(CoreError::Validation(
                "Cannot cancel commitment in current state".to_string(),
            ));
        }

        commitment.mark_cancelled();
        Ok(())
    }

    /// Expire old commitments
    pub fn expire_old_commitments(&mut self, current_time: DateTime<Utc>) -> Vec<String> {
        let mut expired = Vec::new();

        for (id, commitment) in &mut self.commitments {
            if commitment.is_expired(current_time)
                && commitment.state == SubmarineSendState::Committed
            {
                commitment.mark_expired();
                expired.push(id.clone());
            }
        }

        expired
    }

    /// Update revealable commitments
    pub fn update_revealable(&mut self, current_time: DateTime<Utc>) {
        for commitment in self.commitments.values_mut() {
            if commitment.state == SubmarineSendState::Committed
                && current_time >= commitment.reveal_after
                && current_time <= commitment.reveal_before
            {
                commitment.mark_revealable();
            }
        }
    }

    /// Get commitment by ID
    pub fn get_commitment(&self, commitment_id: &str) -> Option<&SubmarineCommitment> {
        self.commitments.get(commitment_id)
    }

    /// Get revealed transaction
    pub fn get_revealed_transaction(&self, commitment_id: &str) -> Option<&SubmarineSend> {
        self.revealed_txs.get(commitment_id)
    }

    /// Get statistics
    pub fn stats(&self) -> SubmarineSendStats {
        let total_commitments = self.commitments.len();
        let revealed_count = self
            .commitments
            .values()
            .filter(|c| c.state == SubmarineSendState::Revealed)
            .count();
        let executed_count = self.executed_txs.len();
        let expired_count = self
            .commitments
            .values()
            .filter(|c| c.state == SubmarineSendState::Expired)
            .count();
        let pending_count = self
            .commitments
            .values()
            .filter(|c| c.state == SubmarineSendState::Committed)
            .count();

        SubmarineSendStats {
            total_commitments,
            pending_commitments: pending_count,
            revealable_commitments: self
                .commitments
                .values()
                .filter(|c| c.state == SubmarineSendState::Revealable)
                .count(),
            revealed_transactions: revealed_count,
            executed_transactions: executed_count,
            expired_commitments: expired_count,
        }
    }
}

impl Default for SubmarineSendManager {
    fn default() -> Self {
        Self::new(
            300,   // 5 minutes commit delay
            600,   // 10 minutes reveal window
            false, // No bond required by default
            Decimal::ZERO,
        )
    }
}

/// Submarine send statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineSendStats {
    /// Total commitments ever made
    pub total_commitments: usize,
    /// Pending commitments (not yet revealable)
    pub pending_commitments: usize,
    /// Revealable commitments (in reveal window)
    pub revealable_commitments: usize,
    /// Revealed transactions (not yet executed)
    pub revealed_transactions: usize,
    /// Executed transactions
    pub executed_transactions: usize,
    /// Expired commitments
    pub expired_commitments: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_submarine_commitment_lifecycle() {
        let commitment = SubmarineCommitment::new(
            "commit-1".to_string(),
            vec![1, 2, 3],
            "user1".to_string(),
            60,  // 1 min delay
            120, // 2 min window
            Some(dec!(10)),
        );

        assert_eq!(commitment.state, SubmarineSendState::Committed);

        // Cannot reveal immediately
        assert!(!commitment.can_reveal(Utc::now()));

        // Can reveal after delay
        let after_delay = Utc::now() + chrono::Duration::seconds(61);
        assert!(commitment.can_reveal(after_delay));

        // Expires after window
        let after_expiry = Utc::now() + chrono::Duration::seconds(200);
        assert!(commitment.is_expired(after_expiry));
    }

    #[test]
    fn test_submarine_send_builder() {
        let (commitment, tx) = SubmarineSendBuilder::new(
            "user1".to_string(),
            "user2".to_string(),
            "token1".to_string(),
            dec!(100),
        )
        .with_data(vec![1, 2, 3])
        .build(
            "commit-1".to_string(),
            "tx-1".to_string(),
            60,
            120,
            Some(dec!(10)),
        );

        assert_eq!(commitment.committer, "user1");
        assert_eq!(tx.amount, dec!(100));
        assert!(tx.verify_commitment(&commitment.commitment_hash));
    }

    #[test]
    fn test_submarine_send_manager() {
        let mut manager = SubmarineSendManager::default();

        // Create and submit commitment
        let (commitment, tx) = SubmarineSendBuilder::new(
            "user1".to_string(),
            "user2".to_string(),
            "token1".to_string(),
            dec!(100),
        )
        .build(
            "commit-1".to_string(),
            "tx-1".to_string(),
            1, // 1 second delay for testing
            60,
            None,
        );

        manager.submit_commitment(commitment).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_commitments, 1);
        assert_eq!(stats.pending_commitments, 1);

        // Wait for reveal window
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Reveal the transaction
        let reveal_time = Utc::now();
        manager.update_revealable(reveal_time);
        manager.reveal_transaction(tx, reveal_time).unwrap();

        // Execute the transaction
        let executed = manager.execute_transaction("commit-1").unwrap();
        assert!(executed.is_executed());

        let stats = manager.stats();
        assert_eq!(stats.executed_transactions, 1);
    }
}
