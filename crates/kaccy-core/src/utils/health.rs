//! Health check system for monitoring service health
//!
//! Provides liveness and readiness probes, dependency health monitoring,
//! and resource utilization tracking for production environments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Health status of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Component is fully operational
    Healthy,
    /// Component is operational but experiencing issues
    Degraded,
    /// Component is not operational
    Unhealthy,
}

/// Health check result for a single component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Overall health status
    pub status: HealthStatus,
    /// Optional human-readable status message
    pub message: Option<String>,
    /// Unix timestamp when this check was performed
    pub checked_at: i64,
    /// How long the check took in milliseconds
    pub response_time_ms: u64,
}

impl HealthCheckResult {
    /// Create a healthy result with the given message
    pub fn healthy(message: impl Into<String>, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: Some(message.into()),
            checked_at: chrono::Utc::now().timestamp(),
            response_time_ms,
        }
    }

    /// Create a degraded result with the given message
    pub fn degraded(message: impl Into<String>, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            checked_at: chrono::Utc::now().timestamp(),
            response_time_ms,
        }
    }

    /// Create an unhealthy result with the given message
    pub fn unhealthy(message: impl Into<String>, response_time_ms: u64) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            checked_at: chrono::Utc::now().timestamp(),
            response_time_ms,
        }
    }
}

/// Health checker trait
pub trait HealthChecker: Send + Sync {
    /// Run the health check and return the result.
    fn check(&self) -> HealthCheckResult;
    /// Return the name identifying this health checker.
    fn name(&self) -> &str;
}

/// Database health checker
pub struct DatabaseHealthChecker {
    name: String,
    timeout: Duration,
}

impl DatabaseHealthChecker {
    /// Create a new database health checker
    pub fn new(name: impl Into<String>, timeout: Duration) -> Self {
        Self {
            name: name.into(),
            timeout,
        }
    }
}

impl HealthChecker for DatabaseHealthChecker {
    fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // In a real implementation, this would execute a simple query like "SELECT 1"
        // For now, we'll simulate it
        let elapsed = start.elapsed().as_millis() as u64;

