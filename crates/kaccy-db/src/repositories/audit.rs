//! Audit log repository for tracking changes and compliance

use crate::error::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

/// Audit log entry representing a change to a database record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    /// Unique identifier for this log entry.
    pub id: Uuid,
    /// Name of the table that was modified.
    pub table_name: String,
    /// Primary key of the record that was changed.
    pub record_id: Uuid,
    /// Action performed (e.g., "INSERT", "UPDATE", "DELETE").
    pub action: String,
    /// User who performed the action, if known.
    pub user_id: Option<Uuid>,
    /// Previous values of changed fields.
    pub old_values: Option<JsonValue>,
    /// New values of changed fields.
    pub new_values: Option<JsonValue>,
    /// List of field names that were changed.
    pub changed_fields: Option<Vec<String>>,
    /// Client IP address.
    pub ip_address: Option<String>,
    /// Client User-Agent.
    pub user_agent: Option<String>,
    /// Timestamp when the change occurred.
    pub created_at: DateTime<Utc>,
}

/// Admin action entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AdminAction {
    /// Unique identifier for this action record.
    pub id: Uuid,
    /// Admin user who performed the action.
    pub admin_user_id: Uuid,
    /// Type of administrative action.
    pub action_type: String,
    /// Type of the target entity, if applicable.
    pub target_type: Option<String>,
    /// Identifier of the target entity.
    pub target_id: Option<Uuid>,
    /// Human-readable description of the action.
    pub description: String,
    /// Additional metadata as JSON.
    pub metadata: Option<JsonValue>,
    /// IP address from which the action was performed.
    pub ip_address: Option<String>,
    /// Timestamp when the action was recorded.
    pub created_at: DateTime<Utc>,
}

/// Audit summary statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditSummary {
    /// Name of the table.
    pub table_name: String,
    /// Action type (e.g., "INSERT").
    pub action: String,
    /// Date of the aggregation bucket.
    pub audit_date: NaiveDate,
    /// Number of events.
    pub event_count: i64,
    /// Number of distinct users.
    pub unique_users: i64,
    /// Number of distinct records affected.
    pub unique_records: i64,
}

/// User activity report
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserAuditActivity {
    /// User identifier.
    pub user_id: Uuid,
    /// Total number of recorded actions.
    pub total_actions: i64,
    /// Number of INSERT operations.
    pub insert_count: i64,
    /// Number of UPDATE operations.
    pub update_count: i64,
    /// Number of DELETE operations.
    pub delete_count: i64,
    /// Number of distinct tables modified.
    pub tables_modified: i64,
    /// Timestamp of the most recent action.
    pub last_activity: DateTime<Utc>,
}

/// Table change report
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TableChangeReport {
    /// Name of the table.
    pub table_name: String,
    /// Total number of changes.
    pub total_changes: i64,
    /// Number of INSERT operations.
    pub insert_count: i64,
    /// Number of UPDATE operations.
    pub update_count: i64,
    /// Number of DELETE operations.
    pub delete_count: i64,
    /// Number of distinct users who made changes.
    pub unique_users: i64,
    /// Timestamp of the earliest recorded change.
    pub first_change: DateTime<Utc>,
    /// Timestamp of the most recent recorded change.
    pub last_change: DateTime<Utc>,
}

/// Audit repository for querying audit logs and admin actions
pub struct AuditRepository {
    pool: PgPool,
}

