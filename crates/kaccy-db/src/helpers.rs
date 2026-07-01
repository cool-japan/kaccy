//! Helper utilities for common database operations
//!
//! This module provides convenience functions and utilities that simplify
//! common database patterns and operations.

use crate::error::{DbError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

/// Helper for building pagination metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginationMeta {
    /// Current page number (1-indexed)
    pub current_page: u32,
    /// Total number of pages
    pub total_pages: u32,
    /// Number of items per page
    pub page_size: u32,
    /// Total number of items
    pub total_items: u64,
    /// Whether there is a next page
    pub has_next: bool,
    /// Whether there is a previous page
    pub has_prev: bool,
}

impl PaginationMeta {
    /// Create pagination metadata
    pub fn new(current_page: u32, page_size: u32, total_items: u64) -> Self {
        let total_pages = ((total_items as f64) / (page_size as f64)).ceil() as u32;

        Self {
            current_page,
            total_pages,
            page_size,
            total_items,
            has_next: current_page < total_pages,
            has_prev: current_page > 1,
        }
    }

    /// Calculate offset for SQL queries
    pub fn offset(&self) -> u64 {
        ((self.current_page - 1) * self.page_size) as u64
    }

    /// Get limit for SQL queries
    pub fn limit(&self) -> u32 {
        self.page_size
    }
}

/// Calculate database query offset from page number and page size
///
/// This is a convenience function to eliminate code duplication across repositories.
/// Common pattern: `(page.saturating_sub(1)) * limit`
///
/// # Arguments
/// * `page` - Page number (1-indexed)
/// * `page_size` - Number of items per page
///
/// # Returns
/// Offset value to use in SQL queries
///
/// # Examples
/// ```
/// use kaccy_db::helpers::calculate_offset;
///
/// let offset = calculate_offset(1, 20); // First page
/// assert_eq!(offset, 0);
///
/// let offset = calculate_offset(3, 20); // Third page
/// assert_eq!(offset, 40);
/// ```
pub fn calculate_offset(page: u32, page_size: u32) -> i64 {
    (page.saturating_sub(1) * page_size) as i64
}

/// Check if a table exists in the database
pub async fn table_exists(pool: &PgPool, table_name: &str) -> Result<bool> {
    let result = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_name = $1
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

/// Check if a column exists in a table
pub async fn column_exists(pool: &PgPool, table_name: &str, column_name: &str) -> Result<bool> {
    let result = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.columns
            WHERE table_schema = 'public'
            AND table_name = $1
            AND column_name = $2
        )
        "#,
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

/// Get the current database timestamp
pub async fn get_database_time(pool: &PgPool) -> Result<DateTime<Utc>> {
    let timestamp = sqlx::query_scalar::<_, DateTime<Utc>>("SELECT NOW()")
        .fetch_one(pool)
        .await?;

    Ok(timestamp)
}

/// Execute a raw SQL query and return the number of affected rows
pub async fn execute_raw(pool: &PgPool, sql: &str) -> Result<u64> {
    let result = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await?;

    Ok(result.rows_affected())
}

/// Safely convert a string to UUID, returning an error if invalid
pub fn parse_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| DbError::Other(format!("Invalid UUID: {}", e)))
}

/// Safely convert a string to Decimal, returning an error if invalid
pub fn parse_decimal(s: &str) -> Result<Decimal> {
    s.parse::<Decimal>()
        .map_err(|e| DbError::Other(format!("Invalid decimal: {}", e)))
}

/// Sanitize a table name for use in SQL queries (prevents SQL injection)
/// Only allows alphanumeric characters and underscores
pub fn sanitize_table_name(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(DbError::Other("Table name cannot be empty".to_string()));
    }

    if name.len() > 63 {
        return Err(DbError::Other(
            "Table name too long (max 63 chars)".to_string(),
        ));
    }

    // Check that name only contains safe characters
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(DbError::Other(
            "Table name can only contain alphanumeric characters and underscores".to_string(),
        ));
    }

    // Check that name doesn't start with a digit
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(DbError::Other(
            "Table name cannot start with a digit".to_string(),
        ));
    }

    Ok(name.to_string())
}

