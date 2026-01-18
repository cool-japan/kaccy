//! Advanced governance features
//!
//! This module provides vote delegation, vote escrow (veToken), and conviction voting
//! mechanisms for enhanced governance participation.

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

/// Vote delegation record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VoteDelegation {
    pub delegation_id: Uuid,
    /// User who is delegating their voting power
    pub delegator_user_id: Uuid,
    /// User who receives the delegated voting power
    pub delegate_user_id: Uuid,
    /// Amount of voting power delegated
    pub voting_power: Decimal,
    /// When the delegation becomes active
    pub start_time: DateTime<Utc>,
    /// When the delegation ends (None = permanent)
    pub end_time: Option<DateTime<Utc>>,
    /// Whether the delegation is currently active
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl VoteDelegation {
    pub fn new(
        delegator_user_id: Uuid,
        delegate_user_id: Uuid,
        voting_power: Decimal,
        duration_days: Option<u32>,
    ) -> Result<Self> {
        if voting_power <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Voting power must be positive".to_string(),
            ));
        }

        if delegator_user_id == delegate_user_id {
            return Err(CoreError::Validation(
                "Cannot delegate to yourself".to_string(),
            ));
        }

        let start_time = Utc::now();
        let end_time = duration_days.map(|days| start_time + chrono::Duration::days(days as i64));

        Ok(Self {
            delegation_id: Uuid::new_v4(),
            delegator_user_id,
            delegate_user_id,
            voting_power,
            start_time,
            end_time,
            is_active: true,
            created_at: Utc::now(),
            revoked_at: None,
        })
    }

    /// Check if delegation is currently valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        self.is_active && now >= self.start_time && self.end_time.is_none_or(|end| now <= end)
    }

    /// Revoke delegation
    pub fn revoke(&mut self) {
        self.is_active = false;
        self.revoked_at = Some(Utc::now());
    }
}

/// Vote escrow (veToken) lock
///
/// Users lock tokens for a duration to receive boosted voting power
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VoteEscrowLock {
    pub lock_id: Uuid,
    pub user_id: Uuid,
    /// Amount of tokens locked
    pub amount: Decimal,
    /// Lock start time
    pub lock_start: DateTime<Utc>,
    /// Lock end time
    pub lock_end: DateTime<Utc>,
    /// Calculated voting power (amount * time multiplier)
    pub voting_power: Decimal,
    /// Whether the lock is still active
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub unlocked_at: Option<DateTime<Utc>>,
}

impl VoteEscrowLock {
    /// Maximum lock duration in days (4 years)
    pub const MAX_LOCK_DAYS: i64 = 1461; // 4 * 365 + 1 for leap year

    /// Create a new vote escrow lock
    pub fn new(user_id: Uuid, amount: Decimal, lock_duration_days: i64) -> Result<Self> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Lock amount must be positive".to_string(),
            ));
        }

        if lock_duration_days <= 0 {
            return Err(CoreError::Validation(
                "Lock duration must be positive".to_string(),
            ));
        }

        if lock_duration_days > Self::MAX_LOCK_DAYS {
            return Err(CoreError::Validation(format!(
                "Lock duration cannot exceed {} days",
                Self::MAX_LOCK_DAYS
            )));
        }

        let lock_start = Utc::now();
        let lock_end = lock_start + chrono::Duration::days(lock_duration_days);

        // Calculate voting power: amount * (lock_duration / max_duration)
        // Longer locks get more voting power per token
        let time_multiplier =
            Decimal::from(lock_duration_days) / Decimal::from(Self::MAX_LOCK_DAYS);
        let voting_power = amount * (Decimal::ONE + time_multiplier);

        Ok(Self {
            lock_id: Uuid::new_v4(),
            user_id,
            amount,
            lock_start,
            lock_end,
            voting_power,
            is_active: true,
            created_at: Utc::now(),
            unlocked_at: None,
        })
    }

    /// Check if lock has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.lock_end
    }

    /// Get remaining lock time in days
    pub fn remaining_days(&self) -> i64 {
        if self.is_expired() {
            return 0;
        }
        (self.lock_end - Utc::now()).num_days()
    }

    /// Calculate current voting power (decays over time)
    pub fn current_voting_power(&self) -> Decimal {
        if !self.is_active || self.is_expired() {
            return Decimal::ZERO;
        }

        let remaining_days = self.remaining_days();
        let time_multiplier = Decimal::from(remaining_days) / Decimal::from(Self::MAX_LOCK_DAYS);
        self.amount * (Decimal::ONE + time_multiplier)
    }

    /// Unlock tokens (with penalty if early)
    pub fn unlock(&mut self, early_unlock_penalty: Decimal) -> Result<Decimal> {
        if !self.is_active {
            return Err(CoreError::InvalidState("Lock is not active".to_string()));
        }

        self.is_active = false;
        self.unlocked_at = Some(Utc::now());

        // Apply penalty if unlocking early
        if !self.is_expired() {
            let penalty_amount = self.amount * early_unlock_penalty;
            Ok(self.amount - penalty_amount)
        } else {
            Ok(self.amount)
        }
    }
}

