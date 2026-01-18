//! Database Sharding
//!
//! This module provides database sharding functionality for horizontal partitioning
//! of data across multiple database instances to improve scalability and performance.
//!
//! # Features
//!
//! - Shard key selection and hashing
//! - Cross-shard query handling
//! - Shard rebalancing
//! - Consistent hashing for data distribution
//!
//! # Examples
//!
//! ```
//! use kaccy_core::utils::db_sharding::{ShardManager, ShardConfig, HashStrategy};
//!
//! let config = ShardConfig {
//!     shard_count: 4,
//!     replication_factor: 2,
//!     hash_strategy: HashStrategy::ConsistentHash,
//! };
//!
//! let mut manager = ShardManager::new(config);
//! manager.add_shard("shard_0", "postgresql://localhost:5432/db_shard_0");
//! manager.add_shard("shard_1", "postgresql://localhost:5432/db_shard_1");
//!
//! // Determine which shard a user belongs to
//! let shard_id = manager.get_shard_for_key("user", &"user_123");
//! ```

use crate::{CoreError as Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

/// Hash strategy for shard selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashStrategy {
    /// Simple modulo-based hashing
    Modulo,
    /// Consistent hashing with virtual nodes
    ConsistentHash,
    /// Range-based partitioning
    Range,
}

/// Shard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    /// Number of shards
    pub shard_count: usize,
    /// Replication factor for each shard
    pub replication_factor: usize,
    /// Hash strategy to use
    pub hash_strategy: HashStrategy,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            shard_count: 4,
            replication_factor: 2,
            hash_strategy: HashStrategy::ConsistentHash,
        }
    }
}

/// Shard information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shard {
    /// Shard identifier
    pub id: String,
    /// Database connection string
    pub connection_string: String,
    /// Weight for consistent hashing (default: 1.0)
    pub weight: f64,
    /// Is shard currently available?
    pub is_available: bool,
    /// Number of keys in this shard
    pub key_count: usize,
}

impl Shard {
    /// Create a new shard
    pub fn new(id: String, connection_string: String) -> Self {
        Self {
            id,
            connection_string,
            weight: 1.0,
            is_available: true,
            key_count: 0,
        }
    }
}

/// Virtual node for consistent hashing
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct VirtualNode {
    shard_id: String,
    node_id: usize,
    hash: u64,
}

/// Shard manager for database sharding
pub struct ShardManager {
    /// Shard configuration
    config: ShardConfig,
    /// Available shards
    shards: HashMap<String, Shard>,
    /// Virtual nodes for consistent hashing (sorted by hash)
    virtual_nodes: BTreeMap<u64, String>,
    /// Number of virtual nodes per shard
    virtual_nodes_per_shard: usize,
}

impl ShardManager {
    /// Create a new shard manager
    pub fn new(config: ShardConfig) -> Self {
        Self {
            config,
            shards: HashMap::new(),
            virtual_nodes: BTreeMap::new(),
            virtual_nodes_per_shard: 150, // Good default for consistent hashing
        }
    }

    /// Add a shard to the manager
    pub fn add_shard(&mut self, id: &str, connection_string: &str) -> Result<()> {
        let shard = Shard::new(id.to_string(), connection_string.to_string());
        self.shards.insert(id.to_string(), shard);

        // Add virtual nodes for consistent hashing
        if self.config.hash_strategy == HashStrategy::ConsistentHash {
            self.add_virtual_nodes(id);
        }

        Ok(())
    }

    /// Remove a shard from the manager
    pub fn remove_shard(&mut self, id: &str) -> Result<()> {
        self.shards.remove(id);

        // Remove virtual nodes
        if self.config.hash_strategy == HashStrategy::ConsistentHash {
            self.remove_virtual_nodes(id);
        }

        Ok(())
    }

    /// Get shard for a given key
    pub fn get_shard_for_key(&self, table: &str, key: &dyn std::fmt::Display) -> Result<String> {
        if self.shards.is_empty() {
            return Err(Error::Validation("No shards available".to_string()));
        }

        let shard_key = format!("{}:{}", table, key);

        match self.config.hash_strategy {
            HashStrategy::Modulo => self.get_shard_modulo(&shard_key),
            HashStrategy::ConsistentHash => self.get_shard_consistent_hash(&shard_key),
            HashStrategy::Range => self.get_shard_range(&shard_key),
        }
    }

    /// Get multiple shards for a query (for scatter-gather)
    pub fn get_all_shards(&self) -> Vec<String> {
        self.shards
            .values()
            .filter(|s| s.is_available)
            .map(|s| s.id.clone())
            .collect()
    }

