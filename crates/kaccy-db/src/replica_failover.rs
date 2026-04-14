//! Replica automatic failover testing and implementation
//!
//! Provides utilities for testing replica failures, automatic primary promotion,
//! and replication lag recovery.

use crate::error::{DbError, Result};
use parking_lot::RwLock;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Health status for replica nodes
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Replica is healthy
    Healthy,
    /// Replica is unhealthy
    Unhealthy,
    /// Health status unknown
    Unknown,
}

/// Replica role in the cluster
#[derive(Debug, Clone, PartialEq)]
pub enum ReplicaRole {
    /// Primary (master) database
    Primary,
    /// Standby replica
    Standby,
    /// Failed replica
    Failed,
}

/// Failover strategy
#[derive(Debug, Clone)]
pub enum FailoverStrategy {
    /// Manual failover (requires explicit trigger)
    Manual,
    /// Automatic failover on primary failure
    Automatic {
        /// Health check interval
        check_interval: Duration,
        /// Number of consecutive failures before failover
        failure_threshold: usize,
    },
}

/// Configuration for replica failover
#[derive(Debug, Clone)]
pub struct FailoverConfig {
    /// Failover strategy
    pub strategy: FailoverStrategy,
    /// Maximum replication lag before considering replica unhealthy
    pub max_replication_lag: Duration,
    /// Timeout for promotion operation
    pub promotion_timeout: Duration,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            strategy: FailoverStrategy::Automatic {
                check_interval: Duration::from_secs(10),
                failure_threshold: 3,
            },
            max_replication_lag: Duration::from_secs(30),
            promotion_timeout: Duration::from_secs(60),
        }
    }
}

/// Replica node in the failover cluster
#[derive(Debug)]
pub struct ReplicaNode {
    /// Database connection pool for this node.
    pub pool: PgPool,
    /// Current role (Primary, Standby, or Failed) protected by a lock.
    pub role: Arc<RwLock<ReplicaRole>>,
    /// Most recent health check result protected by a lock.
    pub health: Arc<RwLock<HealthStatus>>,
    /// Number of health check failures since the last success.
    pub consecutive_failures: Arc<RwLock<usize>>,
    /// Timestamp of the last health check, or `None` if never checked.
    pub last_health_check: Arc<RwLock<Option<Instant>>>,
}

impl ReplicaNode {
    /// Create a new replica node
    pub fn new(pool: PgPool, role: ReplicaRole) -> Self {
        Self {
            pool,
            role: Arc::new(RwLock::new(role)),
            health: Arc::new(RwLock::new(HealthStatus::Healthy)),
            consecutive_failures: Arc::new(RwLock::new(0)),
            last_health_check: Arc::new(RwLock::new(None)),
        }
    }

    /// Get current role
    pub fn role(&self) -> ReplicaRole {
        self.role.read().clone()
    }

    /// Set role
    pub fn set_role(&self, role: ReplicaRole) {
        *self.role.write() = role;
    }

    /// Perform health check
    pub async fn health_check(&self) -> Result<bool> {
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {
                // Update health status
                *self.health.write() = HealthStatus::Healthy;
                *self.consecutive_failures.write() = 0;
                *self.last_health_check.write() = Some(Instant::now());

                Ok(true)
            }
            Err(e) => {
                // Update health status
                *self.health.write() = HealthStatus::Unhealthy;
                *self.consecutive_failures.write() += 1;
                *self.last_health_check.write() = Some(Instant::now());

                Err(DbError::from(e))
            }
        }
    }

    /// Check replication lag (in seconds)
    pub async fn check_replication_lag(&self) -> Result<f64> {
        // Query to check replication lag on PostgreSQL
        let row = sqlx::query_as::<_, (Option<f64>,)>(
            "SELECT EXTRACT(EPOCH FROM (now() - pg_last_xact_replay_timestamp()))",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(row.0.unwrap_or(0.0))
    }

    /// Promote replica to primary
    pub async fn promote_to_primary(&self) -> Result<()> {
        // This would typically execute: pg_ctl promote
        // For testing, we just update the role
        *self.role.write() = ReplicaRole::Primary;
        Ok(())
    }

    /// Demote primary to standby
    pub async fn demote_to_standby(&self) -> Result<()> {
        *self.role.write() = ReplicaRole::Standby;
        Ok(())
    }
}

