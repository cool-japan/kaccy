//! Query execution plan analysis utilities
//!
//! This module provides utilities for analyzing PostgreSQL query execution plans
//! to identify performance issues and optimization opportunities.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Query execution plan node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlanNode {
    /// Node type (e.g., "Seq Scan", "Index Scan", "Hash Join")
    pub node_type: String,
    /// Relation/table name if applicable
    pub relation: Option<String>,
    /// Startup cost estimate
    pub startup_cost: f64,
    /// Total cost estimate
    pub total_cost: f64,
    /// Estimated number of rows
    pub rows: i64,
    /// Estimated width of rows in bytes
    pub width: i32,
    /// Actual time to get first row (if EXPLAIN ANALYZE)
    pub actual_startup_time: Option<f64>,
    /// Actual total time (if EXPLAIN ANALYZE)
    pub actual_total_time: Option<f64>,
    /// Actual number of rows (if EXPLAIN ANALYZE)
    pub actual_rows: Option<i64>,
}

/// Query execution plan analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    /// The SQL query that was analyzed
    pub query: String,
    /// Total estimated cost
    pub total_cost: f64,
    /// Estimated number of rows
    pub estimated_rows: i64,
    /// Planning time in milliseconds (if EXPLAIN ANALYZE)
    pub planning_time: Option<f64>,
    /// Execution time in milliseconds (if EXPLAIN ANALYZE)
    pub execution_time: Option<f64>,
    /// Full plan text
    pub plan_text: String,
    /// Performance warnings
    pub warnings: Vec<String>,
}

/// Analyze a query's execution plan without actually executing it
///
/// Uses EXPLAIN to show the query plan. Useful for analyzing expensive queries
/// before running them.
pub async fn explain_query(pool: &PgPool, query: &str) -> Result<QueryPlan> {
    let explain_query = format!("EXPLAIN (FORMAT JSON, VERBOSE) {}", query);

    let plan_json: serde_json::Value = sqlx::query_scalar(&explain_query).fetch_one(pool).await?;

    parse_plan(query, &plan_json, false)
}

/// Analyze a query's execution plan by actually running it
///
/// Uses EXPLAIN ANALYZE to show the actual query execution statistics.
/// WARNING: This will execute the query, so use with caution on
/// INSERT/UPDATE/DELETE queries. Consider wrapping in a transaction
/// that you roll back.
pub async fn explain_analyze_query(pool: &PgPool, query: &str) -> Result<QueryPlan> {
    let explain_query = format!("EXPLAIN (ANALYZE, FORMAT JSON, VERBOSE, BUFFERS) {}", query);

    let plan_json: serde_json::Value = sqlx::query_scalar(&explain_query).fetch_one(pool).await?;

    parse_plan(query, &plan_json, true)
}

