//! Database schema validation and integrity checking
//!
//! This module provides comprehensive schema validation including:
//! - Foreign key relationship validation
//! - Missing index detection on foreign keys
//! - Orphaned record detection
//! - Constraint validation
//! - Data integrity checks

use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

/// Foreign key constraint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyInfo {
    /// Constraint name
    pub constraint_name: String,
    /// Source table name
    pub table_name: String,
    /// Source column name
    pub column_name: String,
    /// Referenced table name
    pub referenced_table: String,
    /// Referenced column name
    pub referenced_column: String,
    /// Whether the foreign key has an index
    pub has_index: bool,
}

/// Orphaned record information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanedRecordInfo {
    /// Table containing orphaned records
    pub table_name: String,
    /// Foreign key column
    pub column_name: String,
    /// Referenced table
    pub referenced_table: String,
    /// Number of orphaned records
    pub orphaned_count: i64,
}

/// Check constraint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConstraintInfo {
    /// Constraint name
    pub constraint_name: String,
    /// Table name
    pub table_name: String,
    /// Check definition
    pub check_definition: String,
    /// Whether the constraint is validated
    pub is_validated: bool,
}

/// Schema validation issue severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Informational issue
    Info,
    /// Warning - should be addressed
    Warning,
    /// Error - requires immediate attention
    Error,
    /// Critical - data integrity at risk
    Critical,
}

/// Schema validation issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Issue severity
    pub severity: ValidationSeverity,
    /// Issue category
    pub category: String,
    /// Issue description
    pub description: String,
    /// Affected object (table, constraint, etc.)
    pub affected_object: String,
    /// Recommendation for fixing
    pub recommendation: String,
}

/// Complete schema validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaValidationReport {
    /// All validation issues found
    pub issues: Vec<ValidationIssue>,
    /// Foreign key information
    pub foreign_keys: Vec<ForeignKeyInfo>,
    /// Orphaned records found
    pub orphaned_records: Vec<OrphanedRecordInfo>,
    /// Check constraints
    pub check_constraints: Vec<CheckConstraintInfo>,
    /// Total number of issues by severity
    pub critical_count: usize,
    /// Number of errors
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
    /// Number of info items
    pub info_count: usize,
    /// Validation timestamp
    pub validated_at: chrono::DateTime<chrono::Utc>,
}

