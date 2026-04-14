//! Insurance and protocol protection system
//!
//! This module provides insurance pools for protecting users against protocol exploits,
//! smart contract vulnerabilities, and other risks.

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of an insurance claim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimStatus {
    /// Claim submitted and pending review
    Pending,
    /// Claim is under investigation
    UnderReview,
    /// Claim approved for payout
    Approved,
    /// Claim rejected
    Rejected,
    /// Payout completed
    Paid,
}

/// Type of insurance coverage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageType {
    /// Smart contract exploit protection
    SmartContractExploit,
    /// Oracle manipulation protection
    OracleManipulation,
    /// Economic attack protection (e.g., flash loan attacks)
    EconomicAttack,
    /// Slippage protection
    SlippageProtection,
    /// Liquidation protection
    LiquidationProtection,
}

/// Insurance claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsuranceClaim {
    /// Unique identifier for this claim
    pub claim_id: Uuid,
    /// User who filed the claim
    pub user_id: Uuid,
    /// Type of coverage being claimed
    pub coverage_type: CoverageType,
    /// Amount the user is claiming
    pub amount_claimed: Decimal,
    /// Detailed description of the loss event
    pub description: String,
    /// Evidence supporting the claim (e.g., transaction hashes)
    pub evidence: Vec<String>,
    /// Current status of this claim
    pub status: ClaimStatus,
    /// When this claim was submitted
    pub submitted_at: DateTime<Utc>,
    /// When this claim was reviewed
    pub reviewed_at: Option<DateTime<Utc>>,
    /// Approved payout amount (set on approval)
    pub payout_amount: Option<Decimal>,
    /// Reason the claim was rejected (set on rejection)
    pub rejection_reason: Option<String>,
}

impl InsuranceClaim {
    /// Create a new insurance claim
    pub fn new(
        user_id: Uuid,
        coverage_type: CoverageType,
        amount_claimed: Decimal,
        description: String,
        evidence: Vec<String>,
    ) -> Result<Self> {
        if amount_claimed <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Claim amount must be positive".to_string(),
            ));
        }

        Ok(Self {
            claim_id: Uuid::new_v4(),
            user_id,
            coverage_type,
            amount_claimed,
            description,
            evidence,
            status: ClaimStatus::Pending,
            submitted_at: Utc::now(),
            reviewed_at: None,
            payout_amount: None,
            rejection_reason: None,
        })
    }

    /// Approve claim with payout amount
    pub fn approve(&mut self, payout_amount: Decimal) -> Result<()> {
        if self.status != ClaimStatus::Pending && self.status != ClaimStatus::UnderReview {
            return Err(CoreError::InvalidState(
                "Claim must be pending or under review to approve".to_string(),
            ));
        }

        if payout_amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Payout amount must be positive".to_string(),
            ));
        }

        if payout_amount > self.amount_claimed {
            return Err(CoreError::Validation(
                "Payout cannot exceed claimed amount".to_string(),
            ));
        }

        self.status = ClaimStatus::Approved;
        self.payout_amount = Some(payout_amount);
        self.reviewed_at = Some(Utc::now());
        Ok(())
    }

    /// Reject claim with reason
    pub fn reject(&mut self, reason: String) -> Result<()> {
        if self.status != ClaimStatus::Pending && self.status != ClaimStatus::UnderReview {
            return Err(CoreError::InvalidState(
                "Claim must be pending or under review to reject".to_string(),
            ));
        }

        self.status = ClaimStatus::Rejected;
        self.rejection_reason = Some(reason);
        self.reviewed_at = Some(Utc::now());
        Ok(())
    }

    /// Mark claim as paid
    pub fn mark_paid(&mut self) -> Result<()> {
        if self.status != ClaimStatus::Approved {
            return Err(CoreError::InvalidState(
                "Claim must be approved before marking as paid".to_string(),
            ));
        }

        self.status = ClaimStatus::Paid;
        Ok(())
    }
}

