//! Query optimization utilities for index recommendations, query analysis, and N+1 detection

use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Query execution statistics
#[derive(Debug, Clone)]
pub struct QueryStats {
    /// The SQL query
    pub query: String,
    /// Execution time
    pub execution_time: Duration,
    /// Number of rows returned
    pub rows_returned: u64,
    /// Number of rows scanned
    pub rows_scanned: u64,
    /// Whether an index was used
    pub index_used: bool,
    /// Table(s) accessed
    pub tables: Vec<String>,
}

impl QueryStats {
    /// Create new query stats
    pub fn new(query: String) -> Self {
        Self {
            query,
            execution_time: Duration::from_millis(0),
            rows_returned: 0,
            rows_scanned: 0,
            index_used: false,
            tables: Vec::new(),
        }
    }

    /// Calculate query efficiency (rows returned / rows scanned)
    pub fn efficiency(&self) -> f64 {
        if self.rows_scanned == 0 {
            return 1.0;
        }
        self.rows_returned as f64 / self.rows_scanned as f64
    }

    /// Check if query is potentially slow
    pub fn is_slow(&self, threshold: Duration) -> bool {
        self.execution_time > threshold
    }

    /// Check if query is doing a full table scan
    pub fn is_full_table_scan(&self) -> bool {
        !self.index_used && self.rows_scanned > 1000
    }
}

/// Index recommendation
#[derive(Debug, Clone)]
pub struct IndexRecommendation {
    /// Table name
    pub table: String,
    /// Columns to index
    pub columns: Vec<String>,
    /// Reason for the recommendation
    pub reason: String,
    /// Priority (1-5, where 5 is highest)
    pub priority: u8,
    /// Estimated performance improvement (percentage)
    pub estimated_improvement: f64,
}

impl IndexRecommendation {
    /// Create a new index recommendation
    pub fn new(
        table: impl Into<String>,
        columns: Vec<String>,
        reason: impl Into<String>,
        priority: u8,
    ) -> Self {
        Self {
            table: table.into(),
            columns,
            reason: reason.into(),
            priority,
            estimated_improvement: 0.0,
        }
    }

    /// Set estimated improvement
    pub fn with_estimated_improvement(mut self, improvement: f64) -> Self {
        self.estimated_improvement = improvement;
        self
    }

    /// Generate SQL to create the recommended index
    pub fn generate_sql(&self) -> String {
        let index_name = format!("idx_{}_{}", self.table, self.columns.join("_"));
        format!(
            "CREATE INDEX {} ON {} ({});",
            index_name,
            self.table,
            self.columns.join(", ")
        )
    }
}

/// Query analyzer for detecting performance issues
pub struct QueryAnalyzer {
    /// Query statistics collected
    stats: Vec<QueryStats>,
    /// Slow query threshold
    slow_query_threshold: Duration,
    /// Minimum rows scanned for full scan detection
    full_scan_threshold: u64,
}

impl QueryAnalyzer {
    /// Create a new query analyzer
    pub fn new() -> Self {
        Self {
            stats: Vec::new(),
            slow_query_threshold: Duration::from_millis(100),
            full_scan_threshold: 1000,
        }
    }

