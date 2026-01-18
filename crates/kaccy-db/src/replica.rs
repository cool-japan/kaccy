//! Read replica support for database connections
//!
//! Provides automatic routing of read queries to replica databases
//! while keeping writes on the primary.

use rand::prelude::IndexedRandom;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{DbError, Result};
use crate::pool::{health_check, HealthCheck, HealthStatus, RetryConfig};

/// Configuration for read replica setup
#[derive(Debug, Clone, Deserialize)]
pub struct ReplicaConfig {
    /// Primary database URL (for writes)
    pub primary_url: String,
    /// Read replica URLs (for reads)
    pub replica_urls: Vec<String>,
    /// Maximum connections per pool
    pub max_connections: u32,
    /// Minimum connections per pool
    pub min_connections: u32,
    /// Acquire timeout in seconds
    pub acquire_timeout_secs: u64,
    /// Load balancing strategy
    pub load_balance_strategy: LoadBalanceStrategy,
}

impl Default for ReplicaConfig {
    fn default() -> Self {
        Self {
            primary_url: String::new(),
            replica_urls: Vec::new(),
            max_connections: 20,
            min_connections: 5,
            acquire_timeout_secs: 5,
            load_balance_strategy: LoadBalanceStrategy::RoundRobin,
        }
    }
}

impl ReplicaConfig {
    /// Create config with just primary (no replicas)
    pub fn primary_only(url: impl Into<String>) -> Self {
        Self {
            primary_url: url.into(),
            ..Default::default()
        }
    }

    /// Add a replica URL
    pub fn add_replica(mut self, url: impl Into<String>) -> Self {
        self.replica_urls.push(url.into());
        self
    }

    /// Set load balancing strategy
    pub fn strategy(mut self, strategy: LoadBalanceStrategy) -> Self {
        self.load_balance_strategy = strategy;
        self
    }
}

/// Load balancing strategy for read replicas
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    /// Distribute reads evenly across replicas
    #[default]
    RoundRobin,
    /// Randomly select a replica for each read
    Random,
    /// Always use the first available replica
    FirstAvailable,
    /// Use replica with least connections (estimated)
    LeastConnections,
}

/// Replica pool status
#[derive(Debug, Clone, Serialize)]
pub struct ReplicaStatus {
    pub url_masked: String,
    pub is_healthy: bool,
    pub pool_size: u32,
    pub pool_idle: u32,
    pub latency_ms: Option<u64>,
}

/// Database pool manager with read replica support
pub struct ReplicaPoolManager {
    /// Primary pool (for writes)
    primary: PgPool,
    /// Read replica pools
    replicas: Vec<PgPool>,
    /// Current index for round-robin
    round_robin_index: AtomicUsize,
    /// Load balancing strategy
    strategy: LoadBalanceStrategy,
    /// Configuration
    #[allow(dead_code)]
    config: ReplicaConfig,
}

impl ReplicaPoolManager {
    /// Create a new replica pool manager
    pub async fn new(config: ReplicaConfig) -> Result<Self> {
        let retry_config = RetryConfig::default();
        Self::with_retry(config, &retry_config).await
    }

    /// Create with retry configuration
    pub async fn with_retry(config: ReplicaConfig, retry_config: &RetryConfig) -> Result<Self> {
        // Create primary pool
        let primary = create_pool_with_config(&config.primary_url, &config, retry_config).await?;
        tracing::info!("Primary database pool created");

        // Create replica pools
        let mut replicas = Vec::with_capacity(config.replica_urls.len());
        for (i, url) in config.replica_urls.iter().enumerate() {
            match create_pool_with_config(url, &config, retry_config).await {
                Ok(pool) => {
                    replicas.push(pool);
                    tracing::info!(replica_index = i, "Read replica pool created");
                }
                Err(e) => {
                    tracing::warn!(
                        replica_index = i,
                        error = %e,
                        "Failed to create read replica pool, skipping"
                    );
                }
            }
        }

        if replicas.is_empty() && !config.replica_urls.is_empty() {
            tracing::warn!("No read replicas available, falling back to primary for reads");
        }

        Ok(Self {
            primary,
            replicas,
            round_robin_index: AtomicUsize::new(0),
            strategy: config.load_balance_strategy,
            config,
        })
    }

    /// Get a pool for write operations (always primary)
    pub fn write_pool(&self) -> &PgPool {
        &self.primary
    }

    /// Get a pool for read operations
    pub fn read_pool(&self) -> &PgPool {
        if self.replicas.is_empty() {
            return &self.primary;
        }

        match self.strategy {
            LoadBalanceStrategy::RoundRobin => self.round_robin_replica(),
            LoadBalanceStrategy::Random => self.random_replica(),
            LoadBalanceStrategy::FirstAvailable => self.first_available_replica(),
            LoadBalanceStrategy::LeastConnections => self.least_connections_replica(),
        }
    }

    /// Get primary pool (alias for write_pool)
    pub fn primary(&self) -> &PgPool {
        &self.primary
    }

    /// Get all pools (primary + replicas) for operations that need both
    pub fn all_pools(&self) -> impl Iterator<Item = &PgPool> {
        std::iter::once(&self.primary).chain(self.replicas.iter())
    }

    /// Get number of available replicas
    pub fn replica_count(&self) -> usize {
        self.replicas.len()
    }

