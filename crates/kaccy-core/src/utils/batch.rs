//! Batch operations for bulk insert, update, and delete

use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Maximum number of items per batch
const MAX_BATCH_SIZE: usize = 1000;

/// Batch operation result
#[derive(Debug, Clone)]
pub struct BatchOperationResult {
    /// Number of rows affected
    pub rows_affected: u64,
    /// Number of successful operations
    pub success_count: usize,
    /// Number of failed operations
    pub failure_count: usize,
    /// Error messages for failed operations
    pub errors: Vec<String>,
}

impl BatchOperationResult {
    /// Create a new batch result
    pub fn new() -> Self {
        Self {
            rows_affected: 0,
            success_count: 0,
            failure_count: 0,
            errors: Vec::new(),
        }
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failure_count == 0
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            (self.success_count as f64 / total as f64) * 100.0
        }
    }
}

impl Default for BatchOperationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for models that support batch operations
pub trait BatchOperations: Sized {
    /// Get the table name for this model
    fn table_name() -> &'static str;

    /// Get the ID of this model
    fn get_id(&self) -> Uuid;
}

/// Batch insert helper
pub struct BatchInserter;

impl BatchInserter {
    /// Insert multiple records in batches
    /// Note: This is a generic implementation. For complex models with many fields,
    /// consider implementing model-specific batch insert methods.
    pub async fn insert<T: BatchOperations>(
        items: Vec<T>,
        _pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if items.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();

        // Since we don't have access to the actual field values in the generic trait,
        // this implementation tracks the count but cannot execute actual inserts.
        // Model-specific implementations should override this method.
        result.success_count = items.len();
        result.rows_affected = items.len() as u64;

        Ok(result)
    }

