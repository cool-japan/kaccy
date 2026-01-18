//! Audit log system for compliance and debugging

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// Audit log entry for tracking all important system actions
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub log_id: Uuid,
    /// User who performed the action (None for system actions)
    pub user_id: Option<Uuid>,
    /// Type of action performed
    pub action: AuditAction,
    /// Resource type affected (e.g., "user", "token", "order")
    pub resource_type: String,
    /// ID of the affected resource
    pub resource_id: Option<Uuid>,
    /// Additional details in JSON format
    pub details: Option<JsonValue>,
    /// Result of the action
    pub result: AuditResult,
    /// Error message if action failed
    pub error_message: Option<String>,
    /// IP address of the request
    pub ip_address: Option<String>,
    /// User agent of the request
    pub user_agent: Option<String>,
    /// API endpoint or method called
    pub endpoint: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// Severity level
    pub severity: AuditSeverity,
    pub created_at: DateTime<Utc>,
}

/// Type of audit action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum AuditAction {
    // User actions
    UserRegister,
    UserLogin,
    UserLogout,
    UserUpdate,
    UserDelete,
    PasswordChange,
    PasswordReset,
    EmailVerify,
    TwoFactorEnable,
    TwoFactorDisable,

    // KYC actions
    KycSubmit,
    KycApprove,
    KycReject,
    KycUpdate,

    // Token actions
    TokenCreate,
    TokenUpdate,
    TokenPause,
    TokenClose,
    TokenMetadataUpdate,

    // Order/Trade actions
    OrderCreate,
    OrderCancel,
    OrderFill,
    TradeExecute,

    // Payment actions
    PaymentCreate,
    PaymentDetected,
    PaymentConfirmed,
    PaymentExpired,
    PaymentRefund,

    // Balance actions
    BalanceDeposit,
    BalanceWithdraw,
    BalanceTransfer,
    BalanceAdjustment,

    // Reputation actions
    CommitmentCreate,
    CommitmentSubmitEvidence,
    CommitmentVerify,
    ReputationAdjust,

    // Admin actions
    AdminAccessGrant,
    AdminAccessRevoke,
    #[default]
    AdminActionPerformed,
    SystemConfigUpdate,
    EmergencyStop,
    EmergencyResume,

    // Security events
    LoginFailed,
    AccessDenied,
    SuspiciousActivity,
    RateLimitExceeded,
    SecurityAlert,

    // Compliance actions
    AmlScreening,
    ComplianceReport,
    DataExport,
    DataDelete,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Result of the audited action
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum AuditResult {
    /// Action completed successfully
    #[default]
    Success,
    /// Action failed
    Failure,
    /// Action partially completed
    Partial,
}

impl fmt::Display for AuditResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditResult::Success => write!(f, "success"),
            AuditResult::Failure => write!(f, "failure"),
            AuditResult::Partial => write!(f, "partial"),
        }
    }
}

/// Severity level of audit event
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, PartialOrd, Ord,
)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum AuditSeverity {
    /// Informational event
    #[default]
    Info,
    /// Warning event
    Warning,
    /// Error event
    Error,
    /// Critical security or compliance event
    Critical,
}

impl fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditSeverity::Info => write!(f, "info"),
            AuditSeverity::Warning => write!(f, "warning"),
            AuditSeverity::Error => write!(f, "error"),
            AuditSeverity::Critical => write!(f, "critical"),
        }
    }
}

impl fmt::Display for AuditLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AuditLog({}, action={}, result={}, severity={})",
            self.log_id, self.action, self.result, self.severity
        )
    }
}

impl AuditLog {
    /// Create a new audit log entry builder
    pub fn builder() -> AuditLogBuilder {
        AuditLogBuilder::new()
    }

    /// Check if this is a security-related event
    pub fn is_security_event(&self) -> bool {
        matches!(
            self.action,
            AuditAction::LoginFailed
                | AuditAction::AccessDenied
                | AuditAction::SuspiciousActivity
                | AuditAction::SecurityAlert
                | AuditAction::TwoFactorEnable
                | AuditAction::TwoFactorDisable
                | AuditAction::EmergencyStop
                | AuditAction::EmergencyResume
        )
    }