    /// Get shard statistics
    pub fn get_shard_stats(&self) -> Vec<ShardStats> {
        self.shards
            .values()
            .map(|shard| ShardStats {
                shard_id: shard.id.clone(),
                key_count: shard.key_count,
                is_available: shard.is_available,
                weight: shard.weight,
            })
            .collect()
    }

    /// Rebalance shards (returns mapping of keys to move)
    pub fn plan_rebalance(&self) -> Result<RebalancePlan> {
        let stats = self.get_shard_stats();
        let total_keys: usize = stats.iter().map(|s| s.key_count).sum();
        let avg_keys = if self.shards.is_empty() {
            0
        } else {
            total_keys / self.shards.len()
        };

        let mut moves = Vec::new();

        // Find overloaded and underloaded shards
        let overloaded: Vec<_> = stats
            .iter()
            .filter(|s| s.key_count > avg_keys * 12 / 10) // 20% above average
            .collect();

        let underloaded: Vec<_> = stats
            .iter()
            .filter(|s| s.key_count < avg_keys * 8 / 10) // 20% below average
            .collect();

        // Plan moves from overloaded to underloaded
        for over in &overloaded {
            for under in &underloaded {
                let keys_to_move = (over.key_count - avg_keys).min(avg_keys - under.key_count);
                if keys_to_move > 0 {
                    moves.push(RebalanceMove {
                        from_shard: over.shard_id.clone(),
                        to_shard: under.shard_id.clone(),
                        estimated_keys: keys_to_move,
                    });
                }
            }
        }

        Ok(RebalancePlan {
            total_keys,
            avg_keys_per_shard: avg_keys,
            moves,
        })
    }

    /// Add virtual nodes for a shard (consistent hashing)
    fn add_virtual_nodes(&mut self, shard_id: &str) {
        for i in 0..self.virtual_nodes_per_shard {
            let node_key = format!("{}:vnode:{}", shard_id, i);
            let hash = self.hash_string(&node_key);
            self.virtual_nodes.insert(hash, shard_id.to_string());
        }
    }

    /// Remove virtual nodes for a shard
    fn remove_virtual_nodes(&mut self, shard_id: &str) {
        self.virtual_nodes.retain(|_, sid| sid != shard_id);
    }

    /// Get shard using modulo hashing
    fn get_shard_modulo(&self, key: &str) -> Result<String> {
        let hash = self.hash_string(key);
        let shard_index = (hash % self.shards.len() as u64) as usize;

        self.shards
            .values()
            .nth(shard_index)
            .map(|s| s.id.clone())
            .ok_or_else(|| Error::Validation("Shard not found".to_string()))
    }

    /// Get shard using consistent hashing
    fn get_shard_consistent_hash(&self, key: &str) -> Result<String> {
        if self.virtual_nodes.is_empty() {
            return Err(Error::Validation("No virtual nodes configured".to_string()));
        }

        let hash = self.hash_string(key);

        // Find the first virtual node with hash >= key hash
        let shard_id = self
            .virtual_nodes
            .range(hash..)
            .next()
            .or_else(|| self.virtual_nodes.iter().next()) // Wrap around
            .map(|(_, sid)| sid.clone())
            .ok_or_else(|| Error::Validation("No shard found".to_string()))?;

        Ok(shard_id)
    }

    /// Get shard using range-based partitioning
    fn get_shard_range(&self, key: &str) -> Result<String> {
        // For range-based, we use the first character of the key
        // This is a simple implementation; in production, you'd use proper ranges
        let hash = self.hash_string(key);
        let range_size = u64::MAX / self.shards.len() as u64;
        let shard_index = (hash / range_size).min(self.shards.len() as u64 - 1) as usize;

        self.shards
            .values()
            .nth(shard_index)
            .map(|s| s.id.clone())
            .ok_or_else(|| Error::Validation("Shard not found".to_string()))
    }

    /// Hash a string to u64
    fn hash_string(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
}

/// Shard statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStats {
    /// Shard identifier
    pub shard_id: String,
    /// Number of keys in this shard
    pub key_count: usize,
    /// Is shard available?
    pub is_available: bool,
    /// Shard weight
    pub weight: f64,
}

/// Rebalance plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancePlan {
    /// Total number of keys across all shards
    pub total_keys: usize,
    /// Average keys per shard
    pub avg_keys_per_shard: usize,
    /// Moves to execute
    pub moves: Vec<RebalanceMove>,
}

impl RebalancePlan {
    /// Is rebalancing needed?
    pub fn is_needed(&self) -> bool {
        !self.moves.is_empty()
    }

