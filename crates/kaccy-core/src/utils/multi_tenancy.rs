//! Multi-tenancy support for enterprise features
//!
//! Provides tenant isolation, resource quotas, and tenant-specific configurations

use crate::error::CoreError;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Tenant identifier
pub type TenantId = Uuid;

/// Tenant manager for multi-tenancy support
pub struct TenantManager {
    /// Registered tenants
    tenants: Arc<RwLock<HashMap<TenantId, Tenant>>>,
    /// Tenant configurations
    configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
}

/// A tenant in the system
#[derive(Debug, Clone)]
pub struct Tenant {
    /// Tenant ID
    pub id: TenantId,
    /// Tenant name
    pub name: String,
    /// Tenant status
    pub status: TenantStatus,
    /// Created at
    pub created_at: Instant,
    /// Resource quotas
    pub quotas: ResourceQuotas,
    /// Current resource usage
    pub usage: ResourceUsage,
}

/// Tenant status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    /// Active and operational
    Active,
    /// Temporarily suspended
    Suspended,
    /// Archived/deleted
    Archived,
}

/// Resource quotas for a tenant
#[derive(Debug, Clone)]
pub struct ResourceQuotas {
    /// Maximum number of users
    pub max_users: usize,
    /// Maximum number of tokens
    pub max_tokens: usize,
    /// Maximum daily trading volume
    pub max_daily_volume: Decimal,
    /// Maximum number of API calls per minute
    pub max_api_calls_per_minute: usize,
    /// Maximum storage in bytes
    pub max_storage_bytes: usize,
    /// Maximum concurrent connections
    pub max_concurrent_connections: usize,
}

/// Current resource usage for a tenant
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    /// Current number of users
    pub current_users: usize,
    /// Current number of tokens
    pub current_tokens: usize,
    /// Today's trading volume
    pub daily_volume: Decimal,
    /// API calls in current minute
    pub api_calls_current_minute: usize,
    /// Current storage in bytes
    pub current_storage_bytes: usize,
    /// Current concurrent connections
    pub current_connections: usize,
    /// Last reset time
    pub last_reset: Instant,
}

/// Tenant-specific configuration
#[derive(Debug, Clone)]
pub struct TenantConfig {
    /// Custom branding settings
    pub branding: BrandingConfig,
    /// Fee structure overrides
    pub fee_structure: Option<FeeStructure>,
    /// Custom domain
    pub custom_domain: Option<String>,
    /// Feature flags
    pub features: HashMap<String, bool>,
    /// API key prefix
    pub api_key_prefix: String,
}

/// Branding configuration for white-label support
#[derive(Debug, Clone)]
pub struct BrandingConfig {
    /// Platform name
    pub platform_name: String,
    /// Logo URL
    pub logo_url: Option<String>,
    /// Primary color (hex)
    pub primary_color: String,
    /// Secondary color (hex)
    pub secondary_color: String,
    /// Support email
    pub support_email: String,
}

/// Custom fee structure for a tenant
#[derive(Debug, Clone)]
pub struct FeeStructure {
    /// Trading fee percentage
    pub trading_fee: Decimal,
    /// Withdrawal fee percentage
    pub withdrawal_fee: Decimal,
    /// Minimum fee
    pub minimum_fee: Decimal,
}

impl TenantManager {
    /// Create a new tenant manager
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new tenant
    pub fn register_tenant(
        &self,
        name: String,
        quotas: ResourceQuotas,
    ) -> Result<TenantId, CoreError> {
        let id = Uuid::new_v4();
        let tenant = Tenant {
            id,
            name: name.clone(),
            status: TenantStatus::Active,
            created_at: Instant::now(),
            quotas,
            usage: ResourceUsage {
                current_users: 0,
                current_tokens: 0,
                daily_volume: Decimal::ZERO,
                api_calls_current_minute: 0,
                current_storage_bytes: 0,
                current_connections: 0,
                last_reset: Instant::now(),
            },
        };

        // Default configuration
        let config = TenantConfig {
            branding: BrandingConfig {
                platform_name: name,
                logo_url: None,
                primary_color: "#3B82F6".to_string(),
                secondary_color: "#8B5CF6".to_string(),
                support_email: "support@example.com".to_string(),
            },
            fee_structure: None,
            custom_domain: None,
            features: HashMap::new(),
            api_key_prefix: format!("tenant_{}_", &id.to_string()[..8]),
        };

        self.tenants.write().unwrap().insert(id, tenant);
        self.configs.write().unwrap().insert(id, config);

        Ok(id)
    }

