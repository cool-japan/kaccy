//! Query retry policies with idempotency detection and transient error classification
//!
//! Provides configurable retry strategies for database operations with automatic
//! detection of idempotent operations and transient errors.

use crate::error::{DbError, Result};
use sqlx::Error as SqlxError;
use std::time::Duration;

/// Classification of database errors
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClass {
    /// Transient error that may succeed on retry
    Transient,
    /// Permanent error that will not succeed on retry
    Permanent,
    /// Unknown error classification
    Unknown,
}

/// Retry strategy configuration
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// No retries
    None,
    /// Fixed delay between retries
    Fixed {
        /// Maximum number of attempts before giving up.
        max_attempts: usize,
        /// Fixed delay between each attempt.
        delay: Duration,
    },
    /// Exponential backoff
    Exponential {
        /// Maximum number of attempts before giving up.
        max_attempts: usize,
        /// Delay before the first retry.
        initial_delay: Duration,
        /// Upper bound on the computed delay.
        max_delay: Duration,
        /// Factor by which the delay grows each attempt.
        multiplier: f64,
    },
    /// Linear backoff
    Linear {
        /// Maximum number of attempts before giving up.
        max_attempts: usize,
        /// Delay before the first retry.
        initial_delay: Duration,
        /// Amount added to the delay after each attempt.
        increment: Duration,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::Exponential {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            multiplier: 2.0,
        }
    }
}

/// Configuration for retry policy
#[derive(Debug, Clone)]
pub struct RetryPolicyConfig {
    /// Retry strategy to use
    pub strategy: RetryStrategy,
    /// Whether to only retry idempotent operations
    pub idempotent_only: bool,
    /// Custom error classifier
    pub classify_error: Option<fn(&DbError) -> ErrorClass>,
}

impl Default for RetryPolicyConfig {
    fn default() -> Self {
        Self {
            strategy: RetryStrategy::default(),
            idempotent_only: true,
            classify_error: None,
        }
    }
}

/// Idempotency marker for operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Idempotency {
    /// Operation is idempotent (safe to retry)
    Idempotent,
    /// Operation is not idempotent (unsafe to retry)
    NonIdempotent,
    /// Idempotency is unknown (treat as non-idempotent)
    Unknown,
}

/// Retry policy for database operations
pub struct RetryPolicy {
    config: RetryPolicyConfig,
}

impl RetryPolicy {
    /// Create a new retry policy with default configuration
    pub fn new() -> Self {
        Self {
            config: RetryPolicyConfig::default(),
        }
    }

    /// Create a retry policy with custom configuration
    pub fn with_config(config: RetryPolicyConfig) -> Self {
        Self { config }
    }

    /// Classify a database error as transient or permanent
    pub fn classify_error(&self, error: &DbError) -> ErrorClass {
        // Use custom classifier if provided
        if let Some(classifier) = self.config.classify_error {
            return classifier(error);
        }

        // Default classification
        match error {
            DbError::Sqlx(sqlx_err) => classify_sqlx_error(sqlx_err),
            DbError::NotFound(_) => ErrorClass::Permanent,
            DbError::Duplicate(_) => ErrorClass::Permanent,
            DbError::Pool(_) => ErrorClass::Transient,
            DbError::Connection(_) => ErrorClass::Transient,
            DbError::Cache(_) => ErrorClass::Transient,
            DbError::Query(_) => ErrorClass::Permanent,
            DbError::Validation(_) => ErrorClass::Permanent,
            DbError::Other(_) => ErrorClass::Unknown,
        }
    }

    /// Execute an operation with retry policy
    pub async fn execute<F, T>(&self, idempotency: Idempotency, operation: F) -> Result<T>
    where
        F: Fn() -> futures::future::BoxFuture<'static, Result<T>>,
    {
        // Check if we should retry based on idempotency
        if self.config.idempotent_only && idempotency != Idempotency::Idempotent {
            // Execute once without retry
            return operation().await;
        }

        match &self.config.strategy {
            RetryStrategy::None => operation().await,
            RetryStrategy::Fixed {
                max_attempts,
                delay,
            } => {
                self.execute_with_fixed_retry(*max_attempts, *delay, operation)
                    .await
            }
            RetryStrategy::Exponential {
                max_attempts,
                initial_delay,
                max_delay,
                multiplier,
            } => {
                self.execute_with_exponential_retry(
                    *max_attempts,
                    *initial_delay,
                    *max_delay,
                    *multiplier,
                    operation,
                )
                .await
            }
            RetryStrategy::Linear {
                max_attempts,
                initial_delay,
                increment,
            } => {
                self.execute_with_linear_retry(*max_attempts, *initial_delay, *increment, operation)
                    .await
            }
        }
    }