    /// Check if this is a compliance-related event
    pub fn is_compliance_event(&self) -> bool {
        matches!(
            self.action,
            AuditAction::KycSubmit
                | AuditAction::KycApprove
                | AuditAction::KycReject
                | AuditAction::AmlScreening
                | AuditAction::ComplianceReport
                | AuditAction::DataExport
                | AuditAction::DataDelete
        )
    }

    /// Check if this requires immediate attention
    pub fn requires_immediate_attention(&self) -> bool {
        self.severity >= AuditSeverity::Error
            || (self.is_security_event() && self.result == AuditResult::Failure)
    }
}

/// Builder for creating audit log entries
pub struct AuditLogBuilder {
    user_id: Option<Uuid>,
    action: Option<AuditAction>,
    resource_type: Option<String>,
    resource_id: Option<Uuid>,
    details: Option<JsonValue>,
    result: AuditResult,
    error_message: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    endpoint: Option<String>,
    request_id: Option<String>,
    severity: AuditSeverity,
}

impl AuditLogBuilder {
    pub fn new() -> Self {
        Self {
            user_id: None,
            action: None,
            resource_type: None,
            resource_id: None,
            details: None,
            result: AuditResult::Success,
            error_message: None,
            ip_address: None,
            user_agent: None,
            endpoint: None,
            request_id: None,
            severity: AuditSeverity::Info,
        }
    }

    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn action(mut self, action: AuditAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn resource(mut self, resource_type: String, resource_id: Option<Uuid>) -> Self {
        self.resource_type = Some(resource_type);
        self.resource_id = resource_id;
        self
    }

    pub fn details(mut self, details: JsonValue) -> Self {
        self.details = Some(details);
        self
    }

    pub fn result(mut self, result: AuditResult) -> Self {
        self.result = result;
        self
    }

    pub fn error(mut self, error_message: String) -> Self {
        self.error_message = Some(error_message);
        self.result = AuditResult::Failure;
        self
    }

    pub fn ip_address(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    pub fn user_agent(mut self, ua: String) -> Self {
        self.user_agent = Some(ua);
        self
    }

    pub fn endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn build(self) -> Result<AuditLog, String> {
        let action = self
            .action
            .ok_or_else(|| "Action is required".to_string())?;

        Ok(AuditLog {
            log_id: Uuid::new_v4(),
            user_id: self.user_id,
            action,
            resource_type: self.resource_type.unwrap_or_else(|| "unknown".to_string()),
            resource_id: self.resource_id,
            details: self.details,
            result: self.result,
            error_message: self.error_message,
            ip_address: self.ip_address,
            user_agent: self.user_agent,
            endpoint: self.endpoint,
            request_id: self.request_id,
            severity: self.severity,
            created_at: Utc::now(),
        })
    }
}

impl Default for AuditLogBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Query parameters for searching audit logs
#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub user_id: Option<Uuid>,
    pub action: Option<AuditAction>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub result: Option<AuditResult>,
    pub severity: Option<AuditSeverity>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// Audit log statistics
#[derive(Debug, Serialize)]
pub struct AuditStats {
    pub total_events: i64,
    pub events_by_severity: Vec<SeverityCount>,
    pub events_by_action: Vec<ActionCount>,
    pub success_rate_percent: f64,
    pub security_events_count: i64,
    pub compliance_events_count: i64,
    pub failed_login_attempts: i64,
    pub critical_events_last_24h: i64,
}

#[derive(Debug, Serialize)]
pub struct SeverityCount {
    pub severity: AuditSeverity,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ActionCount {
    pub action: AuditAction,
    pub count: i64,
}

/// Audit retention policy
pub struct AuditRetentionPolicy;

impl AuditRetentionPolicy {
    /// Retention period for normal audit logs (days)
    pub const NORMAL_RETENTION_DAYS: i64 = 365; // 1 year

    /// Retention period for security events (days)
    pub const SECURITY_RETENTION_DAYS: i64 = 2555; // 7 years

    /// Retention period for compliance events (days)
    pub const COMPLIANCE_RETENTION_DAYS: i64 = 2555; // 7 years

    /// Get retention period for an audit log
    pub fn get_retention_days(log: &AuditLog) -> i64 {
        if log.is_security_event() {
            Self::SECURITY_RETENTION_DAYS
        } else if log.is_compliance_event() {
            Self::COMPLIANCE_RETENTION_DAYS
        } else {
            Self::NORMAL_RETENTION_DAYS
        }
    }

    /// Check if log should be deleted based on retention policy
    pub fn should_delete(log: &AuditLog) -> bool {
        let retention_days = Self::get_retention_days(log);
        let retention_cutoff = Utc::now() - chrono::Duration::days(retention_days);
        log.created_at < retention_cutoff
    }
}

/// Macro for easy audit logging
#[macro_export]
macro_rules! audit_log {
    ($action:expr, $user_id:expr, $resource_type:expr, $resource_id:expr) => {
        $crate::models::audit::AuditLog::builder()
            .action($action)
            .user_id($user_id)
            .resource($resource_type.to_string(), Some($resource_id))
            .build()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_builder() {
        let log = AuditLog::builder()
            .user_id(Uuid::new_v4())
            .action(AuditAction::UserLogin)
            .resource("user".to_string(), None)
            .severity(AuditSeverity::Info)
            .build()
            .unwrap();

        assert_eq!(log.action, AuditAction::UserLogin);
        assert_eq!(log.result, AuditResult::Success);
        assert_eq!(log.severity, AuditSeverity::Info);
    }

    #[test]
    fn test_security_event_detection() {
        let log = AuditLog::builder()
            .action(AuditAction::LoginFailed)
            .severity(AuditSeverity::Warning)
            .build()
            .unwrap();

        assert!(log.is_security_event());
        assert!(!log.is_compliance_event());
    }

    #[test]
    fn test_compliance_event_detection() {
        let log = AuditLog::builder()
            .action(AuditAction::KycApprove)
            .severity(AuditSeverity::Info)
            .build()
            .unwrap();

        assert!(!log.is_security_event());
        assert!(log.is_compliance_event());
    }

    #[test]
    fn test_requires_immediate_attention() {
        let critical_log = AuditLog::builder()
            .action(AuditAction::SecurityAlert)
            .severity(AuditSeverity::Critical)
            .build()
            .unwrap();

        assert!(critical_log.requires_immediate_attention());

        let normal_log = AuditLog::builder()
            .action(AuditAction::UserLogin)
            .severity(AuditSeverity::Info)
            .build()
            .unwrap();

        assert!(!normal_log.requires_immediate_attention());
    }

    #[test]
    fn test_retention_policy() {
        let security_log = AuditLog::builder()
            .action(AuditAction::LoginFailed)
            .build()
            .unwrap();

        assert_eq!(
            AuditRetentionPolicy::get_retention_days(&security_log),
            AuditRetentionPolicy::SECURITY_RETENTION_DAYS
        );

        let compliance_log = AuditLog::builder()
            .action(AuditAction::KycApprove)
            .build()
            .unwrap();

        assert_eq!(
            AuditRetentionPolicy::get_retention_days(&compliance_log),
            AuditRetentionPolicy::COMPLIANCE_RETENTION_DAYS
        );

        let normal_log = AuditLog::builder()
            .action(AuditAction::TokenCreate)
            .build()
            .unwrap();

        assert_eq!(
            AuditRetentionPolicy::get_retention_days(&normal_log),
            AuditRetentionPolicy::NORMAL_RETENTION_DAYS
        );
    }

    #[test]
    fn test_audit_log_error() {
        let log = AuditLog::builder()
            .action(AuditAction::OrderCreate)
            .error("Insufficient balance".to_string())
            .severity(AuditSeverity::Error)
            .build()
            .unwrap();

        assert_eq!(log.result, AuditResult::Failure);
        assert_eq!(log.error_message, Some("Insufficient balance".to_string()));
    }
}
