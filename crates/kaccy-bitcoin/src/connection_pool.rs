//! Bitcoin RPC connection pooling for high concurrency

use crate::client::{BitcoinClient, BitcoinNetwork, ReconnectConfig};
use crate::error::{BitcoinError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, warn};

/// Configuration for the connection pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Maximum time to wait for an available connection
    pub connection_timeout: Duration,
    /// Health check interval for idle connections
    pub health_check_interval: Duration,
    /// Maximum idle time before closing a connection
    pub max_idle_time: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 2,
            max_connections: 10,
            connection_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(300),
        }
    }
}

/// A pooled Bitcoin RPC connection
struct PooledConnection {
    client: BitcoinClient,
    last_used: Instant,
    is_healthy: bool,
}

impl PooledConnection {
    fn new(client: BitcoinClient) -> Self {
        Self {
            client,
            last_used: Instant::now(),
            is_healthy: true,
        }
    }

    fn update_last_used(&mut self) {
        self.last_used = Instant::now();
    }

    fn is_idle(&self, max_idle_time: Duration) -> bool {
        self.last_used.elapsed() > max_idle_time
    }

    async fn health_check(&mut self) -> bool {
        match self.client.health_check() {
            Ok(healthy) => {
                self.is_healthy = healthy;
                healthy
            }
            Err(e) => {
                warn!(error = %e, "Connection health check failed");
                self.is_healthy = false;
                false
            }
        }
    }
}

/// Bitcoin RPC connection pool
pub struct ConnectionPool {
    url: String,
    user: String,
    password: String,
    network: BitcoinNetwork,
    reconnect_config: ReconnectConfig,
    config: PoolConfig,
    connections: Arc<RwLock<Vec<PooledConnection>>>,
    semaphore: Arc<Semaphore>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub async fn new(
        url: &str,
        user: &str,
        password: &str,
        network: BitcoinNetwork,
    ) -> Result<Self> {
        Self::with_config(url, user, password, network, PoolConfig::default()).await
    }

