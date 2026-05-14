//! Database Read Replicas
//!
//! This module provides read replica management for database scaling.
//! Read replicas allow read queries to be distributed across multiple database
//! instances, improving read performance and availability.
//!
//! # Features
//!
//! - Master-slave replication tracking
//! - Read query routing to replicas
//! - Replication lag monitoring
//! - Automatic failover mechanisms
//!
//! # Examples
//!
//! ```
//! use kaccy_core::utils::db_replicas::{ReplicaManager, ReplicaConfig};
//!
//! let config = ReplicaConfig::default();
//! let mut manager = ReplicaManager::new(
//!     "postgresql://localhost:5432/master",
//!     config
//! );
//!
//! manager.add_replica("replica_1", "postgresql://localhost:5433/db");
//! manager.add_replica("replica_2", "postgresql://localhost:5434/db");
//!
//! // Route a read query
//! let replica_url = manager.get_replica_for_read();
//! ```

use crate::{CoreError as Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Load balancing strategy for read replicas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    /// Round-robin selection
    RoundRobin,
    /// Random selection
    Random,
    /// Least connections
    LeastConnections,
    /// Least replication lag
    LeastLag,
}

/// Replica configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaConfig {
    /// Maximum acceptable replication lag (milliseconds)
    pub max_replication_lag_ms: u64,
    /// Health check interval (seconds)
    pub health_check_interval_secs: u64,
    /// Load balancing strategy
    pub load_balance_strategy: LoadBalanceStrategy,
    /// Automatic failover enabled
    pub auto_failover: bool,
}

impl Default for ReplicaConfig {
    fn default() -> Self {
        Self {
            max_replication_lag_ms: 1000, // 1 second
            health_check_interval_secs: 30,
            load_balance_strategy: LoadBalanceStrategy::LeastLag,
            auto_failover: true,
        }
    }
}

/// Replica information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replica {
    /// Replica identifier
    pub id: String,
    /// Database connection string
    pub connection_string: String,
    /// Is replica currently available?
    pub is_available: bool,
    /// Replication lag in milliseconds
    pub replication_lag_ms: u64,
    /// Number of active connections
    pub active_connections: usize,
    /// Last health check time (not serialized)
    #[serde(skip)]
    pub last_health_check: Option<Instant>,
    /// Weight for load balancing (0.0 to 1.0)
    pub weight: f64,
}

impl Replica {
    /// Create a new replica
    pub fn new(id: String, connection_string: String) -> Self {
        Self {
            id,
            connection_string,
            is_available: true,
            replication_lag_ms: 0,
            active_connections: 0,
            last_health_check: None,
            weight: 1.0,
        }
    }

    /// Check if replica is healthy
    pub fn is_healthy(&self, max_lag_ms: u64) -> bool {
        self.is_available && self.replication_lag_ms <= max_lag_ms
    }
}

/// Replica manager
pub struct ReplicaManager {
    /// Master database connection string
    master_connection: String,
    /// Configuration
    config: ReplicaConfig,
    /// Read replicas
    replicas: HashMap<String, Replica>,
    /// Round-robin index
    round_robin_index: usize,
}

impl ReplicaManager {
    /// Create a new replica manager
    pub fn new(master_connection: &str, config: ReplicaConfig) -> Self {
        Self {
            master_connection: master_connection.to_string(),
            config,
            replicas: HashMap::new(),
            round_robin_index: 0,
        }
    }

    /// Add a read replica
    pub fn add_replica(&mut self, id: &str, connection_string: &str) -> Result<()> {
        let replica = Replica::new(id.to_string(), connection_string.to_string());
        self.replicas.insert(id.to_string(), replica);
        Ok(())
    }

    /// Remove a replica
    pub fn remove_replica(&mut self, id: &str) -> Result<()> {
        self.replicas.remove(id);
        Ok(())
    }

