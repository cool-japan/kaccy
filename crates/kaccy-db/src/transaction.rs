//! Transaction management with savepoint support

use sqlx::{PgPool, Postgres, Transaction};
use std::future::Future;

use crate::error::Result;

/// Transaction manager with savepoint support for nested transactions
pub struct TransactionManager {
    pool: PgPool,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Begin a new transaction
    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>> {
        Ok(self.pool.begin().await?)
    }

    /// Get a reference to the pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Builder for complex transactional operations
pub struct TransactionBuilder {
    pool: PgPool,
    isolation_level: IsolationLevel,
}

/// Transaction isolation levels
#[derive(Debug, Clone, Copy, Default)]
pub enum IsolationLevel {
    /// Read committed (PostgreSQL default)
    #[default]
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable (strongest isolation)
    Serializable,
}

impl IsolationLevel {
    fn as_sql(&self) -> &'static str {
        match self {
            Self::ReadCommitted => "READ COMMITTED",
            Self::RepeatableRead => "REPEATABLE READ",
            Self::Serializable => "SERIALIZABLE",
        }
    }
}

impl TransactionBuilder {
    /// Create a new transaction builder
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            isolation_level: IsolationLevel::default(),
        }
    }

    /// Set the isolation level
    pub fn isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    /// Begin the transaction with configured isolation level
    pub async fn begin(self) -> Result<Transaction<'static, Postgres>> {
        let mut tx = self.pool.begin().await?;

        // Set isolation level
        let query = format!(
            "SET TRANSACTION ISOLATION LEVEL {}",
            self.isolation_level.as_sql()
        );
        sqlx::query(&query).execute(&mut *tx).await?;

        Ok(tx)
    }
}

/// Helper function to run an operation within a savepoint
///
/// Creates a savepoint before executing the operation.
/// If the operation succeeds, the savepoint is released.
/// If the operation fails, the transaction is rolled back to the savepoint.
pub async fn with_savepoint<T, F, Fut>(
    tx: &mut Transaction<'_, Postgres>,
    name: &str,
    f: F,
) -> Result<T>
where
    F: FnOnce(&mut Transaction<'_, Postgres>) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    // Create savepoint
    let create_query = format!("SAVEPOINT {}", name);
    sqlx::query(&create_query).execute(&mut **tx).await?;

    match f(tx).await {
        Ok(result) => {
            // Release savepoint on success
            let release_query = format!("RELEASE SAVEPOINT {}", name);
            sqlx::query(&release_query).execute(&mut **tx).await?;
            Ok(result)
        }
        Err(e) => {
            // Rollback to savepoint on failure
            let rollback_query = format!("ROLLBACK TO SAVEPOINT {}", name);
            sqlx::query(&rollback_query).execute(&mut **tx).await?;
            Err(e)
        }
    }
}

/// Savepoint guard that tracks savepoint state
pub struct SavepointGuard {
    name: String,
    committed: bool,
}

impl SavepointGuard {
    /// Create a new savepoint within a transaction
    pub async fn new(tx: &mut Transaction<'_, Postgres>, name: &str) -> Result<Self> {
        let create_query = format!("SAVEPOINT {}", name);
        sqlx::query(&create_query).execute(&mut **tx).await?;

        Ok(Self {
            name: name.to_string(),
            committed: false,
        })
    }

    /// Release the savepoint (commit the nested transaction)
    pub async fn release(mut self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        let release_query = format!("RELEASE SAVEPOINT {}", self.name);
        sqlx::query(&release_query).execute(&mut **tx).await?;
        self.committed = true;
        Ok(())
    }

    /// Rollback to the savepoint
    pub async fn rollback(mut self, tx: &mut Transaction<'_, Postgres>) -> Result<()> {
        let rollback_query = format!("ROLLBACK TO SAVEPOINT {}", self.name);
        sqlx::query(&rollback_query).execute(&mut **tx).await?;
        self.committed = true;
        Ok(())
    }

