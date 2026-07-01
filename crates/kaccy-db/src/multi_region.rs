//! Multi-region database replication support

use crate::error::{DbError, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

/// Geographic region identifier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Region code (e.g., "us-east-1", "eu-west-1")
    pub code: String,
    /// Region name
    pub name: String,
    /// Latitude (for proximity calculations)
    pub latitude: f64,
    /// Longitude (for proximity calculations)
    pub longitude: f64,
}

impl Region {
    /// Create a new region
    pub fn new(code: String, name: String, latitude: f64, longitude: f64) -> Self {
        Self {
            code,
            name,
            latitude,
            longitude,
        }
    }

    /// Calculate distance to another region in kilometers (Haversine formula)
    pub fn distance_to(&self, other: &Region) -> f64 {
        let r = 6371.0; // Earth radius in km

        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        r * c
    }
}

/// Regional database pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalPoolConfig {
    /// Region identifier
    pub region: Region,
    /// Database URL for this region
    pub database_url: String,
    /// Whether this is the primary region
    pub is_primary: bool,
    /// Priority for failover (lower = higher priority)
    pub failover_priority: u32,
    /// Whether this pool is currently active
    pub is_active: bool,
}

/// Replication lag metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationLag {
    /// Source region
    pub source_region: String,
    /// Replica region
    pub replica_region: String,
    /// Lag in seconds
    pub lag_seconds: f64,
    /// Last measurement timestamp
    pub measured_at: DateTime<Utc>,
    /// Whether the lag is acceptable (< threshold)
    pub is_healthy: bool,
}

/// Multi-region pool manager
pub struct MultiRegionPoolManager {
    /// Regional connection pools
    pools: Arc<RwLock<HashMap<String, PgPool>>>,
    /// Regional configurations
    configs: Arc<RwLock<Vec<RegionalPoolConfig>>>,
    /// Current active region
    current_region: Arc<RwLock<Option<String>>>,
    /// Replication lag monitoring
    replication_lags: Arc<RwLock<HashMap<String, ReplicationLag>>>,
    /// Maximum acceptable replication lag in seconds
    max_lag_seconds: f64,
}

