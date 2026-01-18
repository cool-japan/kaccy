use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::Result;

/// Enhanced audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Audit action types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
    PasswordChange,
    PermissionChange,
    DataExport,
    DataImport,
    Custom(String),
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::Create => write!(f, "CREATE"),
            AuditAction::Read => write!(f, "READ"),
            AuditAction::Update => write!(f, "UPDATE"),
            AuditAction::Delete => write!(f, "DELETE"),
            AuditAction::Login => write!(f, "LOGIN"),
            AuditAction::Logout => write!(f, "LOGOUT"),
            AuditAction::PasswordChange => write!(f, "PASSWORD_CHANGE"),
            AuditAction::PermissionChange => write!(f, "PERMISSION_CHANGE"),
            AuditAction::DataExport => write!(f, "DATA_EXPORT"),
            AuditAction::DataImport => write!(f, "DATA_IMPORT"),
            AuditAction::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Audit context for tracking user actions
#[derive(Debug, Clone)]
pub struct AuditContext {
    pub user_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
}

impl AuditContext {
    pub fn new(user_id: Option<Uuid>) -> Self {
        Self {
            user_id,
            ip_address: None,
            user_agent: None,
            session_id: None,
        }
    }

    pub fn with_ip(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    pub fn with_user_agent(mut self, ua: String) -> Self {
        self.user_agent = Some(ua);
        self
    }

    pub fn with_session(mut self, session: String) -> Self {
        self.session_id = Some(session);
        self
    }
}

/// Enhanced audit logger
#[derive(Debug, Clone)]
pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Log an audit event
    #[allow(clippy::too_many_arguments)]
    pub async fn log(
        &self,
        context: &AuditContext,
        action: AuditAction,
        resource_type: String,
        resource_id: Option<String>,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, user_id, action, resource_type, resource_id,
                old_value, new_value, ip_address, user_agent,
                session_id, metadata, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW())
            "#,
        )
        .bind(id)
        .bind(context.user_id)
        .bind(action.to_string())
        .bind(resource_type)
        .bind(resource_id)
        .bind(old_value)
        .bind(new_value)
        .bind(context.ip_address.clone())
        .bind(context.user_agent.clone())
        .bind(context.session_id.clone())
        .bind(metadata)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    /// Log a user action (simplified method)
    pub async fn log_user_action(
        &self,
        user_id: Uuid,
        action: AuditAction,
        resource_type: String,
        resource_id: Option<String>,
    ) -> Result<Uuid> {
        let context = AuditContext::new(Some(user_id));
        self.log(
            &context,
            action,
            resource_type,
            resource_id,
            None,
            None,
            None,
        )
        .await
    }

    /// Log data access (for GDPR compliance)
    pub async fn log_data_access(
        &self,
        user_id: Uuid,
        accessed_user_id: Uuid,
        resource_type: String,
        purpose: String,
    ) -> Result<Uuid> {
        let context = AuditContext::new(Some(user_id));
        let metadata = serde_json::json!({
            "accessed_user_id": accessed_user_id,
            "purpose": purpose
        });

        self.log(
            &context,
            AuditAction::Read,
            resource_type,
            Some(accessed_user_id.to_string()),
            None,
            None,
            Some(metadata),
        )
        .await
    }

    /// Get audit logs for a user
    pub async fn get_user_audit_logs(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLogEntry>> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<Uuid>,
                String,
                String,
                Option<String>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<serde_json::Value>,
                chrono::NaiveDateTime,
            ),
        >(
            r#"
            SELECT
                id, user_id, action, resource_type, resource_id,
                old_value, new_value, ip_address, user_agent,
                session_id, metadata, created_at
            FROM audit_logs
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AuditLogEntry {
                id: row.0,
                timestamp: row.11.and_utc(),
                user_id: row.1,
                action: serde_json::from_str(&format!(r#""{}""#, row.2))
                    .unwrap_or(AuditAction::Custom(row.2)),
                resource_type: row.3,
                resource_id: row.4,
                old_value: row.5,
                new_value: row.6,
                ip_address: row.7,
                user_agent: row.8,
                session_id: row.9,
                metadata: row.10,
            })
            .collect())
    }

    /// Get audit logs for a resource
    pub async fn get_resource_audit_logs(
        &self,
        resource_type: String,
        resource_id: String,
        limit: i64,
    ) -> Result<Vec<AuditLogEntry>> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<Uuid>,
                String,
                String,
                Option<String>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<serde_json::Value>,
                chrono::NaiveDateTime,
            ),
        >(
            r#"
            SELECT
                id, user_id, action, resource_type, resource_id,
                old_value, new_value, ip_address, user_agent,
                session_id, metadata, created_at
            FROM audit_logs
            WHERE resource_type = $1 AND resource_id = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AuditLogEntry {
                id: row.0,
                timestamp: row.11.and_utc(),
                user_id: row.1,
                action: serde_json::from_str(&format!(r#""{}""#, row.2))
                    .unwrap_or(AuditAction::Custom(row.2)),
                resource_type: row.3,
                resource_id: row.4,
                old_value: row.5,
                new_value: row.6,
                ip_address: row.7,
                user_agent: row.8,
                session_id: row.9,
                metadata: row.10,
            })
            .collect())
    }

    /// Generate compliance report for a date range
    pub async fn generate_compliance_report(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<ComplianceReport> {
        let total_actions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE created_at BETWEEN $1 AND $2",
        )
        .bind(start_date.naive_utc())
        .bind(end_date.naive_utc())
        .fetch_one(&self.pool)
        .await?;

        let unique_users: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT user_id) FROM audit_logs WHERE created_at BETWEEN $1 AND $2",
        )
        .bind(start_date.naive_utc())
        .bind(end_date.naive_utc())
        .fetch_one(&self.pool)
        .await?;

        let data_access_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE action = 'READ' AND created_at BETWEEN $1 AND $2"
        )
        .bind(start_date.naive_utc())
        .bind(end_date.naive_utc())
        .fetch_one(&self.pool)
        .await?;

        let data_export_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE action = 'DATA_EXPORT' AND created_at BETWEEN $1 AND $2"
        )
        .bind(start_date.naive_utc())
        .bind(end_date.naive_utc())
        .fetch_one(&self.pool)
        .await?;

        Ok(ComplianceReport {
            start_date,
            end_date,
            total_actions,
            unique_users,
            data_access_count,
            data_export_count,
        })
    }

    /// Delete audit logs older than retention period (for GDPR compliance)
    pub async fn cleanup_old_logs(&self, retention_days: i32) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM audit_logs WHERE created_at < NOW() - INTERVAL '1 day' * $1")
                .bind(retention_days)
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected())
    }
}