    /// Get connection string for read query
    pub fn get_replica_for_read(&mut self) -> String {
        let max_lag = self.config.max_replication_lag_ms;
        let strategy = self.config.load_balance_strategy;

        let healthy_replicas: Vec<_> = self
            .replicas
            .values()
            .filter(|r| r.is_healthy(max_lag))
            .cloned()
            .collect();

        if healthy_replicas.is_empty() {
            // Fall back to master if no replicas available
            return self.master_connection.clone();
        }

        match strategy {
            LoadBalanceStrategy::RoundRobin => self.get_replica_round_robin(&healthy_replicas),
            LoadBalanceStrategy::Random => self.get_replica_random(&healthy_replicas),
            LoadBalanceStrategy::LeastConnections => {
                self.get_replica_least_connections(&healthy_replicas)
            }
            LoadBalanceStrategy::LeastLag => self.get_replica_least_lag(&healthy_replicas),
        }
    }

    /// Get connection string for write query (always master)
    pub fn get_master_for_write(&self) -> String {
        self.master_connection.clone()
    }

    /// Update replication lag for a replica
    pub fn update_replication_lag(&mut self, replica_id: &str, lag_ms: u64) -> Result<()> {
        if let Some(replica) = self.replicas.get_mut(replica_id) {
            replica.replication_lag_ms = lag_ms;
            replica.last_health_check = Some(Instant::now());
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "Replica {} not found",
                replica_id
            )))
        }
    }

    /// Mark replica as available/unavailable
    pub fn set_replica_availability(&mut self, replica_id: &str, available: bool) -> Result<()> {
        if let Some(replica) = self.replicas.get_mut(replica_id) {
            replica.is_available = available;
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "Replica {} not found",
                replica_id
            )))
        }
    }

    /// Get replica statistics
    pub fn get_replica_stats(&self) -> Vec<ReplicaStats> {
        self.replicas
            .values()
            .map(|r| ReplicaStats {
                replica_id: r.id.clone(),
                is_available: r.is_available,
                replication_lag_ms: r.replication_lag_ms,
                active_connections: r.active_connections,
                is_healthy: r.is_healthy(self.config.max_replication_lag_ms),
            })
            .collect()
    }

    /// Perform health check on all replicas
    pub fn health_check(&mut self) -> HealthCheckReport {
        let total = self.replicas.len();
        let mut healthy = 0;
        let mut lagging = 0;
        let mut unavailable = 0;

        for replica in self.replicas.values() {
            if !replica.is_available {
                unavailable += 1;
            } else if replica.replication_lag_ms > self.config.max_replication_lag_ms {
                lagging += 1;
            } else {
                healthy += 1;
            }
        }

        HealthCheckReport {
            total_replicas: total,
            healthy_replicas: healthy,
            lagging_replicas: lagging,
            unavailable_replicas: unavailable,
        }
    }

    /// Initiate failover to a replica (promote replica to master)
    pub fn failover_to_replica(&mut self, replica_id: &str) -> Result<FailoverResult> {
        if !self.replicas.contains_key(replica_id) {
            return Err(Error::Validation(format!(
                "Replica {} not found",
                replica_id
            )));
        }

        let replica = self.replicas.get(replica_id).expect("replica_id confirmed present via contains_key check above");
        if !replica.is_available {
            return Err(Error::Validation(format!(
                "Replica {} is not available",
                replica_id
            )));
        }

        let old_master = self.master_connection.clone();
        let new_master = replica.connection_string.clone();

        // Update master connection
        self.master_connection = new_master.clone();

        // Remove promoted replica from replica pool
        self.replicas.remove(replica_id);

        Ok(FailoverResult {
            old_master,
            new_master,
            promoted_replica_id: replica_id.to_string(),
        })
    }

    /// Get replica using round-robin strategy
    fn get_replica_round_robin(&mut self, replicas: &[Replica]) -> String {
        let replica = &replicas[self.round_robin_index % replicas.len()];
        self.round_robin_index = (self.round_robin_index + 1) % replicas.len();
        replica.connection_string.clone()
    }

    /// Get replica using random strategy
    fn get_replica_random(&self, replicas: &[Replica]) -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::BuildHasher;

        let random_state = RandomState::new();
        let index = (random_state.hash_one(Instant::now()) as usize) % replicas.len();

        replicas[index].connection_string.clone()
    }

    /// Get replica with least connections
    fn get_replica_least_connections(&self, replicas: &[Replica]) -> String {
        replicas
            .iter()
            .min_by_key(|r| r.active_connections)
            .map(|r| r.connection_string.clone())
            .unwrap_or_else(|| self.master_connection.clone())
    }

    /// Get replica with least replication lag
    fn get_replica_least_lag(&self, replicas: &[Replica]) -> String {
        replicas
            .iter()
            .min_by_key(|r| r.replication_lag_ms)
            .map(|r| r.connection_string.clone())
            .unwrap_or_else(|| self.master_connection.clone())
    }
}