    /// Create a new connection pool with custom configuration
    pub async fn with_config(
        url: &str,
        user: &str,
        password: &str,
        network: BitcoinNetwork,
        config: PoolConfig,
    ) -> Result<Self> {
        let pool = Self {
            url: url.to_string(),
            user: user.to_string(),
            password: password.to_string(),
            network,
            reconnect_config: ReconnectConfig::default(),
            connections: Arc::new(RwLock::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            config,
        };

        // Initialize minimum connections
        pool.initialize_connections().await?;

        // Start background health check task
        pool.start_health_check_task();

        Ok(pool)
    }

    /// Initialize minimum number of connections
    async fn initialize_connections(&self) -> Result<()> {
        let mut connections = self.connections.write().await;

        for i in 0..self.config.min_connections {
            match self.create_connection() {
                Ok(client) => {
                    connections.push(PooledConnection::new(client));
                    debug!(connection = i + 1, "Initialized connection");
                }
                Err(e) => {
                    warn!(error = %e, connection = i + 1, "Failed to initialize connection");
                    if i == 0 {
                        return Err(e); // Fail if we can't create even one connection
                    }
                }
            }
        }

        Ok(())
    }

    /// Create a new connection
    fn create_connection(&self) -> Result<BitcoinClient> {
        BitcoinClient::with_config(
            &self.url,
            &self.user,
            &self.password,
            self.network,
            self.reconnect_config.clone(),
        )
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<PooledConnectionGuard> {
        // Acquire semaphore permit
        let permit = tokio::time::timeout(
            self.config.connection_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await
        .map_err(|_| BitcoinError::ConnectionTimeout {
            timeout_secs: self.config.connection_timeout.as_secs(),
        })?
        .map_err(|_| BitcoinError::ConnectionPoolExhausted)?;

        let mut connections = self.connections.write().await;

        // Try to find a healthy, idle connection
        if let Some(pos) = connections
            .iter()
            .position(|conn| conn.is_healthy && !conn.is_idle(self.config.max_idle_time))
        {
            let mut conn = connections.remove(pos);
            conn.update_last_used();
            debug!("Reusing existing connection");
            return Ok(PooledConnectionGuard {
                connection: Some(conn),
                pool: self.connections.clone(),
                _permit: permit,
            });
        }

        // Try to create a new connection if under max
        if connections.len() < self.config.max_connections {
            match self.create_connection() {
                Ok(client) => {
                    debug!("Created new connection");
                    let conn = PooledConnection::new(client);
                    return Ok(PooledConnectionGuard {
                        connection: Some(conn),
                        pool: self.connections.clone(),
                        _permit: permit,
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to create new connection");
                }
            }
        }

        // If all else fails, try to use any available connection (even if idle)
        if let Some(mut conn) = connections.pop() {
            conn.update_last_used();
            debug!("Using idle connection");
            return Ok(PooledConnectionGuard {
                connection: Some(conn),
                pool: self.connections.clone(),
                _permit: permit,
            });
        }

        Err(BitcoinError::ConnectionPoolExhausted)
    }

    /// Start background task for health checks
    fn start_health_check_task(&self) {
        let connections = self.connections.clone();
        let interval = self.config.health_check_interval;
        let max_idle_time = self.config.max_idle_time;
        let min_connections = self.config.min_connections;
        let url = self.url.clone();
        let user = self.user.clone();
        let password = self.password.clone();
        let network = self.network;
        let reconnect_config = self.reconnect_config.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                let mut conns = connections.write().await;

                // Remove idle connections (but keep minimum)
                let mut i = 0;
                while i < conns.len() {
                    if conns.len() > min_connections && conns[i].is_idle(max_idle_time) {
                        conns.remove(i);
                    } else {
                        i += 1;
                    }
                }

                // Health check remaining connections
                for conn in conns.iter_mut() {
                    conn.health_check().await;
                }

                // Remove unhealthy connections
                conns.retain(|conn| conn.is_healthy);

                // Ensure minimum connections
                while conns.len() < min_connections {
                    match BitcoinClient::with_config(
                        &url,
                        &user,
                        &password,
                        network,
                        reconnect_config.clone(),
                    ) {
                        Ok(client) => {
                            conns.push(PooledConnection::new(client));
                            debug!("Added connection to maintain minimum");
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to create connection during health check");
                            break;
                        }
                    }
                }

                debug!(
                    active_connections = conns.len(),
                    "Connection pool health check completed"
                );
            }
        });
    }

    /// Get pool statistics
    pub async fn stats(&self) -> PoolStats {
        let connections = self.connections.read().await;
        let healthy_count = connections.iter().filter(|c| c.is_healthy).count();

        PoolStats {
            total_connections: connections.len(),
            healthy_connections: healthy_count,
            max_connections: self.config.max_connections,
            available_permits: self.semaphore.available_permits(),
        }
    }
}

/// RAII guard for pooled connections
pub struct PooledConnectionGuard {
    connection: Option<PooledConnection>,
    pool: Arc<RwLock<Vec<PooledConnection>>>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl PooledConnectionGuard {
    /// Get a reference to the Bitcoin client
    pub fn client(&self) -> &BitcoinClient {
        &self
            .connection
            .as_ref()
            .expect("PooledConnectionGuard always holds Some while in use")
            .client
    }
}

impl Drop for PooledConnectionGuard {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                let mut conns = pool.write().await;
                conns.push(connection);
            });
        }
    }
}

/// Connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of connections in the pool
    pub total_connections: usize,
    /// Number of connections passing health checks
    pub healthy_connections: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Number of connections available to acquire
    pub available_permits: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.max_connections, 10);
        assert!(config.connection_timeout.as_secs() > 0);
    }

    #[test]
    fn test_pooled_connection_idle_detection() {
        let client = BitcoinClient::new(
            "http://localhost:8332",
            "user",
            "pass",
            BitcoinNetwork::Regtest,
        )
        .unwrap();

        let conn = PooledConnection::new(client);
        assert!(!conn.is_idle(Duration::from_secs(1)));

        std::thread::sleep(Duration::from_millis(100));
        assert!(conn.is_idle(Duration::from_millis(50)));
    }

    #[test]
    fn test_pool_stats() {
        let stats = PoolStats {
            total_connections: 5,
            healthy_connections: 5,
            max_connections: 10,
            available_permits: 5,
        };

        assert_eq!(stats.total_connections, 5);
        assert_eq!(stats.healthy_connections, 5);
    }
}