    /// Get tenant by ID
    pub fn get_tenant(&self, tenant_id: &TenantId) -> Option<Tenant> {
        self.tenants.read().unwrap().get(tenant_id).cloned()
    }

    /// Update tenant status
    pub fn update_status(
        &self,
        tenant_id: &TenantId,
        status: TenantStatus,
    ) -> Result<(), CoreError> {
        let mut tenants = self.tenants.write().unwrap();
        if let Some(tenant) = tenants.get_mut(tenant_id) {
            tenant.status = status;
            Ok(())
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Check if tenant can perform action based on quotas
    pub fn check_quota(
        &self,
        tenant_id: &TenantId,
        resource: ResourceType,
        amount: usize,
    ) -> Result<bool, CoreError> {
        let tenants = self.tenants.read().unwrap();
        if let Some(tenant) = tenants.get(tenant_id) {
            if tenant.status != TenantStatus::Active {
                return Ok(false);
            }

            let available = match resource {
                ResourceType::Users => {
                    tenant.quotas.max_users > tenant.usage.current_users + amount
                }
                ResourceType::Tokens => {
                    tenant.quotas.max_tokens > tenant.usage.current_tokens + amount
                }
                ResourceType::ApiCalls => {
                    tenant.quotas.max_api_calls_per_minute
                        > tenant.usage.api_calls_current_minute + amount
                }
                ResourceType::Storage => {
                    tenant.quotas.max_storage_bytes > tenant.usage.current_storage_bytes + amount
                }
                ResourceType::Connections => {
                    tenant.quotas.max_concurrent_connections
                        > tenant.usage.current_connections + amount
                }
            };

            Ok(available)
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Increment resource usage
    pub fn increment_usage(
        &self,
        tenant_id: &TenantId,
        resource: ResourceType,
        amount: usize,
    ) -> Result<(), CoreError> {
        let mut tenants = self.tenants.write().unwrap();
        if let Some(tenant) = tenants.get_mut(tenant_id) {
            match resource {
                ResourceType::Users => tenant.usage.current_users += amount,
                ResourceType::Tokens => tenant.usage.current_tokens += amount,
                ResourceType::ApiCalls => tenant.usage.api_calls_current_minute += amount,
                ResourceType::Storage => tenant.usage.current_storage_bytes += amount,
                ResourceType::Connections => tenant.usage.current_connections += amount,
            }
            Ok(())
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Decrement resource usage
    pub fn decrement_usage(
        &self,
        tenant_id: &TenantId,
        resource: ResourceType,
        amount: usize,
    ) -> Result<(), CoreError> {
        let mut tenants = self.tenants.write().unwrap();
        if let Some(tenant) = tenants.get_mut(tenant_id) {
            match resource {
                ResourceType::Users => {
                    tenant.usage.current_users = tenant.usage.current_users.saturating_sub(amount)
                }
                ResourceType::Tokens => {
                    tenant.usage.current_tokens = tenant.usage.current_tokens.saturating_sub(amount)
                }
                ResourceType::ApiCalls => {
                    tenant.usage.api_calls_current_minute =
                        tenant.usage.api_calls_current_minute.saturating_sub(amount)
                }
                ResourceType::Storage => {
                    tenant.usage.current_storage_bytes =
                        tenant.usage.current_storage_bytes.saturating_sub(amount)
                }
                ResourceType::Connections => {
                    tenant.usage.current_connections =
                        tenant.usage.current_connections.saturating_sub(amount)
                }
            }
            Ok(())
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Reset periodic usage counters (e.g., API calls per minute)
    pub fn reset_periodic_usage(&self, tenant_id: &TenantId) -> Result<(), CoreError> {
        let mut tenants = self.tenants.write().unwrap();
        if let Some(tenant) = tenants.get_mut(tenant_id) {
            tenant.usage.api_calls_current_minute = 0;
            tenant.usage.last_reset = Instant::now();
            Ok(())
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Get tenant configuration
    pub fn get_config(&self, tenant_id: &TenantId) -> Option<TenantConfig> {
        self.configs.read().unwrap().get(tenant_id).cloned()
    }

    /// Update tenant configuration
    pub fn update_config(
        &self,
        tenant_id: &TenantId,
        config: TenantConfig,
    ) -> Result<(), CoreError> {
        let mut configs = self.configs.write().unwrap();
        if self.tenants.read().unwrap().contains_key(tenant_id) {
            configs.insert(*tenant_id, config);
            Ok(())
        } else {
            Err(CoreError::NotFound("Tenant not found".to_string()))
        }
    }

    /// Get resource utilization percentage
    pub fn get_utilization(&self, tenant_id: &TenantId) -> Option<ResourceUtilization> {
        let tenants = self.tenants.read().unwrap();
        tenants.get(tenant_id).map(|tenant| {
            let users_pct =
                (tenant.usage.current_users as f64 / tenant.quotas.max_users as f64) * 100.0;
            let tokens_pct =
                (tenant.usage.current_tokens as f64 / tenant.quotas.max_tokens as f64) * 100.0;
            let api_pct = (tenant.usage.api_calls_current_minute as f64
                / tenant.quotas.max_api_calls_per_minute as f64)
                * 100.0;
            let storage_pct = (tenant.usage.current_storage_bytes as f64
                / tenant.quotas.max_storage_bytes as f64)
                * 100.0;
            let connections_pct = (tenant.usage.current_connections as f64
                / tenant.quotas.max_concurrent_connections as f64)
                * 100.0;

            ResourceUtilization {
                users_percentage: users_pct,
                tokens_percentage: tokens_pct,
                api_calls_percentage: api_pct,
                storage_percentage: storage_pct,
                connections_percentage: connections_pct,
            }
        })
    }

    /// List all tenants
    pub fn list_tenants(&self) -> Vec<TenantId> {
        self.tenants.read().unwrap().keys().copied().collect()
    }

    /// Get tenant count
    pub fn tenant_count(&self) -> usize {
        self.tenants.read().unwrap().len()
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource type for quota checking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Active user accounts.
    Users,
    /// Protocol tokens created.
    Tokens,
    /// API request count.
    ApiCalls,
    /// Persistent data storage in bytes.
    Storage,
    /// Concurrent open connections.
    Connections,
}

/// Resource utilization percentages
#[derive(Debug, Clone)]
pub struct ResourceUtilization {
    /// Percentage of the user quota consumed (0–100).
    pub users_percentage: f64,
    /// Percentage of the token quota consumed (0–100).
    pub tokens_percentage: f64,
    /// Percentage of the API-call quota consumed (0–100).
    pub api_calls_percentage: f64,
    /// Percentage of the storage quota consumed (0–100).
    pub storage_percentage: f64,
    /// Percentage of the connection quota consumed (0–100).
    pub connections_percentage: f64,
}

/// Tenant isolation guard
pub struct TenantIsolation {
    /// Allowed tenant ID
    tenant_id: TenantId,
}

impl TenantIsolation {
    /// Create a new tenant isolation guard
    pub fn new(tenant_id: TenantId) -> Self {
        Self { tenant_id }
    }

    /// Verify that an operation is allowed for this tenant
    pub fn verify(&self, resource_tenant_id: &TenantId) -> Result<(), CoreError> {
        if self.tenant_id == *resource_tenant_id {
            Ok(())
        } else {
            Err(CoreError::Validation(
                "Cross-tenant access denied".to_string(),
            ))
        }
    }

    /// Get the tenant ID
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

/// Tenant rate limiter
pub struct TenantRateLimiter {
    /// Rate limits per tenant
    limits: Arc<RwLock<HashMap<TenantId, RateLimit>>>,
}

/// Rate limit state
#[derive(Debug, Clone)]
struct RateLimit {
    /// Requests in current window
    requests: usize,
    /// Window start time
    window_start: Instant,
    /// Window duration
    window_duration: Duration,
    /// Maximum requests per window
    max_requests: usize,
}

impl TenantRateLimiter {
    /// Create a new tenant rate limiter
    pub fn new() -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Configure rate limit for a tenant
    pub fn configure(&self, tenant_id: TenantId, max_requests: usize, window_duration: Duration) {
        let mut limits = self.limits.write().unwrap();
        limits.insert(
            tenant_id,
            RateLimit {
                requests: 0,
                window_start: Instant::now(),
                window_duration,
                max_requests,
            },
        );
    }

    /// Check if request is allowed
    pub fn check_rate_limit(&self, tenant_id: &TenantId) -> Result<bool, CoreError> {
        let mut limits = self.limits.write().unwrap();
        if let Some(limit) = limits.get_mut(tenant_id) {
            let now = Instant::now();

            // Reset window if expired
            if now.duration_since(limit.window_start) >= limit.window_duration {
                limit.requests = 0;
                limit.window_start = now;
            }

            // Check if under limit
            if limit.requests < limit.max_requests {
                limit.requests += 1;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(CoreError::NotFound("Rate limit not configured".to_string()))
        }
    }

    /// Get remaining requests in current window
    pub fn remaining_requests(&self, tenant_id: &TenantId) -> Option<usize> {
        let limits = self.limits.read().unwrap();
        limits
            .get(tenant_id)
            .map(|limit| limit.max_requests.saturating_sub(limit.requests))
    }
}

impl Default for TenantRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_tenant_registration() {
        let manager = TenantManager::new();

        let quotas = ResourceQuotas {
            max_users: 100,
            max_tokens: 50,
            max_daily_volume: dec!(1000000),
            max_api_calls_per_minute: 1000,
            max_storage_bytes: 1024 * 1024 * 100, // 100MB
            max_concurrent_connections: 50,
        };

        let tenant_id = manager
            .register_tenant("Test Tenant".to_string(), quotas)
            .unwrap();

        let tenant = manager.get_tenant(&tenant_id).unwrap();
        assert_eq!(tenant.name, "Test Tenant");
        assert_eq!(tenant.status, TenantStatus::Active);
    }

    #[test]
    fn test_quota_checking() {
        let manager = TenantManager::new();

        let quotas = ResourceQuotas {
            max_users: 10,
            max_tokens: 5,
            max_daily_volume: dec!(1000),
            max_api_calls_per_minute: 100,
            max_storage_bytes: 1024,
            max_concurrent_connections: 5,
        };

        let tenant_id = manager.register_tenant("Test".to_string(), quotas).unwrap();

        // Should allow within quota
        assert!(
            manager
                .check_quota(&tenant_id, ResourceType::Users, 5)
                .unwrap()
        );

        // Increment usage
        manager
            .increment_usage(&tenant_id, ResourceType::Users, 8)
            .unwrap();

        // Should not allow exceeding quota
        assert!(
            !manager
                .check_quota(&tenant_id, ResourceType::Users, 5)
                .unwrap()
        );
    }

    #[test]
    fn test_tenant_isolation() {
        let tenant1 = Uuid::new_v4();
        let tenant2 = Uuid::new_v4();

        let isolation = TenantIsolation::new(tenant1);

        // Should allow same tenant
        assert!(isolation.verify(&tenant1).is_ok());

        // Should deny different tenant
        assert!(isolation.verify(&tenant2).is_err());
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = TenantRateLimiter::new();
        let tenant_id = Uuid::new_v4();

        limiter.configure(tenant_id, 5, Duration::from_secs(1));

        // First 5 requests should succeed
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(&tenant_id).unwrap());
        }

        // 6th request should fail
        assert!(!limiter.check_rate_limit(&tenant_id).unwrap());

        // Wait for window to reset
        std::thread::sleep(Duration::from_millis(1100));

        // Should allow again
        assert!(limiter.check_rate_limit(&tenant_id).unwrap());
    }

    #[test]
    fn test_tenant_status_update() {
        let manager = TenantManager::new();

        let quotas = ResourceQuotas {
            max_users: 10,
            max_tokens: 5,
            max_daily_volume: dec!(1000),
            max_api_calls_per_minute: 100,
            max_storage_bytes: 1024,
            max_concurrent_connections: 5,
        };

        let tenant_id = manager.register_tenant("Test".to_string(), quotas).unwrap();

        // Suspend tenant
        manager
            .update_status(&tenant_id, TenantStatus::Suspended)
            .unwrap();

        let tenant = manager.get_tenant(&tenant_id).unwrap();
        assert_eq!(tenant.status, TenantStatus::Suspended);

        // Quota check should fail for suspended tenant
        assert!(
            !manager
                .check_quota(&tenant_id, ResourceType::Users, 1)
                .unwrap()
        );
    }

    #[test]
    fn test_resource_utilization() {
        let manager = TenantManager::new();

        let quotas = ResourceQuotas {
            max_users: 100,
            max_tokens: 50,
            max_daily_volume: dec!(1000),
            max_api_calls_per_minute: 1000,
            max_storage_bytes: 1000,
            max_concurrent_connections: 10,
        };

        let tenant_id = manager.register_tenant("Test".to_string(), quotas).unwrap();

        manager
            .increment_usage(&tenant_id, ResourceType::Users, 50)
            .unwrap();

        let utilization = manager.get_utilization(&tenant_id).unwrap();
        assert_eq!(utilization.users_percentage, 50.0);
    }
}
