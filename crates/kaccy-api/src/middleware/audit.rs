//! Audit logging for sensitive operations

use axum::extract::Request;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use super::request_id::RequestId;

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    // Authentication events
    UserRegistered,
    UserLogin,
    UserLogout,
    LoginFailed,
    PasswordChanged,
    TwoFactorEnabled,
    TwoFactorDisabled,

    // Authorization events
    RoleChanged,
    PermissionGranted,
    PermissionRevoked,
    AccessDenied,

    // KYC events
    KycSubmitted,
    KycApproved,
    KycRejected,

    // Token events
    TokenCreated,
    TokenUpdated,
    TokenDeleted,

    // Order events
    OrderCreated,
    OrderCancelled,
    OrderCompleted,

    // Admin events
    AdminAction,
    ConfigChanged,
    DataExported,

    // Security events
    ApiKeyCreated,
    ApiKeyRevoked,
    SuspiciousActivity,
    RateLimitExceeded,
}

/// Audit event with all metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: Uuid,

    /// Event type
    pub event_type: AuditEventType,

    /// User ID who performed the action (if authenticated)
    pub user_id: Option<Uuid>,

    /// Target user ID (if action affects another user)
    pub target_user_id: Option<Uuid>,

    /// Resource type (e.g., "token", "order", "user")
    pub resource_type: Option<String>,

    /// Resource ID being acted upon
    pub resource_id: Option<String>,

    /// Action performed
    pub action: String,

    /// Result of the action
    pub result: AuditResult,

    /// IP address of the requester
    pub ip_address: Option<IpAddr>,

    /// User agent string
    pub user_agent: Option<String>,

    /// Request ID for correlation
    pub request_id: Option<String>,

    /// Additional metadata (JSON)
    pub metadata: Option<serde_json::Value>,

    /// Timestamp when event occurred
    pub timestamp: DateTime<Utc>,
}

/// Result of an audited action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditResult {
    Success,
    Failure,
    Denied,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType, action: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            user_id: None,
            target_user_id: None,
            resource_type: None,
            resource_id: None,
            action,
            result: AuditResult::Success,
            ip_address: None,
            user_agent: None,
            request_id: None,
            metadata: None,
            timestamp: Utc::now(),
        }
    }

    /// Set the user who performed the action
    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set the target user (for actions affecting other users)
    pub fn with_target_user(mut self, target_user_id: Uuid) -> Self {
        self.target_user_id = Some(target_user_id);
        self
    }

    /// Set the resource being acted upon
    pub fn with_resource(mut self, resource_type: String, resource_id: String) -> Self {
        self.resource_type = Some(resource_type);
        self.resource_id = Some(resource_id);
        self
    }

    /// Set the result
    pub fn with_result(mut self, result: AuditResult) -> Self {
        self.result = result;
        self
    }

    /// Set IP address
    pub fn with_ip(mut self, ip: IpAddr) -> Self {
        self.ip_address = Some(ip);
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    /// Set request ID for correlation
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Extract request context from HTTP request
    pub fn from_request(req: &Request, event_type: AuditEventType, action: String) -> Self {
        let mut event = Self::new(event_type, action);

        // Extract request ID
        if let Some(req_id) = req.extensions().get::<RequestId>() {
            event = event.with_request_id(req_id.0.clone());
        }

        // Extract user agent
        if let Some(ua) = req.headers().get("user-agent") {
            if let Ok(ua_str) = ua.to_str() {
                event = event.with_user_agent(ua_str.to_string());
            }
        }

        // Note: IP address extraction would need additional middleware/extension
        // to handle proxies and x-forwarded-for headers properly

        event
    }

    /// Log this audit event
    pub fn log(&self) {
        let event_json = serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string());

        match self.result {
            AuditResult::Success => {
                tracing::info!(
                    event_type = ?self.event_type,
                    user_id = ?self.user_id,
                    resource = ?self.resource_type,
                    request_id = ?self.request_id,
                    audit_event = %event_json,
                    "Audit: {}",
                    self.action
                );
            }
            AuditResult::Failure => {
                tracing::warn!(
                    event_type = ?self.event_type,
                    user_id = ?self.user_id,
                    resource = ?self.resource_type,
                    request_id = ?self.request_id,
                    audit_event = %event_json,
                    "Audit (failed): {}",
                    self.action
                );
            }
            AuditResult::Denied => {
                tracing::warn!(
                    event_type = ?self.event_type,
                    user_id = ?self.user_id,
                    resource = ?self.resource_type,
                    request_id = ?self.request_id,
                    audit_event = %event_json,
                    "Audit (denied): {}",
                    self.action
                );
            }
        }
    }
}

/// Helper macro to create and log audit events
#[macro_export]
macro_rules! audit {
    ($event_type:expr, $action:expr) => {
        $crate::middleware::audit::AuditEvent::new($event_type, $action.to_string()).log()
    };
    ($event_type:expr, $action:expr, $($field:ident = $value:expr),+ $(,)?) => {
        {
            let event = $crate::middleware::audit::AuditEvent::new($event_type, $action.to_string());
            $(
                let event = event.$field($value);
            )+
            event.log()
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(AuditEventType::UserLogin, "User logged in".to_string())
            .with_user(Uuid::new_v4())
            .with_result(AuditResult::Success);

        assert_eq!(event.event_type, AuditEventType::UserLogin);
        assert_eq!(event.result, AuditResult::Success);
        assert!(event.user_id.is_some());
    }

    #[test]
    fn test_audit_event_with_resource() {
        let event = AuditEvent::new(AuditEventType::TokenCreated, "Token created".to_string())
            .with_resource("token".to_string(), "token-123".to_string());

        assert_eq!(event.resource_type, Some("token".to_string()));
        assert_eq!(event.resource_id, Some("token-123".to_string()));
    }

    #[test]
    fn test_audit_result_variants() {
        assert_eq!(AuditResult::Success, AuditResult::Success);
        assert_ne!(AuditResult::Success, AuditResult::Failure);
        assert_ne!(AuditResult::Failure, AuditResult::Denied);
    }
}