/// Get all foreign key constraints in the database
pub async fn get_foreign_keys(pool: &PgPool) -> Result<Vec<ForeignKeyInfo>> {
    let rows = sqlx::query(
        r#"
        SELECT
            tc.constraint_name,
            tc.table_name,
            kcu.column_name,
            ccu.table_name AS referenced_table,
            ccu.column_name AS referenced_column,
            EXISTS(
                SELECT 1 FROM pg_indexes
                WHERE tablename = tc.table_name
                AND indexdef LIKE '%' || kcu.column_name || '%'
            ) AS has_index
        FROM information_schema.table_constraints AS tc
        JOIN information_schema.key_column_usage AS kcu
            ON tc.constraint_name = kcu.constraint_name
            AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage AS ccu
            ON ccu.constraint_name = tc.constraint_name
            AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
            AND tc.table_schema = 'public'
        ORDER BY tc.table_name, kcu.column_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut foreign_keys = Vec::new();
    for row in rows {
        foreign_keys.push(ForeignKeyInfo {
            constraint_name: row.try_get("constraint_name")?,
            table_name: row.try_get("table_name")?,
            column_name: row.try_get("column_name")?,
            referenced_table: row.try_get("referenced_table").unwrap_or_default(),
            referenced_column: row.try_get("referenced_column").unwrap_or_default(),
            has_index: row.try_get("has_index").unwrap_or(false),
        });
    }

    Ok(foreign_keys)
}

/// Detect orphaned records for a specific foreign key
pub async fn detect_orphaned_records(
    pool: &PgPool,
    table_name: &str,
    column_name: &str,
    referenced_table: &str,
    referenced_column: &str,
) -> Result<i64> {
    let query = format!(
        r#"
        SELECT COUNT(*) as count
        FROM "{}" AS t
        WHERE t."{}" IS NOT NULL
        AND NOT EXISTS (
            SELECT 1 FROM "{}" AS r
            WHERE r."{}" = t."{}"
        )
        "#,
        table_name, column_name, referenced_table, referenced_column, column_name
    );

    let row: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
    Ok(row.0)
}

/// Get all orphaned records in the database
pub async fn get_all_orphaned_records(pool: &PgPool) -> Result<Vec<OrphanedRecordInfo>> {
    let foreign_keys = get_foreign_keys(pool).await?;
    let mut orphaned_records = Vec::new();

    for fk in foreign_keys {
        let count = detect_orphaned_records(
            pool,
            &fk.table_name,
            &fk.column_name,
            &fk.referenced_table,
            &fk.referenced_column,
        )
        .await?;

        if count > 0 {
            orphaned_records.push(OrphanedRecordInfo {
                table_name: fk.table_name,
                column_name: fk.column_name,
                referenced_table: fk.referenced_table,
                orphaned_count: count,
            });
        }
    }

    Ok(orphaned_records)
}

/// Get all check constraints in the database
pub async fn get_check_constraints(pool: &PgPool) -> Result<Vec<CheckConstraintInfo>> {
    let rows = sqlx::query(
        r#"
        SELECT
            tc.constraint_name,
            tc.table_name,
            pg_get_constraintdef(pgc.oid) AS check_definition,
            pgc.convalidated AS is_validated
        FROM information_schema.table_constraints AS tc
        JOIN pg_constraint AS pgc
            ON pgc.conname = tc.constraint_name
        WHERE tc.constraint_type = 'CHECK'
            AND tc.table_schema = 'public'
        ORDER BY tc.table_name, tc.constraint_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut constraints = Vec::new();
    for row in rows {
        constraints.push(CheckConstraintInfo {
            constraint_name: row.try_get("constraint_name")?,
            table_name: row.try_get("table_name")?,
            check_definition: row.try_get("check_definition").unwrap_or_default(),
            is_validated: row.try_get("is_validated").unwrap_or(false),
        });
    }

    Ok(constraints)
}

/// Validate database schema and generate a comprehensive report
pub async fn validate_schema(pool: &PgPool) -> Result<SchemaValidationReport> {
    let mut issues = Vec::new();

    // Get foreign keys and check for missing indexes
    let foreign_keys = get_foreign_keys(pool).await?;
    for fk in &foreign_keys {
        if !fk.has_index {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "Missing Index".to_string(),
                description: format!(
                    "Foreign key column '{}' in table '{}' lacks an index",
                    fk.column_name, fk.table_name
                ),
                affected_object: format!("{}.{}", fk.table_name, fk.column_name),
                recommendation: format!(
                    "CREATE INDEX idx_{}_{} ON \"{}\" (\"{}\");",
                    fk.table_name, fk.column_name, fk.table_name, fk.column_name
                ),
            });
        }
    }

    // Check for orphaned records
    let orphaned_records = get_all_orphaned_records(pool).await?;
    for orphaned in &orphaned_records {
        let severity = if orphaned.orphaned_count > 100 {
            ValidationSeverity::Critical
        } else if orphaned.orphaned_count > 10 {
            ValidationSeverity::Error
        } else {
            ValidationSeverity::Warning
        };

        issues.push(ValidationIssue {
            severity,
            category: "Orphaned Records".to_string(),
            description: format!(
                "Table '{}' has {} orphaned records in column '{}'",
                orphaned.table_name, orphaned.orphaned_count, orphaned.column_name
            ),
            affected_object: format!("{}.{}", orphaned.table_name, orphaned.column_name),
            recommendation: format!(
                "Clean up orphaned records or fix references to '{}'",
                orphaned.referenced_table
            ),
        });
    }

    // Check for unvalidated constraints
    let check_constraints = get_check_constraints(pool).await?;
    for constraint in &check_constraints {
        if !constraint.is_validated {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "Unvalidated Constraint".to_string(),
                description: format!(
                    "Check constraint '{}' on table '{}' is not validated",
                    constraint.constraint_name, constraint.table_name
                ),
                affected_object: format!(
                    "{}.{}",
                    constraint.table_name, constraint.constraint_name
                ),
                recommendation: format!(
                    "ALTER TABLE \"{}\" VALIDATE CONSTRAINT \"{}\";",
                    constraint.table_name, constraint.constraint_name
                ),
            });
        }
    }

    // Count issues by severity
    let critical_count = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Critical)
        .count();
    let error_count = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Error)
        .count();
    let warning_count = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Warning)
        .count();
    let info_count = issues
        .iter()
        .filter(|i| i.severity == ValidationSeverity::Info)
        .count();

    Ok(SchemaValidationReport {
        issues,
        foreign_keys,
        orphaned_records,
        check_constraints,
        critical_count,
        error_count,
        warning_count,
        info_count,
        validated_at: chrono::Utc::now(),
    })
}

/// Get tables with missing primary keys
pub async fn get_tables_without_primary_keys(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public'
        AND table_type = 'BASE TABLE'
        AND table_name NOT IN (
            SELECT table_name
            FROM information_schema.table_constraints
            WHERE constraint_type = 'PRIMARY KEY'
            AND table_schema = 'public'
        )
        ORDER BY table_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::new();
    for row in rows {
        tables.push(row.try_get("table_name")?);
    }

    Ok(tables)
}

