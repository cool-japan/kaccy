//! Database backup and recovery utilities

use crate::error::{DbError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Directory to store backups
    pub backup_dir: PathBuf,
    /// Database name
    pub database_name: String,
    /// Retention period in days (backups older than this will be cleaned up)
    pub retention_days: u32,
    /// Maximum number of backups to keep
    pub max_backups: usize,
    /// Compress backups with gzip
    pub compress: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("./backups"),
            database_name: "kaccy".to_string(),
            retention_days: 30,
            max_backups: 10,
            compress: true,
        }
    }
}

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Backup file path
    pub file_path: PathBuf,
    /// Backup creation timestamp
    pub created_at: DateTime<Utc>,
    /// Database name
    pub database_name: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// Whether the backup is compressed
    pub compressed: bool,
}

/// Point-in-time recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitrConfig {
    /// WAL archive directory
    pub wal_archive_dir: PathBuf,
    /// Base backup directory
    pub base_backup_dir: PathBuf,
    /// Retention period in days
    pub retention_days: u32,
}

impl Default for PitrConfig {
    fn default() -> Self {
        Self {
            wal_archive_dir: PathBuf::from("./wal_archive"),
            base_backup_dir: PathBuf::from("./base_backups"),
            retention_days: 7,
        }
    }
}

/// Database backup manager
pub struct BackupManager {
    config: BackupConfig,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    /// Ensure backup directory exists
    async fn ensure_backup_dir(&self) -> Result<()> {
        if !self.config.backup_dir.exists() {
            fs::create_dir_all(&self.config.backup_dir)
                .await
                .map_err(|e| DbError::Other(format!("Failed to create backup directory: {}", e)))?;
        }
        Ok(())
    }

    /// Generate backup filename
    fn generate_backup_filename(&self) -> String {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let extension = if self.config.compress {
            "sql.gz"
        } else {
            "sql"
        };
        format!("{}_{}.{}", self.config.database_name, timestamp, extension)
    }

    /// Create a database backup using pg_dump
    pub async fn create_backup(&self, database_url: &str) -> Result<BackupMetadata> {
        self.ensure_backup_dir().await?;

        let filename = self.generate_backup_filename();
        let file_path = self.config.backup_dir.join(&filename);

        // Execute pg_dump command
        let mut cmd = Command::new("pg_dump");
        cmd.arg(database_url)
            .arg("--format=plain")
            .arg("--no-owner")
            .arg("--no-acl");

        if self.config.compress {
            // Use gzip to compress output
            let output = cmd
                .output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to execute pg_dump: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DbError::Other(format!("pg_dump failed: {}", stderr)));
            }