    /// Set slow query threshold
    pub fn with_slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.slow_query_threshold = threshold;
        self
    }

    /// Set full scan threshold
    pub fn with_full_scan_threshold(mut self, threshold: u64) -> Self {
        self.full_scan_threshold = threshold;
        self
    }

    /// Add query statistics
    pub fn add_stats(&mut self, stats: QueryStats) {
        self.stats.push(stats);
    }

    /// Get slow queries
    pub fn slow_queries(&self) -> Vec<&QueryStats> {
        self.stats
            .iter()
            .filter(|s| s.is_slow(self.slow_query_threshold))
            .collect()
    }

    /// Get queries doing full table scans
    pub fn full_table_scans(&self) -> Vec<&QueryStats> {
        self.stats
            .iter()
            .filter(|s| !s.index_used && s.rows_scanned > self.full_scan_threshold)
            .collect()
    }

    /// Generate index recommendations based on collected stats
    pub fn recommend_indexes(&self) -> Vec<IndexRecommendation> {
        let mut recommendations = Vec::new();

        // Analyze slow queries
        for stat in self.slow_queries() {
            if !stat.index_used {
                for table in &stat.tables {
                    // Extract columns from WHERE clauses (simplified)
                    let columns = self.extract_where_columns(&stat.query, table);

                    if !columns.is_empty() {
                        let rec = IndexRecommendation::new(
                            table.clone(),
                            columns,
                            format!("Slow query ({:?}), no index used", stat.execution_time),
                            5,
                        )
                        .with_estimated_improvement(50.0);

                        recommendations.push(rec);
                    }
                }
            }
        }

        // Analyze full table scans
        for stat in self.full_table_scans() {
            for table in &stat.tables {
                let columns = self.extract_where_columns(&stat.query, table);

                if !columns.is_empty() {
                    let rec = IndexRecommendation::new(
                        table.clone(),
                        columns,
                        format!("Full table scan ({} rows)", stat.rows_scanned),
                        4,
                    )
                    .with_estimated_improvement(70.0);

                    recommendations.push(rec);
                }
            }
        }

        // Remove duplicates
        self.deduplicate_recommendations(recommendations)
    }

    /// Extract column names from WHERE clauses (simplified implementation)
    fn extract_where_columns(&self, query: &str, _table: &str) -> Vec<String> {
        let mut columns = Vec::new();
        let query_lower = query.to_lowercase();

        // Look for common patterns like "WHERE column = " or "WHERE column IN"
        if let Some(where_pos) = query_lower.find("where") {
            let where_clause = &query_lower[where_pos..];

            // Simple heuristic: look for words followed by = or IN
            let parts: Vec<&str> = where_clause.split_whitespace().collect();
            for i in 0..parts.len() {
                if i + 1 < parts.len() {
                    let next = parts[i + 1];
                    if next.starts_with('=') || next == "in" || next == ">" || next == "<" {
                        let col = parts[i].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                        if !col.is_empty() && col != "where" && col != "and" && col != "or" {
                            columns.push(col.to_string());
                        }
                    }
                }
            }
        }

        columns
    }

    /// Remove duplicate recommendations
    fn deduplicate_recommendations(
        &self,
        recommendations: Vec<IndexRecommendation>,
    ) -> Vec<IndexRecommendation> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();

        for rec in recommendations {
            let key = format!("{}:{}", rec.table, rec.columns.join(","));
            if !seen.contains(&key) {
                seen.insert(key);
                result.push(rec);
            }
        }

        result
    }

    /// Get query statistics summary
    pub fn summary(&self) -> QueryAnalysisSummary {
        let total_queries = self.stats.len();
        let slow_queries = self.slow_queries().len();
        let full_scans = self.full_table_scans().len();

        let avg_execution_time = if !self.stats.is_empty() {
            self.stats
                .iter()
                .map(|s| s.execution_time.as_millis())
                .sum::<u128>()
                / self.stats.len() as u128
        } else {
            0
        };

        QueryAnalysisSummary {
            total_queries,
            slow_queries,
            full_table_scans: full_scans,
            avg_execution_time: Duration::from_millis(avg_execution_time as u64),
        }
    }
}

impl Default for QueryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of query analysis
#[derive(Debug, Clone)]
pub struct QueryAnalysisSummary {
    /// Total number of queries analyzed
    pub total_queries: usize,
    /// Number of slow queries
    pub slow_queries: usize,
    /// Number of full table scans
    pub full_table_scans: usize,
    /// Average execution time
    pub avg_execution_time: Duration,
}

/// N+1 query detector
pub struct NPlusOneDetector {
    /// Query patterns seen
    query_patterns: HashMap<String, usize>,
    /// Threshold for detecting N+1 (number of similar queries)
    threshold: usize,
}

impl NPlusOneDetector {
    /// Create a new N+1 detector
    pub fn new() -> Self {
        Self {
            query_patterns: HashMap::new(),
            threshold: 3,
        }
    }