    /// Check if replicas are configured
    pub fn has_replicas(&self) -> bool {
        !self.replicas.is_empty()
    }

    /// Get health status of all pools
    pub async fn health_status(&self) -> ReplicaHealthStatus {
        let primary_health = health_check(&self.primary).await;

        let mut replica_health = Vec::with_capacity(self.replicas.len());
        for (i, pool) in self.replicas.iter().enumerate() {
            let health = health_check(pool).await;
            replica_health.push(ReplicaStatus {
                url_masked: format!("replica_{}", i),
                is_healthy: health.status == HealthStatus::Healthy,
                pool_size: health.pool_size,
                pool_idle: health.pool_idle,
                latency_ms: health.latency_ms,
            });
        }

        let healthy_replicas = replica_health.iter().filter(|r| r.is_healthy).count();
        let overall_status = if primary_health.status != HealthStatus::Healthy {
            HealthStatus::Unhealthy
        } else if healthy_replicas < self.replicas.len() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        ReplicaHealthStatus {
            overall_status,
            primary: primary_health,
            replicas: replica_health,
            healthy_replica_count: healthy_replicas,
            total_replica_count: self.replicas.len(),
        }
    }

    // Internal methods for load balancing

    fn round_robin_replica(&self) -> &PgPool {
        let index = self.round_robin_index.fetch_add(1, Ordering::Relaxed) % self.replicas.len();
        &self.replicas[index]
    }

    fn random_replica(&self) -> &PgPool {
        self.replicas
            .choose(&mut rand::rng())
            .unwrap_or(&self.primary)
    }

    fn first_available_replica(&self) -> &PgPool {
        // Return first replica with idle connections
        for replica in &self.replicas {
            if replica.num_idle() > 0 {
                return replica;
            }
        }
        // Fall back to first replica
        &self.replicas[0]
    }

    fn least_connections_replica(&self) -> &PgPool {
        self.replicas
            .iter()
            .max_by_key(|p| p.num_idle())
            .unwrap_or(&self.primary)
    }
}

/// Health status for the replica setup
#[derive(Debug, Serialize)]
pub struct ReplicaHealthStatus {
    pub overall_status: HealthStatus,
    pub primary: HealthCheck,
    pub replicas: Vec<ReplicaStatus>,
    pub healthy_replica_count: usize,
    pub total_replica_count: usize,
}

/// Create a pool with the given configuration
async fn create_pool_with_config(
    url: &str,
    config: &ReplicaConfig,
    retry_config: &RetryConfig,
) -> Result<PgPool> {
    let mut last_error = None;

    for attempt in 0..retry_config.max_attempts {
        match try_create_pool(url, config).await {
            Ok(pool) => return Ok(pool),
            Err(e) => {
                last_error = Some(e);
                if attempt + 1 < retry_config.max_attempts {
                    let delay = retry_config.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(DbError::Connection(format!(
        "Failed to create pool after {} attempts: {}",
        retry_config.max_attempts,
        last_error.map(|e| e.to_string()).unwrap_or_default()
    )))
}

async fn try_create_pool(
    url: &str,
    config: &ReplicaConfig,
) -> std::result::Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(600))
        .connect(url)
        .await
}

/// Smart database client that routes queries appropriately
pub struct SmartDbClient {
    manager: Arc<ReplicaPoolManager>,
}

impl SmartDbClient {
    pub fn new(manager: ReplicaPoolManager) -> Self {
        Self {
            manager: Arc::new(manager),
        }
    }

    pub fn from_arc(manager: Arc<ReplicaPoolManager>) -> Self {
        Self { manager }
    }

    /// Execute a read-only query (uses replicas if available)
    pub fn read(&self) -> &PgPool {
        self.manager.read_pool()
    }

    /// Execute a write query (always uses primary)
    pub fn write(&self) -> &PgPool {
        self.manager.write_pool()
    }

    /// Get the underlying manager
    pub fn manager(&self) -> &ReplicaPoolManager {
        &self.manager
    }

    /// Get shared reference to manager
    pub fn shared_manager(&self) -> Arc<ReplicaPoolManager> {
        self.manager.clone()
    }
}

impl Clone for SmartDbClient {
    fn clone(&self) -> Self {
        Self {
            manager: self.manager.clone(),
        }
    }
}

/// Builder for creating SmartDbClient
pub struct SmartDbClientBuilder {
    config: ReplicaConfig,
}

impl SmartDbClientBuilder {
    pub fn new(primary_url: impl Into<String>) -> Self {
        Self {
            config: ReplicaConfig::primary_only(primary_url),
        }
    }

    pub fn add_replica(mut self, url: impl Into<String>) -> Self {
        self.config.replica_urls.push(url.into());
        self
    }

    pub fn max_connections(mut self, max: u32) -> Self {
        self.config.max_connections = max;
        self
    }

    pub fn min_connections(mut self, min: u32) -> Self {
        self.config.min_connections = min;
        self
    }

    pub fn strategy(mut self, strategy: LoadBalanceStrategy) -> Self {
        self.config.load_balance_strategy = strategy;
        self
    }

    pub async fn build(self) -> Result<SmartDbClient> {
        let manager = ReplicaPoolManager::new(self.config).await?;
        Ok(SmartDbClient::new(manager))
    }
}