    async fn execute_with_fixed_retry<F, T>(
        &self,
        max_attempts: usize,
        delay: Duration,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> futures::future::BoxFuture<'static, Result<T>>,
    {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Check if error is retryable
                    if self.classify_error(&err) == ErrorClass::Permanent {
                        return Err(err);
                    }

                    if attempts >= max_attempts {
                        return Err(err);
                    }

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn execute_with_exponential_retry<F, T>(
        &self,
        max_attempts: usize,
        initial_delay: Duration,
        max_delay: Duration,
        multiplier: f64,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> futures::future::BoxFuture<'static, Result<T>>,
    {
        let mut attempts = 0;
        let mut current_delay = initial_delay;

        loop {
            attempts += 1;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Check if error is retryable
                    if self.classify_error(&err) == ErrorClass::Permanent {
                        return Err(err);
                    }

                    if attempts >= max_attempts {
                        return Err(err);
                    }

                    tokio::time::sleep(current_delay).await;

                    // Calculate next delay with exponential backoff
                    current_delay = std::cmp::min(
                        Duration::from_secs_f64(current_delay.as_secs_f64() * multiplier),
                        max_delay,
                    );
                }
            }
        }
    }

    async fn execute_with_linear_retry<F, T>(
        &self,
        max_attempts: usize,
        initial_delay: Duration,
        increment: Duration,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> futures::future::BoxFuture<'static, Result<T>>,
    {
        let mut attempts = 0;
        let mut current_delay = initial_delay;

        loop {
            attempts += 1;

            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Check if error is retryable
                    if self.classify_error(&err) == ErrorClass::Permanent {
                        return Err(err);
                    }

                    if attempts >= max_attempts {
                        return Err(err);
                    }

                    tokio::time::sleep(current_delay).await;

                    // Calculate next delay with linear backoff
                    current_delay += increment;
                }
            }
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Classify SQLx errors as transient or permanent
fn classify_sqlx_error(error: &SqlxError) -> ErrorClass {
    match error {
        // Connection errors are often transient
        SqlxError::PoolTimedOut => ErrorClass::Transient,
        SqlxError::PoolClosed => ErrorClass::Permanent,
        SqlxError::WorkerCrashed => ErrorClass::Transient,

        // Database errors need inspection
        SqlxError::Database(db_err) => {
            // PostgreSQL error codes
            // See: https://www.postgresql.org/docs/current/errcodes-appendix.html
            if let Some(code) = db_err.code() {
                match code.as_ref() {
                    // Connection errors (08xxx)
                    code if code.starts_with("08") => ErrorClass::Transient,

                    // Serialization failure (40001)
                    "40001" => ErrorClass::Transient,

                    // Deadlock detected (40P01)
                    "40P01" => ErrorClass::Transient,

                    // Lock timeout (55P03)
                    "55P03" => ErrorClass::Transient,

                    // Constraint violations are permanent
                    code if code.starts_with("23") => ErrorClass::Permanent,

                    // Syntax errors are permanent
                    code if code.starts_with("42") => ErrorClass::Permanent,

                    _ => ErrorClass::Unknown,
                }
            } else {
                ErrorClass::Unknown
            }
        }

        // IO errors might be transient
        SqlxError::Io(_) => ErrorClass::Transient,

        // Other errors are unknown
        _ => ErrorClass::Unknown,
    }
}

/// Detect if a SQL statement is idempotent
pub fn detect_idempotency(sql: &str) -> Idempotency {
    let sql_upper = sql.trim().to_uppercase();

    // SELECT queries are idempotent
    if sql_upper.starts_with("SELECT") {
        return Idempotency::Idempotent;
    }

    // UPDATE with WHERE clause may be idempotent if it sets to specific values
    if sql_upper.starts_with("UPDATE") && sql_upper.contains("WHERE") {
        // Check if it's setting to absolute values (not incrementing)
        // Look for patterns like "SET col = col + 1" which are not idempotent
        // This is a simple heuristic - not perfect but catches common cases

        // Check for += style operators (though these are not standard SQL)
        if sql_upper.contains("+=")
            || sql_upper.contains("-=")
            || sql_upper.contains("*=")
            || sql_upper.contains("/=")
        {
            return Idempotency::NonIdempotent;
        }

        // Check for self-referential arithmetic like "col = col + 1"
        // Extract the SET clause (between SET and WHERE)
        if let Some(set_start) = sql_upper.find("SET") {
            if let Some(where_start) = sql_upper.find("WHERE") {
                let set_clause = &sql_upper[set_start..where_start];

                // Check if the column name appears on both sides of = with arithmetic
                // This is a simple check for patterns like "count = count + 1"
                if set_clause.contains(" + ")
                    || set_clause.contains(" - ")
                    || set_clause.contains(" * ")
                    || set_clause.contains(" / ")
                {
                    // Check if it's likely a self-reference (column name appears twice)
                    // Simple heuristic: count how many words appear more than once
                    let words: Vec<&str> = set_clause.split_whitespace().collect();
                    for word in &words {
                        if words.iter().filter(|w| w == &word).count() > 1
                            && word.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            return Idempotency::NonIdempotent;
                        }
                    }
                }
            }
        }

        return Idempotency::Idempotent;
    }

    // DELETE with WHERE clause is idempotent
    if sql_upper.starts_with("DELETE") && sql_upper.contains("WHERE") {
        return Idempotency::Idempotent;
    }

    // INSERT ... ON CONFLICT DO NOTHING is idempotent
    if sql_upper.starts_with("INSERT") && sql_upper.contains("ON CONFLICT DO NOTHING") {
        return Idempotency::Idempotent;
    }

    // INSERT ... ON CONFLICT DO UPDATE with specific values is idempotent
    if sql_upper.starts_with("INSERT") && sql_upper.contains("ON CONFLICT DO UPDATE") {
        // Simple heuristic: if not incrementing values
        if !sql_upper.contains("+=") && !sql_upper.contains("+ 1") {
            return Idempotency::Idempotent;
        }
    }

    // Everything else is considered non-idempotent
    Idempotency::NonIdempotent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_strategy_default() {
        let strategy = RetryStrategy::default();
        match strategy {
            RetryStrategy::Exponential { max_attempts, .. } => {
                assert_eq!(max_attempts, 3);
            }
            _ => panic!("Expected exponential strategy"),
        }
    }

    #[test]
    fn test_error_classification() {
        let error = DbError::NotFound("test".to_string());
        let policy = RetryPolicy::new();
        assert_eq!(policy.classify_error(&error), ErrorClass::Permanent);

        let error = DbError::Duplicate("test".to_string());
        assert_eq!(policy.classify_error(&error), ErrorClass::Permanent);

        let error = DbError::Pool("test".to_string());
        assert_eq!(policy.classify_error(&error), ErrorClass::Transient);
    }

    #[test]
    fn test_idempotency_detection_select() {
        let sql = "SELECT * FROM users WHERE id = 1";
        assert_eq!(detect_idempotency(sql), Idempotency::Idempotent);
    }

    #[test]
    fn test_idempotency_detection_update() {
        let sql = "UPDATE users SET name = 'John' WHERE id = 1";
        assert_eq!(detect_idempotency(sql), Idempotency::Idempotent);

        let sql_increment = "UPDATE users SET count = count + 1 WHERE id = 1";
        assert_eq!(
            detect_idempotency(sql_increment),
            Idempotency::NonIdempotent
        );
    }

    #[test]
    fn test_idempotency_detection_delete() {
        let sql = "DELETE FROM users WHERE id = 1";
        assert_eq!(detect_idempotency(sql), Idempotency::Idempotent);
    }

    #[test]
    fn test_idempotency_detection_insert() {
        let sql = "INSERT INTO users (name) VALUES ('John')";
        assert_eq!(detect_idempotency(sql), Idempotency::NonIdempotent);

        let sql_upsert = "INSERT INTO users (id, name) VALUES (1, 'John') ON CONFLICT DO NOTHING";
        assert_eq!(detect_idempotency(sql_upsert), Idempotency::Idempotent);

        let sql_update = "INSERT INTO users (id, name) VALUES (1, 'John') ON CONFLICT DO UPDATE SET name = 'John'";
        assert_eq!(detect_idempotency(sql_update), Idempotency::Idempotent);
    }

    #[test]
    fn test_idempotency_enum() {
        assert_eq!(Idempotency::Idempotent, Idempotency::Idempotent);
        assert_ne!(Idempotency::Idempotent, Idempotency::NonIdempotent);
        assert_ne!(Idempotency::Idempotent, Idempotency::Unknown);
    }

    #[test]
    fn test_retry_policy_config_default() {
        let config = RetryPolicyConfig::default();
        assert!(config.idempotent_only);
        assert!(config.classify_error.is_none());
    }

    #[test]
    fn test_error_class_enum() {
        assert_eq!(ErrorClass::Transient, ErrorClass::Transient);
        assert_ne!(ErrorClass::Transient, ErrorClass::Permanent);
        assert_ne!(ErrorClass::Transient, ErrorClass::Unknown);
    }
}