/// GDPR/CCPA compliance utilities
#[derive(Debug, Clone)]
pub struct ComplianceManager {
    logger: AuditLogger,
}

impl ComplianceManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            logger: AuditLogger::new(pool),
        }
    }

    /// Export all data for a user (GDPR right to data portability)
    pub async fn export_user_data(&self, user_id: Uuid) -> Result<serde_json::Value> {
        let logs = self.logger.get_user_audit_logs(user_id, 10000, 0).await?;

        let export = serde_json::json!({
            "user_id": user_id,
            "export_date": Utc::now(),
            "audit_logs": logs,
        });

        // Log the export
        self.logger
            .log_user_action(
                user_id,
                AuditAction::DataExport,
                "user_data".to_string(),
                Some(user_id.to_string()),
            )
            .await?;

        Ok(export)
    }

    /// Request user data deletion (GDPR right to be forgotten)
    pub async fn request_data_deletion(&self, user_id: Uuid, reason: String) -> Result<Uuid> {
        let metadata = serde_json::json!({
            "deletion_reason": reason,
            "deletion_requested_at": Utc::now(),
        });

        self.logger
            .log(
                &AuditContext::new(Some(user_id)),
                AuditAction::Custom("DATA_DELETION_REQUEST".to_string()),
                "user".to_string(),
                Some(user_id.to_string()),
                None,
                None,
                Some(metadata),
            )
            .await
    }

    /// Generate compliance report
    pub async fn generate_report(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<ComplianceReport> {
        self.logger
            .generate_compliance_report(start_date, end_date)
            .await
    }
}

/// Compliance report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub total_actions: i64,
    pub unique_users: i64,
    pub data_access_count: i64,
    pub data_export_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_display() {
        assert_eq!(AuditAction::Create.to_string(), "CREATE");
        assert_eq!(AuditAction::Read.to_string(), "READ");
        assert_eq!(AuditAction::Update.to_string(), "UPDATE");
        assert_eq!(AuditAction::Delete.to_string(), "DELETE");
        assert_eq!(AuditAction::Custom("TEST".to_string()).to_string(), "TEST");
    }

    #[test]
    fn test_audit_context_builder() {
        let user_id = Uuid::new_v4();
        let context = AuditContext::new(Some(user_id))
            .with_ip("192.168.1.1".to_string())
            .with_user_agent("Mozilla/5.0".to_string())
            .with_session("session123".to_string());

        assert_eq!(context.user_id, Some(user_id));
        assert_eq!(context.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(context.user_agent, Some("Mozilla/5.0".to_string()));
        assert_eq!(context.session_id, Some("session123".to_string()));
    }

    #[test]
    fn test_audit_action_serialization() {
        let action = AuditAction::Create;
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, r#""CREATE""#);

        let action = AuditAction::Custom("CUSTOM_ACTION".to_string());
        let serialized = serde_json::to_string(&action).unwrap();
        assert!(serialized.contains("CUSTOM_ACTION"));
    }

    #[test]
    fn test_compliance_report_structure() {
        let report = ComplianceReport {
            start_date: Utc::now(),
            end_date: Utc::now(),
            total_actions: 100,
            unique_users: 10,
            data_access_count: 50,
            data_export_count: 5,
        };

        assert_eq!(report.total_actions, 100);
        assert_eq!(report.unique_users, 10);
        assert_eq!(report.data_access_count, 50);
        assert_eq!(report.data_export_count, 5);
    }
}