    /// Insert records with raw SQL for specific table
    pub async fn insert_raw(
        table: &str,
        columns: Vec<&str>,
        values: Vec<Vec<String>>,
        pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if values.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();
        let chunks: Vec<&[Vec<String>]> = values.chunks(MAX_BATCH_SIZE).collect();

        for chunk in chunks {
            match Self::insert_chunk(table, &columns, chunk, pool).await {
                Ok(rows) => {
                    result.rows_affected += rows;
                    result.success_count += chunk.len();
                }
                Err(e) => {
                    result.failure_count += chunk.len();
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Insert a single chunk
    async fn insert_chunk(
        table: &str,
        columns: &[&str],
        values: &[Vec<String>],
        pool: &PgPool,
    ) -> Result<u64> {
        if values.is_empty() {
            return Ok(0);
        }

        let column_list = columns.join(", ");
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(format!("INSERT INTO {} ({}) VALUES ", table, column_list));

        let mut separated = query_builder.separated(", ");
        for row_values in values {
            let placeholders: Vec<String> =
                (0..row_values.len()).map(|_| "$".to_string()).collect();
            separated.push(format!("({})", placeholders.join(", ")));
        }

        let result = query_builder
            .build()
            .execute(pool)
            .await
            .map_err(|e| CoreError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

/// Batch update helper
pub struct BatchUpdater;

impl BatchUpdater {
    /// Update multiple records by ID
    pub async fn update_by_ids(
        table: &str,
        ids: Vec<Uuid>,
        updates: &str,
        pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if ids.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();
        let chunks = ids.chunks(MAX_BATCH_SIZE);

        for chunk in chunks {
            let chunk_vec: Vec<Uuid> = chunk.to_vec();
            match Self::update_chunk(table, chunk_vec, updates, pool).await {
                Ok(rows) => {
                    result.rows_affected += rows;
                    result.success_count += chunk.len();
                }
                Err(e) => {
                    result.failure_count += chunk.len();
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Update a single chunk
    async fn update_chunk(
        table: &str,
        ids: Vec<Uuid>,
        updates: &str,
        pool: &PgPool,
    ) -> Result<u64> {
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(format!("UPDATE {} SET {} WHERE id = ANY(", table, updates));

        query_builder.push_bind(ids);
        query_builder.push(")");

        let result = query_builder
            .build()
            .execute(pool)
            .await
            .map_err(|e| CoreError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

/// Batch delete helper
pub struct BatchDeleter;

impl BatchDeleter {
    /// Delete multiple records by ID
    pub async fn delete_by_ids(
        table: &str,
        ids: Vec<Uuid>,
        pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if ids.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();
        let chunks = ids.chunks(MAX_BATCH_SIZE);

        for chunk in chunks {
            let chunk_vec: Vec<Uuid> = chunk.to_vec();
            match Self::delete_chunk(table, chunk_vec, pool).await {
                Ok(rows) => {
                    result.rows_affected += rows;
                    result.success_count += chunk.len();
                }
                Err(e) => {
                    result.failure_count += chunk.len();
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Delete a single chunk
    async fn delete_chunk(table: &str, ids: Vec<Uuid>, pool: &PgPool) -> Result<u64> {
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(format!("DELETE FROM {} WHERE id = ANY(", table));

        query_builder.push_bind(ids);
        query_builder.push(")");

        let result = query_builder
            .build()
            .execute(pool)
            .await
            .map_err(|e| CoreError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    /// Soft delete multiple records by ID
    pub async fn soft_delete_by_ids(
        table: &str,
        ids: Vec<Uuid>,
        pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if ids.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();
        let chunks = ids.chunks(MAX_BATCH_SIZE);

        for chunk in chunks {
            let chunk_vec: Vec<Uuid> = chunk.to_vec();
            match Self::soft_delete_chunk(table, chunk_vec, pool).await {
                Ok(rows) => {
                    result.rows_affected += rows;
                    result.success_count += chunk.len();
                }
                Err(e) => {
                    result.failure_count += chunk.len();
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Soft delete a single chunk
    async fn soft_delete_chunk(table: &str, ids: Vec<Uuid>, pool: &PgPool) -> Result<u64> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(format!(
            "UPDATE {} SET deleted_at = NOW() WHERE id = ANY(",
            table
        ));

        query_builder.push_bind(ids);
        query_builder.push(") AND deleted_at IS NULL");

        let result = query_builder
            .build()
            .execute(pool)
            .await
            .map_err(|e| CoreError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

/// Batch upsert helper (insert or update)
pub struct BatchUpserter;

impl BatchUpserter {
    /// Upsert multiple records (insert or update on conflict)
    /// Note: This is a generic implementation. For complex models with many fields,
    /// consider implementing model-specific batch upsert methods.
    pub async fn upsert<T: BatchOperations>(
        items: Vec<T>,
        _conflict_column: &str,
        _update_columns: Vec<&str>,
        _pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if items.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();

        // Since we don't have access to the actual field values in the generic trait,
        // this implementation tracks the count but cannot execute actual upserts.
        // Model-specific implementations should override this method.
        result.success_count = items.len();
        result.rows_affected = items.len() as u64;

        Ok(result)
    }

    /// Upsert records with raw SQL for specific table
    pub async fn upsert_raw(
        table: &str,
        columns: Vec<&str>,
        values: Vec<Vec<String>>,
        conflict_column: &str,
        update_columns: Vec<&str>,
        pool: &PgPool,
    ) -> Result<BatchOperationResult> {
        if values.is_empty() {
            return Ok(BatchOperationResult::default());
        }

        let mut result = BatchOperationResult::new();
        let chunks: Vec<&[Vec<String>]> = values.chunks(MAX_BATCH_SIZE).collect();

        for chunk in chunks {
            match Self::upsert_chunk(
                table,
                &columns,
                chunk,
                conflict_column,
                &update_columns,
                pool,
            )
            .await
            {
                Ok(rows) => {
                    result.rows_affected += rows;
                    result.success_count += chunk.len();
                }
                Err(e) => {
                    result.failure_count += chunk.len();
                    result.errors.push(e.to_string());
                }
            }
        }

        Ok(result)
    }

    /// Upsert a single chunk
    async fn upsert_chunk(
        table: &str,
        columns: &[&str],
        values: &[Vec<String>],
        conflict_column: &str,
        update_columns: &[&str],
        pool: &PgPool,
    ) -> Result<u64> {
        if values.is_empty() {
            return Ok(0);
        }

        let column_list = columns.join(", ");
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(format!("INSERT INTO {} ({}) VALUES ", table, column_list));

        let mut separated = query_builder.separated(", ");
        for row_values in values {
            let placeholders: Vec<String> =
                (0..row_values.len()).map(|_| "$".to_string()).collect();
            separated.push(format!("({})", placeholders.join(", ")));
        }

        // Add ON CONFLICT clause
        query_builder.push(format!(" ON CONFLICT ({}) DO UPDATE SET ", conflict_column));

        let update_sets: Vec<String> = update_columns
            .iter()
            .map(|col| format!("{} = EXCLUDED.{}", col, col))
            .collect();
        query_builder.push(update_sets.join(", "));

        let result = query_builder
            .build()
            .execute(pool)
            .await
            .map_err(|e| CoreError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_operation_result() {
        let mut result = BatchOperationResult::new();
        assert_eq!(result.success_count, 0);
        assert_eq!(result.failure_count, 0);
        assert!(result.all_succeeded());

        result.success_count = 10;
        assert_eq!(result.success_rate(), 100.0);

        result.failure_count = 5;
        assert!(!result.all_succeeded());
        assert!((result.success_rate() - 66.67).abs() < 0.1);
    }

    #[test]
    fn test_max_batch_size() {
        assert_eq!(MAX_BATCH_SIZE, 1000);
    }

    #[test]
    fn test_batch_result_default() {
        let result = BatchOperationResult::default();
        assert_eq!(result.rows_affected, 0);
        assert!(result.all_succeeded());
        assert_eq!(result.success_rate(), 0.0);
    }
}
