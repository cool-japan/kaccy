//! Index analysis and optimization suggestions
//!
//! This module analyzes query patterns from the QueryLogger
//! and provides recommendations for index optimization.

use serde::Serialize;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

use crate::error::Result;
use crate::query_logger::{QueryLogger, QueryStats};

/// Index suggestion priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Priority {
    /// Low priority - minor improvement expected
    Low,
    /// Medium priority - moderate improvement expected
    Medium,
    /// High priority - significant improvement expected
    High,
    /// Critical - query is likely causing performance issues
    Critical,
}

/// Type of index suggestion
#[derive(Debug, Clone, Serialize)]
pub enum IndexSuggestionType {
    /// Create a new single-column index
    SingleColumn { table: String, column: String },
    /// Create a composite index
    Composite { table: String, columns: Vec<String> },
    /// Create a partial index
    Partial {
        table: String,
        column: String,
        condition: String,
    },
    /// Create a covering index (INCLUDE)
    Covering {
        table: String,
        index_columns: Vec<String>,
        include_columns: Vec<String>,
    },
    /// Create a GIN index for full-text search
    FullText { table: String, column: String },
    /// Create a BRIN index for range queries on sorted data
    Brin { table: String, column: String },
}

/// Index suggestion
#[derive(Debug, Clone, Serialize)]
pub struct IndexSuggestion {
    /// Suggestion type
    pub suggestion_type: IndexSuggestionType,
    /// Priority level
    pub priority: Priority,
    /// Estimated impact description
    pub impact: String,
    /// SQL to create the index
    pub create_sql: String,
    /// Queries that would benefit
    pub affected_query_count: usize,
    /// Total execution time affected (ms)
    pub total_time_affected_ms: u64,
    /// Reasoning for the suggestion
    pub reasoning: String,
}

/// Existing index information
#[derive(Debug, Clone, Serialize)]
pub struct ExistingIndex {
    /// Index name
    pub name: String,
    /// Table name
    pub table: String,
    /// Columns in the index
    pub columns: Vec<String>,
    /// Whether it's unique
    pub is_unique: bool,
    /// Whether it's a primary key
    pub is_primary: bool,
    /// Index type (btree, hash, gin, gist, brin)
    pub index_type: String,
    /// Index size in bytes
    pub size_bytes: i64,
    /// Number of index scans
    pub index_scans: i64,
    /// Number of tuples read via index
    pub tuples_read: i64,
    /// Number of tuples fetched via index
    pub tuples_fetched: i64,
}

/// Unused index information
#[derive(Debug, Clone, Serialize)]
pub struct UnusedIndex {
    /// Index information
    pub index: ExistingIndex,
    /// Size wasted
    pub size_bytes: i64,
    /// Recommendation
    pub recommendation: String,
}

/// Index analysis result
#[derive(Debug, Clone, Serialize)]
pub struct IndexAnalysis {
    /// Suggested new indexes
    pub suggestions: Vec<IndexSuggestion>,
    /// Existing indexes
    pub existing_indexes: Vec<ExistingIndex>,
    /// Potentially unused indexes
    pub unused_indexes: Vec<UnusedIndex>,
    /// Duplicate indexes (same columns, different names)
    pub duplicate_indexes: Vec<(ExistingIndex, ExistingIndex)>,
    /// Overall health score (0-100)
    pub health_score: u32,
    /// Summary of findings
    pub summary: AnalysisSummary,
}

