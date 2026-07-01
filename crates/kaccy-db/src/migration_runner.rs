//! Migration Runner
//!
//! Automated database migration execution and rollback system.
//! Complements the migration_utils module by providing execution capabilities.
//!
//! # Features
//!
//! - Automatic migration discovery and ordering
//! - Safe migration execution with transactions
//! - Rollback support for failed migrations
//! - Dry-run mode for testing
//! - Progress tracking and reporting
//! - Migration dependency validation
//! - Idempotent execution (safe to run multiple times)
//!
//! # Example
//!
//! ```rust,no_run
//! use kaccy_db::migration_runner::{MigrationRunner, MigrationConfig};
//! use sqlx::PgPool;
//!
//! async fn run_migrations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
//!     let config = MigrationConfig {
//!         migrations_dir: "migrations".to_string(),
//!         allow_missing: false,
//!         dry_run: false,
//!     };
//!
//!     let runner = MigrationRunner::new(pool.clone(), config);
//!     let result = runner.run_pending_migrations().await?;
//!
//!     println!("Applied {} migrations", result.applied_count);
//!     Ok(())
//! }
//! ```

use crate::error::{DbError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::path::Path;
use tracing::{info, warn};

/// Configuration for the migration runner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Directory containing migration SQL files
    pub migrations_dir: String,

    /// Whether to allow missing migrations in sequence
    pub allow_missing: bool,

    /// Whether to run in dry-run mode (no actual changes)
    pub dry_run: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            migrations_dir: "migrations".to_string(),
            allow_missing: false,
            dry_run: false,
        }
    }
}

/// Information about a single migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Migration version (extracted from filename, e.g., "001" from "001_initial.sql")
    pub version: String,

    /// Migration name (e.g., "initial" from "001_initial.sql")
    pub name: String,

    /// SQL content of the migration
    pub sql: String,

    /// File path to the migration
    pub file_path: String,
}

impl Migration {
    /// Parse a migration from a file path
    pub fn from_file(file_path: &str) -> Result<Self> {
        let path = Path::new(file_path);
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DbError::NotFound("Invalid migration filename".to_string()))?;

        // Parse filename: "001_initial.sql" -> version="001", name="initial"
        let parts: Vec<&str> = filename.trim_end_matches(".sql").splitn(2, '_').collect();

        if parts.len() != 2 {
            return Err(DbError::NotFound(format!(
                "Invalid migration filename format: {}",
                filename
            )));
        }

        let version = parts[0].to_string();
        let name = parts[1].to_string();

        let sql = std::fs::read_to_string(file_path)
            .map_err(|e| DbError::NotFound(format!("Failed to read migration file: {}", e)))?;

        Ok(Self {
            version,
            name,
            sql,
            file_path: file_path.to_string(),
        })
    }

    /// Get the migration ID (version_name format for tracking)
    pub fn id(&self) -> String {
        format!("{}_{}", self.version, self.name)
    }
}

/// Result of running migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    /// Number of migrations applied
    pub applied_count: usize,

    /// Number of migrations skipped (already applied)
    pub skipped_count: usize,

    /// List of applied migrations
    pub applied_migrations: Vec<String>,

    /// List of skipped migrations
    pub skipped_migrations: Vec<String>,

    /// Whether the migration run was successful
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Migration execution history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationHistory {
    /// Migration ID
    pub migration_id: String,

    /// When the migration was applied
    pub applied_at: DateTime<Utc>,

    /// Execution time in milliseconds
    pub execution_time_ms: i64,

    /// Whether the migration succeeded
    pub success: bool,
}

/// Migration runner
pub struct MigrationRunner {
    pool: PgPool,
    config: MigrationConfig,
}

impl MigrationRunner {
    /// Create a new migration runner
    pub fn new(pool: PgPool, config: MigrationConfig) -> Self {
        Self { pool, config }
    }

    /// Create a migration runner with default configuration
    pub fn with_defaults(pool: PgPool) -> Self {
        Self::new(pool, MigrationConfig::default())
    }