impl MultiRegionPoolManager {
    /// Create a new multi-region pool manager
    pub fn new(max_lag_seconds: f64) -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(Vec::new())),
            current_region: Arc::new(RwLock::new(None)),
            replication_lags: Arc::new(RwLock::new(HashMap::new())),
            max_lag_seconds,
        }
    }

    /// Add a regional pool
    pub async fn add_regional_pool(&self, config: RegionalPoolConfig) -> Result<()> {
        let pool = PgPool::connect(&config.database_url).await.map_err(|e| {
            DbError::Connection(format!(
                "Failed to connect to {}: {}",
                config.region.code, e
            ))
        })?;

        let region_code = config.region.code.clone();
        let is_primary = config.is_primary;

        let mut pools = self.pools.write();
        let mut configs = self.configs.write();

        pools.insert(region_code.clone(), pool);
        configs.push(config);

        // Set as current region if it's the primary and no current region is set
        if is_primary {
            let mut current = self.current_region.write();
            if current.is_none() {
                *current = Some(region_code);
            }
        }

        Ok(())
    }

    /// Get pool for a specific region
    pub fn get_regional_pool(&self, region_code: &str) -> Result<PgPool> {
        let pools = self.pools.read();
        pools
            .get(region_code)
            .cloned()
            .ok_or_else(|| DbError::Other(format!("Region {} not found", region_code)))
    }

    /// Get pool for the closest region to a given location
    pub fn get_closest_pool(&self, latitude: f64, longitude: f64) -> Result<(String, PgPool)> {
        let configs = self.configs.read();
        let pools = self.pools.read();

        let user_location =
            Region::new("user".to_string(), "User".to_string(), latitude, longitude);

        let mut closest: Option<(f64, String)> = None;

        for config in configs.iter().filter(|c| c.is_active) {
            let distance = user_location.distance_to(&config.region);

            if let Some((min_dist, _)) = closest {
                if distance < min_dist {
                    closest = Some((distance, config.region.code.clone()));
                }
            } else {
                closest = Some((distance, config.region.code.clone()));
            }
        }

        let region_code = closest
            .map(|(_, code)| code)
            .ok_or_else(|| DbError::Other("No active regions available".to_string()))?;

        let pool = pools
            .get(&region_code)
            .cloned()
            .ok_or_else(|| DbError::Other(format!("Pool for region {} not found", region_code)))?;

        Ok((region_code, pool))
    }

    /// Get primary pool
    pub fn get_primary_pool(&self) -> Result<PgPool> {
        let configs = self.configs.read();
        let pools = self.pools.read();

        let primary_region = configs
            .iter()
            .find(|c| c.is_primary && c.is_active)
            .map(|c| c.region.code.clone())
            .ok_or_else(|| DbError::Other("No active primary region found".to_string()))?;

        pools
            .get(&primary_region)
            .cloned()
            .ok_or_else(|| DbError::Other("Primary pool not found".to_string()))
    }

    /// Get pool for the current active region
    pub fn get_current_pool(&self) -> Result<PgPool> {
        let current = self.current_region.read();
        let pools = self.pools.read();

        let region_code = current
            .as_ref()
            .ok_or_else(|| DbError::Other("No current region set".to_string()))?;

        pools
            .get(region_code)
            .cloned()
            .ok_or_else(|| DbError::Other("Current region pool not found".to_string()))
    }

    /// Switch to a different region (failover)
    pub fn failover_to_region(&self, region_code: &str) -> Result<()> {
        let pools = self.pools.read();
        let configs = self.configs.read();

        // Check if region exists and is active
        let _config = configs
            .iter()
            .find(|c| c.region.code == region_code && c.is_active)
            .ok_or_else(|| {
                DbError::Other(format!("Region {} not found or not active", region_code))
            })?;

        if !pools.contains_key(region_code) {
            return Err(DbError::Other(format!(
                "Pool for region {} not found",
                region_code
            )));
        }

        let mut current = self.current_region.write();
        *current = Some(region_code.to_string());

        tracing::info!("Failed over to region: {}", region_code);

        Ok(())
    }

    /// Measure replication lag for a replica
    pub async fn measure_replication_lag(
        &self,
        source_region: &str,
        replica_region: &str,
    ) -> Result<ReplicationLag> {
        let source_pool = self.get_regional_pool(source_region)?;
        let replica_pool = self.get_regional_pool(replica_region)?;

        // Get current LSN (Log Sequence Number) from source
        let source_lsn: (String,) = sqlx::query_as("SELECT pg_current_wal_lsn()::text")
            .fetch_one(&source_pool)
            .await?;

        // Get last received LSN from replica
        let replica_lsn: (Option<String>,) =
            sqlx::query_as("SELECT pg_last_wal_receive_lsn()::text")
                .fetch_one(&replica_pool)
                .await?;

        // Calculate lag in bytes
        let lag_query = format!(
            "SELECT COALESCE(pg_wal_lsn_diff('{}', '{}'), 0) / 1024.0 / 1024.0",
            source_lsn.0,
            replica_lsn.0.unwrap_or_else(|| "0/0".to_string())
        );

        let lag_mb: (f64,) = sqlx::query_as(sqlx::AssertSqlSafe(lag_query))
            .fetch_one(&source_pool)
            .await?;

        // Estimate lag in seconds (assuming ~10MB/s replication speed)
        let lag_seconds = lag_mb.0 / 10.0;

        let lag = ReplicationLag {
            source_region: source_region.to_string(),
            replica_region: replica_region.to_string(),
            lag_seconds,
            measured_at: Utc::now(),
            is_healthy: lag_seconds < self.max_lag_seconds,
        };

        // Store the measurement
        let mut lags = self.replication_lags.write();
        lags.insert(replica_region.to_string(), lag.clone());

        Ok(lag)
    }

    /// Get all replication lags
    pub fn get_replication_lags(&self) -> HashMap<String, ReplicationLag> {
        let lags = self.replication_lags.read();
        lags.clone()
    }

    /// Check if all replicas are healthy
    pub fn are_all_replicas_healthy(&self) -> bool {
        let lags = self.replication_lags.read();
        lags.values().all(|lag| lag.is_healthy)
    }

    /// Get unhealthy replicas
    pub fn get_unhealthy_replicas(&self) -> Vec<String> {
        let lags = self.replication_lags.read();
        lags.iter()
            .filter(|(_, lag)| !lag.is_healthy)
            .map(|(region, _)| region.clone())
            .collect()
    }

    /// Auto-failover to the best available region
    pub fn auto_failover(&self) -> Result<String> {
        let configs = self.configs.read();
        let lags = self.replication_lags.read();

        // Find active regions sorted by failover priority and health
        let mut candidates: Vec<_> = configs
            .iter()
            .filter(|c| c.is_active)
            .map(|c| {
                let lag_ok = lags
                    .get(&c.region.code)
                    .map(|l| l.is_healthy)
                    .unwrap_or(true);
                (c, lag_ok)
            })
            .collect();

        // Sort by: healthy first, then by priority
        candidates.sort_by(|a, b| match (a.1, b.1) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.0.failover_priority.cmp(&b.0.failover_priority),
        });

        let target_region = candidates
            .first()
            .map(|(c, _)| c.region.code.clone())
            .ok_or_else(|| DbError::Other("No suitable region for failover".to_string()))?;

        self.failover_to_region(&target_region)?;

        Ok(target_region)
    }

    /// List all regions
    pub fn list_regions(&self) -> Vec<RegionalPoolConfig> {
        let configs = self.configs.read();
        configs.clone()
    }

    /// Get current region
    pub fn get_current_region(&self) -> Option<String> {
        let current = self.current_region.read();
        current.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_distance() {
        let us_east = Region::new("us-east-1".to_string(), "US East".to_string(), 39.0, -77.0);
        let eu_west = Region::new("eu-west-1".to_string(), "EU West".to_string(), 53.0, -8.0);

        let distance = us_east.distance_to(&eu_west);
        assert!(distance > 4000.0); // Should be over 4000km
    }

    #[test]
    fn test_multi_region_manager_creation() {
        let manager = MultiRegionPoolManager::new(5.0);
        assert!(manager.get_current_region().is_none());
        assert_eq!(manager.list_regions().len(), 0);
    }

    #[test]
    fn test_region_health_check() {
        let manager = MultiRegionPoolManager::new(5.0);
        assert!(manager.are_all_replicas_healthy());
        assert_eq!(manager.get_unhealthy_replicas().len(), 0);
    }
}