/// Conviction voting record
///
/// Votes gain strength over time, encouraging long-term commitment
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConvictionVote {
    pub vote_id: Uuid,
    pub proposal_id: Uuid,
    pub user_id: Uuid,
    /// Base voting power (from tokens)
    pub base_voting_power: Decimal,
    /// Vote direction (true = for, false = against)
    pub vote_for: bool,
    /// When the vote was cast
    pub vote_time: DateTime<Utc>,
    /// When the vote was removed (if applicable)
    pub removed_at: Option<DateTime<Utc>>,
    /// Whether the vote is still active
    pub is_active: bool,
}

impl ConvictionVote {
    pub fn new(
        proposal_id: Uuid,
        user_id: Uuid,
        base_voting_power: Decimal,
        vote_for: bool,
    ) -> Result<Self> {
        if base_voting_power <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Voting power must be positive".to_string(),
            ));
        }

        Ok(Self {
            vote_id: Uuid::new_v4(),
            proposal_id,
            user_id,
            base_voting_power,
            vote_for,
            vote_time: Utc::now(),
            removed_at: None,
            is_active: true,
        })
    }

    /// Calculate current conviction (voting power grows over time)
    ///
    /// Uses formula: conviction = base_power * (1 + time_factor)
    /// where time_factor = sqrt(days_active) / 10
    pub fn current_conviction(&self) -> Decimal {
        if !self.is_active {
            return Decimal::ZERO;
        }

        let days_active = (Utc::now() - self.vote_time).num_days() as f64;
        if days_active < 0.0 {
            return self.base_voting_power;
        }

        // Conviction grows with square root of time (diminishing returns)
        let time_factor = (days_active.sqrt() / 10.0).min(2.0); // Cap at 3x multiplier
        let multiplier = 1.0 + time_factor;

        self.base_voting_power * Decimal::from_f64(multiplier).unwrap_or(Decimal::ONE)
    }

    /// Remove vote (triggers conviction decay)
    pub fn remove(&mut self) {
        self.is_active = false;
        self.removed_at = Some(Utc::now());
    }

    /// Calculate conviction decay (half-life of 7 days after removal)
    pub fn conviction_after_removal(&self) -> Decimal {
        let Some(removed_at) = self.removed_at else {
            return self.current_conviction();
        };

        let days_since_removal = (Utc::now() - removed_at).num_days() as f64;
        if days_since_removal < 0.0 {
            return Decimal::ZERO;
        }

        // Half-life decay: conviction * 0.5^(days / 7)
        let half_lives = days_since_removal / 7.0;
        let decay_factor = 0.5_f64.powf(half_lives);

        self.base_voting_power * Decimal::from_f64(decay_factor).unwrap_or(Decimal::ZERO)
    }
}