/// Insurance pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsurancePoolConfig {
    /// Premium rate as percentage of covered amount (e.g., 0.01 for 1%)
    pub premium_rate: Decimal,
    /// Maximum coverage per user
    pub max_coverage_per_user: Decimal,
    /// Maximum total pool coverage
    pub max_total_coverage: Decimal,
    /// Minimum claim amount
    pub min_claim_amount: Decimal,
    /// Coverage duration in days
    pub coverage_duration_days: u32,
}

impl Default for InsurancePoolConfig {
    fn default() -> Self {
        Self {
            premium_rate: Decimal::from_str("0.01").unwrap(), // 1%
            max_coverage_per_user: Decimal::from_str("100000").unwrap(),
            max_total_coverage: Decimal::from_str("10000000").unwrap(),
            min_claim_amount: Decimal::from_str("100").unwrap(),
            coverage_duration_days: 365,
        }
    }
}

/// Insurance coverage policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePolicy {
    /// Unique identifier for this policy
    pub policy_id: Uuid,
    /// Policy holder
    pub user_id: Uuid,
    /// Type of risk covered by this policy
    pub coverage_type: CoverageType,
    /// Maximum amount this policy will pay out
    pub coverage_amount: Decimal,
    /// Premium amount paid for this policy
    pub premium_paid: Decimal,
    /// When this policy becomes active
    pub start_date: DateTime<Utc>,
    /// When this policy expires
    pub end_date: DateTime<Utc>,
    /// Whether this policy has been cancelled
    pub is_active: bool,
}

impl CoveragePolicy {
    /// Create a new coverage policy for the given user and risk type
    pub fn new(
        user_id: Uuid,
        coverage_type: CoverageType,
        coverage_amount: Decimal,
        premium_paid: Decimal,
        duration_days: u32,
    ) -> Result<Self> {
        if coverage_amount <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Coverage amount must be positive".to_string(),
            ));
        }

        if premium_paid < Decimal::ZERO {
            return Err(CoreError::Validation(
                "Premium cannot be negative".to_string(),
            ));
        }

        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(duration_days as i64);

        Ok(Self {
            policy_id: Uuid::new_v4(),
            user_id,
            coverage_type,
            coverage_amount,
            premium_paid,
            start_date,
            end_date,
            is_active: true,
        })
    }

    /// Check if policy is currently valid
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        self.is_active && now >= self.start_date && now <= self.end_date
    }

    /// Cancel policy
    pub fn cancel(&mut self) {
        self.is_active = false;
    }
}

/// Insurance pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsurancePoolStats {
    /// Total premiums collected since pool inception
    pub total_premiums_collected: Decimal,
    /// Total claim payouts made since pool inception
    pub total_claims_paid: Decimal,
    /// Sum of all active policy coverage amounts
    pub total_active_coverage: Decimal,
    /// Total number of claims ever submitted
    pub total_claims: usize,
    /// Number of claims that were approved
    pub approved_claims: usize,
    /// Number of claims that were rejected
    pub rejected_claims: usize,
    /// Number of claims awaiting review
    pub pending_claims: usize,
    /// Current funds available in the pool
    pub pool_balance: Decimal,
}

impl Default for InsurancePoolStats {
    fn default() -> Self {
        Self {
            total_premiums_collected: Decimal::ZERO,
            total_claims_paid: Decimal::ZERO,
            total_active_coverage: Decimal::ZERO,
            total_claims: 0,
            approved_claims: 0,
            rejected_claims: 0,
            pending_claims: 0,
            pool_balance: Decimal::ZERO,
        }
    }
}

/// Insurance pool manager
pub struct InsurancePool {
    /// Pool configuration
    config: InsurancePoolConfig,
    /// Active coverage policies indexed by policy ID
    policies: HashMap<Uuid, CoveragePolicy>,
    /// Claims indexed by claim ID
    claims: HashMap<Uuid, InsuranceClaim>,
    /// Aggregated pool statistics
    stats: InsurancePoolStats,
    /// Total active coverage amount per user
    user_coverage: HashMap<Uuid, Decimal>,
}

impl InsurancePool {
    /// Create a new insurance pool with the given configuration
    pub fn new(config: InsurancePoolConfig) -> Self {
        Self {
            config,
            policies: HashMap::new(),
            claims: HashMap::new(),
            stats: InsurancePoolStats::default(),
            user_coverage: HashMap::new(),
        }
    }

