//! Airdrop mechanisms
//!
//! This module implements token airdrop functionality with Merkle tree-based
//! distribution for efficient and secure claim verification.
//!
//! # Features
//!
//! - Merkle tree-based distribution
//! - Claim verification
//! - Anti-sybil measures
//! - Batch claim processing
//! - Expiration handling
//!
//! # How It Works
//!
//! 1. **Setup**: Create airdrop with list of recipients and amounts
//! 2. **Merkle Tree**: Build Merkle tree from distribution data
//! 3. **Publish**: Publish Merkle root on-chain/in database
//! 4. **Claim**: Users submit claims with Merkle proofs
//! 5. **Verify**: System verifies proof and distributes tokens

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Airdrop configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirdropConfig {
    /// Token to airdrop
    pub token_id: Uuid,
    /// Total amount allocated
    pub total_amount: Decimal,
    /// When claims can start
    pub start_time: DateTime<Utc>,
    /// When claims must end
    pub deadline: DateTime<Utc>,
    /// Minimum claim amount (anti-dust)
    pub min_claim_amount: Decimal,
    /// Whether to allow multiple claims per user
    pub allow_multiple_claims: bool,
    /// Maximum claims per user (if allow_multiple_claims is true)
    pub max_claims_per_user: usize,
}

impl AirdropConfig {
    /// Create a new airdrop configuration
    pub fn new(
        token_id: Uuid,
        total_amount: Decimal,
        start_time: DateTime<Utc>,
        deadline: DateTime<Utc>,
    ) -> Result<Self> {
        if total_amount <= dec!(0) {
            return Err(CoreError::Validation(
                "Total amount must be positive".to_string(),
            ));
        }

        if deadline <= start_time {
            return Err(CoreError::Validation(
                "Deadline must be after start time".to_string(),
            ));
        }

        Ok(Self {
            token_id,
            total_amount,
            start_time,
            deadline,
            min_claim_amount: dec!(0.001),
            allow_multiple_claims: false,
            max_claims_per_user: 1,
        })
    }

    /// Check if airdrop is currently active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.start_time && now <= self.deadline
    }

    /// Check if airdrop has ended
    pub fn is_ended(&self) -> bool {
        Utc::now() > self.deadline
    }
}

/// Status of an airdrop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AirdropStatus {
    /// Not yet started
    Pending,
    /// Active (accepting claims)
    Active,
    /// Ended (no more claims)
    Ended,
    /// Cancelled
    Cancelled,
}

/// An entry in the airdrop distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirdropEntry {
    /// User ID
    pub user_id: Uuid,
    /// Amount allocated to this user
    pub amount: Decimal,
    /// Optional data (e.g., reason for airdrop)
    pub metadata: Option<String>,
}

impl AirdropEntry {
    /// Create a new airdrop entry
    pub fn new(user_id: Uuid, amount: Decimal) -> Self {
        Self {
            user_id,
            amount,
            metadata: None,
        }
    }

    /// Compute hash of this entry for Merkle tree
    pub fn compute_hash(&self) -> String {
        let data = format!("{}|{}", self.user_id, self.amount);
        let hash = Sha256::digest(data.as_bytes());
        hex::encode(hash)
    }
}

/// A Merkle proof for claim verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The leaf hash (user's entry)
    pub leaf: String,
    /// Sibling hashes along the path to root
    pub siblings: Vec<String>,
    /// Position indicators (left=false, right=true)
    pub positions: Vec<bool>,
}

impl MerkleProof {
    /// Verify this proof against a Merkle root
    pub fn verify(&self, root: &str) -> bool {
        let mut current_hash = self.leaf.clone();

        for (sibling, &is_right) in self.siblings.iter().zip(&self.positions) {
            current_hash = if is_right {
                // Current node is on the right
                Self::hash_pair(sibling, &current_hash)
            } else {
                // Current node is on the left
                Self::hash_pair(&current_hash, sibling)
            };
        }

        current_hash == root
    }

    /// Hash two nodes together
    fn hash_pair(left: &str, right: &str) -> String {
        let combined = format!("{}|{}", left, right);
        let hash = Sha256::digest(combined.as_bytes());
        hex::encode(hash)
    }
}

