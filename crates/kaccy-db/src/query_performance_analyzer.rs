//! Query Performance Analyzer
//!
//! Analyzes query performance and provides actionable optimization suggestions.
//! Complements the query_plan module with automatic analysis and recommendations.
//!
//! # Features
//!
//! - Automatic slow query detection from pg_stat_statements
//! - Query plan analysis with cost breakdown
//! - Index usage recommendations
//! - Query rewrite suggestions
//! - Performance bottleneck identification
//! - Historical performance tracking
//! - Configurable analysis thresholds
//!
//! # Example
//!
//! ```rust
//! use kaccy_db::query_performance_analyzer::{QueryPerformanceAnalyzer, AnalyzerConfig};
//! use std::time::Duration;
//!
//! let config = AnalyzerConfig {
//!     slow_query_threshold_ms: 1000, // 1 second
//!     min_calls: 10,
//!     analysis_limit: 50,
//! };
//!
//! let analyzer = QueryPerformanceAnalyzer::new(config);
//! ```

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info};

/// Configuration for the query performance analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Queries taking longer than this (in ms) are considered slow
    pub slow_query_threshold_ms: u64,

    /// Minimum number of calls before analyzing
    pub min_calls: i64,

    /// Maximum number of queries to analyze
    pub analysis_limit: i64,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            slow_query_threshold_ms: 1000, // 1 second
            min_calls: 10,
            analysis_limit: 50,
        }
    }
}

/// Optimization suggestion type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionType {
    /// Add a missing index
    AddIndex,

    /// Rewrite query for better performance
    RewriteQuery,

    /// Increase work_mem
    IncreaseWorkMem,

    /// Use materialized view
    UseMaterializedView,

    /// Partition table
    PartitionTable,

    /// Update statistics
    UpdateStatistics,

    /// Remove unused index
    RemoveUnusedIndex,

    /// Add covering index
    AddCoveringIndex,
}

impl SuggestionType {
    /// Get a human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::AddIndex => "Add missing index",
            Self::RewriteQuery => "Rewrite query",
            Self::IncreaseWorkMem => "Increase work_mem",
            Self::UseMaterializedView => "Use materialized view",
            Self::PartitionTable => "Partition table",
            Self::UpdateStatistics => "Update statistics",
            Self::RemoveUnusedIndex => "Remove unused index",
            Self::AddCoveringIndex => "Add covering index",
        }
    }
}

/// Optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Type of suggestion
    pub suggestion_type: SuggestionType,

    /// Detailed description
    pub description: String,

    /// SQL to apply the suggestion (if applicable)
    pub sql: Option<String>,

    /// Expected performance improvement (0.0-1.0)
    pub expected_improvement: f64,

    /// Priority (1-10, higher is more important)
    pub priority: u8,
}

/// Query performance issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    /// Type of issue (e.g., "Sequential Scan", "High Cost", "Many Rows")
    pub issue_type: String,

    /// Description of the issue
    pub description: String,

    /// Severity (1-10, higher is more severe)
    pub severity: u8,

    /// Affected table or operation
    pub affected_object: Option<String>,
}

/// Query performance analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysis {
    /// Query text (normalized)
    pub query: String,

    /// Average execution time in milliseconds
    pub avg_time_ms: f64,

    /// Total execution time in milliseconds
    pub total_time_ms: f64,

    /// Number of calls
    pub calls: i64,

    /// Average rows returned
    pub avg_rows: f64,

    /// Identified performance issues
    pub issues: Vec<PerformanceIssue>,

    /// Optimization suggestions
    pub suggestions: Vec<OptimizationSuggestion>,

    /// Overall performance score (0-100, higher is better)
    pub performance_score: u8,

    /// When this analysis was performed
    pub analyzed_at: DateTime<Utc>,
}

impl QueryAnalysis {
    /// Calculate performance score based on issues and timing
    pub fn calculate_score(&mut self) {
        let mut score = 100u8;

        // Deduct points for execution time
        if self.avg_time_ms > 5000.0 {
            score = score.saturating_sub(30);
        } else if self.avg_time_ms > 1000.0 {
            score = score.saturating_sub(20);
        } else if self.avg_time_ms > 500.0 {
            score = score.saturating_sub(10);
        }

        // Deduct points for issues
        for issue in &self.issues {
            score = score.saturating_sub(issue.severity);
        }

        self.performance_score = score;
    }
}