/// Parse the JSON plan output from PostgreSQL
fn parse_plan(query: &str, plan_json: &serde_json::Value, is_analyze: bool) -> Result<QueryPlan> {
    let plan_array = plan_json.as_array().ok_or_else(|| {
        crate::error::DbError::Other("Invalid plan JSON: expected array".to_string())
    })?;

    let plan = plan_array.first().ok_or_else(|| {
        crate::error::DbError::Other("Invalid plan JSON: empty array".to_string())
    })?;

    let plan_obj = plan
        .get("Plan")
        .ok_or_else(|| crate::error::DbError::Other("No Plan object found".to_string()))?;

    let total_cost = plan_obj
        .get("Total Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let estimated_rows = plan_obj
        .get("Plan Rows")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let planning_time = if is_analyze {
        plan.get("Planning Time").and_then(|v| v.as_f64())
    } else {
        None
    };

    let execution_time = if is_analyze {
        plan.get("Execution Time").and_then(|v| v.as_f64())
    } else {
        None
    };

    // Convert plan to formatted text
    let plan_text = serde_json::to_string_pretty(plan)
        .map_err(|e| crate::error::DbError::Other(format!("Failed to serialize plan: {}", e)))?;

    // Generate warnings based on plan characteristics
    let mut warnings = Vec::new();

    if let Some(node_type) = plan_obj.get("Node Type").and_then(|v| v.as_str()) {
        if node_type.contains("Seq Scan") {
            warnings.push("Sequential scan detected - consider adding an index".to_string());
        }
    }

    if total_cost > 1000.0 {
        warnings.push(format!(
            "High estimated cost ({:.2}) - query may be expensive",
            total_cost
        ));
    }

    if let Some(exec_time) = execution_time {
        if exec_time > 1000.0 {
            warnings.push(format!("Slow query: execution took {:.2}ms", exec_time));
        }
    }

    Ok(QueryPlan {
        query: query.to_string(),
        total_cost,
        estimated_rows,
        planning_time,
        execution_time,
        plan_text,
        warnings,
    })
}

/// Get the top N most expensive queries from pg_stat_statements
///
/// Requires the pg_stat_statements extension to be installed.
/// Returns queries sorted by total execution time.
pub async fn get_expensive_queries(pool: &PgPool, limit: i64) -> Result<Vec<ExpensiveQuery>> {
    let queries = sqlx::query_as::<_, (String, i64, f64, f64, f64)>(
        r#"
        SELECT
            query,
            calls,
            total_exec_time,
            mean_exec_time,
            stddev_exec_time
        FROM pg_stat_statements
        WHERE query NOT LIKE '%pg_stat_statements%'
        ORDER BY total_exec_time DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await;

    match queries {
        Ok(rows) => Ok(rows
            .into_iter()
            .map(|r| ExpensiveQuery {
                query: r.0,
                calls: r.1,
                total_time: r.2,
                mean_time: r.3,
                stddev_time: r.4,
            })
            .collect()),
        Err(_) => {
            // pg_stat_statements might not be installed
            Ok(Vec::new())
        }
    }
}

/// Information about an expensive query from pg_stat_statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpensiveQuery {
    /// The query text
    pub query: String,
    /// Number of times executed
    pub calls: i64,
    /// Total execution time in milliseconds
    pub total_time: f64,
    /// Mean execution time in milliseconds
    pub mean_time: f64,
    /// Standard deviation of execution time
    pub stddev_time: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_plan_structure() {
        let plan = QueryPlan {
            query: "SELECT * FROM users".to_string(),
            total_cost: 100.0,
            estimated_rows: 1000,
            planning_time: Some(0.5),
            execution_time: Some(10.0),
            plan_text: "{}".to_string(),
            warnings: vec!["Sequential scan".to_string()],
        };

        assert_eq!(plan.total_cost, 100.0);
        assert_eq!(plan.warnings.len(), 1);
    }

    #[test]
    fn test_expensive_query_structure() {
        let query = ExpensiveQuery {
            query: "SELECT * FROM large_table".to_string(),
            calls: 1000,
            total_time: 5000.0,
            mean_time: 5.0,
            stddev_time: 1.0,
        };

        assert_eq!(query.calls, 1000);
        assert_eq!(query.mean_time, 5.0);
    }

    #[test]
    fn test_query_plan_serialization() {
        let plan = QueryPlan {
            query: "SELECT id FROM users".to_string(),
            total_cost: 50.0,
            estimated_rows: 100,
            planning_time: None,
            execution_time: None,
            plan_text: "test".to_string(),
            warnings: vec![],
        };

        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: QueryPlan = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.query, plan.query);
        assert_eq!(deserialized.total_cost, plan.total_cost);
    }

    #[test]
    fn test_query_plan_node_structure() {
        let node = QueryPlanNode {
            node_type: "Index Scan".to_string(),
            relation: Some("users".to_string()),
            startup_cost: 0.0,
            total_cost: 10.0,
            rows: 100,
            width: 32,
            actual_startup_time: Some(0.1),
            actual_total_time: Some(1.0),
            actual_rows: Some(95),
        };

        assert_eq!(node.node_type, "Index Scan");
        assert_eq!(node.relation, Some("users".to_string()));
        assert_eq!(node.rows, 100);
    }

    #[test]
    fn test_expensive_query_serialization() {
        let query = ExpensiveQuery {
            query: "SELECT * FROM orders".to_string(),
            calls: 500,
            total_time: 2500.0,
            mean_time: 5.0,
            stddev_time: 0.5,
        };

        let json = serde_json::to_string(&query).unwrap();
        let deserialized: ExpensiveQuery = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.calls, query.calls);
        assert_eq!(deserialized.total_time, query.total_time);
    }
}