/// Failover manager for replica cluster
pub struct FailoverManager {
    nodes: Vec<ReplicaNode>,
    config: FailoverConfig,
    primary_index: Arc<RwLock<Option<usize>>>,
}

impl FailoverManager {
    /// Create a new failover manager
    pub fn new(nodes: Vec<ReplicaNode>, config: FailoverConfig) -> Self {
        // Find primary node
        let primary_index = nodes.iter().position(|n| n.role() == ReplicaRole::Primary);

        Self {
            nodes,
            config,
            primary_index: Arc::new(RwLock::new(primary_index)),
        }
    }

    /// Get current primary node
    pub fn get_primary(&self) -> Option<&ReplicaNode> {
        self.primary_index
            .read()
            .and_then(|idx| self.nodes.get(idx))
    }

    /// Get all standby nodes
    pub fn get_standbys(&self) -> Vec<&ReplicaNode> {
        self.nodes
            .iter()
            .filter(|n| n.role() == ReplicaRole::Standby)
            .collect()
    }

    /// Perform health check on all nodes
    pub async fn health_check_all(&self) -> Vec<(usize, Result<bool>)> {
        let mut results = Vec::new();

        for (idx, node) in self.nodes.iter().enumerate() {
            let result = node.health_check().await;
            results.push((idx, result));
        }

        results
    }

    /// Check if failover is needed
    pub async fn needs_failover(&self) -> bool {
        if let Some(primary) = self.get_primary() {
            let failures = *primary.consecutive_failures.read();

            match &self.config.strategy {
                FailoverStrategy::Manual => false,
                FailoverStrategy::Automatic {
                    failure_threshold, ..
                } => failures >= *failure_threshold,
            }
        } else {
            // No primary, definitely need failover
            true
        }
    }

    /// Select best standby for promotion
    pub async fn select_promotion_candidate(&self) -> Option<usize> {
        let standbys = self.get_standbys();

        if standbys.is_empty() {
            return None;
        }

        // Select standby with lowest replication lag
        let mut best_idx = None;
        let mut best_lag = f64::MAX;

        for (node_idx, node) in self.nodes.iter().enumerate() {
            if node.role() != ReplicaRole::Standby {
                continue;
            }

            if let Ok(lag) = node.check_replication_lag().await {
                if lag < best_lag && *node.health.read() == HealthStatus::Healthy {
                    best_lag = lag;
                    best_idx = Some(node_idx);
                }
            }
        }

        best_idx
    }

    /// Perform automatic failover
    pub async fn failover(&self) -> Result<()> {
        // Mark current primary as failed
        if let Some(idx) = *self.primary_index.read() {
            if let Some(node) = self.nodes.get(idx) {
                node.set_role(ReplicaRole::Failed);
            }
        }

        // Select promotion candidate
        let candidate_idx = self.select_promotion_candidate().await.ok_or_else(|| {
            DbError::Other("No healthy standby available for promotion".to_string())
        })?;

        // Promote candidate to primary
        if let Some(candidate) = self.nodes.get(candidate_idx) {
            candidate.promote_to_primary().await?;
            *self.primary_index.write() = Some(candidate_idx);
            Ok(())
        } else {
            Err(DbError::Other("Promotion candidate not found".to_string()))
        }
    }

    /// Simulate replica failure (for testing)
    pub fn simulate_failure(&self, node_idx: usize) {
        if let Some(node) = self.nodes.get(node_idx) {
            node.set_role(ReplicaRole::Failed);
            *node.consecutive_failures.write() = 999; // High value to trigger failover
        }
    }

