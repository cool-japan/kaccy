//! Token migration system
//!
//! This module implements token migration functionality, allowing users to swap
//! old tokens for new tokens during a migration period.
//!
//! # Use Cases
//!
//! - Token contract upgrades
//! - Fixing critical bugs in token logic
//! - Changing token economics
//! - Merging or splitting tokens
//!
//! # How It Works
//!
//! 1. **Snapshot**: Take a snapshot of all token holders at a specific time
//! 2. **Migration Period**: Allow users to swap old tokens for new tokens
//! 3. **Migration Ratio**: Define the conversion ratio (e.g., 1:1, 1:2, etc.)
//! 4. **Deadline**: Set a deadline for migration
//! 5. **Post-Migration**: Handle unmigrated tokens

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Token migration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Old token ID
    pub old_token_id: Uuid,
    /// New token ID
    pub new_token_id: Uuid,
    /// Migration ratio (old:new)
    /// e.g., 1.0 means 1:1, 2.0 means 1:2 (1 old = 2 new)
    pub migration_ratio: Decimal,
    /// When the migration period starts
    pub start_time: DateTime<Utc>,
    /// When the migration period ends
    pub deadline: DateTime<Utc>,
    /// Whether to allow partial migrations
    pub allow_partial: bool,
    /// Minimum amount to migrate
    pub min_migration_amount: Decimal,
}

impl MigrationConfig {
    /// Create a new migration configuration
    pub fn new(
        old_token_id: Uuid,
        new_token_id: Uuid,
        migration_ratio: Decimal,
        start_time: DateTime<Utc>,
        deadline: DateTime<Utc>,
    ) -> Result<Self> {
        // Validate ratio
        if migration_ratio <= dec!(0) {
            return Err(CoreError::Validation(
                "Migration ratio must be positive".to_string(),
            ));
        }

        // Validate dates
        if deadline <= start_time {
            return Err(CoreError::Validation(
                "Deadline must be after start time".to_string(),
            ));
        }

        Ok(Self {
            old_token_id,
            new_token_id,
            migration_ratio,
            start_time,
            deadline,
            allow_partial: true,
            min_migration_amount: dec!(1),
        })
    }

    /// Check if migration is currently active
    pub fn is_active(&self) -> bool {
        let now = Utc::now();
        now >= self.start_time && now <= self.deadline
    }

    /// Check if migration has ended
    pub fn is_ended(&self) -> bool {
        Utc::now() > self.deadline
    }

    /// Calculate new token amount from old token amount
    pub fn calculate_new_amount(&self, old_amount: Decimal) -> Decimal {
        old_amount * self.migration_ratio
    }
}

/// Status of a token migration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStatus {
    /// Migration not yet started
    Pending,
    /// Migration is active
    Active,
    /// Migration has ended
    Ended,
    /// Migration was cancelled
    Cancelled,
}

/// A snapshot of token holder balances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSnapshot {
    /// Snapshot ID
    pub id: Uuid,
    /// Token ID
    pub token_id: Uuid,
    /// When the snapshot was taken
    pub snapshot_time: DateTime<Utc>,
    /// User balances at snapshot time
    pub balances: HashMap<Uuid, Decimal>,
    /// Total supply at snapshot time
    pub total_supply: Decimal,
}

impl TokenSnapshot {
    /// Create a new snapshot
    pub fn new(token_id: Uuid, balances: HashMap<Uuid, Decimal>) -> Self {
        let total_supply: Decimal = balances.values().sum();

        Self {
            id: Uuid::new_v4(),
            token_id,
            snapshot_time: Utc::now(),
            balances,
            total_supply,
        }
    }

    /// Get balance for a user
    pub fn get_balance(&self, user_id: &Uuid) -> Decimal {
        self.balances.get(user_id).copied().unwrap_or(dec!(0))
    }

    /// Get number of holders
    pub fn holder_count(&self) -> usize {
        self.balances.len()
    }
}

