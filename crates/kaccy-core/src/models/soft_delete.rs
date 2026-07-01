//! Soft delete support for models

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Trait for models that support soft deletion
#[async_trait::async_trait]
pub trait SoftDelete: Sized {
    /// Get the deleted_at timestamp
    fn deleted_at(&self) -> Option<DateTime<Utc>>;

    /// Set the deleted_at timestamp
    fn set_deleted_at(&mut self, timestamp: Option<DateTime<Utc>>);

    /// Check if the model is soft deleted
    fn is_deleted(&self) -> bool {
        self.deleted_at().is_some()
    }

    /// Soft delete the model
    async fn soft_delete(&mut self, pool: &PgPool) -> Result<()> {
        self.set_deleted_at(Some(Utc::now()));
        self.save_deleted_at(pool).await
    }

    /// Restore a soft deleted model
    async fn restore(&mut self, pool: &PgPool) -> Result<()> {
        self.set_deleted_at(None);
        self.save_deleted_at(pool).await
    }

    /// Save the deleted_at timestamp to the database
    async fn save_deleted_at(&self, pool: &PgPool) -> Result<()>;

    /// Force delete (permanently remove from database)
    async fn force_delete(&self, pool: &PgPool) -> Result<u64>;
}

/// Query scope for filtering soft deleted records
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoftDeleteScope {
    /// Only non-deleted records (default)
    #[default]
    Active,
    /// Only deleted records
    Deleted,
    /// All records (including deleted)
    All,
}

impl SoftDeleteScope {
    /// Get the SQL WHERE clause for this scope
    pub fn to_sql(&self) -> &'static str {
        match self {
            Self::Active => "deleted_at IS NULL",
            Self::Deleted => "deleted_at IS NOT NULL",
            Self::All => "TRUE",
        }
    }

    /// Get the SQL clause to append to WHERE
    pub fn append_to_where(&self, existing_where: &str) -> String {
        if existing_where.is_empty() {
            format!("WHERE {}", self.to_sql())
        } else {
            format!("{} AND {}", existing_where, self.to_sql())
        }
    }
}