    /// Calculate premium for coverage
    pub fn calculate_premium(&self, coverage_amount: Decimal) -> Decimal {
        coverage_amount * self.config.premium_rate
    }

    /// Purchase insurance coverage
    pub fn purchase_coverage(
        &mut self,
        user_id: Uuid,
        coverage_type: CoverageType,
        coverage_amount: Decimal,
    ) -> Result<Uuid> {
        if coverage_amount < self.config.min_claim_amount {
            return Err(CoreError::Validation(format!(
                "Coverage amount must be at least {}",
                self.config.min_claim_amount
            )));
        }

        // Check per-user coverage limit
        let current_coverage = self
            .user_coverage
            .get(&user_id)
            .copied()
            .unwrap_or(Decimal::ZERO);
        if current_coverage + coverage_amount > self.config.max_coverage_per_user {
            return Err(CoreError::Validation(format!(
                "Exceeds maximum coverage per user: {}",
                self.config.max_coverage_per_user
            )));
        }

        // Check total pool coverage limit
        if self.stats.total_active_coverage + coverage_amount > self.config.max_total_coverage {
            return Err(CoreError::InsufficientLiquidity(
                "Insurance pool capacity exceeded".to_string(),
            ));
        }

        let premium = self.calculate_premium(coverage_amount);
        let policy = CoveragePolicy::new(
            user_id,
            coverage_type,
            coverage_amount,
            premium,
            self.config.coverage_duration_days,
        )?;

        let policy_id = policy.policy_id;

        // Update stats
        self.stats.total_premiums_collected += premium;
        self.stats.total_active_coverage += coverage_amount;
        self.stats.pool_balance += premium;

        // Update user coverage
        *self.user_coverage.entry(user_id).or_insert(Decimal::ZERO) += coverage_amount;

        self.policies.insert(policy_id, policy);
        Ok(policy_id)
    }

    /// Submit insurance claim
    pub fn submit_claim(
        &mut self,
        user_id: Uuid,
        coverage_type: CoverageType,
        amount_claimed: Decimal,
        description: String,
        evidence: Vec<String>,
    ) -> Result<Uuid> {
        if amount_claimed < self.config.min_claim_amount {
            return Err(CoreError::Validation(format!(
                "Claim amount must be at least {}",
                self.config.min_claim_amount
            )));
        }

        // Verify user has active coverage of this type
        let has_coverage = self
            .policies
            .values()
            .any(|p| p.user_id == user_id && p.coverage_type == coverage_type && p.is_valid());

        if !has_coverage {
            return Err(CoreError::Validation(
                "No active coverage found for this claim type".to_string(),
            ));
        }

        let claim = InsuranceClaim::new(
            user_id,
            coverage_type,
            amount_claimed,
            description,
            evidence,
        )?;
        let claim_id = claim.claim_id;

        self.stats.total_claims += 1;
        self.stats.pending_claims += 1;

        self.claims.insert(claim_id, claim);
        Ok(claim_id)
    }

    /// Approve a claim
    pub fn approve_claim(&mut self, claim_id: Uuid, payout_amount: Decimal) -> Result<()> {
        let claim = self
            .claims
            .get_mut(&claim_id)
            .ok_or_else(|| CoreError::NotFound("Claim not found".to_string()))?;

        // Check if pool has sufficient balance
        if payout_amount > self.stats.pool_balance {
            return Err(CoreError::InsufficientBalance {
                required: payout_amount,
                available: self.stats.pool_balance,
            });
        }

        claim.approve(payout_amount)?;

        // Update stats
        self.stats.pending_claims = self.stats.pending_claims.saturating_sub(1);
        self.stats.approved_claims += 1;

        Ok(())
    }

    /// Reject a claim
    pub fn reject_claim(&mut self, claim_id: Uuid, reason: String) -> Result<()> {
        let claim = self
            .claims
            .get_mut(&claim_id)
            .ok_or_else(|| CoreError::NotFound("Claim not found".to_string()))?;

        claim.reject(reason)?;

        // Update stats
        self.stats.pending_claims = self.stats.pending_claims.saturating_sub(1);
        self.stats.rejected_claims += 1;

        Ok(())
    }