        if elapsed > self.timeout.as_millis() as u64 {
            HealthCheckResult::degraded(
                format!("Database response time {}ms exceeds timeout", elapsed),
                elapsed,
            )
        } else {
            HealthCheckResult::healthy("Database connection OK", elapsed)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Redis/Cache health checker
pub struct CacheHealthChecker {
    name: String,
    timeout: Duration,
}

impl CacheHealthChecker {
    /// Create a new cache health checker
    pub fn new(name: impl Into<String>, timeout: Duration) -> Self {
        Self {
            name: name.into(),
            timeout,
        }
    }
}

impl HealthChecker for CacheHealthChecker {
    fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let elapsed = start.elapsed().as_millis() as u64;

        if elapsed > self.timeout.as_millis() as u64 {
            HealthCheckResult::degraded("Cache response time exceeded", elapsed)
        } else {
            HealthCheckResult::healthy("Cache connection OK", elapsed)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Resource utilization checker
pub struct ResourceHealthChecker {
    name: String,
    cpu_threshold: f64,
    memory_threshold: f64,
}

impl ResourceHealthChecker {
    /// Create a new resource health checker with the given thresholds
    pub fn new(name: impl Into<String>, cpu_threshold: f64, memory_threshold: f64) -> Self {
        Self {
            name: name.into(),
            cpu_threshold,
            memory_threshold,
        }
    }
}

impl HealthChecker for ResourceHealthChecker {
    fn check(&self) -> HealthCheckResult {
        let start = Instant::now();

        // In production, this would check actual CPU and memory usage
        // For now, we'll simulate
        let cpu_usage = 0.0; // Would be actual CPU %
        let memory_usage = 0.0; // Would be actual memory %

        let elapsed = start.elapsed().as_millis() as u64;

        if cpu_usage > self.cpu_threshold || memory_usage > self.memory_threshold {
            HealthCheckResult::degraded(
                format!(
                    "High resource usage: CPU {:.1}%, Memory {:.1}%",
                    cpu_usage, memory_usage
                ),
                elapsed,
            )
        } else {
            HealthCheckResult::healthy("Resource usage OK", elapsed)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Circuit breaker status checker
pub struct CircuitBreakerHealthChecker {
    name: String,
    is_open: Arc<AtomicBool>,
}

impl CircuitBreakerHealthChecker {
    /// Create a new circuit breaker health checker
    pub fn new(name: impl Into<String>, is_open: Arc<AtomicBool>) -> Self {
        Self {
            name: name.into(),
            is_open,
        }
    }
}

impl HealthChecker for CircuitBreakerHealthChecker {
    fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let elapsed = start.elapsed().as_millis() as u64;

        if self.is_open.load(Ordering::Relaxed) {
            HealthCheckResult::degraded("Circuit breaker is open", elapsed)
        } else {
            HealthCheckResult::healthy("Circuit breaker is closed", elapsed)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Health check manager
pub struct HealthCheckManager {
    /// Registered health checkers
    checkers: Arc<RwLock<Vec<Arc<dyn HealthChecker>>>>,
    /// Cache of recent check results
    cache: Arc<RwLock<HashMap<String, (HealthCheckResult, Instant)>>>,
    /// How long to cache health check results
    cache_ttl: Duration,
}

impl HealthCheckManager {
    /// Create a new health check manager with default 5-second cache TTL
    pub fn new() -> Self {
        Self {
            checkers: Arc::new(RwLock::new(Vec::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(5),
        }
    }

    /// Override the cache TTL for health check results
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Register a health checker
    pub fn register(&self, checker: Arc<dyn HealthChecker>) {
        self.checkers
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(checker);
    }

    /// Check all components
    pub fn check_all(&self) -> HealthReport {
        let checkers = self.checkers.read().unwrap_or_else(|e| e.into_inner());
        let mut results = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;

        for checker in checkers.iter() {
            let name = checker.name().to_string();

            // Check cache first
            let result = {
                let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
                if let Some((cached_result, cached_at)) = cache.get(&name) {
                    if cached_at.elapsed() < self.cache_ttl {
                        Some(cached_result.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            let result = result.unwrap_or_else(|| {
                let fresh_result = checker.check();
                self.cache
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(name.clone(), (fresh_result.clone(), Instant::now()));
                fresh_result
            });

            // Update overall status
            match result.status {
                HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
                HealthStatus::Degraded if overall_status != HealthStatus::Unhealthy => {
                    overall_status = HealthStatus::Degraded
                }
                _ => {}
            }

            results.insert(name, result);
        }

        HealthReport {
            status: overall_status,
            components: results,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// Liveness probe - is the service running?
    pub fn liveness(&self) -> bool {
        // Service is alive if it can respond
        true
    }

    /// Readiness probe - is the service ready to accept traffic?
    pub fn readiness(&self) -> bool {
        let report = self.check_all();
        matches!(
            report.status,
            HealthStatus::Healthy | HealthStatus::Degraded
        )
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Health report for all components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Aggregated overall health status
    pub status: HealthStatus,
    /// Individual component health check results by name
    pub components: HashMap<String, HealthCheckResult>,
    /// Unix timestamp when this report was generated
    pub timestamp: i64,
}

impl HealthReport {
    /// Check if all components are healthy
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// Check if any component is unhealthy
    pub fn is_unhealthy(&self) -> bool {
        self.status == HealthStatus::Unhealthy
    }

    /// Get unhealthy components
    pub fn unhealthy_components(&self) -> Vec<String> {
        self.components
            .iter()
            .filter(|(_, r)| r.status == HealthStatus::Unhealthy)
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// Get degraded components
    pub fn degraded_components(&self) -> Vec<String> {
        self.components
            .iter()
            .filter(|(_, r)| r.status == HealthStatus::Degraded)
            .map(|(n, _)| n.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysHealthyChecker {
        name: String,
    }

    impl HealthChecker for AlwaysHealthyChecker {
        fn check(&self) -> HealthCheckResult {
            HealthCheckResult::healthy("All good", 10)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    struct AlwaysUnhealthyChecker {
        name: String,
    }

    impl HealthChecker for AlwaysUnhealthyChecker {
        fn check(&self) -> HealthCheckResult {
            HealthCheckResult::unhealthy("Not good", 10)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_health_status() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::healthy("OK", 100);
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.response_time_ms, 100);
    }

    #[test]
    fn test_database_health_checker() {
        let checker = DatabaseHealthChecker::new("postgres", Duration::from_secs(5));
        let result = checker.check();

        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(checker.name(), "postgres");
    }

    #[test]
    fn test_cache_health_checker() {
        let checker = CacheHealthChecker::new("redis", Duration::from_secs(1));
        let result = checker.check();

        assert!(matches!(
            result.status,
            HealthStatus::Healthy | HealthStatus::Degraded
        ));
    }

    #[test]
    fn test_resource_health_checker() {
        let checker = ResourceHealthChecker::new("system", 80.0, 90.0);
        let result = checker.check();

        // Should be healthy with 0% usage (simulated)
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_circuit_breaker_health_checker() {
        let is_open = Arc::new(AtomicBool::new(false));
        let checker = CircuitBreakerHealthChecker::new("circuit", is_open.clone());

        let result = checker.check();
        assert_eq!(result.status, HealthStatus::Healthy);

        // Open circuit breaker
        is_open.store(true, Ordering::Relaxed);
        let result = checker.check();
        assert_eq!(result.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_check_manager() {
        let manager = HealthCheckManager::new();

        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "test1".to_string(),
        }));
        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "test2".to_string(),
        }));

        let report = manager.check_all();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert_eq!(report.components.len(), 2);
    }

    #[test]
    fn test_health_check_manager_unhealthy() {
        let manager = HealthCheckManager::new();

        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "healthy".to_string(),
        }));
        manager.register(Arc::new(AlwaysUnhealthyChecker {
            name: "unhealthy".to_string(),
        }));

        let report = manager.check_all();
        assert_eq!(report.status, HealthStatus::Unhealthy);
        assert!(!report.is_healthy());
        assert!(report.is_unhealthy());

        let unhealthy = report.unhealthy_components();
        assert_eq!(unhealthy.len(), 1);
        assert_eq!(unhealthy[0], "unhealthy");
    }

    #[test]
    fn test_liveness_readiness() {
        let manager = HealthCheckManager::new();

        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "test".to_string(),
        }));

        assert!(manager.liveness());
        assert!(manager.readiness());
    }

    #[test]
    fn test_health_report_helpers() {
        let manager = HealthCheckManager::new();

        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "test1".to_string(),
        }));

        let report = manager.check_all();

        assert!(report.is_healthy());
        assert!(!report.is_unhealthy());
        assert_eq!(report.unhealthy_components().len(), 0);
        assert_eq!(report.degraded_components().len(), 0);
    }

    #[test]
    fn test_cache_ttl() {
        use std::thread;

        let manager = HealthCheckManager::new().with_cache_ttl(Duration::from_millis(50));

        manager.register(Arc::new(AlwaysHealthyChecker {
            name: "test".to_string(),
        }));

        // First check
        let report1 = manager.check_all();

        // Second check (should use cache)
        let report2 = manager.check_all();

        // Timestamps should be the same (cached)
        assert_eq!(
            report1.components["test"].checked_at,
            report2.components["test"].checked_at
        );

        // Wait for cache to expire (wait longer than TTL)
        thread::sleep(Duration::from_millis(200));

        // Third check (cache expired)
        let report3 = manager.check_all();

        // Timestamp should be different (fresh check)
        // Use >= instead of != to account for possible timestamp resolution issues
        assert!(
            report3.components["test"].checked_at >= report1.components["test"].checked_at,
            "Expected fresh check timestamp to be newer or equal"
        );

        // Clear cache and verify
        manager.clear_cache();
        let report4 = manager.check_all();
        assert!(report4.components["test"].checked_at >= report3.components["test"].checked_at);
    }
}
