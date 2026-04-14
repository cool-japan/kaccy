//! Database connection pool management

use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur when creating or managing connection pools.
#[derive(Error, Debug)]
pub enum PoolError {
    /// Initial connection to the database failed.
    #[error("Failed to create connection pool: {0}")]
    ConnectionFailed(#[from] sqlx::Error),

    /// All retry attempts were exhausted without a successful connection.
    #[error("Connection failed after {attempts} retry attempts: {last_error}")]
    RetryExhausted {
        /// Number of attempts that were made.
        attempts: u32,
        /// Error message from the last attempt.
        last_error: String,
    },
}

/// Retry configuration for database connections
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given attempt using exponential backoff
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay_ms as f64;
        let multiplier = self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = (base_delay * multiplier).min(self.max_delay_ms as f64) as u64;
        Duration::from_millis(delay_ms)
    }
}

/// Create a PostgreSQL connection pool with optimized settings
pub async fn create_pool(database_url: &str) -> Result<PgPool, PoolError> {
    create_pool_with_retry(database_url, &RetryConfig::default()).await
}

/// Create a PostgreSQL connection pool with retry logic
pub async fn create_pool_with_retry(
    database_url: &str,
    retry_config: &RetryConfig,
) -> Result<PgPool, PoolError> {
    let mut last_error = String::new();

    for attempt in 0..retry_config.max_attempts {
        match try_create_pool(database_url).await {
            Ok(pool) => {
                if attempt > 0 {
                    tracing::info!(
                        attempt = attempt + 1,
                        "Database connection pool created after retry"
                    );
                } else {
                    tracing::info!("Database connection pool created successfully");
                }
                return Ok(pool);
            }
            Err(e) => {
                last_error = e.to_string();
                let remaining = retry_config.max_attempts - attempt - 1;

                if remaining > 0 {
                    let delay = retry_config.delay_for_attempt(attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        remaining_attempts = remaining,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "Database connection failed, retrying..."
                    );
                    tokio::time::sleep(delay).await;
                } else {
                    tracing::error!(
                        attempts = retry_config.max_attempts,
                        error = %e,
                        "Database connection failed, no retries remaining"
                    );
                }
            }
        }
    }

    Err(PoolError::RetryExhausted {
        attempts: retry_config.max_attempts,
        last_error,
    })
}

/// Internal function to attempt pool creation
async fn try_create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(database_url)
        .await
}

/// Create a pool with custom settings
pub async fn create_pool_with_options(
    database_url: &str,
    max_connections: u32,
    min_connections: u32,
    acquire_timeout_secs: u64,
) -> Result<PgPool, PoolError> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(600))
        .connect(database_url)
        .await?;

    tracing::info!(
        max_connections = max_connections,
        min_connections = min_connections,
        "Database connection pool created with custom settings"
    );
    Ok(pool)
}

/// Health check result
#[derive(Debug, Serialize)]
pub struct HealthCheck {
    /// Overall health status of the pool.
    pub status: HealthStatus,
    /// Whether a connection to the database was established.
    pub database_connected: bool,
    /// Current total number of connections in the pool.
    pub pool_size: u32,
    /// Number of idle connections available.
    pub pool_idle: u32,
    /// Round-trip query latency in milliseconds.
    pub latency_ms: Option<u64>,
    /// PostgreSQL server version string.
    pub version: Option<String>,
}

/// Health status of a database connection or pool.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// All checks passed and latency is acceptable.
    Healthy,
    /// Connected but latency is elevated.
    Degraded,
    /// Could not connect or query the database.
    Unhealthy,
}

/// Perform a health check on the database connection
pub async fn health_check(pool: &PgPool) -> HealthCheck {
    let pool_size = pool.size();
    let pool_idle = pool.num_idle() as u32;

    let start = std::time::Instant::now();
    let query_result = sqlx::query("SELECT version()").fetch_optional(pool).await;
    let latency = start.elapsed().as_millis() as u64;

    match query_result {
        Ok(Some(row)) => {
            let version: String = row.get(0);
            let status = if latency > 1000 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };

            HealthCheck {
                status,
                database_connected: true,
                pool_size,
                pool_idle,
                latency_ms: Some(latency),
                version: Some(version),
            }
        }
        Ok(None) => HealthCheck {
            status: HealthStatus::Degraded,
            database_connected: true,
            pool_size,
            pool_idle,
            latency_ms: Some(latency),
            version: None,
        },
        Err(e) => {
            tracing::error!(error = %e, "Database health check failed");
            HealthCheck {
                status: HealthStatus::Unhealthy,
                database_connected: false,
                pool_size,
                pool_idle,
                latency_ms: None,
                version: None,
            }
        }
    }
}

