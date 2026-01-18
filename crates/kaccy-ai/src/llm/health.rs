//! Provider health monitoring and automatic failover
//!
//! This module tracks LLM provider health and availability for intelligent failover decisions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Health status of a provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Provider is healthy and available
    Healthy,
    /// Provider is experiencing issues but operational
    Degraded,
    /// Provider is unhealthy and should be avoided
    Unhealthy,
    /// Provider status is unknown (not enough data)
    Unknown,
}

/// Provider health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    /// Provider name
    pub provider_name: String,
    /// Current health status
    pub status: HealthStatus,
    /// Health score (0-100, higher is better)
    pub health_score: f64,
    /// Average response time (milliseconds)
    pub avg_response_time_ms: f64,
    /// Success rate (0-100%)
    pub success_rate: f64,
    /// Total requests
    pub total_requests: u64,
    /// Failed requests
    pub failed_requests: u64,
    /// Last successful request timestamp
    pub last_success: Option<SystemTime>,
    /// Last failed request timestamp
    pub last_failure: Option<SystemTime>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Last health check timestamp
    pub last_check: SystemTime,
}

impl ProviderHealth {
    /// Create a new provider health tracker
    pub fn new(provider_name: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            status: HealthStatus::Unknown,
            health_score: 100.0,
            avg_response_time_ms: 0.0,
            success_rate: 100.0,
            total_requests: 0,
            failed_requests: 0,
            last_success: None,
            last_failure: None,
            consecutive_failures: 0,
            last_check: SystemTime::now(),
        }
    }

    /// Record a successful request
    pub fn record_success(&mut self, response_time_ms: f64) {
        self.total_requests += 1;
        self.consecutive_failures = 0;
        self.last_success = Some(SystemTime::now());
        self.last_check = SystemTime::now();

        // Update average response time with exponential moving average
        if self.avg_response_time_ms == 0.0 {
            self.avg_response_time_ms = response_time_ms;
        } else {
            self.avg_response_time_ms = 0.7 * self.avg_response_time_ms + 0.3 * response_time_ms;
        }

        self.update_metrics();
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.total_requests += 1;
        self.failed_requests += 1;
        self.consecutive_failures += 1;
        self.last_failure = Some(SystemTime::now());
        self.last_check = SystemTime::now();

        self.update_metrics();
    }

    /// Update calculated metrics and health status
    fn update_metrics(&mut self) {
        // Calculate success rate
        if self.total_requests > 0 {
            let successful = self.total_requests - self.failed_requests;
            self.success_rate = (successful as f64 / self.total_requests as f64) * 100.0;
        }

        // Calculate health score (composite metric)
        let response_time_score = if self.avg_response_time_ms < 1000.0 {
            100.0
        } else if self.avg_response_time_ms < 3000.0 {
            80.0
        } else if self.avg_response_time_ms < 5000.0 {
            50.0
        } else {
            20.0
        };

        let success_rate_score = self.success_rate;

        let consecutive_failure_penalty = (f64::from(self.consecutive_failures) * 10.0).min(50.0);

        self.health_score = ((response_time_score * 0.3 + success_rate_score * 0.7)
            - consecutive_failure_penalty)
            .max(0.0);

        // Determine health status
        self.status = if self.consecutive_failures >= 5 {
            HealthStatus::Unhealthy
        } else if self.health_score >= 80.0 {
            HealthStatus::Healthy
        } else if self.health_score >= 50.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };
    }

    /// Check if the provider is available for requests
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self.status, HealthStatus::Healthy | HealthStatus::Degraded)
    }

    /// Get time since last success
    #[must_use]
    pub fn time_since_last_success(&self) -> Option<Duration> {
        self.last_success
            .and_then(|t| SystemTime::now().duration_since(t).ok())
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval
    pub check_interval: Duration,
    /// Threshold for marking provider as unhealthy (consecutive failures)
    pub unhealthy_threshold: u32,
    /// Threshold for marking provider as degraded (success rate %)
    pub degraded_threshold: f64,
    /// Minimum requests before calculating health
    pub min_requests: u64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(60),
            unhealthy_threshold: 5,
            degraded_threshold: 80.0,
            min_requests: 10,
        }
    }
}