    /// Total keys to move
    pub fn total_keys_to_move(&self) -> usize {
        self.moves.iter().map(|m| m.estimated_keys).sum()
    }
}

/// Single rebalance move
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceMove {
    /// Source shard
    pub from_shard: String,
    /// Destination shard
    pub to_shard: String,
    /// Estimated number of keys to move
    pub estimated_keys: usize,
}

/// Cross-shard query executor
pub struct CrossShardQuery {
    /// Shards to query
    pub shard_ids: Vec<String>,
    /// Query template (with placeholder for shard-specific filters)
    pub query_template: String,
}

impl CrossShardQuery {
    /// Create a new cross-shard query
    pub fn new(shard_ids: Vec<String>, query_template: String) -> Self {
        Self {
            shard_ids,
            query_template,
        }
    }

    /// Get the number of shards to query
    pub fn shard_count(&self) -> usize {
        self.shard_ids.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_manager_creation() {
        let config = ShardConfig::default();
        let manager = ShardManager::new(config);
        assert_eq!(manager.shards.len(), 0);
    }

    #[test]
    fn test_add_shard() {
        let config = ShardConfig::default();
        let mut manager = ShardManager::new(config);

        assert!(
            manager
                .add_shard("shard_0", "postgresql://localhost/db_0")
                .is_ok()
        );
        assert_eq!(manager.shards.len(), 1);
    }

    #[test]
    fn test_remove_shard() {
        let config = ShardConfig::default();
        let mut manager = ShardManager::new(config);

        manager
            .add_shard("shard_0", "postgresql://localhost/db_0")
            .unwrap();
        assert_eq!(manager.shards.len(), 1);

        assert!(manager.remove_shard("shard_0").is_ok());
        assert_eq!(manager.shards.len(), 0);
    }

    #[test]
    fn test_get_shard_for_key_modulo() {
        let config = ShardConfig {
            hash_strategy: HashStrategy::Modulo,
            ..Default::default()
        };
        let mut manager = ShardManager::new(config);

        manager
            .add_shard("shard_0", "postgresql://localhost/db_0")
            .unwrap();
        manager
            .add_shard("shard_1", "postgresql://localhost/db_1")
            .unwrap();

        let shard_id = manager.get_shard_for_key("users", &"user_123").unwrap();
        assert!(shard_id == "shard_0" || shard_id == "shard_1");

        // Same key should always go to same shard
        let shard_id2 = manager.get_shard_for_key("users", &"user_123").unwrap();
        assert_eq!(shard_id, shard_id2);
    }

    #[test]
    fn test_get_shard_for_key_consistent_hash() {
        let config = ShardConfig::default(); // Uses ConsistentHash by default
        let mut manager = ShardManager::new(config);

        manager
            .add_shard("shard_0", "postgresql://localhost/db_0")
            .unwrap();
        manager
            .add_shard("shard_1", "postgresql://localhost/db_1")
            .unwrap();

        let shard_id = manager.get_shard_for_key("users", &"user_123").unwrap();
        assert!(shard_id == "shard_0" || shard_id == "shard_1");

        // Same key should always go to same shard
        let shard_id2 = manager.get_shard_for_key("users", &"user_123").unwrap();
        assert_eq!(shard_id, shard_id2);
    }

    #[test]
    fn test_get_all_shards() {
        let config = ShardConfig::default();
        let mut manager = ShardManager::new(config);

        manager
            .add_shard("shard_0", "postgresql://localhost/db_0")
            .unwrap();
        manager
            .add_shard("shard_1", "postgresql://localhost/db_1")
            .unwrap();

        let all_shards = manager.get_all_shards();
        assert_eq!(all_shards.len(), 2);
    }

    #[test]
    fn test_cross_shard_query() {
        let query = CrossShardQuery::new(
            vec!["shard_0".to_string(), "shard_1".to_string()],
            "SELECT * FROM users WHERE id = ?".to_string(),
        );

        assert_eq!(query.shard_count(), 2);
    }

    #[test]
    fn test_rebalance_plan() {
        let config = ShardConfig::default();
        let mut manager = ShardManager::new(config);

        manager
            .add_shard("shard_0", "postgresql://localhost/db_0")
            .unwrap();
        manager
            .add_shard("shard_1", "postgresql://localhost/db_1")
            .unwrap();

        // Simulate unbalanced shards
        if let Some(shard) = manager.shards.get_mut("shard_0") {
            shard.key_count = 1000;
        }
        if let Some(shard) = manager.shards.get_mut("shard_1") {
            shard.key_count = 100;
        }

        let plan = manager.plan_rebalance().unwrap();
        assert!(plan.is_needed());
        assert!(plan.total_keys_to_move() > 0);
    }
}