    /// Get the savepoint name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if the savepoint has been committed or rolled back
    pub fn is_handled(&self) -> bool {
        self.committed
    }
}

impl Drop for SavepointGuard {
    fn drop(&mut self) {
        if !self.committed {
            tracing::warn!(
                savepoint = %self.name,
                "Savepoint guard dropped without release or rollback"
            );
        }
    }
}

/// Extension trait for transactions with savepoint support
#[async_trait::async_trait]
pub trait TransactionExt {
    /// Create a savepoint within this transaction
    async fn create_savepoint(&mut self, name: &str) -> Result<()>;

    /// Release a savepoint
    async fn release_savepoint(&mut self, name: &str) -> Result<()>;

    /// Rollback to a savepoint
    async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()>;
}

#[async_trait::async_trait]
impl TransactionExt for Transaction<'_, Postgres> {
    async fn create_savepoint(&mut self, name: &str) -> Result<()> {
        let query = format!("SAVEPOINT {}", name);
        sqlx::query(&query).execute(&mut **self).await?;
        Ok(())
    }

    async fn release_savepoint(&mut self, name: &str) -> Result<()> {
        let query = format!("RELEASE SAVEPOINT {}", name);
        sqlx::query(&query).execute(&mut **self).await?;
        Ok(())
    }

    async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
        let query = format!("ROLLBACK TO SAVEPOINT {}", name);
        sqlx::query(&query).execute(&mut **self).await?;
        Ok(())
    }
}

/// Macro for executing code within a savepoint
///
/// Usage:
/// ```ignore
/// nested_transaction!(tx, "savepoint_name", {
///     // your code here
/// })
/// ```
#[macro_export]
macro_rules! nested_transaction {
    ($tx:expr, $name:expr, $body:block) => {{
        use $crate::transaction::TransactionExt;

        $tx.create_savepoint($name).await?;
        let result = (|| async $body)().await;

        match result {
            Ok(value) => {
                $tx.release_savepoint($name).await?;
                Ok(value)
            }
            Err(e) => {
                $tx.rollback_to_savepoint($name).await?;
                Err(e)
            }
        }
    }};
}

/// Configuration for transaction retry logic
#[derive(Debug, Clone)]
pub struct TransactionRetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration in milliseconds
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds
    pub max_backoff_ms: u64,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for TransactionRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 10,
            max_backoff_ms: 1000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Execute a transaction with automatic retry on serialization failures
///
/// This function automatically retries the transaction if it fails due to
/// serialization errors (SQLSTATE 40001) or deadlock errors (SQLSTATE 40P01).
///
/// # Arguments
/// * `pool` - Database connection pool (cloneable)
/// * `config` - Retry configuration
/// * `f` - Async function that performs the transactional work
///
/// # Returns
/// Result of the transaction operation
///
/// # Example
/// ```ignore
/// use kaccy_db::transaction::{retry_transaction, TransactionRetryConfig};
///
/// let pool_clone = pool.clone();
/// let result = retry_transaction(
///     pool_clone,
///     TransactionRetryConfig::default(),
///     |pool| async move {
///         let mut tx = pool.begin().await?;
///         // Your transactional work here
///         sqlx::query("UPDATE accounts SET balance = balance + $1 WHERE id = $2")
///             .bind(amount)
///             .bind(account_id)
///             .execute(&mut *tx)
///             .await?;
///         tx.commit().await?;
///         Ok(())
///     }
/// ).await?;
/// ```
pub async fn retry_transaction<T, F, Fut>(
    pool: PgPool,
    config: TransactionRetryConfig,
    f: F,
) -> Result<T>
where
    F: Fn(PgPool) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 0;
    let mut backoff_ms = config.initial_backoff_ms;

    loop {
        attempt += 1;

        match f(pool.clone()).await {
            Ok(result) => {
                return Ok(result);
            }
            Err(e) => {
                // Check if the error is retriable
                let is_retriable = is_retriable_error(&e);

                if !is_retriable || attempt >= config.max_retries {
                    tracing::warn!(
                        attempt = attempt,
                        max_retries = config.max_retries,
                        error = %e,
                        "Transaction failed after retries"
                    );
                    return Err(e);
                }

                // Exponential backoff with jitter
                let jitter = (rand::random::<f64>() * 0.3) + 0.85; // 0.85-1.15 range
                let sleep_ms = (backoff_ms as f64 * jitter) as u64;

                tracing::debug!(
                    attempt = attempt,
                    max_retries = config.max_retries,
                    backoff_ms = sleep_ms,
                    error = %e,
                    "Transaction failed, retrying"
                );

                tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;

                // Increase backoff for next iteration
                backoff_ms = ((backoff_ms as f64 * config.backoff_multiplier) as u64)
                    .min(config.max_backoff_ms);
            }
        }
    }
}