    /// Set detection threshold
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }

    /// Record a query execution
    pub fn record_query(&mut self, query: &str) {
        let pattern = self.normalize_query(query);
        *self.query_patterns.entry(pattern).or_insert(0) += 1;
    }

    /// Detect potential N+1 queries
    pub fn detect(&self) -> Vec<NPlusOneProblem> {
        let mut problems = Vec::new();

        for (pattern, count) in &self.query_patterns {
            if *count >= self.threshold {
                problems.push(NPlusOneProblem {
                    query_pattern: pattern.clone(),
                    occurrence_count: *count,
                    suggestion: self.generate_suggestion(pattern),
                });
            }
        }

        // Sort by occurrence count (descending)
        problems.sort_by_key(|b| std::cmp::Reverse(b.occurrence_count));

        problems
    }

    /// Normalize a query to detect patterns
    fn normalize_query(&self, query: &str) -> String {
        // Replace specific IDs with placeholders
        let mut normalized = query.to_lowercase();

        // Replace UUIDs
        normalized =
            regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
                .expect("UUID regex pattern is valid")
                .replace_all(&normalized, "?")
                .to_string();

        // Replace numbers
        normalized = regex::Regex::new(r"\b\d+\b")
            .expect("digit regex pattern is valid")
            .replace_all(&normalized, "?")
            .to_string();

        // Replace string literals
        normalized = regex::Regex::new(r"'[^']*'")
            .expect("string literal regex pattern is valid")
            .replace_all(&normalized, "?")
            .to_string();

        normalized
    }

    /// Generate suggestion for fixing N+1 problem
    fn generate_suggestion(&self, pattern: &str) -> String {
        if pattern.contains("select") && pattern.contains("where") {
            "Consider using eager loading or batch fetching to reduce the number of queries"
        } else {
            "Consider optimizing this query pattern to reduce database round trips"
        }
        .to_string()
    }

    /// Clear recorded queries
    pub fn clear(&mut self) {
        self.query_patterns.clear();
    }

    /// Get statistics
    pub fn stats(&self) -> NPlusOneStats {
        let total_patterns = self.query_patterns.len();
        let total_queries: usize = self.query_patterns.values().sum();
        let problematic_patterns = self
            .query_patterns
            .iter()
            .filter(|(_, count)| **count >= self.threshold)
            .count();

        NPlusOneStats {
            total_patterns,
            total_queries,
            problematic_patterns,
        }
    }
}

impl Default for NPlusOneDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// N+1 query problem detected
#[derive(Debug, Clone)]
pub struct NPlusOneProblem {
    /// The normalized query pattern
    pub query_pattern: String,
    /// Number of times this query was executed
    pub occurrence_count: usize,
    /// Suggestion for fixing the problem
    pub suggestion: String,
}

/// N+1 detector statistics
#[derive(Debug, Clone)]
pub struct NPlusOneStats {
    /// Total unique query patterns
    pub total_patterns: usize,
    /// Total queries executed
    pub total_queries: usize,
    /// Number of problematic patterns
    pub problematic_patterns: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_stats_efficiency() {
        let mut stats = QueryStats::new("SELECT * FROM users".to_string());
        stats.rows_returned = 10;
        stats.rows_scanned = 100;

        assert_eq!(stats.efficiency(), 0.1);
    }

    #[test]
    fn test_query_stats_is_slow() {
        let mut stats = QueryStats::new("SELECT * FROM users".to_string());
        stats.execution_time = Duration::from_millis(150);

        assert!(stats.is_slow(Duration::from_millis(100)));
        assert!(!stats.is_slow(Duration::from_millis(200)));
    }

    #[test]
    fn test_index_recommendation_sql() {
        let rec = IndexRecommendation::new(
            "users",
            vec!["email".to_string(), "created_at".to_string()],
            "Slow query",
            5,
        );

        let sql = rec.generate_sql();
        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("users"));
        assert!(sql.contains("email"));
        assert!(sql.contains("created_at"));
    }

    #[test]
    fn test_query_analyzer_slow_queries() {
        let mut analyzer =
            QueryAnalyzer::new().with_slow_query_threshold(Duration::from_millis(100));

        let mut slow_stats = QueryStats::new("SELECT * FROM users".to_string());
        slow_stats.execution_time = Duration::from_millis(150);
        analyzer.add_stats(slow_stats);

        let mut fast_stats = QueryStats::new("SELECT * FROM tokens".to_string());
        fast_stats.execution_time = Duration::from_millis(50);
        analyzer.add_stats(fast_stats);

        let slow_queries = analyzer.slow_queries();
        assert_eq!(slow_queries.len(), 1);
        assert!(slow_queries[0].query.contains("users"));
    }

    #[test]
    fn test_n_plus_one_detector() {
        let mut detector = NPlusOneDetector::new().with_threshold(3);

        // Simulate N+1 problem
        detector.record_query("SELECT * FROM orders WHERE user_id = '123'");
        detector.record_query("SELECT * FROM orders WHERE user_id = '456'");
        detector.record_query("SELECT * FROM orders WHERE user_id = '789'");

        let problems = detector.detect();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].occurrence_count, 3);
    }

    #[test]
    fn test_n_plus_one_detector_stats() {
        let mut detector = NPlusOneDetector::new();

        detector.record_query("SELECT * FROM users WHERE id = 1");
        detector.record_query("SELECT * FROM users WHERE id = 2");
        detector.record_query("SELECT * FROM tokens WHERE id = 1");

        let stats = detector.stats();
        assert_eq!(stats.total_queries, 3);
        assert_eq!(stats.total_patterns, 2); // users and tokens
    }
}