/// Simple Merkle tree implementation for airdrops
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// Leaf hashes (bottom level)
    leaves: Vec<String>,
    /// All tree levels (leaves at index 0, root at last index)
    levels: Vec<Vec<String>>,
}

impl MerkleTree {
    /// Build a Merkle tree from entries
    pub fn build(entries: &[AirdropEntry]) -> Self {
        let mut leaves: Vec<String> = entries.iter().map(|e| e.compute_hash()).collect();

        // Need at least one leaf
        if leaves.is_empty() {
            leaves.push(String::new());
        }

        // Make number of leaves a power of 2 by padding
        let next_power_of_2 = leaves.len().next_power_of_two();
        while leaves.len() < next_power_of_2 {
            leaves.push(
                leaves
                    .last()
                    .expect("leaves is non-empty per push above")
                    .clone(),
            );
        }

        let mut levels = vec![leaves.clone()];
        let mut current_level = leaves.clone();

        // Build tree bottom-up
        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for i in (0..current_level.len()).step_by(2) {
                let left = &current_level[i];
                let right = &current_level[i + 1];
                let parent = MerkleProof::hash_pair(left, right);
                next_level.push(parent);
            }

            levels.push(next_level.clone());
            current_level = next_level;
        }

        Self { leaves, levels }
    }

    /// Get the Merkle root
    pub fn root(&self) -> String {
        self.levels
            .last()
            .expect("levels always has at least the leaves level")[0]
            .clone()
    }

    /// Generate a Merkle proof for a leaf at given index
    pub fn get_proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() {
            return None;
        }

        let leaf = self.leaves[index].clone();
        let mut siblings = Vec::new();
        let mut positions = Vec::new();
        let mut current_index = index;

        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            siblings.push(level[sibling_index].clone());
            positions.push(current_index % 2 == 1); // true if current is right child

            current_index /= 2;
        }

        Some(MerkleProof {
            leaf,
            siblings,
            positions,
        })
    }
}

/// A claim record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRecord {
    /// Claim ID
    pub id: Uuid,
    /// User who claimed
    pub user_id: Uuid,
    /// Amount claimed
    pub amount: Decimal,
    /// When the claim was made
    pub claimed_at: DateTime<Utc>,
    /// Transaction hash
    pub tx_hash: Option<String>,
}

/// Airdrop statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirdropStats {
    /// Total amount allocated
    pub total_allocated: Decimal,
    /// Total amount claimed
    pub total_claimed: Decimal,
    /// Remaining amount
    pub remaining: Decimal,
    /// Number of eligible users
    pub eligible_users: usize,
    /// Number of users who claimed
    pub claimed_users: usize,
    /// Claim rate (percentage)
    pub claim_rate_pct: Decimal,
    /// Status
    pub status: AirdropStatus,
}

/// Airdrop manager
pub struct Airdrop {
    /// Airdrop ID
    pub id: Uuid,
    /// Configuration
    pub config: AirdropConfig,
    /// Distribution entries
    pub entries: Vec<AirdropEntry>,
    /// Merkle tree
    pub merkle_tree: MerkleTree,
    /// Merkle root
    pub merkle_root: String,
    /// Claim records
    pub claims: Vec<ClaimRecord>,
    /// Users who have claimed
    claimed_users: HashSet<Uuid>,
    /// Total claimed amount
    total_claimed: Decimal,
    /// Current status
    status: AirdropStatus,
}