    /// Process approved claim payout
    pub fn payout_claim(&mut self, claim_id: Uuid) -> Result<Decimal> {
        let claim = self
            .claims
            .get_mut(&claim_id)
            .ok_or_else(|| CoreError::NotFound("Claim not found".to_string()))?;

        if claim.status != ClaimStatus::Approved {
            return Err(CoreError::InvalidState(
                "Claim must be approved before payout".to_string(),
            ));
        }

        let payout_amount = claim.payout_amount.unwrap();

        // Check balance again
        if payout_amount > self.stats.pool_balance {
            return Err(CoreError::InsufficientBalance {
                required: payout_amount,
                available: self.stats.pool_balance,
            });
        }

        claim.mark_paid()?;

        // Update stats
        self.stats.total_claims_paid += payout_amount;
        self.stats.pool_balance -= payout_amount;

        Ok(payout_amount)
    }

    /// Get claim by ID
    pub fn get_claim(&self, claim_id: Uuid) -> Option<&InsuranceClaim> {
        self.claims.get(&claim_id)
    }

    /// Get all claims for a user
    pub fn get_user_claims(&self, user_id: Uuid) -> Vec<&InsuranceClaim> {
        self.claims
            .values()
            .filter(|c| c.user_id == user_id)
            .collect()
    }

    /// Get policy by ID
    pub fn get_policy(&self, policy_id: Uuid) -> Option<&CoveragePolicy> {
        self.policies.get(&policy_id)
    }