/// A migration record for a single user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// Migration record ID
    pub id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Old token ID
    pub old_token_id: Uuid,
    /// New token ID
    pub new_token_id: Uuid,
    /// Amount of old tokens migrated
    pub old_amount: Decimal,
    /// Amount of new tokens received
    pub new_amount: Decimal,
    /// When the migration occurred
    pub migrated_at: DateTime<Utc>,
    /// Transaction hash (if applicable)
    pub tx_hash: Option<String>,
}

impl MigrationRecord {
    /// Create a new migration record
    pub fn new(
        user_id: Uuid,
        old_token_id: Uuid,
        new_token_id: Uuid,
        old_amount: Decimal,
        new_amount: Decimal,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            old_token_id,
            new_token_id,
            old_amount,
            new_amount,
            migrated_at: Utc::now(),
            tx_hash: None,
        }
    }
}

/// Statistics about a migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStats {
    /// Total old tokens migrated
    pub total_old_migrated: Decimal,
    /// Total new tokens issued
    pub total_new_issued: Decimal,
    /// Number of users who migrated
    pub migrated_users: usize,
    /// Total users eligible
    pub total_eligible: usize,
    /// Migration rate (percentage of users who migrated)
    pub migration_rate_pct: Decimal,
    /// Migration status
    pub status: MigrationStatus,
}

/// Token migration manager
pub struct TokenMigration {
    /// Migration configuration
    pub config: MigrationConfig,
    /// Snapshot of old token balances
    pub snapshot: TokenSnapshot,
    /// Migration records
    pub migrations: Vec<MigrationRecord>,
    /// Users who have migrated
    migrated_users: HashMap<Uuid, Decimal>,
    /// Current status
    status: MigrationStatus,
}

impl TokenMigration {
    /// Create a new token migration
    pub fn new(config: MigrationConfig, snapshot: TokenSnapshot) -> Result<Self> {
        // Verify snapshot is for old token
        if snapshot.token_id != config.old_token_id {
            return Err(CoreError::Validation(
                "Snapshot token ID does not match old token ID".to_string(),
            ));
        }

        let status = if config.is_active() {
            MigrationStatus::Active
        } else if config.is_ended() {
            MigrationStatus::Ended
        } else {
            MigrationStatus::Pending
        };

        Ok(Self {
            config,
            snapshot,
            migrations: Vec::new(),
            migrated_users: HashMap::new(),
            status,
        })
    }

    /// Update migration status based on current time
    pub fn update_status(&mut self) {
        if self.status == MigrationStatus::Cancelled {
            return;
        }

        self.status = if self.config.is_active() {
            MigrationStatus::Active
        } else if self.config.is_ended() {
            MigrationStatus::Ended
        } else {
            MigrationStatus::Pending
        };
    }

    /// Check if a user is eligible to migrate
    pub fn is_eligible(&self, user_id: &Uuid) -> bool {
        self.snapshot.get_balance(user_id) > dec!(0)
    }

    /// Get user's eligible balance
    pub fn get_eligible_balance(&self, user_id: &Uuid) -> Decimal {
        let snapshot_balance = self.snapshot.get_balance(user_id);
        let already_migrated = self.migrated_users.get(user_id).copied().unwrap_or(dec!(0));
        snapshot_balance - already_migrated
    }