/// Delegation manager
pub struct DelegationManager {
    /// All delegations by ID
    delegations: HashMap<Uuid, VoteDelegation>,
    /// Delegator -> List of delegation IDs
    delegator_index: HashMap<Uuid, Vec<Uuid>>,
    /// Delegate -> List of delegation IDs
    delegate_index: HashMap<Uuid, Vec<Uuid>>,
}

impl DelegationManager {
    pub fn new() -> Self {
        Self {
            delegations: HashMap::new(),
            delegator_index: HashMap::new(),
            delegate_index: HashMap::new(),
        }
    }

    /// Create a new delegation
    pub fn delegate(
        &mut self,
        delegator_user_id: Uuid,
        delegate_user_id: Uuid,
        voting_power: Decimal,
        duration_days: Option<u32>,
    ) -> Result<Uuid> {
        let delegation = VoteDelegation::new(
            delegator_user_id,
            delegate_user_id,
            voting_power,
            duration_days,
        )?;

        let delegation_id = delegation.delegation_id;

        // Update indexes
        self.delegator_index
            .entry(delegator_user_id)
            .or_default()
            .push(delegation_id);

        self.delegate_index
            .entry(delegate_user_id)
            .or_default()
            .push(delegation_id);

        self.delegations.insert(delegation_id, delegation);
        Ok(delegation_id)
    }

    /// Revoke a delegation
    pub fn revoke_delegation(&mut self, delegation_id: Uuid) -> Result<()> {
        let delegation = self
            .delegations
            .get_mut(&delegation_id)
            .ok_or_else(|| CoreError::NotFound("Delegation not found".to_string()))?;

        delegation.revoke();
        Ok(())
    }

    /// Get total voting power for a user (including delegations)
    pub fn get_total_voting_power(&self, user_id: Uuid) -> Decimal {
        let mut total = Decimal::ZERO;

        // Get delegations where user is the delegate
        if let Some(delegation_ids) = self.delegate_index.get(&user_id) {
            for delegation_id in delegation_ids {
                if let Some(delegation) = self.delegations.get(delegation_id) {
                    if delegation.is_valid() {
                        total += delegation.voting_power;
                    }
                }
            }
        }

        total
    }