/// Performance analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,

    /// Total queries analyzed
    pub total_queries_analyzed: usize,

    /// Queries with performance issues
    pub queries_with_issues: usize,

    /// Individual query analyses
    pub analyses: Vec<QueryAnalysis>,

    /// Summary statistics
    pub summary: ReportSummary,
}

/// Report summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total number of suggestions
    pub total_suggestions: usize,

    /// Average performance score
    pub avg_performance_score: f64,

    /// Total slow query time (ms)
    pub total_slow_query_time_ms: f64,

    /// Most common issue type
    pub most_common_issue: Option<String>,
}

/// Query performance analyzer
pub struct QueryPerformanceAnalyzer {
    config: AnalyzerConfig,
}

impl QueryPerformanceAnalyzer {
    /// Create a new query performance analyzer
    pub fn new(config: AnalyzerConfig) -> Self {
        Self { config }
    }

    /// Create an analyzer with default configuration
    pub fn with_defaults() -> Self {
        Self::new(AnalyzerConfig::default())
    }

    /// Analyze slow queries and generate report
    pub async fn analyze_slow_queries(&self, pool: &PgPool) -> Result<PerformanceReport> {
        info!("Starting query performance analysis");

        let slow_queries = self.get_slow_queries(pool).await?;
        let mut analyses = Vec::new();

        for (query, avg_time_ms, total_time_ms, calls, avg_rows) in slow_queries {
            let mut analysis = QueryAnalysis {
                query: query.clone(),
                avg_time_ms,
                total_time_ms,
                calls,
                avg_rows,
                issues: Vec::new(),
                suggestions: Vec::new(),
                performance_score: 100,
                analyzed_at: Utc::now(),
            };

            // Analyze query plan
            self.analyze_query_plan(pool, &query, &mut analysis).await?;

            // Generate suggestions based on issues
            self.generate_suggestions(&mut analysis);

            // Calculate performance score
            analysis.calculate_score();

            analyses.push(analysis);
        }

        let queries_with_issues = analyses.iter().filter(|a| !a.issues.is_empty()).count();

        let summary = self.calculate_summary(&analyses);

        debug!(
            total_analyzed = analyses.len(),
            with_issues = queries_with_issues,
            "Query analysis complete"
        );

        Ok(PerformanceReport {
            generated_at: Utc::now(),
            total_queries_analyzed: analyses.len(),
            queries_with_issues,
            analyses,
            summary,
        })
    }

    /// Get slow queries from pg_stat_statements
    async fn get_slow_queries(&self, pool: &PgPool) -> Result<Vec<(String, f64, f64, i64, f64)>> {
        let query = r#"
            SELECT
                query,
                mean_exec_time,
                total_exec_time,
                calls,
                COALESCE(rows::float / NULLIF(calls, 0), 0) as avg_rows
            FROM pg_stat_statements
            WHERE mean_exec_time > $1
                AND calls >= $2
                AND query NOT LIKE '%pg_stat_statements%'
            ORDER BY mean_exec_time DESC
            LIMIT $3
        "#;

        let rows = sqlx::query_as::<_, (String, f64, f64, i64, f64)>(query)
            .bind(self.config.slow_query_threshold_ms as f64)
            .bind(self.config.min_calls)
            .bind(self.config.analysis_limit)
            .fetch_all(pool)
            .await?;

        Ok(rows)
    }

    /// Analyze query execution plan
    async fn analyze_query_plan(
        &self,
        pool: &PgPool,
        query: &str,
        analysis: &mut QueryAnalysis,
    ) -> Result<()> {
        // Try to get explain plan
        let explain_query = format!("EXPLAIN (FORMAT JSON) {}", query);

        match sqlx::query_scalar::<_, serde_json::Value>(&explain_query)
            .fetch_one(pool)
            .await
        {
            Ok(plan) => {
                self.analyze_plan_json(&plan, analysis);
            }
            Err(_) => {
                debug!("Could not analyze query plan for: {}", query);
            }
        }

        Ok(())
    }

    /// Analyze JSON query plan
    fn analyze_plan_json(&self, plan: &serde_json::Value, analysis: &mut QueryAnalysis) {
        if let Some(plans) = plan.get(0).and_then(|p| p.get("Plan")) {
            self.analyze_plan_node(plans, analysis);
        }
    }