    /// Migrate tokens for a user
    pub fn migrate(&mut self, user_id: Uuid, amount: Decimal) -> Result<MigrationRecord> {
        self.update_status();

        // Check status
        if self.status != MigrationStatus::Active {
            return Err(CoreError::InvalidState(format!(
                "Migration is not active (status: {:?})",
                self.status
            )));
        }

        // Check minimum amount
        if amount < self.config.min_migration_amount {
            return Err(CoreError::InvalidOrderQuantity(format!(
                "Amount {} is below minimum {}",
                amount, self.config.min_migration_amount
            )));
        }

        // Check eligibility
        let eligible_amount = self.get_eligible_balance(&user_id);
        if eligible_amount.is_zero() {
            return Err(CoreError::Validation(
                "User has no eligible balance to migrate".to_string(),
            ));
        }

        // Check amount
        if amount > eligible_amount {
            return Err(CoreError::InsufficientBalance {
                required: amount,
                available: eligible_amount,
            });
        }

        // Calculate new token amount
        let new_amount = self.config.calculate_new_amount(amount);

        // Create migration record
        let record = MigrationRecord::new(
            user_id,
            self.config.old_token_id,
            self.config.new_token_id,
            amount,
            new_amount,
        );

        // Update state
        *self.migrated_users.entry(user_id).or_insert(dec!(0)) += amount;
        self.migrations.push(record.clone());

        Ok(record)
    }

    /// Migrate all eligible tokens for a user
    pub fn migrate_all(&mut self, user_id: Uuid) -> Result<MigrationRecord> {
        let eligible_amount = self.get_eligible_balance(&user_id);
        self.migrate(user_id, eligible_amount)
    }

    /// Get migration statistics
    pub fn stats(&self) -> MigrationStats {
        let total_old_migrated: Decimal = self.migrations.iter().map(|m| m.old_amount).sum();
        let total_new_issued: Decimal = self.migrations.iter().map(|m| m.new_amount).sum();
        let migrated_users = self.migrated_users.len();
        let total_eligible = self.snapshot.holder_count();

        let migration_rate_pct = if total_eligible > 0 {
            (Decimal::from(migrated_users) / Decimal::from(total_eligible)) * dec!(100)
        } else {
            dec!(0)
        };

        MigrationStats {
            total_old_migrated,
            total_new_issued,
            migrated_users,
            total_eligible,
            migration_rate_pct,
            status: self.status,
        }
    }

    /// Get migrations for a specific user
    pub fn get_user_migrations(&self, user_id: &Uuid) -> Vec<&MigrationRecord> {
        self.migrations
            .iter()
            .filter(|m| &m.user_id == user_id)
            .collect()
    }

    /// Cancel the migration (can only be done before it starts)
    pub fn cancel(&mut self) -> Result<()> {
        if self.status != MigrationStatus::Pending {
            return Err(CoreError::InvalidState(
                "Can only cancel pending migrations".to_string(),
            ));
        }

        self.status = MigrationStatus::Cancelled;
        Ok(())
    }

    /// Get unmigrated balances
    pub fn get_unmigrated_balances(&self) -> HashMap<Uuid, Decimal> {
        self.snapshot
            .balances
            .iter()
            .map(|(user_id, balance)| {
                let migrated = self.migrated_users.get(user_id).copied().unwrap_or(dec!(0));
                (*user_id, *balance - migrated)
            })
            .filter(|(_, remaining)| *remaining > dec!(0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_migration_config_creation() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let start = Utc::now();
        let deadline = start + Duration::days(30);

        let config = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline).unwrap();

        assert_eq!(config.old_token_id, old_token);
        assert_eq!(config.new_token_id, new_token);
        assert_eq!(config.migration_ratio, dec!(1));
    }

    #[test]
    fn test_migration_config_invalid_ratio() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let start = Utc::now();
        let deadline = start + Duration::days(30);

        let result = MigrationConfig::new(old_token, new_token, dec!(0), start, deadline);
        assert!(result.is_err());
    }

    #[test]
    fn test_migration_config_invalid_dates() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let start = Utc::now();
        let deadline = start - Duration::days(1); // Deadline before start

        let result = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_new_amount() {
        let config = MigrationConfig {
            old_token_id: Uuid::new_v4(),
            new_token_id: Uuid::new_v4(),
            migration_ratio: dec!(2),
            start_time: Utc::now(),
            deadline: Utc::now() + Duration::days(30),
            allow_partial: true,
            min_migration_amount: dec!(1),
        };

        assert_eq!(config.calculate_new_amount(dec!(100)), dec!(200));
    }