    /// Get all delegations for a user (as delegator)
    pub fn get_delegations_by_delegator(&self, user_id: Uuid) -> Vec<&VoteDelegation> {
        self.delegator_index
            .get(&user_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.delegations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all delegations received by a user (as delegate)
    pub fn get_delegations_received(&self, user_id: Uuid) -> Vec<&VoteDelegation> {
        self.delegate_index
            .get(&user_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.delegations.get(id))
                    .filter(|d| d.is_valid())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for DelegationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Vote escrow manager
pub struct VoteEscrowManager {
    locks: HashMap<Uuid, VoteEscrowLock>,
    user_locks: HashMap<Uuid, Vec<Uuid>>,
    /// Early unlock penalty (e.g., 0.5 = 50% penalty)
    early_unlock_penalty: Decimal,
}

impl VoteEscrowManager {
    pub fn new(early_unlock_penalty: Decimal) -> Result<Self> {
        if early_unlock_penalty < Decimal::ZERO || early_unlock_penalty > Decimal::ONE {
            return Err(CoreError::Validation(
                "Early unlock penalty must be between 0 and 1".to_string(),
            ));
        }

        Ok(Self {
            locks: HashMap::new(),
            user_locks: HashMap::new(),
            early_unlock_penalty,
        })
    }

    /// Create a new vote escrow lock
    pub fn create_lock(
        &mut self,
        user_id: Uuid,
        amount: Decimal,
        lock_duration_days: i64,
    ) -> Result<Uuid> {
        let lock = VoteEscrowLock::new(user_id, amount, lock_duration_days)?;
        let lock_id = lock.lock_id;

        self.user_locks.entry(user_id).or_default().push(lock_id);

        self.locks.insert(lock_id, lock);
        Ok(lock_id)
    }

    /// Unlock tokens
    pub fn unlock(&mut self, lock_id: Uuid) -> Result<Decimal> {
        let lock = self
            .locks
            .get_mut(&lock_id)
            .ok_or_else(|| CoreError::NotFound("Lock not found".to_string()))?;

        lock.unlock(self.early_unlock_penalty)
    }

    /// Get total voting power for a user from all active locks
    pub fn get_user_voting_power(&self, user_id: Uuid) -> Decimal {
        self.user_locks
            .get(&user_id)
            .map(|lock_ids| {
                lock_ids
                    .iter()
                    .filter_map(|id| self.locks.get(id))
                    .map(|lock| lock.current_voting_power())
                    .sum()
            })
            .unwrap_or(Decimal::ZERO)
    }

    /// Get all locks for a user
    pub fn get_user_locks(&self, user_id: Uuid) -> Vec<&VoteEscrowLock> {
        self.user_locks
            .get(&user_id)
            .map(|ids| ids.iter().filter_map(|id| self.locks.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get lock by ID
    pub fn get_lock(&self, lock_id: Uuid) -> Option<&VoteEscrowLock> {
        self.locks.get(&lock_id)
    }
}

/// Conviction voting manager
pub struct ConvictionVotingManager {
    votes: HashMap<Uuid, ConvictionVote>,
    /// Proposal ID -> List of vote IDs
    proposal_votes: HashMap<Uuid, Vec<Uuid>>,
    /// User ID -> List of vote IDs
    user_votes: HashMap<Uuid, Vec<Uuid>>,
}

impl ConvictionVotingManager {
    pub fn new() -> Self {
        Self {
            votes: HashMap::new(),
            proposal_votes: HashMap::new(),
            user_votes: HashMap::new(),
        }
    }

    /// Cast a conviction vote
    pub fn cast_vote(
        &mut self,
        proposal_id: Uuid,
        user_id: Uuid,
        base_voting_power: Decimal,
        vote_for: bool,
    ) -> Result<Uuid> {
        let vote = ConvictionVote::new(proposal_id, user_id, base_voting_power, vote_for)?;
        let vote_id = vote.vote_id;

        self.proposal_votes
            .entry(proposal_id)
            .or_default()
            .push(vote_id);

        self.user_votes.entry(user_id).or_default().push(vote_id);

        self.votes.insert(vote_id, vote);
        Ok(vote_id)
    }

    /// Remove a vote
    pub fn remove_vote(&mut self, vote_id: Uuid) -> Result<()> {
        let vote = self
            .votes
            .get_mut(&vote_id)
            .ok_or_else(|| CoreError::NotFound("Vote not found".to_string()))?;

        vote.remove();
        Ok(())
    }

    /// Get total conviction for a proposal
    pub fn get_proposal_conviction(&self, proposal_id: Uuid) -> (Decimal, Decimal) {
        let mut votes_for = Decimal::ZERO;
        let mut votes_against = Decimal::ZERO;

        if let Some(vote_ids) = self.proposal_votes.get(&proposal_id) {
            for vote_id in vote_ids {
                if let Some(vote) = self.votes.get(vote_id) {
                    let conviction = vote.current_conviction();
                    if vote.vote_for {
                        votes_for += conviction;
                    } else {
                        votes_against += conviction;
                    }
                }
            }
        }

        (votes_for, votes_against)
    }

    /// Get all votes for a proposal
    pub fn get_proposal_votes(&self, proposal_id: Uuid) -> Vec<&ConvictionVote> {
        self.proposal_votes
            .get(&proposal_id)
            .map(|ids| ids.iter().filter_map(|id| self.votes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all votes by a user
    pub fn get_user_votes(&self, user_id: Uuid) -> Vec<&ConvictionVote> {
        self.user_votes
            .get(&user_id)
            .map(|ids| ids.iter().filter_map(|id| self.votes.get(id)).collect())
            .unwrap_or_default()
    }
}

impl Default for ConvictionVotingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Quadratic voting vote record
///
/// Quadratic voting uses a quadratic cost function where the cost of n votes
/// is n^2, making it expensive to concentrate votes on a single option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadraticVote {
    pub vote_id: Uuid,
    pub proposal_id: Uuid,
    pub user_id: Uuid,
    /// Number of votes cast (actual voting power)
    pub vote_count: u32,
    /// Cost in vote credits (vote_count^2)
    pub credit_cost: Decimal,
    /// Whether this is a vote for or against
    pub in_favor: bool,
    pub created_at: DateTime<Utc>,
}

impl QuadraticVote {
    /// Create a new quadratic vote
    pub fn new(proposal_id: Uuid, user_id: Uuid, vote_count: u32, in_favor: bool) -> Result<Self> {
        if vote_count == 0 {
            return Err(CoreError::Validation(
                "Vote count must be positive".to_string(),
            ));
        }

        // Quadratic cost: n^2
        let credit_cost = Decimal::from(vote_count) * Decimal::from(vote_count);

        Ok(Self {
            vote_id: Uuid::new_v4(),
            proposal_id,
            user_id,
            vote_count,
            credit_cost,
            in_favor,
            created_at: Utc::now(),
        })
    }

    /// Calculate the cost for a given number of votes
    pub fn calculate_cost(vote_count: u32) -> Decimal {
        Decimal::from(vote_count) * Decimal::from(vote_count)
    }

    /// Calculate how many votes can be purchased with given credits
    pub fn votes_from_credits(credits: Decimal) -> u32 {
        // sqrt(credits) = votes
        let credits_f64 = credits.to_f64().unwrap_or(0.0);
        credits_f64.sqrt() as u32
    }
}

/// User's quadratic voting credits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuadraticVotingCredits {
    pub user_id: Uuid,
    /// Total credits allocated to the user
    pub total_credits: Decimal,
    /// Credits spent on votes
    pub spent_credits: Decimal,
    /// Credits available for voting
    pub available_credits: Decimal,
    /// When credits were last allocated
    pub last_allocation: DateTime<Utc>,
}

impl QuadraticVotingCredits {
    /// Create new voting credits for a user
    pub fn new(user_id: Uuid, total_credits: Decimal) -> Self {
        Self {
            user_id,
            total_credits,
            spent_credits: Decimal::ZERO,
            available_credits: total_credits,
            last_allocation: Utc::now(),
        }
    }

    /// Spend credits on a vote
    pub fn spend(&mut self, amount: Decimal) -> Result<()> {
        if amount > self.available_credits {
            return Err(CoreError::Validation(
                "Insufficient voting credits".to_string(),
            ));
        }

        self.spent_credits += amount;
        self.available_credits -= amount;

        Ok(())
    }

    /// Allocate additional credits
    pub fn allocate(&mut self, amount: Decimal) {
        self.total_credits += amount;
        self.available_credits += amount;
        self.last_allocation = Utc::now();
    }

    /// Refund credits (when vote is cancelled)
    pub fn refund(&mut self, amount: Decimal) {
        self.spent_credits = (self.spent_credits - amount).max(Decimal::ZERO);
        self.available_credits = (self.available_credits + amount).min(self.total_credits);
    }
}

/// Quadratic voting manager
#[derive(Debug, Clone, Default)]
pub struct QuadraticVotingManager {
    votes: HashMap<Uuid, Vec<QuadraticVote>>,
    user_credits: HashMap<Uuid, QuadraticVotingCredits>,
    /// Tracks total votes per proposal (for/against)
    proposal_votes: HashMap<Uuid, (u32, u32)>, // (votes_for, votes_against)
}

impl QuadraticVotingManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate credits to a user
    pub fn allocate_credits(&mut self, user_id: Uuid, credits: Decimal) {
        self.user_credits
            .entry(user_id)
            .and_modify(|c| c.allocate(credits))
            .or_insert_with(|| QuadraticVotingCredits::new(user_id, credits));
    }

    /// Cast a quadratic vote
    pub fn cast_vote(
        &mut self,
        proposal_id: Uuid,
        user_id: Uuid,
        vote_count: u32,
        in_favor: bool,
    ) -> Result<QuadraticVote> {
        // Create vote and calculate cost
        let vote = QuadraticVote::new(proposal_id, user_id, vote_count, in_favor)?;

        // Check if user has enough credits
        let credits = self
            .user_credits
            .get_mut(&user_id)
            .ok_or(CoreError::Validation(
                "User has no voting credits".to_string(),
            ))?;

        // Spend credits
        credits.spend(vote.credit_cost)?;

        // Record vote
        self.votes
            .entry(proposal_id)
            .or_default()
            .push(vote.clone());

        // Update proposal vote totals
        let (for_votes, against_votes) = self.proposal_votes.entry(proposal_id).or_insert((0, 0));
        if in_favor {
            *for_votes += vote_count;
        } else {
            *against_votes += vote_count;
        }

        Ok(vote)
    }

    /// Get user's available credits
    pub fn get_user_credits(&self, user_id: Uuid) -> Option<&QuadraticVotingCredits> {
        self.user_credits.get(&user_id)
    }

    /// Get all votes for a proposal
    pub fn get_proposal_votes(&self, proposal_id: Uuid) -> Vec<&QuadraticVote> {
        self.votes
            .get(&proposal_id)
            .map(|votes| votes.iter().collect())
            .unwrap_or_default()
    }

    /// Get vote totals for a proposal (votes_for, votes_against)
    pub fn get_proposal_totals(&self, proposal_id: Uuid) -> (u32, u32) {
        self.proposal_votes
            .get(&proposal_id)
            .copied()
            .unwrap_or((0, 0))
    }

    /// Calculate quadratic voting result (percentage in favor)
    pub fn calculate_result(&self, proposal_id: Uuid) -> Decimal {
        let (votes_for, votes_against) = self.get_proposal_totals(proposal_id);
        let total_votes = votes_for + votes_against;

        if total_votes == 0 {
            return Decimal::ZERO;
        }

        (Decimal::from(votes_for) / Decimal::from(total_votes)) * Decimal::from(100)
    }

    /// Detect potential collusion (simplified)
    /// Returns users who might be coordinating votes
    pub fn detect_collusion(&self, proposal_id: Uuid) -> Vec<Uuid> {
        let votes = self.get_proposal_votes(proposal_id);
        let mut suspicious_users = Vec::new();

        // Simple heuristic: users who cast exactly the same number of votes
        let mut vote_counts: HashMap<u32, Vec<Uuid>> = HashMap::new();

        for vote in votes {
            vote_counts
                .entry(vote.vote_count)
                .or_default()
                .push(vote.user_id);
        }

        // If 3+ users cast the exact same number of votes, flag them
        for (_count, users) in vote_counts {
            if users.len() >= 3 {
                suspicious_users.extend(users);
            }
        }

        suspicious_users
    }

    /// Calculate Sybil resistance score (0-100)
    /// Higher scores indicate better resistance to Sybil attacks
    pub fn calculate_sybil_score(&self, proposal_id: Uuid) -> Decimal {
        let votes = self.get_proposal_votes(proposal_id);

        if votes.is_empty() {
            return Decimal::ZERO;
        }

        // Calculate Gini coefficient of vote distribution
        // More equal distribution = higher score
        let total_votes: u32 = votes.iter().map(|v| v.vote_count).sum();
        let num_voters = votes.len();

        if total_votes == 0 || num_voters == 0 {
            return Decimal::ZERO;
        }

        let avg_votes = Decimal::from(total_votes) / Decimal::from(num_voters);

        // Calculate variance
        let mut variance = Decimal::ZERO;
        for vote in votes {
            let diff = Decimal::from(vote.vote_count) - avg_votes;
            variance += diff * diff;
        }
        variance /= Decimal::from(num_voters);

        // Lower variance = higher score
        // Normalize to 0-100 scale (simplified)
        let score = Decimal::from(100) / (Decimal::from(1) + variance / Decimal::from(10));

        score.min(Decimal::from(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_vote_delegation() {
        let delegator = Uuid::new_v4();
        let delegate = Uuid::new_v4();

        let delegation = VoteDelegation::new(delegator, delegate, dec!(1000), Some(30)).unwrap();

        assert!(delegation.is_valid());
        assert_eq!(delegation.voting_power, dec!(1000));
    }

    #[test]
    fn test_vote_delegation_revoke() {
        let delegator = Uuid::new_v4();
        let delegate = Uuid::new_v4();

        let mut delegation =
            VoteDelegation::new(delegator, delegate, dec!(1000), Some(30)).unwrap();

        delegation.revoke();
        assert!(!delegation.is_valid());
    }

    #[test]
    fn test_vote_escrow_lock() {
        let user = Uuid::new_v4();
        let lock = VoteEscrowLock::new(user, dec!(1000), 365).unwrap();

        assert!(lock.voting_power > dec!(1000)); // Should have bonus from lock duration
        assert!(!lock.is_expired());
    }

    #[test]
    fn test_vote_escrow_voting_power() {
        let user = Uuid::new_v4();
        let lock = VoteEscrowLock::new(user, dec!(1000), VoteEscrowLock::MAX_LOCK_DAYS).unwrap();

        // Maximum lock should give 2x voting power
        assert!(lock.voting_power >= dec!(1500)); // At least 1.5x
    }

    #[test]
    fn test_conviction_vote() {
        let proposal = Uuid::new_v4();
        let user = Uuid::new_v4();

        let vote = ConvictionVote::new(proposal, user, dec!(1000), true).unwrap();

        let initial_conviction = vote.current_conviction();
        assert_eq!(initial_conviction, dec!(1000)); // Initial conviction equals base power
    }

    #[test]
    fn test_delegation_manager() {
        let mut manager = DelegationManager::new();
        let delegator = Uuid::new_v4();
        let delegate = Uuid::new_v4();

        let delegation_id = manager
            .delegate(delegator, delegate, dec!(1000), Some(30))
            .unwrap();

        let total_power = manager.get_total_voting_power(delegate);
        assert_eq!(total_power, dec!(1000));

        manager.revoke_delegation(delegation_id).unwrap();
        let total_power_after = manager.get_total_voting_power(delegate);
        assert_eq!(total_power_after, dec!(0));
    }

    #[test]
    fn test_vote_escrow_manager() {
        let mut manager = VoteEscrowManager::new(dec!(0.5)).unwrap();
        let user = Uuid::new_v4();

        let lock_id = manager.create_lock(user, dec!(1000), 365).unwrap();
        let voting_power = manager.get_user_voting_power(user);

        assert!(voting_power > dec!(1000));

        // Early unlock should apply penalty
        let unlocked = manager.unlock(lock_id).unwrap();
        assert_eq!(unlocked, dec!(500)); // 50% penalty
    }

    #[test]
    fn test_conviction_voting_manager() {
        let mut manager = ConvictionVotingManager::new();
        let proposal = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        manager
            .cast_vote(proposal, user1, dec!(1000), true)
            .unwrap();
        manager
            .cast_vote(proposal, user2, dec!(500), false)
            .unwrap();

        let (votes_for, votes_against) = manager.get_proposal_conviction(proposal);
        assert_eq!(votes_for, dec!(1000));
        assert_eq!(votes_against, dec!(500));
    }

    #[test]
    fn test_cannot_delegate_to_self() {
        let user = Uuid::new_v4();
        let result = VoteDelegation::new(user, user, dec!(1000), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_vote_escrow_max_lock_validation() {
        let user = Uuid::new_v4();
        let result = VoteEscrowLock::new(user, dec!(1000), VoteEscrowLock::MAX_LOCK_DAYS + 1);
        assert!(result.is_err());
    }
}
