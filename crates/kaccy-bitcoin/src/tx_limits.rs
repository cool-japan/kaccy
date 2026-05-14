//! Transaction Limits Module
//!
//! This module provides transaction limit enforcement including per-user
//! daily/monthly limits and platform-wide limits for security.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{BitcoinError, Result};

/// Limit period
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimitPeriod {
    /// Hourly limit
    Hourly,
    /// Daily limit
    Daily,
    /// Weekly limit
    Weekly,
    /// Monthly limit
    Monthly,
}

impl LimitPeriod {
    /// Get duration for this period
    pub fn duration(&self) -> Duration {
        match self {
            LimitPeriod::Hourly => Duration::hours(1),
            LimitPeriod::Daily => Duration::days(1),
            LimitPeriod::Weekly => Duration::weeks(1),
            LimitPeriod::Monthly => Duration::days(30),
        }
    }

    /// Get period start time
    pub fn period_start(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            LimitPeriod::Hourly => now - Duration::hours(1),
            LimitPeriod::Daily => now - Duration::days(1),
            LimitPeriod::Weekly => now - Duration::weeks(1),
            LimitPeriod::Monthly => now - Duration::days(30),
        }
    }
}

/// Transaction limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLimit {
    /// Maximum amount in satoshis
    pub max_amount_sats: u64,
    /// Maximum number of transactions
    pub max_count: u32,
    /// Limit period
    pub period: LimitPeriod,
}

/// Limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitConfig {
    /// Per-user limits
    pub user_limits: Vec<TransactionLimit>,
    /// Platform-wide limits
    pub platform_limits: Vec<TransactionLimit>,
    /// Single transaction maximum (instant rejection)
    pub single_tx_max_sats: u64,
}

impl Default for LimitConfig {
    fn default() -> Self {
        Self {
            user_limits: vec![
                TransactionLimit {
                    max_amount_sats: 10_000_000, // 0.1 BTC per day
                    max_count: 10,
                    period: LimitPeriod::Daily,
                },
                TransactionLimit {
                    max_amount_sats: 50_000_000, // 0.5 BTC per month
                    max_count: 100,
                    period: LimitPeriod::Monthly,
                },
            ],
            platform_limits: vec![
                TransactionLimit {
                    max_amount_sats: 100_000_000, // 1 BTC per hour platform-wide
                    max_count: 100,
                    period: LimitPeriod::Hourly,
                },
                TransactionLimit {
                    max_amount_sats: 1_000_000_000, // 10 BTC per day platform-wide
                    max_count: 1000,
                    period: LimitPeriod::Daily,
                },
            ],
            single_tx_max_sats: 50_000_000, // 0.5 BTC max per transaction
        }
    }
}

/// Usage record for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// User ID
    pub user_id: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Transaction ID (if available)
    pub txid: Option<String>,
}

/// Limit violation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitViolation {
    /// Type of limit violated
    pub limit_type: String,
    /// Period
    pub period: LimitPeriod,
    /// Current usage
    pub current_usage_sats: u64,
    /// Limit amount
    pub limit_sats: u64,
    /// Current transaction count
    pub current_count: u32,
    /// Max count
    pub max_count: u32,
}

/// Limit enforcer for transaction limits
pub struct LimitEnforcer {
    config: LimitConfig,
    /// User usage records
    user_usage: Arc<RwLock<HashMap<String, Vec<UsageRecord>>>>,
    /// Platform usage records
    platform_usage: Arc<RwLock<Vec<UsageRecord>>>,
}