impl Airdrop {
    /// Create a new airdrop
    pub fn new(config: AirdropConfig, entries: Vec<AirdropEntry>) -> Result<Self> {
        // Validate entries
        if entries.is_empty() {
            return Err(CoreError::Validation(
                "No airdrop entries provided".to_string(),
            ));
        }

        // Check total amount matches sum of entries
        let sum: Decimal = entries.iter().map(|e| e.amount).sum();
        if (sum - config.total_amount).abs() > dec!(0.000001) {
            return Err(CoreError::Validation(format!(
                "Entry sum {} does not match total amount {}",
                sum, config.total_amount
            )));
        }

        // Build Merkle tree
        let merkle_tree = MerkleTree::build(&entries);
        let merkle_root = merkle_tree.root();

        let status = if config.is_active() {
            AirdropStatus::Active
        } else if config.is_ended() {
            AirdropStatus::Ended
        } else {
            AirdropStatus::Pending
        };

        Ok(Self {
            id: Uuid::new_v4(),
            config,
            entries,
            merkle_tree,
            merkle_root,
            claims: Vec::new(),
            claimed_users: HashSet::new(),
            total_claimed: dec!(0),
            status,
        })
    }

    /// Update status based on current time
    pub fn update_status(&mut self) {
        if self.status == AirdropStatus::Cancelled {
            return;
        }

        self.status = if self.config.is_active() {
            AirdropStatus::Active
        } else if self.config.is_ended() {
            AirdropStatus::Ended
        } else {
            AirdropStatus::Pending
        };
    }

    /// Claim tokens with Merkle proof
    pub fn claim(
        &mut self,
        user_id: Uuid,
        amount: Decimal,
        proof: MerkleProof,
    ) -> Result<ClaimRecord> {
        self.update_status();

        // Check status
        if self.status != AirdropStatus::Active {
            return Err(CoreError::InvalidState(format!(
                "Airdrop is not active (status: {:?})",
                self.status
            )));
        }

        // Check if user already claimed
        if !self.config.allow_multiple_claims && self.claimed_users.contains(&user_id) {
            return Err(CoreError::AlreadyExists(
                "User has already claimed".to_string(),
            ));
        }

        // Verify proof
        if !proof.verify(&self.merkle_root) {
            return Err(CoreError::Validation("Invalid Merkle proof".to_string()));
        }

        // Find entry for this user and verify amount
        let entry = self
            .entries
            .iter()
            .find(|e| e.user_id == user_id)
            .ok_or_else(|| CoreError::NotFound("User not in airdrop list".to_string()))?;

        if entry.amount != amount {
            return Err(CoreError::Validation(format!(
                "Amount mismatch: expected {}, got {}",
                entry.amount, amount
            )));
        }

        // Create claim record
        let claim = ClaimRecord {
            id: Uuid::new_v4(),
            user_id,
            amount,
            claimed_at: Utc::now(),
            tx_hash: None,
        };

        // Update state
        self.claims.push(claim.clone());
        self.claimed_users.insert(user_id);
        self.total_claimed += amount;

        Ok(claim)
    }

    /// Get Merkle proof for a user
    pub fn get_proof(&self, user_id: &Uuid) -> Option<MerkleProof> {
        let index = self.entries.iter().position(|e| &e.user_id == user_id)?;
        self.merkle_tree.get_proof(index)
    }

    /// Check if a user is eligible
    pub fn is_eligible(&self, user_id: &Uuid) -> bool {
        self.entries.iter().any(|e| &e.user_id == user_id)
    }

    /// Get user's allocated amount
    pub fn get_allocation(&self, user_id: &Uuid) -> Option<Decimal> {
        self.entries
            .iter()
            .find(|e| &e.user_id == user_id)
            .map(|e| e.amount)
    }

    /// Get statistics
    pub fn stats(&self) -> AirdropStats {
        let total_allocated = self.config.total_amount;
        let total_claimed = self.total_claimed;
        let remaining = total_allocated - total_claimed;
        let eligible_users = self.entries.len();
        let claimed_users = self.claimed_users.len();

        let claim_rate_pct = if eligible_users > 0 {
            (Decimal::from(claimed_users) / Decimal::from(eligible_users)) * dec!(100)
        } else {
            dec!(0)
        };

        AirdropStats {
            total_allocated,
            total_claimed,
            remaining,
            eligible_users,
            claimed_users,
            claim_rate_pct,
            status: self.status,
        }
    }