    /// Ensure the migrations tracking table exists
    async fn ensure_migrations_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                id SERIAL PRIMARY KEY,
                migration_id VARCHAR(255) NOT NULL UNIQUE,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                execution_time_ms BIGINT NOT NULL,
                success BOOLEAN NOT NULL DEFAULT TRUE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get list of applied migrations
    async fn get_applied_migrations(&self) -> Result<Vec<String>> {
        let records = sqlx::query_scalar::<_, String>(
            "SELECT migration_id FROM _migrations WHERE success = TRUE ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Check if a migration has been applied
    #[allow(dead_code)]
    async fn is_migration_applied(&self, migration_id: &str) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM _migrations WHERE migration_id = $1 AND success = TRUE",
        )
        .bind(migration_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Record a migration as applied
    async fn record_migration(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        migration_id: &str,
        execution_time_ms: i64,
        success: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO _migrations (migration_id, applied_at, execution_time_ms, success)
            VALUES ($1, NOW(), $2, $3)
            "#,
        )
        .bind(migration_id)
        .bind(execution_time_ms)
        .bind(success)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Discover all migration files in the migrations directory
    pub fn discover_migrations(&self) -> Result<Vec<Migration>> {
        let migrations_dir = Path::new(&self.config.migrations_dir);

        if !migrations_dir.exists() {
            return Err(DbError::NotFound(format!(
                "Migrations directory not found: {}",
                self.config.migrations_dir
            )));
        }

        let mut migrations = Vec::new();

        let entries = std::fs::read_dir(migrations_dir).map_err(|e| {
            DbError::NotFound(format!("Failed to read migrations directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry
                .map_err(|e| DbError::NotFound(format!("Failed to read directory entry: {}", e)))?;

            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                let path_str = path.to_str().ok_or_else(|| {
                    DbError::Validation(format!(
                        "Migration path is not valid UTF-8: {}",
                        path.display()
                    ))
                })?;
                let migration = Migration::from_file(path_str)?;
                migrations.push(migration);
            }
        }

        // Sort by version
        migrations.sort_by(|a, b| a.version.cmp(&b.version));

        Ok(migrations)
    }

    /// Run all pending migrations
    pub async fn run_pending_migrations(&self) -> Result<MigrationResult> {
        let start_time = std::time::Instant::now();

        // Ensure migrations table exists
        self.ensure_migrations_table().await?;

        // Discover all migrations
        let all_migrations = self.discover_migrations()?;
        let applied_migrations = self.get_applied_migrations().await?;

        let mut result = MigrationResult {
            applied_count: 0,
            skipped_count: 0,
            applied_migrations: Vec::new(),
            skipped_migrations: Vec::new(),
            success: true,
            error: None,
            execution_time_ms: 0,
        };

        for migration in all_migrations {
            let migration_id = migration.id();

            // Skip if already applied
            if applied_migrations.contains(&migration_id) {
                info!(
                    migration_id = migration_id,
                    "Skipping already applied migration"
                );
                result.skipped_count += 1;
                result.skipped_migrations.push(migration_id);
                continue;
            }

            if self.config.dry_run {
                info!(
                    migration_id = migration_id,
                    "Dry-run: Would apply migration"
                );
                result.applied_count += 1;
                result.applied_migrations.push(migration_id);
                continue;
            }

            // Execute migration in transaction
            match self.execute_migration(&migration).await {
                Ok(execution_time_ms) => {
                    info!(
                        migration_id = migration_id,
                        execution_time_ms = execution_time_ms,
                        "Migration applied successfully"
                    );
                    result.applied_count += 1;
                    result.applied_migrations.push(migration_id);
                }
                Err(e) => {
                    warn!(
                        migration_id = migration_id,
                        error = %e,
                        "Migration failed"
                    );
                    result.success = false;
                    result.error = Some(format!("Migration {} failed: {}", migration_id, e));
                    break;
                }
            }
        }

        result.execution_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(result)
    }

    /// Execute a single migration
    async fn execute_migration(&self, migration: &Migration) -> Result<i64> {
        let start_time = std::time::Instant::now();

        let mut tx = self.pool.begin().await?;

        // Execute the migration SQL
        match sqlx::query(sqlx::AssertSqlSafe(migration.sql.as_str()))
            .execute(&mut *tx)
            .await
        {
            Ok(_) => {
                let execution_time_ms = start_time.elapsed().as_millis() as i64;

                // Record the migration
                self.record_migration(&mut tx, &migration.id(), execution_time_ms, true)
                    .await?;

                // Commit transaction
                tx.commit().await?;

                Ok(execution_time_ms)
            }
            Err(e) => {
                // Rollback on error
                tx.rollback().await?;

                Err(DbError::from(e))
            }
        }
    }

    /// Get migration history
    pub async fn get_migration_history(&self) -> Result<Vec<MigrationHistory>> {
        self.ensure_migrations_table().await?;

        let records = sqlx::query_as::<_, (String, DateTime<Utc>, i64, bool)>(
            "SELECT migration_id, applied_at, execution_time_ms, success FROM _migrations ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records
            .into_iter()
            .map(
                |(migration_id, applied_at, execution_time_ms, success)| MigrationHistory {
                    migration_id,
                    applied_at,
                    execution_time_ms,
                    success,
                },
            )
            .collect())
    }

    /// Verify all migrations are in order (no missing versions)
    pub fn verify_migration_order(&self, migrations: &[Migration]) -> Result<()> {
        if self.config.allow_missing {
            return Ok(());
        }

        for (i, migration) in migrations.iter().enumerate() {
            let expected_version = format!("{:03}", i + 1);
            if migration.version != expected_version {
                return Err(DbError::NotFound(format!(
                    "Migration version mismatch: expected {}, found {}",
                    expected_version, migration.version
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_config_default() {
        let config = MigrationConfig::default();
        assert_eq!(config.migrations_dir, "migrations");
        assert!(!config.allow_missing);
        assert!(!config.dry_run);
    }

    #[test]
    fn test_migration_id() {
        let migration = Migration {
            version: "001".to_string(),
            name: "initial".to_string(),
            sql: "CREATE TABLE test();".to_string(),
            file_path: "migrations/001_initial.sql".to_string(),
        };

        assert_eq!(migration.id(), "001_initial");
    }

    #[test]
    fn test_migration_result_serialization() {
        let result = MigrationResult {
            applied_count: 3,
            skipped_count: 2,
            applied_migrations: vec!["001_initial".to_string()],
            skipped_migrations: vec!["000_setup".to_string()],
            success: true,
            error: None,
            execution_time_ms: 1500,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("applied_count"));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_migration_history_serialization() {
        let history = MigrationHistory {
            migration_id: "001_initial".to_string(),
            applied_at: Utc::now(),
            execution_time_ms: 1000,
            success: true,
        };

        let json = serde_json::to_string(&history).unwrap();
        assert!(json.contains("migration_id"));
        assert!(json.contains("001_initial"));
    }

    #[tokio::test]
    async fn test_verify_migration_order_success() {
        let migrations = vec![
            Migration {
                version: "001".to_string(),
                name: "first".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
            Migration {
                version: "002".to_string(),
                name: "second".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
            Migration {
                version: "003".to_string(),
                name: "third".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
        ];

        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let runner = MigrationRunner::with_defaults(pool);

        assert!(runner.verify_migration_order(&migrations).is_ok());
    }

    #[tokio::test]
    async fn test_verify_migration_order_failure() {
        let migrations = vec![
            Migration {
                version: "001".to_string(),
                name: "first".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
            Migration {
                version: "003".to_string(), // Skipped 002
                name: "third".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
        ];

        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let runner = MigrationRunner::with_defaults(pool);

        assert!(runner.verify_migration_order(&migrations).is_err());
    }

    #[tokio::test]
    async fn test_verify_migration_order_with_allow_missing() {
        let migrations = vec![
            Migration {
                version: "001".to_string(),
                name: "first".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
            Migration {
                version: "003".to_string(),
                name: "third".to_string(),
                sql: String::new(),
                file_path: String::new(),
            },
        ];

        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let config = MigrationConfig {
            allow_missing: true,
            ..Default::default()
        };
        let runner = MigrationRunner::new(pool, config);

        assert!(runner.verify_migration_order(&migrations).is_ok());
    }
}