/// Health monitor for tracking multiple providers
pub struct HealthMonitor {
    providers: Arc<RwLock<HashMap<String, ProviderHealth>>>,
    #[allow(dead_code)]
    config: HealthCheckConfig,
}

impl HealthMonitor {
    /// Create a new health monitor
    #[must_use]
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a provider for monitoring
    pub async fn register_provider(&self, provider_name: impl Into<String>) {
        let name = provider_name.into();
        let mut providers = self.providers.write().await;
        providers
            .entry(name.clone())
            .or_insert_with(|| ProviderHealth::new(name));
    }

    /// Record a successful request
    pub async fn record_success(&self, provider_name: &str, response_time_ms: f64) {
        let mut providers = self.providers.write().await;
        if let Some(health) = providers.get_mut(provider_name) {
            health.record_success(response_time_ms);
        } else {
            let mut health = ProviderHealth::new(provider_name);
            health.record_success(response_time_ms);
            providers.insert(provider_name.to_string(), health);
        }
    }

    /// Record a failed request
    pub async fn record_failure(&self, provider_name: &str) {
        let mut providers = self.providers.write().await;
        if let Some(health) = providers.get_mut(provider_name) {
            health.record_failure();
        } else {
            let mut health = ProviderHealth::new(provider_name);
            health.record_failure();
            providers.insert(provider_name.to_string(), health);
        }
    }

    /// Get health status for a provider
    pub async fn get_health(&self, provider_name: &str) -> Option<ProviderHealth> {
        let providers = self.providers.read().await;
        providers.get(provider_name).cloned()
    }

    /// Get health status for all providers
    pub async fn get_all_health(&self) -> HashMap<String, ProviderHealth> {
        self.providers.read().await.clone()
    }