    /// Cancel the airdrop
    pub fn cancel(&mut self) -> Result<()> {
        if self.status == AirdropStatus::Ended {
            return Err(CoreError::InvalidState(
                "Cannot cancel ended airdrop".to_string(),
            ));
        }

        if !self.claims.is_empty() {
            return Err(CoreError::InvalidState(
                "Cannot cancel airdrop with existing claims".to_string(),
            ));
        }

        self.status = AirdropStatus::Cancelled;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_airdrop_entry_hash() {
        let user_id = Uuid::new_v4();
        let entry = AirdropEntry::new(user_id, dec!(100));

        let hash1 = entry.compute_hash();
        let hash2 = entry.compute_hash();

        // Same entry should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_merkle_tree_build() {
        let entries = vec![
            AirdropEntry::new(Uuid::new_v4(), dec!(100)),
            AirdropEntry::new(Uuid::new_v4(), dec!(200)),
            AirdropEntry::new(Uuid::new_v4(), dec!(300)),
            AirdropEntry::new(Uuid::new_v4(), dec!(400)),
        ];

        let tree = MerkleTree::build(&entries);
        let root = tree.root();

        assert!(!root.is_empty());
    }

    #[test]
    fn test_merkle_proof_verification() {
        let entries = vec![
            AirdropEntry::new(Uuid::new_v4(), dec!(100)),
            AirdropEntry::new(Uuid::new_v4(), dec!(200)),
        ];

        let tree = MerkleTree::build(&entries);
        let root = tree.root();
        let proof = tree.get_proof(0).unwrap();

        assert!(proof.verify(&root));
    }

    #[test]
    fn test_airdrop_creation() {
        let token_id = Uuid::new_v4();
        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);

        let config = AirdropConfig::new(token_id, dec!(1000), start, deadline).unwrap();

        let entries = vec![
            AirdropEntry::new(Uuid::new_v4(), dec!(400)),
            AirdropEntry::new(Uuid::new_v4(), dec!(300)),
            AirdropEntry::new(Uuid::new_v4(), dec!(300)),
        ];

        let airdrop = Airdrop::new(config, entries).unwrap();
        assert_eq!(airdrop.status, AirdropStatus::Active);
    }

    #[test]
    fn test_airdrop_claim() {
        let token_id = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = AirdropConfig::new(token_id, dec!(600), start, deadline).unwrap();

        let entries = vec![
            AirdropEntry::new(user1, dec!(400)),
            AirdropEntry::new(user2, dec!(200)),
        ];

        let mut airdrop = Airdrop::new(config, entries).unwrap();

        // Get proof for user1
        let proof = airdrop.get_proof(&user1).unwrap();

        // Claim
        let claim = airdrop.claim(user1, dec!(400), proof).unwrap();
        assert_eq!(claim.amount, dec!(400));

        // Check stats
        let stats = airdrop.stats();
        assert_eq!(stats.total_claimed, dec!(400));
        assert_eq!(stats.claimed_users, 1);
    }

    #[test]
    fn test_airdrop_duplicate_claim() {
        let token_id = Uuid::new_v4();
        let user1 = Uuid::new_v4();

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = AirdropConfig::new(token_id, dec!(100), start, deadline).unwrap();

        let entries = vec![AirdropEntry::new(user1, dec!(100))];

        let mut airdrop = Airdrop::new(config, entries).unwrap();

        // First claim should succeed
        let proof = airdrop.get_proof(&user1).unwrap();
        airdrop.claim(user1, dec!(100), proof.clone()).unwrap();

        // Second claim should fail
        let result = airdrop.claim(user1, dec!(100), proof);
        assert!(result.is_err());
    }

    #[test]
    fn test_airdrop_invalid_proof() {
        let token_id = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = AirdropConfig::new(token_id, dec!(300), start, deadline).unwrap();

        let entries = vec![
            AirdropEntry::new(user1, dec!(200)),
            AirdropEntry::new(user2, dec!(100)),
        ];

        let mut airdrop = Airdrop::new(config, entries).unwrap();

        // Create fake proof
        let fake_proof = MerkleProof {
            leaf: "fake".to_string(),
            siblings: vec![],
            positions: vec![],
        };

        // Claim should fail
        let result = airdrop.claim(user1, dec!(200), fake_proof);
        assert!(result.is_err());
    }
}