    #[test]
    fn test_snapshot_creation() {
        let token_id = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));
        balances.insert(user2, dec!(200));

        let snapshot = TokenSnapshot::new(token_id, balances);

        assert_eq!(snapshot.total_supply, dec!(300));
        assert_eq!(snapshot.holder_count(), 2);
        assert_eq!(snapshot.get_balance(&user1), dec!(100));
        assert_eq!(snapshot.get_balance(&user2), dec!(200));
    }

    #[test]
    fn test_token_migration() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));
        balances.insert(user2, dec!(200));

        let snapshot = TokenSnapshot::new(old_token, balances);

        let start = Utc::now() - Duration::days(1); // Started yesterday
        let deadline = Utc::now() + Duration::days(29); // Ends in 29 days
        let config = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline).unwrap();

        let mut migration = TokenMigration::new(config, snapshot).unwrap();

        // Migrate user1's tokens
        let record = migration.migrate(user1, dec!(100)).unwrap();
        assert_eq!(record.old_amount, dec!(100));
        assert_eq!(record.new_amount, dec!(100));

        // Check stats
        let stats = migration.stats();
        assert_eq!(stats.total_old_migrated, dec!(100));
        assert_eq!(stats.migrated_users, 1);
    }

    #[test]
    fn test_migration_ratio() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let user1 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));

        let snapshot = TokenSnapshot::new(old_token, balances);

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = MigrationConfig::new(old_token, new_token, dec!(2), start, deadline).unwrap(); // 1:2 ratio

        let mut migration = TokenMigration::new(config, snapshot).unwrap();

        let record = migration.migrate(user1, dec!(100)).unwrap();
        assert_eq!(record.old_amount, dec!(100));
        assert_eq!(record.new_amount, dec!(200)); // 1:2 ratio
    }

    #[test]
    fn test_partial_migration() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let user1 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));

        let snapshot = TokenSnapshot::new(old_token, balances);

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline).unwrap();

        let mut migration = TokenMigration::new(config, snapshot).unwrap();

        // Migrate part of the balance
        migration.migrate(user1, dec!(60)).unwrap();

        // Check remaining eligible balance
        assert_eq!(migration.get_eligible_balance(&user1), dec!(40));

        // Migrate the rest
        migration.migrate(user1, dec!(40)).unwrap();

        assert_eq!(migration.get_eligible_balance(&user1), dec!(0));
    }

    #[test]
    fn test_migration_all() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let user1 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));

        let snapshot = TokenSnapshot::new(old_token, balances);

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline).unwrap();

        let mut migration = TokenMigration::new(config, snapshot).unwrap();

        let record = migration.migrate_all(user1).unwrap();
        assert_eq!(record.old_amount, dec!(100));
        assert_eq!(migration.get_eligible_balance(&user1), dec!(0));
    }

    #[test]
    fn test_unmigrated_balances() {
        let old_token = Uuid::new_v4();
        let new_token = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let mut balances = HashMap::new();
        balances.insert(user1, dec!(100));
        balances.insert(user2, dec!(200));

        let snapshot = TokenSnapshot::new(old_token, balances);

        let start = Utc::now() - Duration::days(1);
        let deadline = Utc::now() + Duration::days(29);
        let config = MigrationConfig::new(old_token, new_token, dec!(1), start, deadline).unwrap();

        let mut migration = TokenMigration::new(config, snapshot).unwrap();

        // Only user1 migrates
        migration.migrate(user1, dec!(100)).unwrap();

        let unmigrated = migration.get_unmigrated_balances();
        assert_eq!(unmigrated.len(), 1);
        assert_eq!(unmigrated.get(&user2), Some(&dec!(200)));
    }
}
