//! Migration utilities for tracking and managing database schema versions
//!
//! This module provides utilities for checking migration status, validating
//! schema integrity, and managing database migrations programmatically.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Information about a database migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationInfo {
    /// Migration version number
    pub version: i64,
    /// Migration description/name
    pub description: String,
    /// When the migration was installed
    pub installed_on: DateTime<Utc>,
    /// Execution time in milliseconds
    pub execution_time: i64,
    /// Whether the migration was successful
    pub success: bool,
    /// Checksum of the migration file
    pub checksum: Option<String>,
}

/// Summary of migration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStatus {
    /// Current schema version
    pub current_version: i64,
    /// Total number of applied migrations
    pub total_migrations: usize,
    /// Pending migrations (if any)
    pub pending_count: usize,
    /// Whether all migrations are up to date
    pub is_up_to_date: bool,
    /// Last migration applied
    pub last_migration: Option<MigrationInfo>,
}

/// Get the current migration status
///
/// Returns information about applied migrations and current schema version.
/// This assumes sqlx's _sqlx_migrations table exists.
pub async fn get_migration_status(pool: &PgPool) -> Result<MigrationStatus> {
    // Check if migrations table exists
    let table_exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_name = '_sqlx_migrations'
        )
        "#,
    )
    .fetch_one(pool)
    .await?;

    if !table_exists {
        return Ok(MigrationStatus {
            current_version: 0,
            total_migrations: 0,
            pending_count: 0,
            is_up_to_date: false,
            last_migration: None,
        });
    }

    // Get migration information
    let migrations = sqlx::query_as::<_, (i64, String, DateTime<Utc>, i64, bool)>(
        r#"
        SELECT version, description, installed_on, execution_time, success
        FROM _sqlx_migrations
        ORDER BY version DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let total_migrations = migrations.len();
    let current_version = migrations.first().map(|m| m.0).unwrap_or(0);

    let last_migration = migrations.first().map(|m| MigrationInfo {
        version: m.0,
        description: m.1.clone(),
        installed_on: m.2,
        execution_time: m.3,
        success: m.4,
        checksum: None,
    });

    Ok(MigrationStatus {
        current_version,
        total_migrations,
        pending_count: 0, // Would need to compare with migration files
        is_up_to_date: true,
        last_migration,
    })
}

/// List all applied migrations
pub async fn list_applied_migrations(pool: &PgPool) -> Result<Vec<MigrationInfo>> {
    let migrations = sqlx::query_as::<_, (i64, String, DateTime<Utc>, i64, bool)>(
        r#"
        SELECT version, description, installed_on, execution_time, success
        FROM _sqlx_migrations
        ORDER BY version ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(migrations
        .into_iter()
        .map(|m| MigrationInfo {
            version: m.0,
            description: m.1,
            installed_on: m.2,
            execution_time: m.3,
            success: m.4,
            checksum: None,
        })
        .collect())
}

/// Check if a specific migration version has been applied
pub async fn is_migration_applied(pool: &PgPool, version: i64) -> Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM _sqlx_migrations
            WHERE version = $1 AND success = true
        )
        "#,
    )
    .bind(version)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Verify schema integrity by checking for required tables
pub async fn verify_schema_integrity(pool: &PgPool) -> Result<SchemaIntegrity> {
    let required_tables = vec![
        "users",
        "tokens",
        "balances",
        "orders",
        "trades",
        "reputation_events",
        "output_commitments",
        "admin_actions",
    ];

    let mut missing_tables = Vec::new();
    let mut existing_tables = Vec::new();

    for table in required_tables {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT FROM information_schema.tables
                WHERE table_schema = 'public'
                AND table_name = $1
            )
            "#,
        )
        .bind(table)
        .fetch_one(pool)
        .await?;

        if exists {
            existing_tables.push(table.to_string());
        } else {
            missing_tables.push(table.to_string());
        }
    }

    Ok(SchemaIntegrity {
        is_valid: missing_tables.is_empty(),
        existing_tables,
        missing_tables,
    })
}

/// Schema integrity check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaIntegrity {
    /// Whether all required tables exist
    pub is_valid: bool,
    /// List of existing required tables
    pub existing_tables: Vec<String>,
    /// List of missing required tables
    pub missing_tables: Vec<String>,
}

/// Get database schema version from a custom version table
///
/// This is useful if you maintain a separate version table for semantic versioning
pub async fn get_schema_version(pool: &PgPool) -> Result<Option<String>> {
    // Check if custom schema_version table exists
    let table_exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_name = 'schema_version'
        )
        "#,
    )
    .fetch_one(pool)
    .await?;

    if !table_exists {
        return Ok(None);
    }

    let version = sqlx::query_scalar::<_, String>(
        r#"
        SELECT version FROM schema_version
        ORDER BY installed_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_info_structure() {
        let info = MigrationInfo {
            version: 1,
            description: "initial".to_string(),
            installed_on: Utc::now(),
            execution_time: 100,
            success: true,
            checksum: Some("abc123".to_string()),
        };

        assert_eq!(info.version, 1);
        assert!(info.success);
    }

    #[test]
    fn test_migration_status_structure() {
        let status = MigrationStatus {
            current_version: 5,
            total_migrations: 5,
            pending_count: 0,
            is_up_to_date: true,
            last_migration: None,
        };

        assert_eq!(status.current_version, 5);
        assert!(status.is_up_to_date);
    }

    #[test]
    fn test_schema_integrity_valid() {
        let integrity = SchemaIntegrity {
            is_valid: true,
            existing_tables: vec!["users".to_string(), "tokens".to_string()],
            missing_tables: vec![],
        };

        assert!(integrity.is_valid);
        assert!(integrity.missing_tables.is_empty());
    }

    #[test]
    fn test_schema_integrity_invalid() {
        let integrity = SchemaIntegrity {
            is_valid: false,
            existing_tables: vec!["users".to_string()],
            missing_tables: vec!["tokens".to_string()],
        };

        assert!(!integrity.is_valid);
        assert_eq!(integrity.missing_tables.len(), 1);
    }

    #[test]
    fn test_migration_info_serialization() {
        let info = MigrationInfo {
            version: 1,
            description: "test".to_string(),
            installed_on: Utc::now(),
            execution_time: 50,
            success: true,
            checksum: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: MigrationInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, info.version);
        assert_eq!(deserialized.description, info.description);
    }
}
