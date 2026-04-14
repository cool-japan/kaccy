//! Enhanced health check system for Bitcoin components

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::client::BitcoinClient;
use crate::error::Result;
use crate::lightning::LightningProvider;

/// Health status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Degraded performance but operational
    Degraded,
    /// Critical issues, may not be operational
    Unhealthy,
    /// Status unknown or not yet checked
    Unknown,
}

impl HealthStatus {
    /// Check if the status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Check if the status is degraded or worse
    pub fn is_degraded_or_worse(&self) -> bool {
        !matches!(self, HealthStatus::Healthy)
    }
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Name of the component being monitored
    pub name: String,
    /// Current health status of the component
    pub status: HealthStatus,
    /// Optional message describing the health state
    pub message: Option<String>,
    /// Timestamp of the most recent health check
    pub last_check: DateTime<Utc>,
    /// Additional metadata about the component's health
    pub metadata: serde_json::Value,
}

impl ComponentHealth {
    /// Create a healthy component
    pub fn healthy(name: String) -> Self {
        Self {
            name,
            status: HealthStatus::Healthy,
            message: None,
            last_check: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a degraded component
    pub fn degraded(name: String, message: String) -> Self {
        Self {
            name,
            status: HealthStatus::Degraded,
            message: Some(message),
            last_check: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Create an unhealthy component
    pub fn unhealthy(name: String, message: String) -> Self {
        Self {
            name,
            status: HealthStatus::Unhealthy,
            message: Some(message),
            last_check: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Add metadata to the component health
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Dependency health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    /// Health status of the Bitcoin Core node
    pub bitcoin_core: ComponentHealth,
    /// Health status of the Lightning node, if configured
    pub lightning_node: Option<ComponentHealth>,
}

/// Resource utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUtilization {
    /// Memory usage percentage (0-100)
    pub memory_usage_percent: Option<f64>,
    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: Option<f64>,
    /// Disk usage percentage (0-100)
    pub disk_usage_percent: Option<f64>,
    /// Open file descriptors
    pub open_file_descriptors: Option<usize>,
}

/// Overall health check report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall health status of the system
    pub status: HealthStatus,
    /// Timestamp when this report was generated
    pub timestamp: DateTime<Utc>,
    /// Health status of individual application components
    pub components: Vec<ComponentHealth>,
    /// Health status of external dependencies
    pub dependencies: DependencyHealth,
    /// Current resource utilization metrics
    pub resource_utilization: ResourceUtilization,
    /// Number of seconds the service has been running
    pub uptime_seconds: u64,
}

impl HealthReport {
    /// Check if all components are healthy
    pub fn is_all_healthy(&self) -> bool {
        self.status.is_healthy()
            && self.components.iter().all(|c| c.status.is_healthy())
            && self.dependencies.bitcoin_core.status.is_healthy()
            && self
                .dependencies
                .lightning_node
                .as_ref()
                .is_none_or(|ln| ln.status.is_healthy())
    }
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Check interval
    pub check_interval: Duration,
    /// Timeout for health checks
    pub check_timeout: Duration,
    /// Maximum acceptable block age in seconds
    pub max_block_age_secs: u64,
    /// Enable resource utilization monitoring
    pub monitor_resources: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(10),
            max_block_age_secs: 3600, // 1 hour
            monitor_resources: false, // Disabled by default as it requires system access
        }
    }
}

/// Health check manager
pub struct HealthCheckManager {
    config: HealthCheckConfig,
    start_time: DateTime<Utc>,
    last_report: Arc<RwLock<Option<HealthReport>>>,
    lightning_provider: Option<Arc<dyn LightningProvider>>,
}

impl HealthCheckManager {
    /// Create a new health check manager
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            start_time: Utc::now(),
            last_report: Arc::new(RwLock::new(None)),
            lightning_provider: None,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(HealthCheckConfig::default())
    }

    /// Set the Lightning provider for health checks
    pub fn with_lightning_provider(mut self, provider: Arc<dyn LightningProvider>) -> Self {
        self.lightning_provider = Some(provider);
        self
    }

    /// Perform a full health check
    pub async fn check_health(&self, bitcoin_client: &BitcoinClient) -> Result<HealthReport> {
        let mut components = Vec::new();

        // Check RPC connection
        let rpc_health = self.check_rpc_connection(bitcoin_client).await;
        components.push(rpc_health);

        // Check blockchain sync status
        let sync_health = self.check_sync_status(bitcoin_client).await;
        components.push(sync_health);

        // Check mempool
        let mempool_health = self.check_mempool(bitcoin_client).await;
        components.push(mempool_health);

        // Check dependencies
        let dependencies = self.check_dependencies(bitcoin_client).await;

        // Check resources if enabled
        let resource_utilization = if self.config.monitor_resources {
            self.check_resources().await
        } else {
            ResourceUtilization::default()
        };

        // Determine overall status
        let overall_status = self.determine_overall_status(&components, &dependencies);

        let uptime_seconds = (Utc::now() - self.start_time).num_seconds() as u64;

        let report = HealthReport {
            status: overall_status,
            timestamp: Utc::now(),
            components,
            dependencies,
            resource_utilization,
            uptime_seconds,
        };

        // Store latest report
        let mut last_report = self.last_report.write().await;
        *last_report = Some(report.clone());

        Ok(report)
    }

    /// Check RPC connection health
    async fn check_rpc_connection(&self, client: &BitcoinClient) -> ComponentHealth {
        match tokio::time::timeout(
            self.config.check_timeout,
            tokio::task::spawn_blocking({
                let client_health = client.health_check();
                move || client_health
            }),
        )
        .await
        {
            Ok(Ok(Ok(true))) => ComponentHealth::healthy("rpc_connection".to_string()),
            Ok(Ok(Ok(false))) => ComponentHealth::unhealthy(
                "rpc_connection".to_string(),
                "RPC connection failed".to_string(),
            ),
            Ok(Ok(Err(e))) => ComponentHealth::unhealthy(
                "rpc_connection".to_string(),
                format!("RPC error: {}", e),
            ),
            Ok(Err(e)) => ComponentHealth::unhealthy(
                "rpc_connection".to_string(),
                format!("Task error: {}", e),
            ),
            Err(_) => ComponentHealth::unhealthy(
                "rpc_connection".to_string(),
                "Health check timeout".to_string(),
            ),
        }
    }

    /// Check blockchain sync status
    async fn check_sync_status(&self, client: &BitcoinClient) -> ComponentHealth {
        match tokio::task::spawn_blocking({
            let info = client.get_blockchain_info();
            move || info
        })
        .await
        {
            Ok(Ok(info)) => {
                let blocks = info.blocks;
                let headers = info.headers;
                let syncing = blocks < headers;

                let metadata = serde_json::json!({
                    "blocks": blocks,
                    "headers": headers,
                    "syncing": syncing,
                    "verification_progress": info.verification_progress,
                });

                if syncing && (headers - blocks) > 10 {
                    ComponentHealth::degraded(
                        "blockchain_sync".to_string(),
                        format!("Syncing: {} blocks behind", headers - blocks),
                    )
                    .with_metadata(metadata)
                } else {
                    ComponentHealth::healthy("blockchain_sync".to_string()).with_metadata(metadata)
                }
            }
            Ok(Err(e)) => ComponentHealth::unhealthy(
                "blockchain_sync".to_string(),
                format!("Failed to get blockchain info: {}", e),
            ),
            Err(e) => ComponentHealth::unhealthy(
                "blockchain_sync".to_string(),
                format!("Task error: {}", e),
            ),
        }
    }

    /// Check mempool health
    async fn check_mempool(&self, client: &BitcoinClient) -> ComponentHealth {
        match tokio::task::spawn_blocking({
            let mempool_info = client.get_mempool_info();
            move || mempool_info
        })
        .await
        {
            Ok(Ok(info)) => {
                let size = info.size;
                let bytes = info.bytes;

                let metadata = serde_json::json!({
                    "size": size,
                    "bytes": bytes,
                    "usage": info.usage,
                });

                // Warn if mempool is very large (>50MB)
                if bytes > 50_000_000 {
                    ComponentHealth::degraded(
                        "mempool".to_string(),
                        format!("Large mempool: {} bytes", bytes),
                    )
                    .with_metadata(metadata)
                } else {
                    ComponentHealth::healthy("mempool".to_string()).with_metadata(metadata)
                }
            }
            Ok(Err(e)) => ComponentHealth::degraded(
                "mempool".to_string(),
                format!("Failed to get mempool info: {}", e),
            ),
            Err(e) => {
                ComponentHealth::degraded("mempool".to_string(), format!("Task error: {}", e))
            }
        }
    }

    /// Check dependencies (Bitcoin Core, Lightning Node)
    async fn check_dependencies(&self, client: &BitcoinClient) -> DependencyHealth {
        let bitcoin_core = match tokio::task::spawn_blocking({
            let network_info = client.get_network_info();
            move || network_info
        })
        .await
        {
            Ok(Ok(info)) => {
                let metadata = serde_json::json!({
                    "version": info.version,
                    "subversion": info.subversion,
                    "protocol_version": info.protocol_version,
                    "connections": info.connections,
                });

                if info.connections == 0 {
                    ComponentHealth::degraded(
                        "bitcoin_core".to_string(),
                        "No peer connections".to_string(),
                    )
                    .with_metadata(metadata)
                } else {
                    ComponentHealth::healthy("bitcoin_core".to_string()).with_metadata(metadata)
                }
            }
            Ok(Err(e)) => ComponentHealth::unhealthy(
                "bitcoin_core".to_string(),
                format!("Connection failed: {}", e),
            ),
            Err(e) => {
                ComponentHealth::unhealthy("bitcoin_core".to_string(), format!("Task error: {}", e))
            }
        };

        let lightning_node = if let Some(provider) = &self.lightning_provider {
            Some(self.check_lightning_node(provider.as_ref()).await)
        } else {
            None
        };

        DependencyHealth {
            bitcoin_core,
            lightning_node,
        }
    }

    /// Check Lightning Node health
    async fn check_lightning_node(&self, provider: &dyn LightningProvider) -> ComponentHealth {
        match tokio::time::timeout(self.config.check_timeout, provider.get_info()).await {
            Ok(Ok(info)) => {
                let metadata = serde_json::json!({
                    "pubkey": info.pubkey,
                    "alias": info.alias,
                    "version": info.version,
                    "num_active_channels": info.num_active_channels,
                    "num_pending_channels": info.num_pending_channels,
                    "num_peers": info.num_peers,
                    "block_height": info.block_height,
                    "synced_to_chain": info.synced_to_chain,
                    "network": info.network,
                });

                if !info.synced_to_chain {
                    ComponentHealth::degraded(
                        "lightning_node".to_string(),
                        "Node not synced to chain".to_string(),
                    )
                    .with_metadata(metadata)
                } else if info.num_active_channels == 0 {
                    ComponentHealth::degraded(
                        "lightning_node".to_string(),
                        "No active channels".to_string(),
                    )
                    .with_metadata(metadata)
                } else if info.num_peers == 0 {
                    ComponentHealth::degraded(
                        "lightning_node".to_string(),
                        "No connected peers".to_string(),
                    )
                    .with_metadata(metadata)
                } else {
                    ComponentHealth::healthy("lightning_node".to_string()).with_metadata(metadata)
                }
            }
            Ok(Err(e)) => ComponentHealth::unhealthy(
                "lightning_node".to_string(),
                format!("Failed to get node info: {}", e),
            ),
            Err(_) => ComponentHealth::unhealthy(
                "lightning_node".to_string(),
                "Health check timeout".to_string(),
            ),
        }
    }

    /// Check resource utilization (placeholder - would need system access)
    async fn check_resources(&self) -> ResourceUtilization {
        // This is a placeholder. In a real implementation, you would use
        // libraries like `sysinfo` to get actual system metrics
        ResourceUtilization::default()
    }

    /// Determine overall health status from components
    fn determine_overall_status(
        &self,
        components: &[ComponentHealth],
        dependencies: &DependencyHealth,
    ) -> HealthStatus {
        // Check if any component is unhealthy
        if components
            .iter()
            .any(|c| c.status == HealthStatus::Unhealthy)
            || dependencies.bitcoin_core.status == HealthStatus::Unhealthy
            || dependencies
                .lightning_node
                .as_ref()
                .is_some_and(|ln| ln.status == HealthStatus::Unhealthy)
        {
            return HealthStatus::Unhealthy;
        }

        // Check if any component is degraded
        if components
            .iter()
            .any(|c| c.status == HealthStatus::Degraded)
            || dependencies.bitcoin_core.status == HealthStatus::Degraded
            || dependencies
                .lightning_node
                .as_ref()
                .is_some_and(|ln| ln.status == HealthStatus::Degraded)
        {
            return HealthStatus::Degraded;
        }

        HealthStatus::Healthy
    }

    /// Get the last health report
    pub async fn get_last_report(&self) -> Option<HealthReport> {
        self.last_report.read().await.clone()
    }

    /// Start background health checking
    pub fn start_background_checks(
        self: Arc<Self>,
        bitcoin_client: Arc<BitcoinClient>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.check_interval);

            loop {
                interval.tick().await;

                match self.check_health(&bitcoin_client).await {
                    Ok(report) => {
                        if !report.is_all_healthy() {
                            warn!(
                                status = ?report.status,
                                "Health check completed with issues"
                            );
                        } else {
                            debug!("Health check passed");
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Health check failed");
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(HealthStatus::Unhealthy.is_degraded_or_worse());
    }

    #[test]
    fn test_component_health() {
        let healthy = ComponentHealth::healthy("test".to_string());
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert!(healthy.message.is_none());

        let degraded = ComponentHealth::degraded("test".to_string(), "Warning".to_string());
        assert_eq!(degraded.status, HealthStatus::Degraded);
        assert_eq!(degraded.message, Some("Warning".to_string()));
    }

    #[test]
    fn test_component_health_with_metadata() {
        let metadata = serde_json::json!({"key": "value"});
        let health = ComponentHealth::healthy("test".to_string()).with_metadata(metadata.clone());

        assert_eq!(health.metadata, metadata);
    }

    #[test]
    fn test_health_check_config_defaults() {
        let config = HealthCheckConfig::default();
        assert!(config.check_interval.as_secs() > 0);
        assert!(config.max_block_age_secs > 0);
    }

    #[test]
    fn test_resource_utilization_default() {
        let resources = ResourceUtilization::default();
        assert!(resources.memory_usage_percent.is_none());
        assert!(resources.cpu_usage_percent.is_none());
    }

    #[tokio::test]
    async fn test_health_check_manager() {
        let manager = HealthCheckManager::with_defaults();
        assert!(manager.get_last_report().await.is_none());
    }

    #[test]
    fn test_determine_overall_status() {
        let manager = HealthCheckManager::with_defaults();

        let healthy_components = vec![
            ComponentHealth::healthy("comp1".to_string()),
            ComponentHealth::healthy("comp2".to_string()),
        ];

        let deps = DependencyHealth {
            bitcoin_core: ComponentHealth::healthy("bitcoin".to_string()),
            lightning_node: None,
        };

        let status = manager.determine_overall_status(&healthy_components, &deps);
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_determine_overall_status_degraded() {
        let manager = HealthCheckManager::with_defaults();

        let components = vec![
            ComponentHealth::healthy("comp1".to_string()),
            ComponentHealth::degraded("comp2".to_string(), "Issue".to_string()),
        ];

        let deps = DependencyHealth {
            bitcoin_core: ComponentHealth::healthy("bitcoin".to_string()),
            lightning_node: None,
        };

        let status = manager.determine_overall_status(&components, &deps);
        assert_eq!(status, HealthStatus::Degraded);
    }

    #[test]
    fn test_determine_overall_status_unhealthy() {
        let manager = HealthCheckManager::with_defaults();

        let components = vec![ComponentHealth::unhealthy(
            "comp1".to_string(),
            "Critical error".to_string(),
        )];

        let deps = DependencyHealth {
            bitcoin_core: ComponentHealth::healthy("bitcoin".to_string()),
            lightning_node: None,
        };

        let status = manager.determine_overall_status(&components, &deps);
        assert_eq!(status, HealthStatus::Unhealthy);
    }
}
