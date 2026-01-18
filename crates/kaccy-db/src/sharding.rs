//! Database sharding utilities for horizontal scaling

use crate::error::{DbError, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use uuid::Uuid;

/// Shard key type for routing queries to appropriate shards
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShardKey {
    /// Shard by user ID
    UserId(Uuid),
    /// Shard by token ID
    TokenId(Uuid),
    /// Shard by custom string key
    Custom(String),
    /// Shard by integer key
    Integer(i64),
}

impl ShardKey {
    /// Get the hash value for this shard key
    pub fn hash_value(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

/// Sharding strategy for distributing data across shards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardingStrategy {
    /// Hash-based sharding (consistent hashing)
    Hash,
    /// Range-based sharding
    Range,
    /// Geographic sharding
    Geographic,
    /// Custom sharding logic
    Custom,
}

/// Shard information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    /// Shard ID
    pub id: u32,
    /// Shard name
    pub name: String,
    /// Database connection string
    pub connection_string: String,
    /// Whether this shard is active
    pub is_active: bool,
    /// Shard weight for load balancing (higher = more load)
    pub weight: u32,
    /// Geographic region (optional)
    pub region: Option<String>,
    /// Range start (for range-based sharding)
    pub range_start: Option<u64>,
    /// Range end (for range-based sharding)
    pub range_end: Option<u64>,
}

/// Shard pool manager
pub struct ShardPoolManager {
    /// Map of shard ID to connection pool
    pools: Arc<RwLock<HashMap<u32, PgPool>>>,
    /// Shard metadata
    shards: Arc<RwLock<Vec<ShardInfo>>>,
    /// Sharding strategy
    strategy: ShardingStrategy,
}

impl ShardPoolManager {
    /// Create a new shard pool manager
    pub fn new(strategy: ShardingStrategy) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            shards: Arc::new(RwLock::new(Vec::new())),
            strategy,
        }
    }

    /// Add a shard to the manager
    pub async fn add_shard(&self, shard_info: ShardInfo) -> Result<()> {
        let pool = PgPool::connect(&shard_info.connection_string)
            .await
            .map_err(|e| DbError::Connection(format!("Failed to connect to shard: {}", e)))?;

        let mut pools = self.pools.write();
        let mut shards = self.shards.write();

        pools.insert(shard_info.id, pool);
        shards.push(shard_info);

        Ok(())
    }

    /// Remove a shard from the manager
    pub async fn remove_shard(&self, shard_id: u32) -> Result<()> {
        let mut pools = self.pools.write();
        let mut shards = self.shards.write();

        pools.remove(&shard_id);
        shards.retain(|s| s.id != shard_id);

        Ok(())
    }

    /// Get shard ID for a given shard key
    pub fn get_shard_id(&self, key: &ShardKey) -> Result<u32> {
        let shards = self.shards.read();

        if shards.is_empty() {
            return Err(DbError::Other("No shards configured".to_string()));
        }

        match self.strategy {
            ShardingStrategy::Hash => {
                let hash = key.hash_value();
                let active_shards: Vec<_> = shards.iter().filter(|s| s.is_active).collect();

                if active_shards.is_empty() {
                    return Err(DbError::Other("No active shards available".to_string()));
                }

                let index = (hash % active_shards.len() as u64) as usize;
                Ok(active_shards[index].id)
            }
            ShardingStrategy::Range => {
                let hash = key.hash_value();

                for shard in shards.iter().filter(|s| s.is_active) {
                    if let (Some(start), Some(end)) = (shard.range_start, shard.range_end) {
                        if hash >= start && hash < end {
                            return Ok(shard.id);
                        }
                    }
                }

                Err(DbError::Other(
                    "No shard found for key in range".to_string(),
                ))
            }
            ShardingStrategy::Geographic | ShardingStrategy::Custom => {
                // For geographic/custom, use first active shard as fallback
                shards
                    .iter()
                    .find(|s| s.is_active)
                    .map(|s| s.id)
                    .ok_or_else(|| DbError::Other("No active shards available".to_string()))
            }
        }
    }

    /// Get pool for a given shard key
    pub fn get_pool(&self, key: &ShardKey) -> Result<PgPool> {
        let shard_id = self.get_shard_id(key)?;
        self.get_pool_by_id(shard_id)
    }

    /// Get pool by shard ID
    pub fn get_pool_by_id(&self, shard_id: u32) -> Result<PgPool> {
        let pools = self.pools.read();
        pools
            .get(&shard_id)
            .cloned()
            .ok_or_else(|| DbError::Other(format!("Shard {} not found", shard_id)))
    }

    /// Get all active shard pools
    pub fn get_all_active_pools(&self) -> Vec<(u32, PgPool)> {
        let pools = self.pools.read();
        let shards = self.shards.read();

        shards
            .iter()
            .filter(|s| s.is_active)
            .filter_map(|s| pools.get(&s.id).map(|pool| (s.id, pool.clone())))
            .collect()
    }

    /// Get shard info by ID
    pub fn get_shard_info(&self, shard_id: u32) -> Option<ShardInfo> {
        let shards = self.shards.read();
        shards.iter().find(|s| s.id == shard_id).cloned()
    }

    /// List all shards
    pub fn list_shards(&self) -> Vec<ShardInfo> {
        let shards = self.shards.read();
        shards.clone()
    }

    /// Set shard active/inactive
    pub fn set_shard_active(&self, shard_id: u32, active: bool) -> Result<()> {
        let mut shards = self.shards.write();

        if let Some(shard) = shards.iter_mut().find(|s| s.id == shard_id) {
            shard.is_active = active;
            Ok(())
        } else {
            Err(DbError::Other(format!("Shard {} not found", shard_id)))
        }
    }

    /// Get shard count
    pub fn shard_count(&self) -> usize {
        let shards = self.shards.read();
        shards.len()
    }

    /// Get active shard count
    pub fn active_shard_count(&self) -> usize {
        let shards = self.shards.read();
        shards.iter().filter(|s| s.is_active).count()
    }
}