/// Query builder with soft delete support
#[derive(Debug, Clone)]
pub struct SoftDeleteQuery {
    scope: SoftDeleteScope,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl SoftDeleteQuery {
    /// Create a new query with default scope (active only)
    pub fn new() -> Self {
        Self {
            scope: SoftDeleteScope::Active,
            limit: None,
            offset: None,
        }
    }

    /// Set the scope
    pub fn scope(mut self, scope: SoftDeleteScope) -> Self {
        self.scope = scope;
        self
    }

    /// Include only active (non-deleted) records
    pub fn active(mut self) -> Self {
        self.scope = SoftDeleteScope::Active;
        self
    }

    /// Include only deleted records
    pub fn deleted(mut self) -> Self {
        self.scope = SoftDeleteScope::Deleted;
        self
    }

    /// Include all records (deleted and non-deleted)
    pub fn with_deleted(mut self) -> Self {
        self.scope = SoftDeleteScope::All;
        self
    }

    /// Set the limit
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the offset
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Get the scope
    pub fn get_scope(&self) -> SoftDeleteScope {
        self.scope
    }

    /// Build the WHERE clause
    pub fn build_where(&self) -> String {
        self.scope.to_sql().to_string()
    }

    /// Build the complete SQL suffix (WHERE, LIMIT, OFFSET)
    pub fn build_sql_suffix(&self) -> String {
        let mut sql = format!("WHERE {}", self.scope.to_sql());

        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        sql
    }
}

impl Default for SoftDeleteQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for soft delete operations
pub struct SoftDeleteHelper;

impl SoftDeleteHelper {
    /// Soft delete a record by ID
    pub async fn soft_delete_by_id(table: &str, id: Uuid, pool: &PgPool) -> Result<u64> {
        let sql = format!(
            "UPDATE {} SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
            table
        );

        let result = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Restore a soft deleted record by ID
    pub async fn restore_by_id(table: &str, id: Uuid, pool: &PgPool) -> Result<u64> {
        let sql = format!(
            "UPDATE {} SET deleted_at = NULL WHERE id = $1 AND deleted_at IS NOT NULL",
            table
        );

        let result = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Force delete a record by ID
    pub async fn force_delete_by_id(table: &str, id: Uuid, pool: &PgPool) -> Result<u64> {
        let sql = format!("DELETE FROM {} WHERE id = $1", table);

        let result = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Clean up old soft deleted records (permanently delete)
    pub async fn cleanup_old_deleted(table: &str, days: i64, pool: &PgPool) -> Result<u64> {
        let sql = format!(
            "DELETE FROM {} WHERE deleted_at IS NOT NULL AND deleted_at < NOW() - INTERVAL '{} days'",
            table, days
        );

        let result = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await?;

        Ok(result.rows_affected())
    }

    /// Count soft deleted records
    pub async fn count_deleted(table: &str, pool: &PgPool) -> Result<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM {} WHERE deleted_at IS NOT NULL",
            table
        );

        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(pool)
            .await?;

        Ok(count)
    }

    /// Count active records
    pub async fn count_active(table: &str, pool: &PgPool) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {} WHERE deleted_at IS NULL", table);

        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(pool)
            .await?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soft_delete_scope_to_sql() {
        assert_eq!(SoftDeleteScope::Active.to_sql(), "deleted_at IS NULL");
        assert_eq!(SoftDeleteScope::Deleted.to_sql(), "deleted_at IS NOT NULL");
        assert_eq!(SoftDeleteScope::All.to_sql(), "TRUE");
    }

    #[test]
    fn test_soft_delete_scope_append_to_where() {
        let scope = SoftDeleteScope::Active;

        let result = scope.append_to_where("");
        assert_eq!(result, "WHERE deleted_at IS NULL");

        let result = scope.append_to_where("WHERE user_id = $1");
        assert_eq!(result, "WHERE user_id = $1 AND deleted_at IS NULL");
    }

    #[test]
    fn test_soft_delete_query() {
        let query = SoftDeleteQuery::new();
        assert_eq!(query.get_scope(), SoftDeleteScope::Active);

        let query = query.deleted();
        assert_eq!(query.get_scope(), SoftDeleteScope::Deleted);

        let query = query.with_deleted();
        assert_eq!(query.get_scope(), SoftDeleteScope::All);
    }

    #[test]
    fn test_soft_delete_query_build_where() {
        let query = SoftDeleteQuery::new().active();
        assert_eq!(query.build_where(), "deleted_at IS NULL");

        let query = SoftDeleteQuery::new().deleted();
        assert_eq!(query.build_where(), "deleted_at IS NOT NULL");
    }

    #[test]
    fn test_soft_delete_query_build_sql_suffix() {
        let query = SoftDeleteQuery::new().active().limit(10).offset(5);

        let sql = query.build_sql_suffix();
        assert!(sql.contains("WHERE deleted_at IS NULL"));
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("OFFSET 5"));
    }

    #[derive(Debug)]
    struct TestModel {
        #[allow(dead_code)]
        id: Uuid,
        deleted_at: Option<DateTime<Utc>>,
    }

    #[async_trait::async_trait]
    impl SoftDelete for TestModel {
        fn deleted_at(&self) -> Option<DateTime<Utc>> {
            self.deleted_at
        }

        fn set_deleted_at(&mut self, timestamp: Option<DateTime<Utc>>) {
            self.deleted_at = timestamp;
        }

        async fn save_deleted_at(&self, _pool: &PgPool) -> Result<()> {
            Ok(())
        }

        async fn force_delete(&self, _pool: &PgPool) -> Result<u64> {
            Ok(1)
        }
    }

    #[test]
    fn test_soft_delete_trait() {
        let mut model = TestModel {
            id: Uuid::new_v4(),
            deleted_at: None,
        };

        assert!(!model.is_deleted());

        model.set_deleted_at(Some(Utc::now()));
        assert!(model.is_deleted());

        model.set_deleted_at(None);
        assert!(!model.is_deleted());
    }
}