    /// Recover failed replica (for testing)
    pub async fn recover_replica(&self, node_idx: usize) -> Result<()> {
        if let Some(node) = self.nodes.get(node_idx) {
            // Reset health
            node.set_role(ReplicaRole::Standby);
            *node.consecutive_failures.write() = 0;

            // Perform health check
            node.health_check().await?;

            Ok(())
        } else {
            Err(DbError::Other("Node not found".to_string()))
        }
    }

    /// Get cluster status
    pub fn cluster_status(&self) -> ClusterStatus {
        let nodes_status: Vec<_> = self
            .nodes
            .iter()
            .map(|n| NodeStatus {
                role: format!("{:?}", n.role()),
                health: n.health.read().clone(),
                consecutive_failures: *n.consecutive_failures.read(),
            })
            .collect();

        ClusterStatus {
            primary_index: *self.primary_index.read(),
            nodes: nodes_status,
            total_nodes: self.nodes.len(),
        }
    }
}

/// Cluster status information
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    /// Index of the current primary node in the nodes list, if one exists.
    pub primary_index: Option<usize>,
    /// Status of each node in the cluster.
    pub nodes: Vec<NodeStatus>,
    /// Total number of nodes in the cluster.
    pub total_nodes: usize,
}

/// Individual node status
#[derive(Debug, Clone)]
pub struct NodeStatus {
    /// Current role of the node (e.g., "Primary", "Standby", "Failed").
    pub role: String,
    /// Most recent health check outcome.
    pub health: HealthStatus,
    /// Number of consecutive health check failures.
    pub consecutive_failures: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replica_role_enum() {
        assert_eq!(ReplicaRole::Primary, ReplicaRole::Primary);
        assert_ne!(ReplicaRole::Primary, ReplicaRole::Standby);
        assert_ne!(ReplicaRole::Primary, ReplicaRole::Failed);
    }

    #[test]
    fn test_failover_config_default() {
        let config = FailoverConfig::default();
        match config.strategy {
            FailoverStrategy::Automatic {
                check_interval,
                failure_threshold,
            } => {
                assert_eq!(check_interval, Duration::from_secs(10));
                assert_eq!(failure_threshold, 3);
            }
            _ => panic!("Expected automatic failover"),
        }
        assert_eq!(config.max_replication_lag, Duration::from_secs(30));
    }

    #[test]
    fn test_failover_strategy_enum() {
        let manual = FailoverStrategy::Manual;
        match manual {
            FailoverStrategy::Manual => {}
            _ => panic!("Expected manual strategy"),
        }

        let automatic = FailoverStrategy::Automatic {
            check_interval: Duration::from_secs(5),
            failure_threshold: 2,
        };
        match automatic {
            FailoverStrategy::Automatic {
                failure_threshold, ..
            } => {
                assert_eq!(failure_threshold, 2);
            }
            _ => panic!("Expected automatic strategy"),
        }
    }

    #[test]
    fn test_cluster_status_structure() {
        let status = ClusterStatus {
            primary_index: Some(0),
            nodes: vec![NodeStatus {
                role: "Primary".to_string(),
                health: HealthStatus::Healthy,
                consecutive_failures: 0,
            }],
            total_nodes: 1,
        };

        assert_eq!(status.primary_index, Some(0));
        assert_eq!(status.total_nodes, 1);
        assert_eq!(status.nodes.len(), 1);
    }

    #[test]
    fn test_node_status_structure() {
        let status = NodeStatus {
            role: "Standby".to_string(),
            health: HealthStatus::Healthy,
            consecutive_failures: 0,
        };

        assert_eq!(status.role, "Standby");
        assert_eq!(status.health, HealthStatus::Healthy);
        assert_eq!(status.consecutive_failures, 0);
    }
}