            // Compress the output
            let mut gzip_cmd = Command::new("gzip");
            gzip_cmd
                .arg("-c")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped());

            let mut child = gzip_cmd
                .spawn()
                .map_err(|e| DbError::Other(format!("Failed to spawn gzip: {}", e)))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin
                    .write_all(&output.stdout)
                    .await
                    .map_err(|e| DbError::Other(format!("Failed to write to gzip: {}", e)))?;
            }

            let output = child
                .wait_with_output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to wait for gzip: {}", e)))?;

            fs::write(&file_path, output.stdout)
                .await
                .map_err(|e| DbError::Other(format!("Failed to write backup file: {}", e)))?;
        } else {
            cmd.arg(format!("--file={}", file_path.display()));

            let output = cmd
                .output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to execute pg_dump: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DbError::Other(format!("pg_dump failed: {}", stderr)));
            }
        }

        let metadata = fs::metadata(&file_path)
            .await
            .map_err(|e| DbError::Other(format!("Failed to read backup metadata: {}", e)))?;

        Ok(BackupMetadata {
            file_path,
            created_at: Utc::now(),
            database_name: self.config.database_name.clone(),
            size_bytes: metadata.len(),
            compressed: self.config.compress,
        })
    }

    /// List all available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        if !self.config.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        let mut entries = fs::read_dir(&self.config.backup_dir)
            .await
            .map_err(|e| DbError::Other(format!("Failed to read backup directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| DbError::Other(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if file_name.starts_with(&self.config.database_name)
                    && (file_name.ends_with(".sql") || file_name.ends_with(".sql.gz"))
                {
                    let is_compressed = file_name.ends_with(".gz");
                    let metadata = fs::metadata(&path).await.map_err(|e| {
                        DbError::Other(format!("Failed to read file metadata: {}", e))
                    })?;

                    backups.push(BackupMetadata {
                        file_path: path,
                        created_at: metadata
                            .created()
                            .ok()
                            .and_then(|t| {
                                DateTime::from_timestamp(
                                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                                    0,
                                )
                            })
                            .unwrap_or_else(Utc::now),
                        database_name: self.config.database_name.clone(),
                        size_bytes: metadata.len(),
                        compressed: is_compressed,
                    });
                }
            }
        }

        // Sort by creation time, newest first
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Restore database from a backup file
    pub async fn restore_backup(&self, backup_path: &Path, database_url: &str) -> Result<()> {
        if !backup_path.exists() {
            return Err(DbError::Other("Backup file not found".to_string()));
        }

        let compressed = backup_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "gz")
            .unwrap_or(false);

        if compressed {
            // Decompress and pipe to psql
            let mut gunzip_cmd = Command::new("gunzip");
            gunzip_cmd.arg("-c").arg(backup_path);

            let gunzip_output = gunzip_cmd
                .output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to execute gunzip: {}", e)))?;

            if !gunzip_output.status.success() {
                let stderr = String::from_utf8_lossy(&gunzip_output.stderr);
                return Err(DbError::Other(format!("gunzip failed: {}", stderr)));
            }

            let mut psql_cmd = Command::new("psql");
            psql_cmd
                .arg(database_url)
                .stdin(std::process::Stdio::piped());

            let mut child = psql_cmd
                .spawn()
                .map_err(|e| DbError::Other(format!("Failed to spawn psql: {}", e)))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin
                    .write_all(&gunzip_output.stdout)
                    .await
                    .map_err(|e| DbError::Other(format!("Failed to write to psql: {}", e)))?;
            }

            let output = child
                .wait_with_output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to wait for psql: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DbError::Other(format!("psql failed: {}", stderr)));
            }
        } else {
            // Direct restore with psql
            let mut cmd = Command::new("psql");
            cmd.arg(database_url).arg("-f").arg(backup_path);

            let output = cmd
                .output()
                .await
                .map_err(|e| DbError::Other(format!("Failed to execute psql: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DbError::Other(format!("psql failed: {}", stderr)));
            }
        }

        Ok(())
    }

    /// Clean up old backups based on retention policy
    pub async fn cleanup_old_backups(&self) -> Result<Vec<PathBuf>> {
        let backups = self.list_backups().await?;
        let mut deleted = Vec::new();

        let cutoff_date = Utc::now() - chrono::Duration::days(self.config.retention_days as i64);

        for (i, backup) in backups.iter().enumerate() {
            // Delete if older than retention period OR exceeds max backups
            if backup.created_at < cutoff_date || i >= self.config.max_backups {
                fs::remove_file(&backup.file_path)
                    .await
                    .map_err(|e| DbError::Other(format!("Failed to delete backup: {}", e)))?;
                deleted.push(backup.file_path.clone());
            }
        }

        Ok(deleted)
    }

    /// Get total size of all backups
    pub async fn get_total_backup_size(&self) -> Result<u64> {
        let backups = self.list_backups().await?;
        Ok(backups.iter().map(|b| b.size_bytes).sum())
    }
}

/// Verify database integrity
pub async fn verify_database_integrity(pool: &PgPool) -> Result<bool> {
    // Check if database is accessible
    let result: (bool,) = sqlx::query_as("SELECT true").fetch_one(pool).await?;

    if !result.0 {
        return Ok(false);
    }

    // Verify critical tables exist
    let tables = vec![
        "users",
        "tokens",
        "balances",
        "orders",
        "trades",
        "audit_logs",
    ];
    for table in tables {
        let count: (i64,) = sqlx::query_as(&format!(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = '{}'",
            table
        ))
        .fetch_one(pool)
        .await?;

        if count.0 == 0 {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Get database statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DatabaseStats {
    /// Name of the database.
    pub database_name: String,
    /// Total size of the database in bytes.
    pub size_bytes: i64,
    /// Number of tables in the public schema.
    pub table_count: i64,
    /// Number of indexes in the public schema.
    pub index_count: i64,
}

/// Retrieve current database size and object counts.
pub async fn get_database_stats(pool: &PgPool, db_name: &str) -> Result<DatabaseStats> {
    let size: (i64,) = sqlx::query_as("SELECT pg_database_size(current_database())")
        .fetch_one(pool)
        .await?;

    let table_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
    )
    .fetch_one(pool)
    .await?;

    let index_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public'")
            .fetch_one(pool)
            .await?;

    Ok(DatabaseStats {
        database_name: db_name.to_string(),
        size_bytes: size.0,
        table_count: table_count.0,
        index_count: index_count.0,
    })
}