    /// Get all active policies for a user
    pub fn get_user_policies(&self, user_id: Uuid) -> Vec<&CoveragePolicy> {
        self.policies
            .values()
            .filter(|p| p.user_id == user_id && p.is_valid())
            .collect()
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> &InsurancePoolStats {
        &self.stats
    }

    /// Add funds to pool (e.g., from protocol fees)
    pub fn add_funds(&mut self, amount: Decimal) -> Result<()> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::Validation("Amount must be positive".to_string()));
        }

        self.stats.pool_balance += amount;
        Ok(())
    }

    /// Clean up expired policies
    pub fn cleanup_expired_policies(&mut self) -> usize {
        let mut removed_coverage = Decimal::ZERO;
        let mut updated_users: HashMap<Uuid, Decimal> = HashMap::new();

        // Mark expired policies as inactive
        for policy in self.policies.values_mut() {
            if policy.is_active && !policy.is_valid() {
                policy.cancel();
                removed_coverage += policy.coverage_amount;
                *updated_users.entry(policy.user_id).or_insert(Decimal::ZERO) +=
                    policy.coverage_amount;
            }
        }

        // Update user coverage totals
        for (user_id, amount) in updated_users {
            if let Some(coverage) = self.user_coverage.get_mut(&user_id) {
                *coverage = (*coverage - amount).max(Decimal::ZERO);
            }
        }

        // Update total active coverage
        self.stats.total_active_coverage =
            (self.stats.total_active_coverage - removed_coverage).max(Decimal::ZERO);

        removed_coverage.to_usize().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_insurance_claim_creation() {
        let user_id = Uuid::new_v4();
        let claim = InsuranceClaim::new(
            user_id,
            CoverageType::SmartContractExploit,
            dec!(1000),
            "Lost funds due to exploit".to_string(),
            vec!["tx_hash_123".to_string()],
        );

        assert!(claim.is_ok());
        let claim = claim.unwrap();
        assert_eq!(claim.status, ClaimStatus::Pending);
        assert_eq!(claim.amount_claimed, dec!(1000));
    }

    #[test]
    fn test_claim_approval() {
        let user_id = Uuid::new_v4();
        let mut claim = InsuranceClaim::new(
            user_id,
            CoverageType::SmartContractExploit,
            dec!(1000),
            "Lost funds".to_string(),
            vec![],
        )
        .unwrap();

        let result = claim.approve(dec!(800));
        assert!(result.is_ok());
        assert_eq!(claim.status, ClaimStatus::Approved);
        assert_eq!(claim.payout_amount, Some(dec!(800)));
    }

    #[test]
    fn test_claim_rejection() {
        let user_id = Uuid::new_v4();
        let mut claim = InsuranceClaim::new(
            user_id,
            CoverageType::SmartContractExploit,
            dec!(1000),
            "Lost funds".to_string(),
            vec![],
        )
        .unwrap();

        let result = claim.reject("Insufficient evidence".to_string());
        assert!(result.is_ok());
        assert_eq!(claim.status, ClaimStatus::Rejected);
        assert!(claim.rejection_reason.is_some());
    }

    #[test]
    fn test_coverage_policy_creation() {
        let user_id = Uuid::new_v4();
        let policy = CoveragePolicy::new(
            user_id,
            CoverageType::SmartContractExploit,
            dec!(10000),
            dec!(100),
            365,
        );

        assert!(policy.is_ok());
        let policy = policy.unwrap();
        assert!(policy.is_valid());
        assert_eq!(policy.coverage_amount, dec!(10000));
    }

    #[test]
    fn test_insurance_pool_purchase_coverage() {
        let config = InsurancePoolConfig::default();
        let mut pool = InsurancePool::new(config);
        let user_id = Uuid::new_v4();

        let result =
            pool.purchase_coverage(user_id, CoverageType::SmartContractExploit, dec!(10000));

        assert!(result.is_ok());
        let stats = pool.get_stats();
        assert_eq!(stats.total_active_coverage, dec!(10000));
        assert!(stats.total_premiums_collected > dec!(0));
    }

    #[test]
    fn test_insurance_pool_submit_claim() {
        let config = InsurancePoolConfig::default();
        let mut pool = InsurancePool::new(config);
        let user_id = Uuid::new_v4();

        // Purchase coverage first
        pool.purchase_coverage(user_id, CoverageType::SmartContractExploit, dec!(10000))
            .unwrap();

        // Submit claim
        let result = pool.submit_claim(
            user_id,
            CoverageType::SmartContractExploit,
            dec!(1000),
            "Exploit occurred".to_string(),
            vec!["evidence".to_string()],
        );

        assert!(result.is_ok());
        let stats = pool.get_stats();
        assert_eq!(stats.total_claims, 1);
        assert_eq!(stats.pending_claims, 1);
    }

    #[test]
    fn test_insurance_pool_claim_workflow() {
        let config = InsurancePoolConfig::default();
        let mut pool = InsurancePool::new(config);
        let user_id = Uuid::new_v4();

        // Add funds to pool to cover potential payouts
        pool.add_funds(dec!(5000)).unwrap();

        // Purchase coverage
        pool.purchase_coverage(user_id, CoverageType::SmartContractExploit, dec!(10000))
            .unwrap();

        // Submit claim
        let claim_id = pool
            .submit_claim(
                user_id,
                CoverageType::SmartContractExploit,
                dec!(1000),
                "Exploit".to_string(),
                vec![],
            )
            .unwrap();

        // Approve claim
        pool.approve_claim(claim_id, dec!(800)).unwrap();

        // Payout claim
        let payout = pool.payout_claim(claim_id).unwrap();
        assert_eq!(payout, dec!(800));

        let stats = pool.get_stats();
        assert_eq!(stats.total_claims_paid, dec!(800));
        assert_eq!(stats.approved_claims, 1);
    }

    #[test]
    fn test_premium_calculation() {
        let config = InsurancePoolConfig::default();
        let pool = InsurancePool::new(config);

        let premium = pool.calculate_premium(dec!(10000));
        assert_eq!(premium, dec!(100)); // 1% of 10000
    }

    #[test]
    fn test_coverage_limits() {
        let config = InsurancePoolConfig {
            max_coverage_per_user: dec!(1000),
            ..Default::default()
        };
        let mut pool = InsurancePool::new(config);
        let user_id = Uuid::new_v4();

        // Should fail - exceeds per-user limit
        let result =
            pool.purchase_coverage(user_id, CoverageType::SmartContractExploit, dec!(2000));

        assert!(result.is_err());
    }
}