impl AuditRepository {
    /// Create a new audit repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get audit logs for a specific record
    pub async fn get_record_history(
        &self,
        table_name: &str,
        record_id: Uuid,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE table_name = $1 AND record_id = $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(table_name)
        .bind(record_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Get audit logs for a specific user
    pub async fn get_user_audit_logs(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
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

        Ok(logs)
    }

    /// Get recent audit logs with pagination
    pub async fn get_recent_logs(&self, limit: i64, offset: i64) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Get audit logs by table name
    pub async fn get_table_logs(
        &self,
        table_name: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE table_name = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(table_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Get audit logs by action type
    pub async fn get_logs_by_action(
        &self,
        action: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE action = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(action)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Get audit logs within a time range
    pub async fn get_logs_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE created_at >= $1 AND created_at <= $2
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Create an admin action record
    #[allow(clippy::too_many_arguments)]
    pub async fn create_admin_action(
        &self,
        admin_user_id: Uuid,
        action_type: &str,
        target_type: Option<&str>,
        target_id: Option<Uuid>,
        description: &str,
        metadata: Option<JsonValue>,
        ip_address: Option<&str>,
    ) -> Result<AdminAction> {
        let action = sqlx::query_as::<_, AdminAction>(
            r#"
            INSERT INTO admin_actions (
                admin_user_id, action_type, target_type, target_id,
                description, metadata, ip_address
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, admin_user_id, action_type, target_type, target_id,
                      description, metadata, ip_address, created_at
            "#,
        )
        .bind(admin_user_id)
        .bind(action_type)
        .bind(target_type)
        .bind(target_id)
        .bind(description)
        .bind(metadata)
        .bind(ip_address)
        .fetch_one(&self.pool)
        .await?;

        Ok(action)
    }

    /// Get admin actions for a specific admin user
    pub async fn get_admin_actions(
        &self,
        admin_user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AdminAction>> {
        let actions = sqlx::query_as::<_, AdminAction>(
            r#"
            SELECT id, admin_user_id, action_type, target_type, target_id,
                   description, metadata, ip_address, created_at
            FROM admin_actions
            WHERE admin_user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(admin_user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(actions)
    }

    /// Get all admin actions with pagination
    pub async fn get_all_admin_actions(&self, limit: i64, offset: i64) -> Result<Vec<AdminAction>> {
        let actions = sqlx::query_as::<_, AdminAction>(
            r#"
            SELECT id, admin_user_id, action_type, target_type, target_id,
                   description, metadata, ip_address, created_at
            FROM admin_actions
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(actions)
    }

    /// Get audit summary from materialized view
    pub async fn get_audit_summary(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<AuditSummary>> {
        let summary = sqlx::query_as::<_, AuditSummary>(
            r#"
            SELECT table_name, action, audit_date,
                   event_count, unique_users, unique_records
            FROM audit_summary
            WHERE audit_date >= $1 AND audit_date <= $2
            ORDER BY audit_date DESC, table_name, action
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Get user activity report
    pub async fn get_user_activity_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UserAuditActivity>> {
        let report = sqlx::query_as::<_, UserAuditActivity>(
            r#"
            SELECT
                user_id,
                COUNT(*) as total_actions,
                COUNT(*) FILTER (WHERE action = 'INSERT') as insert_count,
                COUNT(*) FILTER (WHERE action = 'UPDATE') as update_count,
                COUNT(*) FILTER (WHERE action = 'DELETE') as delete_count,
                COUNT(DISTINCT table_name) as tables_modified,
                MAX(created_at) as last_activity
            FROM audit_logs
            WHERE user_id IS NOT NULL
              AND created_at >= $1 AND created_at <= $2
            GROUP BY user_id
            ORDER BY total_actions DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(report)
    }

    /// Get table change report
    pub async fn get_table_change_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TableChangeReport>> {
        let report = sqlx::query_as::<_, TableChangeReport>(
            r#"
            SELECT
                table_name,
                COUNT(*) as total_changes,
                COUNT(*) FILTER (WHERE action = 'INSERT') as insert_count,
                COUNT(*) FILTER (WHERE action = 'UPDATE') as update_count,
                COUNT(*) FILTER (WHERE action = 'DELETE') as delete_count,
                COUNT(DISTINCT user_id) as unique_users,
                MIN(created_at) as first_change,
                MAX(created_at) as last_change
            FROM audit_logs
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY table_name
            ORDER BY total_changes DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(report)
    }

    /// Search audit logs by changed field
    pub async fn search_by_changed_field(
        &self,
        field_name: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE changed_fields @> ARRAY[$1]::TEXT[]
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(field_name)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Get compliance report for deleted records
    pub async fn get_deleted_records_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            r#"
            SELECT id, table_name, record_id, action, user_id,
                   old_values, new_values, changed_fields,
                   ip_address, user_agent, created_at
            FROM audit_logs
            WHERE action = 'DELETE'
              AND created_at >= $1 AND created_at <= $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(logs)
    }

    /// Count total audit logs
    pub async fn count_audit_logs(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs")
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0)
    }

    /// Count audit logs by table
    pub async fn count_logs_by_table(&self, table_name: &str) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE table_name = $1")
            .bind(table_name)
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0)
    }

    /// Count audit logs by user
    pub async fn count_logs_by_user(&self, user_id: Uuid) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(count.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use serde_json::json;
    use uuid::Uuid;

    // ── AuditLog construction & field verification ───────────────────────────

    #[test]
    fn audit_log_fields_are_set_correctly() {
        let id = Uuid::new_v4();
        let record_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let log = AuditLog {
            id,
            table_name: "orders".to_string(),
            record_id,
            action: "INSERT".to_string(),
            user_id: Some(user_id),
            old_values: None,
            new_values: Some(json!({"amount": 100})),
            changed_fields: Some(vec!["amount".to_string()]),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("kaccy-client/1.0".to_string()),
            created_at: now,
        };

        assert_eq!(log.id, id);
        assert_eq!(log.table_name, "orders");
        assert_eq!(log.record_id, record_id);
        assert_eq!(log.action, "INSERT");
        assert_eq!(log.user_id, Some(user_id));
        assert!(log.old_values.is_none());
        assert!(log.new_values.is_some());
    }

    #[test]
    fn audit_log_optional_fields_can_be_none() {
        let log = AuditLog {
            id: Uuid::new_v4(),
            table_name: "users".to_string(),
            record_id: Uuid::new_v4(),
            action: "DELETE".to_string(),
            user_id: None,
            old_values: None,
            new_values: None,
            changed_fields: None,
            ip_address: None,
            user_agent: None,
            created_at: Utc::now(),
        };

        assert!(log.user_id.is_none());
        assert!(log.old_values.is_none());
        assert!(log.changed_fields.is_none());
        assert!(log.ip_address.is_none());
        assert!(log.user_agent.is_none());
    }

    #[test]
    fn audit_log_serde_roundtrip() {
        let log = AuditLog {
            id: Uuid::new_v4(),
            table_name: "tokens".to_string(),
            record_id: Uuid::new_v4(),
            action: "UPDATE".to_string(),
            user_id: Some(Uuid::new_v4()),
            old_values: Some(json!({"price": "1.00"})),
            new_values: Some(json!({"price": "2.00"})),
            changed_fields: Some(vec!["price".to_string()]),
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: None,
            created_at: Utc::now(),
        };

        let json_str = serde_json::to_string(&log).expect("AuditLog must serialize to JSON");
        let restored: AuditLog =
            serde_json::from_str(&json_str).expect("AuditLog must deserialize from JSON");

        assert_eq!(log.id, restored.id);
        assert_eq!(log.table_name, restored.table_name);
        assert_eq!(log.action, restored.action);
        assert_eq!(log.old_values, restored.old_values);
        assert_eq!(log.new_values, restored.new_values);
        assert_eq!(log.changed_fields, restored.changed_fields);
    }

    #[test]
    fn audit_log_changed_fields_stored_as_vec() {
        let fields = vec![
            "email".to_string(),
            "username".to_string(),
            "bio".to_string(),
        ];
        let log = AuditLog {
            id: Uuid::new_v4(),
            table_name: "users".to_string(),
            record_id: Uuid::new_v4(),
            action: "UPDATE".to_string(),
            user_id: None,
            old_values: None,
            new_values: None,
            changed_fields: Some(fields.clone()),
            ip_address: None,
            user_agent: None,
            created_at: Utc::now(),
        };

        let stored = log
            .changed_fields
            .expect("changed_fields should be present");
        assert_eq!(stored.len(), 3);
        assert!(stored.contains(&"email".to_string()));
        assert!(stored.contains(&"username".to_string()));
    }

    // ── AdminAction construction & field verification ────────────────────────

    #[test]
    fn admin_action_fields_are_set_correctly() {
        let id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let now = Utc::now();

        let action = AdminAction {
            id,
            admin_user_id: admin_id,
            action_type: "SUSPEND_USER".to_string(),
            target_type: Some("user".to_string()),
            target_id: Some(target_id),
            description: "User violated ToS".to_string(),
            metadata: Some(json!({"reason": "spam", "severity": "high"})),
            ip_address: Some("192.168.0.1".to_string()),
            created_at: now,
        };

        assert_eq!(action.id, id);
        assert_eq!(action.admin_user_id, admin_id);
        assert_eq!(action.action_type, "SUSPEND_USER");
        assert_eq!(action.target_type, Some("user".to_string()));
        assert_eq!(action.target_id, Some(target_id));
        assert!(!action.description.is_empty());
    }

    #[test]
    fn admin_action_serde_roundtrip() {
        let action = AdminAction {
            id: Uuid::new_v4(),
            admin_user_id: Uuid::new_v4(),
            action_type: "DELETE_TOKEN".to_string(),
            target_type: Some("token".to_string()),
            target_id: Some(Uuid::new_v4()),
            description: "Token removed for policy violation".to_string(),
            metadata: Some(json!({"policy": "rule_42"})),
            ip_address: None,
            created_at: Utc::now(),
        };

        let json_str = serde_json::to_string(&action).expect("AdminAction must serialize to JSON");
        let restored: AdminAction =
            serde_json::from_str(&json_str).expect("AdminAction must deserialize from JSON");

        assert_eq!(action.id, restored.id);
        assert_eq!(action.action_type, restored.action_type);
        assert_eq!(action.description, restored.description);
        assert_eq!(action.metadata, restored.metadata);
    }

    #[test]
    fn admin_action_optional_target_can_be_none() {
        let action = AdminAction {
            id: Uuid::new_v4(),
            admin_user_id: Uuid::new_v4(),
            action_type: "SYSTEM_MAINTENANCE".to_string(),
            target_type: None,
            target_id: None,
            description: "Scheduled maintenance window".to_string(),
            metadata: None,
            ip_address: None,
            created_at: Utc::now(),
        };

        assert!(action.target_type.is_none());
        assert!(action.target_id.is_none());
        assert!(action.metadata.is_none());
    }

    // ── AuditSummary construction & field verification ───────────────────────

    #[test]
    fn audit_summary_fields_are_set_correctly() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 13).expect("date must be valid");

        let summary = AuditSummary {
            table_name: "orders".to_string(),
            action: "UPDATE".to_string(),
            audit_date: date,
            event_count: 500,
            unique_users: 42,
            unique_records: 300,
        };

        assert_eq!(summary.table_name, "orders");
        assert_eq!(summary.action, "UPDATE");
        assert_eq!(summary.audit_date, date);
        assert_eq!(summary.event_count, 500);
        assert_eq!(summary.unique_users, 42);
        assert_eq!(summary.unique_records, 300);
    }

    #[test]
    fn audit_summary_serde_roundtrip() {
        let summary = AuditSummary {
            table_name: "users".to_string(),
            action: "INSERT".to_string(),
            audit_date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date must be valid"),
            event_count: 1200,
            unique_users: 80,
            unique_records: 1200,
        };

        let json_str =
            serde_json::to_string(&summary).expect("AuditSummary must serialize to JSON");
        let restored: AuditSummary =
            serde_json::from_str(&json_str).expect("AuditSummary must deserialize from JSON");

        assert_eq!(summary.table_name, restored.table_name);
        assert_eq!(summary.event_count, restored.event_count);
        assert_eq!(summary.audit_date, restored.audit_date);
    }

    // ── UserAuditActivity construction & field verification ──────────────────

    #[test]
    fn user_audit_activity_counts_are_correct() {
        let user_id = Uuid::new_v4();
        let now = Utc::now();

        let activity = UserAuditActivity {
            user_id,
            total_actions: 150,
            insert_count: 30,
            update_count: 100,
            delete_count: 20,
            tables_modified: 5,
            last_activity: now,
        };

        assert_eq!(activity.user_id, user_id);
        assert_eq!(
            activity.insert_count + activity.update_count + activity.delete_count,
            150
        );
        assert_eq!(activity.tables_modified, 5);
    }

    #[test]
    fn user_audit_activity_serde_roundtrip() {
        let activity = UserAuditActivity {
            user_id: Uuid::new_v4(),
            total_actions: 42,
            insert_count: 10,
            update_count: 22,
            delete_count: 10,
            tables_modified: 3,
            last_activity: Utc::now(),
        };

        let json_str =
            serde_json::to_string(&activity).expect("UserAuditActivity must serialize to JSON");
        let restored: UserAuditActivity =
            serde_json::from_str(&json_str).expect("UserAuditActivity must deserialize from JSON");

        assert_eq!(activity.user_id, restored.user_id);
        assert_eq!(activity.total_actions, restored.total_actions);
        assert_eq!(activity.insert_count, restored.insert_count);
    }

    // ── TableChangeReport construction & field verification ──────────────────

    #[test]
    fn table_change_report_fields_are_set_correctly() {
        let start = Utc::now();
        let end = Utc::now();

        let report = TableChangeReport {
            table_name: "balances".to_string(),
            total_changes: 2000,
            insert_count: 500,
            update_count: 1400,
            delete_count: 100,
            unique_users: 250,
            first_change: start,
            last_change: end,
        };

        assert_eq!(report.table_name, "balances");
        assert_eq!(
            report.insert_count + report.update_count + report.delete_count,
            2000
        );
        assert_eq!(report.unique_users, 250);
    }

    #[test]
    fn table_change_report_serde_roundtrip() {
        let now = Utc::now();

        let report = TableChangeReport {
            table_name: "trades".to_string(),
            total_changes: 9999,
            insert_count: 9000,
            update_count: 999,
            delete_count: 0,
            unique_users: 100,
            first_change: now,
            last_change: now,
        };

        let json_str =
            serde_json::to_string(&report).expect("TableChangeReport must serialize to JSON");
        let restored: TableChangeReport =
            serde_json::from_str(&json_str).expect("TableChangeReport must deserialize from JSON");

        assert_eq!(report.table_name, restored.table_name);
        assert_eq!(report.total_changes, restored.total_changes);
        assert_eq!(report.delete_count, restored.delete_count);
    }
}