    /// Recursively analyze plan nodes
    #[allow(clippy::only_used_in_recursion)]
    fn analyze_plan_node(&self, node: &serde_json::Value, analysis: &mut QueryAnalysis) {
        // Check for sequential scans
        if let Some(node_type) = node.get("Node Type").and_then(|v| v.as_str()) {
            if node_type == "Seq Scan" {
                if let Some(relation) = node.get("Relation Name").and_then(|v| v.as_str()) {
                    analysis.issues.push(PerformanceIssue {
                        issue_type: "Sequential Scan".to_string(),
                        description: format!("Sequential scan on table '{}'", relation),
                        severity: 7,
                        affected_object: Some(relation.to_string()),
                    });
                }
            }

            // Check for high cost operations
            if let Some(total_cost) = node.get("Total Cost").and_then(|v| v.as_f64()) {
                if total_cost > 10000.0 {
                    analysis.issues.push(PerformanceIssue {
                        issue_type: "High Cost".to_string(),
                        description: format!("Operation has high cost: {:.2}", total_cost),
                        severity: 8,
                        affected_object: Some(node_type.to_string()),
                    });
                }
            }

            // Check for many rows
            if let Some(plan_rows) = node.get("Plan Rows").and_then(|v| v.as_f64()) {
                if plan_rows > 100000.0 {
                    analysis.issues.push(PerformanceIssue {
                        issue_type: "Many Rows".to_string(),
                        description: format!("Processing many rows: {:.0}", plan_rows),
                        severity: 6,
                        affected_object: Some(node_type.to_string()),
                    });
                }
            }
        }

        // Recursively check child plans
        if let Some(plans) = node.get("Plans").and_then(|v| v.as_array()) {
            for child_plan in plans {
                self.analyze_plan_node(child_plan, analysis);
            }
        }
    }

    /// Generate optimization suggestions based on issues
    fn generate_suggestions(&self, analysis: &mut QueryAnalysis) {
        for issue in &analysis.issues {
            match issue.issue_type.as_str() {
                "Sequential Scan" => {
                    if let Some(table) = &issue.affected_object {
                        analysis.suggestions.push(OptimizationSuggestion {
                            suggestion_type: SuggestionType::AddIndex,
                            description: format!(
                                "Consider adding an index on table '{}' for columns used in WHERE/JOIN clauses",
                                table
                            ),
                            sql: None,
                            expected_improvement: 0.7,
                            priority: 8,
                        });
                    }
                }
                "High Cost" => {
                    analysis.suggestions.push(OptimizationSuggestion {
                        suggestion_type: SuggestionType::RewriteQuery,
                        description: "Consider rewriting the query to reduce complexity"
                            .to_string(),
                        sql: None,
                        expected_improvement: 0.5,
                        priority: 7,
                    });
                }
                "Many Rows" => {
                    analysis.suggestions.push(OptimizationSuggestion {
                        suggestion_type: SuggestionType::AddCoveringIndex,
                        description:
                            "Consider adding a covering index to avoid accessing the table"
                                .to_string(),
                        sql: None,
                        expected_improvement: 0.6,
                        priority: 6,
                    });
                }
                _ => {}
            }
        }

        // Add general suggestions for very slow queries
        if analysis.avg_time_ms > 5000.0 {
            analysis.suggestions.push(OptimizationSuggestion {
                suggestion_type: SuggestionType::UpdateStatistics,
                description: "Run ANALYZE to update table statistics".to_string(),
                sql: None,
                expected_improvement: 0.3,
                priority: 5,
            });
        }
    }

    /// Calculate report summary
    fn calculate_summary(&self, analyses: &[QueryAnalysis]) -> ReportSummary {
        let total_suggestions: usize = analyses.iter().map(|a| a.suggestions.len()).sum();

        let avg_performance_score = if !analyses.is_empty() {
            analyses
                .iter()
                .map(|a| a.performance_score as f64)
                .sum::<f64>()
                / analyses.len() as f64
        } else {
            100.0
        };

        let total_slow_query_time_ms: f64 = analyses.iter().map(|a| a.total_time_ms).sum();

        // Find most common issue type
        let mut issue_counts = std::collections::HashMap::new();
        for analysis in analyses {
            for issue in &analysis.issues {
                *issue_counts.entry(issue.issue_type.clone()).or_insert(0) += 1;
            }
        }

        let most_common_issue = issue_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(issue_type, _)| issue_type);