/// Check if an error is retriable (serialization or deadlock)
fn is_retriable_error(error: &crate::error::DbError) -> bool {
    match error {
        crate::error::DbError::Sqlx(sqlx_error) => {
            if let Some(db_error) = sqlx_error.as_database_error() {
                let code = db_error.code();
                // 40001 = serialization_failure
                // 40P01 = deadlock_detected
                code.as_deref() == Some("40001") || code.as_deref() == Some("40P01")
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Execute a transaction with retry on serialization failures and custom isolation level
///
/// # Arguments
/// * `pool` - Database connection pool (cloneable)
/// * `config` - Retry configuration
/// * `isolation_level` - Transaction isolation level
/// * `f` - Async function that performs the transactional work
pub async fn retry_transaction_with_isolation<T, F, Fut>(
    pool: PgPool,
    config: TransactionRetryConfig,
    isolation_level: IsolationLevel,
    f: F,
) -> Result<T>
where
    F: Fn(PgPool, IsolationLevel) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 0;
    let mut backoff_ms = config.initial_backoff_ms;

    loop {
        attempt += 1;

        match f(pool.clone(), isolation_level).await {
            Ok(result) => {
                return Ok(result);
            }
            Err(e) => {
                let is_retriable = is_retriable_error(&e);

                if !is_retriable || attempt >= config.max_retries {
                    tracing::warn!(
                        attempt = attempt,
                        max_retries = config.max_retries,
                        isolation_level = ?isolation_level,
                        error = %e,
                        "Transaction failed after retries"
                    );
                    return Err(e);
                }

                let jitter = (rand::random::<f64>() * 0.3) + 0.85;
                let sleep_ms = (backoff_ms as f64 * jitter) as u64;

                tracing::debug!(
                    attempt = attempt,
                    max_retries = config.max_retries,
                    backoff_ms = sleep_ms,
                    isolation_level = ?isolation_level,
                    error = %e,
                    "Transaction failed, retrying"
                );

                tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;
                backoff_ms = ((backoff_ms as f64 * config.backoff_multiplier) as u64)
                    .min(config.max_backoff_ms);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = TransactionRetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_backoff_ms, 10);
        assert_eq!(config.max_backoff_ms, 1000);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = TransactionRetryConfig {
            max_retries: 5,
            initial_backoff_ms: 50,
            max_backoff_ms: 5000,
            backoff_multiplier: 1.5,
        };
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_backoff_ms, 50);
        assert_eq!(config.max_backoff_ms, 5000);
        assert_eq!(config.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_isolation_level_sql() {
        assert_eq!(IsolationLevel::ReadCommitted.as_sql(), "READ COMMITTED");
        assert_eq!(IsolationLevel::RepeatableRead.as_sql(), "REPEATABLE READ");
        assert_eq!(IsolationLevel::Serializable.as_sql(), "SERIALIZABLE");
    }
}