/// Shard coordinator for cross-shard operations
pub struct ShardCoordinator {
    manager: Arc<ShardPoolManager>,
}

impl ShardCoordinator {
    /// Create a new shard coordinator
    pub fn new(manager: Arc<ShardPoolManager>) -> Self {
        Self { manager }
    }

    /// Execute a query on a specific shard
    pub async fn execute_on_shard<F, T>(&self, key: &ShardKey, f: F) -> Result<T>
    where
        F: FnOnce(
                &PgPool,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send,
        T: Send,
    {
        let pool = self.manager.get_pool(key)?;
        f(&pool).await
    }

    /// Execute a query on all active shards and collect results
    pub async fn execute_on_all_shards<F, T>(&self, f: F) -> Result<Vec<(u32, T)>>
    where
        F: Fn(&PgPool) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send
            + Sync,
        T: Send,
    {
        let pools = self.manager.get_all_active_pools();
        let mut results = Vec::new();

        for (shard_id, pool) in pools {
            match f(&pool).await {
                Ok(result) => results.push((shard_id, result)),
                Err(e) => {
                    tracing::warn!("Error executing on shard {}: {}", shard_id, e);
                    // Continue with other shards
                }
            }
        }

        Ok(results)
    }

    /// Aggregate results from all shards
    pub async fn aggregate_from_all_shards<F, T, R>(
        &self,
        query: F,
        aggregator: fn(Vec<T>) -> R,
    ) -> Result<R>
    where
        F: Fn(&PgPool) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send
            + Sync,
        T: Send,
        R: Send,
    {
        let results = self.execute_on_all_shards(query).await?;
        let values: Vec<T> = results.into_iter().map(|(_, v)| v).collect();
        Ok(aggregator(values))
    }
}

/// Consistent hashing ring for shard routing
pub struct ConsistentHashRing {
    /// Virtual nodes per shard
    virtual_nodes: u32,
    /// Ring of hash values to shard IDs
    ring: Arc<RwLock<Vec<(u64, u32)>>>,
}

impl ConsistentHashRing {
    /// Create a new consistent hash ring
    pub fn new(virtual_nodes: u32) -> Self {
        Self {
            virtual_nodes,
            ring: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a shard to the ring
    pub fn add_shard(&self, shard_id: u32) {
        let mut ring = self.ring.write();

        for i in 0..self.virtual_nodes {
            let key = format!("shard-{}-vnode-{}", shard_id, i);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut hasher);
            let hash = hasher.finish();
            ring.push((hash, shard_id));
        }

        ring.sort_by_key(|(hash, _)| *hash);
    }

    /// Remove a shard from the ring
    pub fn remove_shard(&self, shard_id: u32) {
        let mut ring = self.ring.write();
        ring.retain(|(_, id)| *id != shard_id);
    }

    /// Get shard ID for a given key
    pub fn get_shard(&self, key: &ShardKey) -> Option<u32> {
        let ring = self.ring.read();

        if ring.is_empty() {
            return None;
        }

        let hash = key.hash_value();

        // Binary search for the first node >= hash
        match ring.binary_search_by_key(&hash, |(h, _)| *h) {
            Ok(idx) => Some(ring[idx].1),
            Err(idx) => {
                if idx >= ring.len() {
                    // Wrap around to first node
                    Some(ring[0].1)
                } else {
                    Some(ring[idx].1)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_key_hash() {
        let key1 = ShardKey::UserId(Uuid::nil());
        let key2 = ShardKey::UserId(Uuid::nil());
        assert_eq!(key1.hash_value(), key2.hash_value());
    }

    #[test]
    fn test_consistent_hash_ring() {
        let ring = ConsistentHashRing::new(100);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        let key = ShardKey::UserId(Uuid::nil());
        let shard = ring.get_shard(&key);
        assert!(shard.is_some());
        assert!(shard.unwrap() <= 3);
    }

    #[test]
    fn test_shard_manager_hash_strategy() {
        let manager = ShardPoolManager::new(ShardingStrategy::Hash);
        assert_eq!(manager.shard_count(), 0);
        assert_eq!(manager.active_shard_count(), 0);
    }
}