    /// Get the healthiest available provider
    pub async fn get_healthiest_provider(&self) -> Option<String> {
        let providers = self.providers.read().await;

        providers
            .iter()
            .filter(|(_, health)| health.is_available())
            .max_by(|(_, a), (_, b)| {
                a.health_score
                    .partial_cmp(&b.health_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone())
    }

    /// Get providers ranked by health score
    pub async fn get_providers_by_health(&self) -> Vec<(String, f64)> {
        let providers = self.providers.read().await;

        let mut ranked: Vec<_> = providers
            .iter()
            .map(|(name, health)| (name.clone(), health.health_score))
            .collect();

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
    }

    /// Check if a provider is healthy enough to use
    pub async fn is_provider_healthy(&self, provider_name: &str) -> bool {
        let providers = self.providers.read().await;
        providers
            .get(provider_name)
            .map_or(true, ProviderHealth::is_available) // Unknown providers are assumed healthy
    }

    /// Reset health statistics for a provider
    pub async fn reset_provider(&self, provider_name: &str) {
        let mut providers = self.providers.write().await;
        if let Some(health) = providers.get_mut(provider_name) {
            *health = ProviderHealth::new(provider_name);
        }
    }

    /// Get health summary
    pub async fn get_summary(&self) -> HealthSummary {
        let providers = self.providers.read().await;

        let total_providers = providers.len();
        let healthy = providers
            .values()
            .filter(|h| h.status == HealthStatus::Healthy)
            .count();
        let degraded = providers
            .values()
            .filter(|h| h.status == HealthStatus::Degraded)
            .count();
        let unhealthy = providers
            .values()
            .filter(|h| h.status == HealthStatus::Unhealthy)
            .count();

        let avg_health_score = if total_providers > 0 {
            providers.values().map(|h| h.health_score).sum::<f64>() / total_providers as f64
        } else {
            0.0
        };

        HealthSummary {
            total_providers,
            healthy,
            degraded,
            unhealthy,
            avg_health_score,
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(HealthCheckConfig::default())
    }
}

/// Health summary across all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Total number of providers
    pub total_providers: usize,
    /// Number of healthy providers
    pub healthy: usize,
    /// Number of degraded providers
    pub degraded: usize,
    /// Number of unhealthy providers
    pub unhealthy: usize,
    /// Average health score across all providers
    pub avg_health_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_health_creation() {
        let health = ProviderHealth::new("openai");
        assert_eq!(health.provider_name, "openai");
        assert_eq!(health.status, HealthStatus::Unknown);
        assert_eq!(health.total_requests, 0);
    }

    #[test]
    fn test_provider_health_success() {
        let mut health = ProviderHealth::new("openai");
        health.record_success(500.0);

        assert_eq!(health.total_requests, 1);
        assert_eq!(health.failed_requests, 0);
        assert_eq!(health.consecutive_failures, 0);
        assert!((health.success_rate - 100.0).abs() < f64::EPSILON);
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_provider_health_failure() {
        let mut health = ProviderHealth::new("openai");
        health.record_failure();

        assert_eq!(health.total_requests, 1);
        assert_eq!(health.failed_requests, 1);
        assert_eq!(health.consecutive_failures, 1);
        assert!((health.success_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_health_consecutive_failures() {
        let mut health = ProviderHealth::new("openai");

        for _ in 0..5 {
            health.record_failure();
        }

        assert_eq!(health.consecutive_failures, 5);
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert!(!health.is_available());
    }

    #[test]
    fn test_provider_health_recovery() {
        let mut health = ProviderHealth::new("openai");

        // Record failures
        for _ in 0..3 {
            health.record_failure();
        }
        assert_eq!(health.consecutive_failures, 3);

        // Record success - should reset consecutive failures
        health.record_success(500.0);
        assert_eq!(health.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_health_monitor_registration() {
        let monitor = HealthMonitor::default();
        monitor.register_provider("openai").await;

        let health = monitor.get_health("openai").await;
        assert!(health.is_some());
        assert_eq!(health.unwrap().provider_name, "openai");
    }

    #[tokio::test]
    async fn test_health_monitor_success_tracking() {
        let monitor = HealthMonitor::default();
        monitor.record_success("openai", 500.0).await;

        let health = monitor.get_health("openai").await.unwrap();
        assert_eq!(health.total_requests, 1);
        assert_eq!(health.failed_requests, 0);
        assert!((health.avg_response_time_ms - 500.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_health_monitor_failure_tracking() {
        let monitor = HealthMonitor::default();
        monitor.record_failure("openai").await;

        let health = monitor.get_health("openai").await.unwrap();
        assert_eq!(health.total_requests, 1);
        assert_eq!(health.failed_requests, 1);
    }

    #[tokio::test]
    async fn test_health_monitor_healthiest_provider() {
        let monitor = HealthMonitor::default();

        monitor.record_success("openai", 500.0).await;
        monitor.record_success("anthropic", 300.0).await;
        monitor.record_failure("gemini").await;

        let healthiest = monitor.get_healthiest_provider().await;
        assert!(healthiest.is_some());
        // Both openai and anthropic should be healthy, might be either one
        let name = healthiest.unwrap();
        assert!(name == "openai" || name == "anthropic");
    }

    #[tokio::test]
    async fn test_health_monitor_ranking() {
        let monitor = HealthMonitor::default();

        monitor.record_success("openai", 500.0).await;
        monitor.record_success("anthropic", 300.0).await;
        monitor.record_failure("gemini").await;

        let ranked = monitor.get_providers_by_health().await;
        assert_eq!(ranked.len(), 3);

        // Health scores should be in descending order
        for i in 1..ranked.len() {
            assert!(ranked[i - 1].1 >= ranked[i].1);
        }
    }

    #[tokio::test]
    async fn test_health_monitor_summary() {
        let monitor = HealthMonitor::default();

        monitor.record_success("openai", 500.0).await;
        monitor.record_success("anthropic", 300.0).await;
        monitor.record_failure("gemini").await;

        let summary = monitor.get_summary().await;
        assert_eq!(summary.total_providers, 3);
        assert!(summary.avg_health_score > 0.0);
    }

    #[tokio::test]
    async fn test_health_monitor_reset() {
        let monitor = HealthMonitor::default();

        monitor.record_failure("openai").await;
        let health_before = monitor.get_health("openai").await.unwrap();
        assert_eq!(health_before.failed_requests, 1);

        monitor.reset_provider("openai").await;
        let health_after = monitor.get_health("openai").await.unwrap();
        assert_eq!(health_after.failed_requests, 0);
    }
}