/// Sanitize a column name for use in SQL queries (prevents SQL injection)
/// Only allows alphanumeric characters and underscores
pub fn sanitize_column_name(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(DbError::Other("Column name cannot be empty".to_string()));
    }

    if name.len() > 63 {
        return Err(DbError::Other(
            "Column name too long (max 63 chars)".to_string(),
        ));
    }

    // Check that name only contains safe characters
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(DbError::Other(
            "Column name can only contain alphanumeric characters and underscores".to_string(),
        ));
    }

    // Check that name doesn't start with a digit
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(DbError::Other(
            "Column name cannot start with a digit".to_string(),
        ));
    }

    Ok(name.to_string())
}

/// Calculate percentage with proper precision
pub fn calculate_percentage(part: u64, total: u64) -> Decimal {
    if total == 0 {
        return Decimal::ZERO;
    }

    let part_dec = Decimal::from(part);
    let total_dec = Decimal::from(total);

    (part_dec / total_dec) * Decimal::from(100)
}

/// Format bytes to human-readable size
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_meta() {
        let meta = PaginationMeta::new(1, 20, 100);
        assert_eq!(meta.total_pages, 5);
        assert_eq!(meta.offset(), 0);
        assert_eq!(meta.limit(), 20);
        assert!(meta.has_next);
        assert!(!meta.has_prev);

        let meta2 = PaginationMeta::new(3, 20, 100);
        assert_eq!(meta2.offset(), 40);
        assert!(meta2.has_next);
        assert!(meta2.has_prev);

        let meta3 = PaginationMeta::new(5, 20, 100);
        assert!(!meta3.has_next);
        assert!(meta3.has_prev);
    }

    #[test]
    fn test_parse_uuid() {
        let valid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(parse_uuid(valid).is_ok());

        let invalid = "not-a-uuid";
        assert!(parse_uuid(invalid).is_err());
    }

    #[test]
    fn test_parse_decimal() {
        assert!(parse_decimal("123.45").is_ok());
        assert!(parse_decimal("0.001").is_ok());
        assert!(parse_decimal("invalid").is_err());
    }

    #[test]
    fn test_sanitize_table_name() {
        assert!(sanitize_table_name("users").is_ok());
        assert!(sanitize_table_name("user_balances").is_ok());
        assert!(sanitize_table_name("table_123").is_ok());

        assert!(sanitize_table_name("").is_err());
        assert!(sanitize_table_name("123_table").is_err()); // starts with digit
        assert!(sanitize_table_name("user-table").is_err()); // contains hyphen
        assert!(sanitize_table_name("user@table").is_err()); // contains @
        assert!(sanitize_table_name("DROP TABLE users; --").is_err());
    }

    #[test]
    fn test_sanitize_column_name() {
        assert!(sanitize_column_name("id").is_ok());
        assert!(sanitize_column_name("user_id").is_ok());
        assert!(sanitize_column_name("column_123").is_ok());

        assert!(sanitize_column_name("").is_err());
        assert!(sanitize_column_name("123_column").is_err());
        assert!(sanitize_column_name("col-name").is_err());
        assert!(sanitize_column_name("col;DROP TABLE").is_err());
    }

    #[test]
    fn test_calculate_percentage() {
        assert_eq!(calculate_percentage(50, 100), Decimal::from(50));

        // 1/3 * 100 = 33.333...
        let result = calculate_percentage(1, 3);
        assert!(result > Decimal::new(3333, 2)); // greater than 33.33
        assert!(result < Decimal::new(3334, 2)); // less than 33.34

        assert_eq!(calculate_percentage(0, 100), Decimal::ZERO);
        assert_eq!(calculate_percentage(100, 0), Decimal::ZERO); // division by zero
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_calculate_offset() {
        // First page
        assert_eq!(calculate_offset(1, 20), 0);

        // Second page
        assert_eq!(calculate_offset(2, 20), 20);

        // Third page
        assert_eq!(calculate_offset(3, 20), 40);

        // Different page sizes
        assert_eq!(calculate_offset(1, 50), 0);
        assert_eq!(calculate_offset(5, 10), 40);

        // Edge case: page 0 should be treated as page 1 (saturating_sub)
        assert_eq!(calculate_offset(0, 20), 0);
    }
}