/// Summary of index analysis
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisSummary {
    /// Total indexes analyzed
    pub total_indexes: usize,
    /// Unused indexes found
    pub unused_count: usize,
    /// Duplicate indexes found
    pub duplicate_count: usize,
    /// New indexes suggested
    pub suggestion_count: usize,
    /// Critical suggestions
    pub critical_count: usize,
    /// Total wasted space from unused indexes (bytes)
    pub wasted_space_bytes: i64,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Index analyzer
pub struct IndexAnalyzer {
    /// Minimum scans to consider an index "used"
    pub min_scans_threshold: i64,
    /// Minimum query calls to consider for indexing
    pub min_query_calls: u64,
    /// Minimum average duration (ms) to consider for indexing
    pub min_avg_duration_ms: f64,
}

impl Default for IndexAnalyzer {
    fn default() -> Self {
        Self {
            min_scans_threshold: 10,
            min_query_calls: 5,
            min_avg_duration_ms: 10.0,
        }
    }
}

impl IndexAnalyzer {
    /// Create a new index analyzer
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum scans threshold for used index detection
    pub fn with_min_scans(mut self, threshold: i64) -> Self {
        self.min_scans_threshold = threshold;
        self
    }

    /// Analyze indexes and generate suggestions
    pub async fn analyze(
        &self,
        pool: &PgPool,
        query_logger: Option<&QueryLogger>,
    ) -> Result<IndexAnalysis> {
        // Get existing indexes
        let existing_indexes = self.get_existing_indexes(pool).await?;

        // Get index usage stats
        let index_usage = self.get_index_usage_stats(pool).await?;

        // Find unused indexes
        let unused_indexes = self.find_unused_indexes(&existing_indexes, &index_usage);

        // Find duplicate indexes
        let duplicate_indexes = self.find_duplicate_indexes(&existing_indexes);

        // Generate suggestions from query patterns
        let suggestions = if let Some(logger) = query_logger {
            self.generate_suggestions_from_queries(logger, &existing_indexes)
        } else {
            Vec::new()
        };

        // Calculate health score
        let health_score = self.calculate_health_score(
            &existing_indexes,
            &unused_indexes,
            &duplicate_indexes,
            &suggestions,
        );

        // Generate summary
        let wasted_space: i64 = unused_indexes.iter().map(|u| u.size_bytes).sum();
        let critical_count = suggestions
            .iter()
            .filter(|s| s.priority == Priority::Critical)
            .count();

        let mut recommendations = Vec::new();
        if !unused_indexes.is_empty() {
            recommendations.push(format!(
                "Consider dropping {} unused indexes to save {} bytes",
                unused_indexes.len(),
                wasted_space
            ));
        }
        if !duplicate_indexes.is_empty() {
            recommendations.push(format!(
                "Found {} duplicate index pairs - consider consolidating",
                duplicate_indexes.len()
            ));
        }
        if critical_count > 0 {
            recommendations.push(format!(
                "{} critical index suggestions - address these first",
                critical_count
            ));
        }

        let summary = AnalysisSummary {
            total_indexes: existing_indexes.len(),
            unused_count: unused_indexes.len(),
            duplicate_count: duplicate_indexes.len(),
            suggestion_count: suggestions.len(),
            critical_count,
            wasted_space_bytes: wasted_space,
            recommendations,
        };

        Ok(IndexAnalysis {
            suggestions,
            existing_indexes,
            unused_indexes,
            duplicate_indexes,
            health_score,
            summary,
        })
    }

    /// Get existing indexes from the database
    async fn get_existing_indexes(&self, pool: &PgPool) -> Result<Vec<ExistingIndex>> {
        let rows = sqlx::query_as::<_, (String, String, String, bool, bool, String, i64)>(
            r#"
            SELECT
                i.relname as index_name,
                t.relname as table_name,
                array_to_string(array_agg(a.attname ORDER BY k.n), ', ') as columns,
                ix.indisunique as is_unique,
                ix.indisprimary as is_primary,
                am.amname as index_type,
                pg_relation_size(i.oid) as size_bytes
            FROM pg_index ix
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_class t ON t.oid = ix.indrelid
            JOIN pg_namespace n ON n.oid = t.relnamespace
            JOIN pg_am am ON am.oid = i.relam
            CROSS JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, n)
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = k.attnum
            WHERE n.nspname = 'public'
            GROUP BY i.relname, t.relname, ix.indisunique, ix.indisprimary, am.amname, i.oid
            ORDER BY t.relname, i.relname
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(name, table, columns, is_unique, is_primary, index_type, size_bytes)| {
                    ExistingIndex {
                        name,
                        table,
                        columns: columns.split(", ").map(String::from).collect(),
                        is_unique,
                        is_primary,
                        index_type,
                        size_bytes,
                        index_scans: 0,
                        tuples_read: 0,
                        tuples_fetched: 0,
                    }
                },
            )
            .collect())
    }

    /// Get index usage statistics
    async fn get_index_usage_stats(
        &self,
        pool: &PgPool,
    ) -> Result<HashMap<String, (i64, i64, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64, i64, i64)>(
            r#"
            SELECT
                indexrelname as index_name,
                COALESCE(idx_scan, 0) as index_scans,
                COALESCE(idx_tup_read, 0) as tuples_read,
                COALESCE(idx_tup_fetch, 0) as tuples_fetched
            FROM pg_stat_user_indexes
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(name, scans, read, fetched)| (name, (scans, read, fetched)))
            .collect())
    }

    /// Find unused indexes
    fn find_unused_indexes(
        &self,
        indexes: &[ExistingIndex],
        usage: &HashMap<String, (i64, i64, i64)>,
    ) -> Vec<UnusedIndex> {
        indexes
            .iter()
            .filter_map(|idx| {
                // Skip primary keys - they're always needed
                if idx.is_primary {
                    return None;
                }

                let (scans, _, _) = usage.get(&idx.name).copied().unwrap_or((0, 0, 0));

                if scans < self.min_scans_threshold {
                    let recommendation = if scans == 0 {
                        "Index has never been used - safe to drop".to_string()
                    } else {
                        format!(
                            "Index has only {} scans - consider dropping if not needed for constraints",
                            scans
                        )
                    };

                    Some(UnusedIndex {
                        index: idx.clone(),
                        size_bytes: idx.size_bytes,
                        recommendation,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find duplicate indexes (same columns on same table)
    fn find_duplicate_indexes(
        &self,
        indexes: &[ExistingIndex],
    ) -> Vec<(ExistingIndex, ExistingIndex)> {
        let mut duplicates = Vec::new();
        let mut seen: HashMap<(String, Vec<String>), ExistingIndex> = HashMap::new();

        for idx in indexes {
            // Skip unique/primary - they might be intentional
            if idx.is_unique || idx.is_primary {
                continue;
            }

            let key = (idx.table.clone(), idx.columns.clone());

            if let Some(existing) = seen.get(&key) {
                duplicates.push((existing.clone(), idx.clone()));
            } else {
                seen.insert(key, idx.clone());
            }
        }

        duplicates
    }

    /// Generate index suggestions from query patterns
    fn generate_suggestions_from_queries(
        &self,
        logger: &QueryLogger,
        existing_indexes: &[ExistingIndex],
    ) -> Vec<IndexSuggestion> {
        let stats = logger.get_stats();
        let mut suggestions = Vec::new();

        // Build a set of existing indexed columns for quick lookup
        let mut indexed_columns: HashMap<String, HashSet<Vec<String>>> = HashMap::new();
        for idx in existing_indexes {
            indexed_columns
                .entry(idx.table.clone())
                .or_default()
                .insert(idx.columns.clone());
        }

        for stat in &stats {
            // Skip queries that aren't called frequently or aren't slow
            if stat.call_count < self.min_query_calls
                || stat.avg_duration_ms < self.min_avg_duration_ms
            {
                continue;
            }

            // Analyze query for potential indexes
            if let Some(suggestion) = self.analyze_query_for_index(stat, &indexed_columns) {
                suggestions.push(suggestion);
            }
        }

        // Sort by priority (critical first)
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions
    }

    /// Analyze a single query for index suggestions
    fn analyze_query_for_index(
        &self,
        stat: &QueryStats,
        indexed_columns: &HashMap<String, HashSet<Vec<String>>>,
    ) -> Option<IndexSuggestion> {
        let query = stat.query_preview.to_uppercase();

        // Extract table name and WHERE conditions
        let (table, conditions) = self.extract_query_info(&query)?;

        // Check if this combination is already indexed
        let existing = indexed_columns.get(&table);
        let condition_columns: Vec<String> = conditions.iter().map(|c| c.to_lowercase()).collect();

        if let Some(indexes) = existing {
            // Check if any existing index covers these columns
            for idx_cols in indexes {
                if condition_columns.iter().all(|c| idx_cols.contains(c)) {
                    return None; // Already indexed
                }
            }
        }

        // Determine priority based on frequency and duration
        let priority = if stat.slow_count > 0 && stat.avg_duration_ms > 100.0 {
            Priority::Critical
        } else if stat.avg_duration_ms > 50.0 || stat.call_count > 100 {
            Priority::High
        } else if stat.avg_duration_ms > 20.0 || stat.call_count > 50 {
            Priority::Medium
        } else {
            Priority::Low
        };

        let (suggestion_type, create_sql, reasoning) = if condition_columns.len() == 1 {
            let col = &condition_columns[0];
            (
                IndexSuggestionType::SingleColumn {
                    table: table.clone(),
                    column: col.clone(),
                },
                format!(
                    "CREATE INDEX CONCURRENTLY idx_{}_{} ON {} ({});",
                    table, col, table, col
                ),
                format!(
                    "Query frequently filters on {} with avg {}ms execution time",
                    col, stat.avg_duration_ms as u32
                ),
            )
        } else {
            let cols_str = condition_columns.join(", ");
            let cols_name = condition_columns.join("_");
            (
                IndexSuggestionType::Composite {
                    table: table.clone(),
                    columns: condition_columns.clone(),
                },
                format!(
                    "CREATE INDEX CONCURRENTLY idx_{}_{} ON {} ({});",
                    table, cols_name, table, cols_str
                ),
                format!(
                    "Query frequently filters on multiple columns ({}) with avg {}ms execution time",
                    cols_str, stat.avg_duration_ms as u32
                ),
            )
        };

        let impact = format!(
            "Could improve {} queries totaling {}ms execution time",
            stat.call_count, stat.total_duration_ms
        );

        Some(IndexSuggestion {
            suggestion_type,
            priority,
            impact,
            create_sql,
            affected_query_count: 1,
            total_time_affected_ms: stat.total_duration_ms,
            reasoning,
        })
    }

    /// Extract table name and WHERE conditions from a query
    fn extract_query_info(&self, query: &str) -> Option<(String, Vec<String>)> {
        // Simple parser - extracts table from FROM clause and columns from WHERE clause

        // Find table name
        let from_pos = query.find("FROM ")?;
        let after_from = &query[from_pos + 5..];
        let table_end = after_from.find(|c: char| c.is_whitespace() || c == ',')?;
        let table = after_from[..table_end].trim().to_lowercase();

        // Skip system tables
        if table.starts_with("pg_") || table.starts_with("information_schema") {
            return None;
        }

        // Find WHERE conditions
        let mut conditions = Vec::new();
        if let Some(where_pos) = query.find("WHERE ") {
            let where_clause = &query[where_pos + 6..];

            // Simple extraction of column names before = or IN
            let parts: Vec<&str> = where_clause.split(['=', '<', '>', ' ']).collect();

            for part in parts {
                let trimmed = part.trim();
                // Check if it looks like a column name (not a value or keyword)
                if !trimmed.is_empty()
                    && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && ![
                        "AND", "OR", "NOT", "IN", "IS", "NULL", "TRUE", "FALSE", "LIKE",
                    ]
                    .contains(&trimmed)
                {
                    let col = trimmed.to_lowercase();
                    if !conditions.contains(&col) {
                        conditions.push(col);
                    }
                }
            }
        }

        if conditions.is_empty() {
            return None;
        }

        Some((table, conditions))
    }

    /// Calculate overall index health score
    fn calculate_health_score(
        &self,
        existing: &[ExistingIndex],
        unused: &[UnusedIndex],
        duplicates: &[(ExistingIndex, ExistingIndex)],
        suggestions: &[IndexSuggestion],
    ) -> u32 {
        let mut score = 100u32;

        // Deduct for unused indexes
        let unused_ratio = unused.len() as f32 / existing.len().max(1) as f32;
        score = score.saturating_sub((unused_ratio * 20.0) as u32);

        // Deduct for duplicates
        score = score.saturating_sub((duplicates.len() * 5) as u32);

        // Deduct for missing critical indexes
        let critical_count = suggestions
            .iter()
            .filter(|s| s.priority == Priority::Critical)
            .count();
        score = score.saturating_sub((critical_count * 10) as u32);

        // Deduct for high priority suggestions
        let high_count = suggestions
            .iter()
            .filter(|s| s.priority == Priority::High)
            .count();
        score = score.saturating_sub((high_count * 5) as u32);

        score
    }

    /// Generate SQL script for all suggested indexes
    pub fn generate_migration_script(suggestions: &[IndexSuggestion]) -> String {
        let mut script = String::from("-- Index optimization migration\n");
        script.push_str("-- Generated by IndexAnalyzer\n\n");

        for (i, suggestion) in suggestions.iter().enumerate() {
            script.push_str(&format!(
                "-- Suggestion {}: {:?} priority\n",
                i + 1,
                suggestion.priority
            ));
            script.push_str(&format!("-- Reasoning: {}\n", suggestion.reasoning));
            script.push_str(&format!("-- Impact: {}\n", suggestion.impact));
            script.push_str(&suggestion.create_sql);
            script.push_str("\n\n");
        }

        script
    }

    /// Generate SQL to drop unused indexes
    pub fn generate_cleanup_script(unused: &[UnusedIndex]) -> String {
        let mut script = String::from("-- Unused index cleanup\n");
        script.push_str("-- Review each index before dropping!\n\n");

        for unused_idx in unused {
            script.push_str(&format!("-- {}\n", unused_idx.recommendation));
            script.push_str(&format!("-- Size: {} bytes\n", unused_idx.size_bytes));
            script.push_str(&format!(
                "DROP INDEX IF EXISTS {};\n\n",
                unused_idx.index.name
            ));
        }

        script
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_query_info() {
        let analyzer = IndexAnalyzer::new();

        let query = "SELECT * FROM users WHERE email = $1";
        let result = analyzer.extract_query_info(&query.to_uppercase());
        assert!(result.is_some());
        let (table, conditions) = result.unwrap();
        assert_eq!(table, "users");
        assert!(conditions.contains(&"email".to_string()));
    }

    #[test]
    fn test_extract_composite_conditions() {
        let analyzer = IndexAnalyzer::new();

        let query = "SELECT * FROM orders WHERE user_id = $1 AND status = $2";
        let result = analyzer.extract_query_info(&query.to_uppercase());
        assert!(result.is_some());
        let (table, conditions) = result.unwrap();
        assert_eq!(table, "orders");
        assert!(conditions.contains(&"user_id".to_string()));
        assert!(conditions.contains(&"status".to_string()));
    }

    #[test]
    fn test_find_duplicates() {
        let analyzer = IndexAnalyzer::new();

        let indexes = vec![
            ExistingIndex {
                name: "idx_users_email".to_string(),
                table: "users".to_string(),
                columns: vec!["email".to_string()],
                is_unique: false,
                is_primary: false,
                index_type: "btree".to_string(),
                size_bytes: 1000,
                index_scans: 100,
                tuples_read: 1000,
                tuples_fetched: 1000,
            },
            ExistingIndex {
                name: "idx_users_email_2".to_string(),
                table: "users".to_string(),
                columns: vec!["email".to_string()],
                is_unique: false,
                is_primary: false,
                index_type: "btree".to_string(),
                size_bytes: 1000,
                index_scans: 50,
                tuples_read: 500,
                tuples_fetched: 500,
            },
        ];

        let duplicates = analyzer.find_duplicate_indexes(&indexes);
        assert_eq!(duplicates.len(), 1);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);
    }
}
