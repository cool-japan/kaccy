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
    pub id: Uuid,
    pub table_name: String,
    pub record_id: Uuid,
    pub action: String,
    pub user_id: Option<Uuid>,
    pub old_values: Option<JsonValue>,
    pub new_values: Option<JsonValue>,
    pub changed_fields: Option<Vec<String>>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Admin action entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AdminAction {
    pub id: Uuid,
    pub admin_user_id: Uuid,
    pub action_type: String,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub description: String,
    pub metadata: Option<JsonValue>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Audit summary statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditSummary {
    pub table_name: String,
    pub action: String,
    pub audit_date: NaiveDate,
    pub event_count: i64,
    pub unique_users: i64,
    pub unique_records: i64,
}

/// User activity report
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserAuditActivity {
    pub user_id: Uuid,
    pub total_actions: i64,
    pub insert_count: i64,
    pub update_count: i64,
    pub delete_count: i64,
    pub tables_modified: i64,
    pub last_activity: DateTime<Utc>,
}

/// Table change report
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TableChangeReport {
    pub table_name: String,
    pub total_changes: i64,
    pub insert_count: i64,
    pub update_count: i64,
    pub delete_count: i64,
    pub unique_users: i64,
    pub first_change: DateTime<Utc>,
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