impl LimitEnforcer {
    /// Create a new limit enforcer
    pub fn new(config: LimitConfig) -> Self {
        Self {
            config,
            user_usage: Arc::new(RwLock::new(HashMap::new())),
            platform_usage: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if a transaction is allowed
    pub async fn check_transaction(&self, user_id: &str, amount_sats: u64) -> Result<()> {
        // Check single transaction maximum
        if amount_sats > self.config.single_tx_max_sats {
            return Err(BitcoinError::LimitExceeded(format!(
                "Transaction amount {} sats exceeds single transaction limit of {} sats",
                amount_sats, self.config.single_tx_max_sats
            )));
        }

        // Check user limits
        self.check_user_limits(user_id, amount_sats).await?;

        // Check platform limits
        self.check_platform_limits(amount_sats).await?;

        Ok(())
    }

    /// Record a transaction
    pub async fn record_transaction(&self, user_id: &str, amount_sats: u64, txid: Option<String>) {
        let record = UsageRecord {
            user_id: user_id.to_string(),
            amount_sats,
            timestamp: Utc::now(),
            txid,
        };

        // Record user usage
        let mut user_usage = self.user_usage.write().await;
        user_usage
            .entry(user_id.to_string())
            .or_insert_with(Vec::new)
            .push(record.clone());

        // Record platform usage
        let mut platform_usage = self.platform_usage.write().await;
        platform_usage.push(record);

        // Cleanup old records
        drop(user_usage);
        drop(platform_usage);
        self.cleanup_old_records().await;
    }

    /// Get user usage for a period
    pub async fn get_user_usage(&self, user_id: &str, period: LimitPeriod) -> (u64, u32) {
        let user_usage = self.user_usage.read().await;
        let records = user_usage.get(user_id);

        if records.is_none() {
            return (0, 0);
        }

        let now = Utc::now();
        let period_start = period.period_start(now);

        let filtered: Vec<_> = records
            .expect("records is Some per is_none check above")
            .iter()
            .filter(|r| r.timestamp >= period_start)
            .collect();

        let total_amount: u64 = filtered.iter().map(|r| r.amount_sats).sum();
        let count = filtered.len() as u32;

        (total_amount, count)
    }

    /// Get platform usage for a period
    pub async fn get_platform_usage(&self, period: LimitPeriod) -> (u64, u32) {
        let platform_usage = self.platform_usage.read().await;

        let now = Utc::now();
        let period_start = period.period_start(now);

        let filtered: Vec<_> = platform_usage
            .iter()
            .filter(|r| r.timestamp >= period_start)
            .collect();

        let total_amount: u64 = filtered.iter().map(|r| r.amount_sats).sum();
        let count = filtered.len() as u32;

        (total_amount, count)
    }

    /// Check user limits
    async fn check_user_limits(&self, user_id: &str, amount_sats: u64) -> Result<()> {
        for limit in &self.config.user_limits {
            let (current_usage, current_count) = self.get_user_usage(user_id, limit.period).await;

            // Check amount limit
            if current_usage + amount_sats > limit.max_amount_sats {
                return Err(BitcoinError::LimitExceeded(format!(
                    "User {:?} limit exceeded for amount: {} + {} > {} sats",
                    limit.period, current_usage, amount_sats, limit.max_amount_sats
                )));
            }

            // Check count limit
            if current_count + 1 > limit.max_count {
                return Err(BitcoinError::LimitExceeded(format!(
                    "User {:?} limit exceeded for count: {} + 1 > {}",
                    limit.period, current_count, limit.max_count
                )));
            }
        }

        Ok(())
    }

    /// Check platform limits
    async fn check_platform_limits(&self, amount_sats: u64) -> Result<()> {
        for limit in &self.config.platform_limits {
            let (current_usage, current_count) = self.get_platform_usage(limit.period).await;

            // Check amount limit
            if current_usage + amount_sats > limit.max_amount_sats {
                return Err(BitcoinError::LimitExceeded(format!(
                    "Platform {:?} limit exceeded for amount: {} + {} > {} sats",
                    limit.period, current_usage, amount_sats, limit.max_amount_sats
                )));
            }

            // Check count limit
            if current_count + 1 > limit.max_count {
                return Err(BitcoinError::LimitExceeded(format!(
                    "Platform {:?} limit exceeded for count: {} + 1 > {}",
                    limit.period, current_count, limit.max_count
                )));
            }
        }

        Ok(())
    }

    /// Cleanup old records (older than the longest period)
    async fn cleanup_old_records(&self) {
        let now = Utc::now();
        let max_period = Duration::days(30); // Monthly is the longest
        let cutoff = now - max_period;

        // Cleanup user records
        let mut user_usage = self.user_usage.write().await;
        for records in user_usage.values_mut() {
            records.retain(|r| r.timestamp >= cutoff);
        }

        // Cleanup platform records
        let mut platform_usage = self.platform_usage.write().await;
        platform_usage.retain(|r| r.timestamp >= cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limit_period_duration() {
        assert_eq!(LimitPeriod::Hourly.duration(), Duration::hours(1));
        assert_eq!(LimitPeriod::Daily.duration(), Duration::days(1));
        assert_eq!(LimitPeriod::Weekly.duration(), Duration::weeks(1));
        assert_eq!(LimitPeriod::Monthly.duration(), Duration::days(30));
    }

    #[test]
    fn test_limit_config_defaults() {
        let config = LimitConfig::default();
        assert_eq!(config.user_limits.len(), 2);
        assert_eq!(config.platform_limits.len(), 2);
        assert_eq!(config.single_tx_max_sats, 50_000_000);
    }

    #[tokio::test]
    async fn test_single_tx_limit() {
        let enforcer = LimitEnforcer::new(LimitConfig::default());

        // Should fail - exceeds single tx limit
        let result = enforcer.check_transaction("user1", 100_000_000).await;
        assert!(result.is_err());

        // Should succeed
        let result = enforcer.check_transaction("user1", 1_000_000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let enforcer = LimitEnforcer::new(LimitConfig::default());

        // Record some transactions
        enforcer.record_transaction("user1", 1_000_000, None).await;
        enforcer.record_transaction("user1", 2_000_000, None).await;

        // Check usage
        let (usage, count) = enforcer.get_user_usage("user1", LimitPeriod::Daily).await;
        assert_eq!(usage, 3_000_000);
        assert_eq!(count, 2);
    }
}