/// Pool statistics
#[derive(Debug, Serialize)]
pub struct PoolStats {
    /// Total number of connections (idle + in-use).
    pub size: u32,
    /// Number of connections currently idle.
    pub idle: u32,
    /// Number of connections currently in use.
    pub in_use: u32,
}

/// Get current pool statistics
pub fn pool_stats(pool: &PgPool) -> PoolStats {
    let size = pool.size();
    let idle = pool.num_idle() as u32;
    PoolStats {
        size,
        idle,
        in_use: size.saturating_sub(idle),
    }
}

/// Warm-up strategy for connection pool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmupStrategy {
    /// No warm-up, connections created on-demand
    None,
    /// Pre-populate pool to minimum connections
    MinConnections,
    /// Pre-populate pool to half capacity
    HalfCapacity,
    /// Pre-populate pool to full capacity
    FullCapacity,
}

/// Warm up a connection pool by pre-creating connections
///
/// This helps reduce initial latency by establishing connections
/// before they're needed.
pub async fn warmup_pool(pool: &PgPool, strategy: WarmupStrategy) -> Result<(), PoolError> {
    let target = match strategy {
        WarmupStrategy::None => return Ok(()),
        WarmupStrategy::MinConnections => 5, // Default min connections
        WarmupStrategy::HalfCapacity => pool.size() / 2,
        WarmupStrategy::FullCapacity => pool.size(),
    };

    tracing::info!(
        strategy = ?strategy,
        target = target,
        "Warming up connection pool"
    );

    // Acquire and immediately release connections to populate the pool
    let mut connections = Vec::new();
    for i in 0..target {
        match pool.acquire().await {
            Ok(conn) => {
                connections.push(conn);
                tracing::debug!(acquired = i + 1, target = target, "Pool warm-up progress");
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    acquired = i,
                    target = target,
                    "Failed to warm up pool"
                );
                return Err(PoolError::ConnectionFailed(e));
            }
        }
    }

    // Release all connections back to the pool
    drop(connections);

    tracing::info!(
        warmed_connections = target,
        "Connection pool warm-up completed"
    );

    Ok(())
}

/// Validate pool connections by running a simple query
///
/// This ensures all connections in the pool are healthy
pub async fn validate_pool_connections(pool: &PgPool) -> Result<u32, PoolError> {
    let pool_size = pool.size();
    let mut valid_count = 0;

    tracing::info!(pool_size = pool_size, "Validating pool connections");

    for i in 0..pool_size {
        match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => {
                valid_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    connection = i,
                    error = %e,
                    "Connection validation failed"
                );
            }
        }
    }

    tracing::info!(
        valid_count = valid_count,
        total = pool_size,
        "Connection validation completed"
    );

    Ok(valid_count)
}

/// Periodically refresh pool connections
///
/// This helps maintain connection health by cycling through
/// idle connections
pub async fn refresh_pool_connections(pool: &PgPool, interval_secs: u64) -> Result<(), PoolError> {
    let interval = Duration::from_secs(interval_secs);

    loop {
        tokio::time::sleep(interval).await;

        tracing::debug!("Refreshing pool connections");

        // Acquire and release a connection to ensure freshness
        match pool.acquire().await {
            Ok(mut conn) => {
                // Simple validation query
                if let Err(e) = sqlx::query("SELECT 1").execute(&mut *conn).await {
                    tracing::warn!(error = %e, "Connection refresh validation failed");
                }
                drop(conn);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to acquire connection for refresh");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_create_pool() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = create_pool(&url).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_health_check() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = create_pool(&url).await.unwrap();
        let health = health_check(&pool).await;
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.database_connected);
    }

    #[test]
    fn test_warmup_strategy() {
        assert_eq!(WarmupStrategy::None, WarmupStrategy::None);
        assert_ne!(WarmupStrategy::None, WarmupStrategy::MinConnections);
    }

    #[test]
    fn test_retry_config_delay() {
        let config = RetryConfig::default();

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        assert_eq!(delay0.as_millis(), 100);
        assert_eq!(delay1.as_millis(), 200);
        assert_eq!(delay2.as_millis(), 400);
    }

    #[test]
    fn test_retry_config_max_delay() {
        let config = RetryConfig {
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            backoff_multiplier: 10.0,
            max_attempts: 10,
        };

        let delay = config.delay_for_attempt(5);
        assert_eq!(delay.as_millis(), 5000); // Capped at max_delay_ms
    }
}