/// Replica statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaStats {
    /// Replica identifier
    pub replica_id: String,
    /// Is replica available?
    pub is_available: bool,
    /// Replication lag in milliseconds
    pub replication_lag_ms: u64,
    /// Number of active connections
    pub active_connections: usize,
    /// Is replica healthy?
    pub is_healthy: bool,
}

/// Health check report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckReport {
    /// Total number of replicas
    pub total_replicas: usize,
    /// Number of healthy replicas
    pub healthy_replicas: usize,
    /// Number of lagging replicas
    pub lagging_replicas: usize,
    /// Number of unavailable replicas
    pub unavailable_replicas: usize,
}

impl HealthCheckReport {
    /// Get health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_replicas == 0 {
            100.0
        } else {
            (self.healthy_replicas as f64 / self.total_replicas as f64) * 100.0
        }
    }
}

/// Failover result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverResult {
    /// Old master connection string
    pub old_master: String,
    /// New master connection string
    pub new_master: String,
    /// ID of the promoted replica
    pub promoted_replica_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replica_manager_creation() {
        let config = ReplicaConfig::default();
        let manager = ReplicaManager::new("postgresql://localhost/master", config);
        assert_eq!(manager.replicas.len(), 0);
    }

    #[test]
    fn test_add_replica() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        assert!(
            manager
                .add_replica("replica_1", "postgresql://localhost/replica_1")
                .is_ok()
        );
        assert_eq!(manager.replicas.len(), 1);
    }

    #[test]
    fn test_remove_replica() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        manager
            .add_replica("replica_1", "postgresql://localhost/replica_1")
            .unwrap();
        assert_eq!(manager.replicas.len(), 1);

        assert!(manager.remove_replica("replica_1").is_ok());
        assert_eq!(manager.replicas.len(), 0);
    }

    #[test]
    fn test_get_replica_for_read() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        manager
            .add_replica("replica_1", "postgresql://localhost/replica_1")
            .unwrap();

        let conn = manager.get_replica_for_read();
        assert!(conn.contains("postgresql://"));
    }

    #[test]
    fn test_get_master_for_write() {
        let config = ReplicaConfig::default();
        let manager = ReplicaManager::new("postgresql://localhost/master", config);

        let conn = manager.get_master_for_write();
        assert_eq!(conn, "postgresql://localhost/master");
    }

    #[test]
    fn test_update_replication_lag() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        manager
            .add_replica("replica_1", "postgresql://localhost/replica_1")
            .unwrap();

        assert!(manager.update_replication_lag("replica_1", 500).is_ok());

        let stats = manager.get_replica_stats();
        assert_eq!(stats[0].replication_lag_ms, 500);
    }

    #[test]
    fn test_health_check() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        manager
            .add_replica("replica_1", "postgresql://localhost/replica_1")
            .unwrap();
        manager
            .add_replica("replica_2", "postgresql://localhost/replica_2")
            .unwrap();

        let report = manager.health_check();
        assert_eq!(report.total_replicas, 2);
        assert_eq!(report.healthy_replicas, 2);
    }

    #[test]
    fn test_failover() {
        let config = ReplicaConfig::default();
        let mut manager = ReplicaManager::new("postgresql://localhost/master", config);

        manager
            .add_replica("replica_1", "postgresql://localhost/replica_1")
            .unwrap();

        let result = manager.failover_to_replica("replica_1").unwrap();
        assert_eq!(result.old_master, "postgresql://localhost/master");
        assert_eq!(result.new_master, "postgresql://localhost/replica_1");
        assert_eq!(manager.replicas.len(), 0); // Promoted replica removed from pool
    }

    #[test]
    fn test_health_percentage() {
        let report = HealthCheckReport {
            total_replicas: 4,
            healthy_replicas: 3,
            lagging_replicas: 1,
            unavailable_replicas: 0,
        };
        assert_eq!(report.health_percentage(), 75.0);
    }
}