/// Get columns that should be NOT NULL but aren't
pub async fn get_nullable_foreign_key_columns(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT
            kcu.table_name || '.' || kcu.column_name AS column_path
        FROM information_schema.key_column_usage AS kcu
        JOIN information_schema.table_constraints AS tc
            ON kcu.constraint_name = tc.constraint_name
        JOIN information_schema.columns AS c
            ON c.table_name = kcu.table_name
            AND c.column_name = kcu.column_name
        WHERE tc.constraint_type = 'FOREIGN KEY'
            AND c.is_nullable = 'YES'
            AND kcu.table_schema = 'public'
        ORDER BY column_path
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut columns = Vec::new();
    for row in rows {
        columns.push(row.try_get("column_path")?);
    }

    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreign_key_info_creation() {
        let fk = ForeignKeyInfo {
            constraint_name: "fk_user_id".to_string(),
            table_name: "orders".to_string(),
            column_name: "user_id".to_string(),
            referenced_table: "users".to_string(),
            referenced_column: "id".to_string(),
            has_index: true,
        };

        assert_eq!(fk.table_name, "orders");
        assert_eq!(fk.referenced_table, "users");
        assert!(fk.has_index);
    }

    #[test]
    fn test_orphaned_record_info_creation() {
        let orphaned = OrphanedRecordInfo {
            table_name: "orders".to_string(),
            column_name: "user_id".to_string(),
            referenced_table: "users".to_string(),
            orphaned_count: 42,
        };

        assert_eq!(orphaned.orphaned_count, 42);
    }

    #[test]
    fn test_validation_issue_severity() {
        let issue = ValidationIssue {
            severity: ValidationSeverity::Critical,
            category: "Data Integrity".to_string(),
            description: "Test issue".to_string(),
            affected_object: "test_table".to_string(),
            recommendation: "Fix it".to_string(),
        };

        assert_eq!(issue.severity, ValidationSeverity::Critical);
    }

    #[test]
    fn test_validation_severity_ordering() {
        assert!(ValidationSeverity::Critical != ValidationSeverity::Error);
        assert!(ValidationSeverity::Warning != ValidationSeverity::Info);
    }

    #[test]
    fn test_check_constraint_info() {
        let constraint = CheckConstraintInfo {
            constraint_name: "check_positive".to_string(),
            table_name: "balances".to_string(),
            check_definition: "CHECK (amount >= 0)".to_string(),
            is_validated: true,
        };

        assert_eq!(constraint.table_name, "balances");
        assert!(constraint.is_validated);
    }

    #[test]
    fn test_schema_validation_report_counts() {
        let report = SchemaValidationReport {
            issues: vec![
                ValidationIssue {
                    severity: ValidationSeverity::Critical,
                    category: "Test".to_string(),
                    description: "Critical issue".to_string(),
                    affected_object: "obj1".to_string(),
                    recommendation: "Fix".to_string(),
                },
                ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "Test".to_string(),
                    description: "Warning issue".to_string(),
                    affected_object: "obj2".to_string(),
                    recommendation: "Fix".to_string(),
                },
            ],
            foreign_keys: vec![],
            orphaned_records: vec![],
            check_constraints: vec![],
            critical_count: 1,
            error_count: 0,
            warning_count: 1,
            info_count: 0,
            validated_at: chrono::Utc::now(),
        };

        assert_eq!(report.critical_count, 1);
        assert_eq!(report.warning_count, 1);
        assert_eq!(report.issues.len(), 2);
    }

    #[test]
    fn test_validation_issue_serialization() {
        let issue = ValidationIssue {
            severity: ValidationSeverity::Error,
            category: "Integrity".to_string(),
            description: "Test".to_string(),
            affected_object: "table.column".to_string(),
            recommendation: "Fix immediately".to_string(),
        };

        let json = serde_json::to_string(&issue).unwrap();
        let deserialized: ValidationIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(issue.severity, deserialized.severity);
        assert_eq!(issue.category, deserialized.category);
    }

    #[test]
    fn test_foreign_key_serialization() {
        let fk = ForeignKeyInfo {
            constraint_name: "fk_test".to_string(),
            table_name: "test".to_string(),
            column_name: "id".to_string(),
            referenced_table: "ref".to_string(),
            referenced_column: "id".to_string(),
            has_index: false,
        };

        let json = serde_json::to_string(&fk).unwrap();
        let deserialized: ForeignKeyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(fk.table_name, deserialized.table_name);
        assert_eq!(fk.has_index, deserialized.has_index);
    }

    #[test]
    fn test_orphaned_record_serialization() {
        let orphaned = OrphanedRecordInfo {
            table_name: "orders".to_string(),
            column_name: "user_id".to_string(),
            referenced_table: "users".to_string(),
            orphaned_count: 10,
        };

        let json = serde_json::to_string(&orphaned).unwrap();
        let deserialized: OrphanedRecordInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(orphaned.orphaned_count, deserialized.orphaned_count);
    }

    #[test]
    fn test_schema_validation_report_serialization() {
        let report = SchemaValidationReport {
            issues: vec![],
            foreign_keys: vec![],
            orphaned_records: vec![],
            check_constraints: vec![],
            critical_count: 0,
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            validated_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: SchemaValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.critical_count, deserialized.critical_count);
    }
}