        ReportSummary {
            total_suggestions,
            avg_performance_score,
            total_slow_query_time_ms,
            most_common_issue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_config_default() {
        let config = AnalyzerConfig::default();
        assert_eq!(config.slow_query_threshold_ms, 1000);
        assert_eq!(config.min_calls, 10);
        assert_eq!(config.analysis_limit, 50);
    }

    #[test]
    fn test_suggestion_type_description() {
        assert_eq!(SuggestionType::AddIndex.description(), "Add missing index");
        assert_eq!(SuggestionType::RewriteQuery.description(), "Rewrite query");
    }

    #[test]
    fn test_query_analysis_score_calculation() {
        let mut analysis = QueryAnalysis {
            query: "SELECT * FROM users".to_string(),
            avg_time_ms: 6000.0,
            total_time_ms: 60000.0,
            calls: 10,
            avg_rows: 100.0,
            issues: vec![PerformanceIssue {
                issue_type: "Sequential Scan".to_string(),
                description: "Seq scan on users".to_string(),
                severity: 7,
                affected_object: Some("users".to_string()),
            }],
            suggestions: Vec::new(),
            performance_score: 100,
            analyzed_at: Utc::now(),
        };

        analysis.calculate_score();

        // Should be 100 - 30 (>5000ms) - 7 (severity) = 63
        assert_eq!(analysis.performance_score, 63);
    }

    #[test]
    fn test_performance_issue_serialization() {
        let issue = PerformanceIssue {
            issue_type: "Sequential Scan".to_string(),
            description: "Test description".to_string(),
            severity: 7,
            affected_object: Some("users".to_string()),
        };

        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("Sequential Scan"));
        assert!(json.contains("users"));
    }

    #[test]
    fn test_optimization_suggestion_serialization() {
        let suggestion = OptimizationSuggestion {
            suggestion_type: SuggestionType::AddIndex,
            description: "Add index on email".to_string(),
            sql: Some("CREATE INDEX idx_email ON users(email)".to_string()),
            expected_improvement: 0.7,
            priority: 8,
        };

        let json = serde_json::to_string(&suggestion).unwrap();
        assert!(json.contains("AddIndex"));
        assert!(json.contains("expected_improvement"));
    }

    #[test]
    fn test_report_summary_calculation() {
        let analyses = vec![
            QueryAnalysis {
                query: "SELECT 1".to_string(),
                avg_time_ms: 100.0,
                total_time_ms: 1000.0,
                calls: 10,
                avg_rows: 1.0,
                issues: vec![],
                suggestions: vec![],
                performance_score: 90,
                analyzed_at: Utc::now(),
            },
            QueryAnalysis {
                query: "SELECT 2".to_string(),
                avg_time_ms: 200.0,
                total_time_ms: 2000.0,
                calls: 10,
                avg_rows: 1.0,
                issues: vec![],
                suggestions: vec![],
                performance_score: 80,
                analyzed_at: Utc::now(),
            },
        ];

        let analyzer = QueryPerformanceAnalyzer::with_defaults();
        let summary = analyzer.calculate_summary(&analyses);

        assert_eq!(summary.avg_performance_score, 85.0);
        assert_eq!(summary.total_slow_query_time_ms, 3000.0);
    }

    #[test]
    fn test_analyzer_with_defaults() {
        let analyzer = QueryPerformanceAnalyzer::with_defaults();
        assert_eq!(analyzer.config.slow_query_threshold_ms, 1000);
    }

    #[test]
    fn test_query_analysis_with_issues() {
        let mut analysis = QueryAnalysis {
            query: "SELECT * FROM users WHERE email = 'test@example.com'".to_string(),
            avg_time_ms: 2500.0,
            total_time_ms: 25000.0,
            calls: 10,
            avg_rows: 1.0,
            issues: vec![PerformanceIssue {
                issue_type: "Sequential Scan".to_string(),
                description: "Sequential scan on users table".to_string(),
                severity: 7,
                affected_object: Some("users".to_string()),
            }],
            suggestions: Vec::new(),
            performance_score: 100,
            analyzed_at: Utc::now(),
        };

        let analyzer = QueryPerformanceAnalyzer::with_defaults();
        analyzer.generate_suggestions(&mut analysis);

        assert!(!analysis.suggestions.is_empty());
        assert!(analysis
            .suggestions
            .iter()
            .any(|s| s.suggestion_type == SuggestionType::AddIndex));
    }

    #[test]
    fn test_performance_report_serialization() {
        let report = PerformanceReport {
            generated_at: Utc::now(),
            total_queries_analyzed: 10,
            queries_with_issues: 5,
            analyses: vec![],
            summary: ReportSummary {
                total_suggestions: 15,
                avg_performance_score: 75.0,
                total_slow_query_time_ms: 50000.0,
                most_common_issue: Some("Sequential Scan".to_string()),
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("total_queries_analyzed"));
        assert!(json.contains("\"queries_with_issues\":5"));
    }
}
