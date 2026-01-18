//! Database lock monitoring and deadlock detection utilities
//!
//! This module provides utilities for monitoring database locks, detecting
//! deadlocks, and identifying blocking queries.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Type of database lock
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LockType {
    /// Access Share lock
    AccessShare,
    /// Row Share lock
    RowShare,
    /// Row Exclusive lock
    RowExclusive,
    /// Share Update Exclusive lock
    ShareUpdateExclusive,
    /// Share lock
    Share,
    /// Share Row Exclusive lock
    ShareRowExclusive,
    /// Exclusive lock
    Exclusive,
    /// Access Exclusive lock
    AccessExclusive,
    /// Unknown lock type
    Unknown(String),
}

impl From<String> for LockType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "AccessShareLock" => LockType::AccessShare,
            "RowShareLock" => LockType::RowShare,
            "RowExclusiveLock" => LockType::RowExclusive,
            "ShareUpdateExclusiveLock" => LockType::ShareUpdateExclusive,
            "ShareLock" => LockType::Share,
            "ShareRowExclusiveLock" => LockType::ShareRowExclusive,
            "ExclusiveLock" => LockType::Exclusive,
            "AccessExclusiveLock" => LockType::AccessExclusive,
            _ => LockType::Unknown(s),
        }
    }
}

/// Information about a database lock
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    /// Process ID holding the lock
    pub pid: i32,
    /// Lock type
    pub lock_type: LockType,
    /// Database name
    pub database: String,
    /// Relation (table) name if applicable
    pub relation: Option<String>,
    /// Whether the lock is granted
    pub granted: bool,
    /// When the lock was acquired
    pub lock_acquired: Option<DateTime<Utc>>,
}

/// Information about a blocking query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingQuery {
    /// Blocking process ID
    pub blocking_pid: i32,
    /// Blocked process ID
    pub blocked_pid: i32,
    /// Blocking query text
    pub blocking_query: String,
    /// Blocked query text
    pub blocked_query: String,
    /// Lock type being waited for
    pub lock_type: LockType,
    /// Table being locked
    pub table_name: Option<String>,
    /// Duration the query has been blocked (ms)
    pub blocked_duration_ms: Option<i64>,
}

/// Deadlock detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockInfo {
    /// Processes involved in the deadlock
    pub involved_pids: Vec<i32>,
    /// Lock cycle description
    pub cycle_description: String,
    /// When the deadlock was detected
    pub detected_at: DateTime<Utc>,
}

/// Get all current locks in the database
pub async fn get_current_locks(pool: &PgPool) -> Result<Vec<LockInfo>> {
    let locks = sqlx::query_as::<_, (i32, String, String, Option<String>, bool)>(
        r#"
        SELECT
            l.pid,
            l.mode as lock_type,
            d.datname as database,
            c.relname as relation,
            l.granted
        FROM pg_locks l
        LEFT JOIN pg_database d ON l.database = d.oid
        LEFT JOIN pg_class c ON l.relation = c.oid
        WHERE l.pid != pg_backend_pid()
        ORDER BY l.pid
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(locks
        .into_iter()
        .map(|l| LockInfo {
            pid: l.0,
            lock_type: LockType::from(l.1),
            database: l.2,
            relation: l.3,
            granted: l.4,
            lock_acquired: None,
        })
        .collect())
}

/// Get blocking queries
///
/// Returns information about queries that are blocking other queries from proceeding.
pub async fn get_blocking_queries(pool: &PgPool) -> Result<Vec<BlockingQuery>> {
    let blocking = sqlx::query_as::<
        _,
        (
            i32,
            i32,
            String,
            String,
            String,
            Option<String>,
            Option<DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT
            blocking.pid AS blocking_pid,
            blocked.pid AS blocked_pid,
            blocking_activity.query AS blocking_query,
            blocked_activity.query AS blocked_query,
            blocked_lock.mode AS lock_type,
            c.relname AS table_name,
            blocked_activity.query_start
        FROM pg_locks blocked_lock
        JOIN pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_lock.pid
        JOIN pg_locks blocking_lock ON blocking_lock.locktype = blocked_lock.locktype
            AND blocking_lock.database IS NOT DISTINCT FROM blocked_lock.database
            AND blocking_lock.relation IS NOT DISTINCT FROM blocked_lock.relation
            AND blocking_lock.page IS NOT DISTINCT FROM blocked_lock.page
            AND blocking_lock.tuple IS NOT DISTINCT FROM blocked_lock.tuple
            AND blocking_lock.virtualxid IS NOT DISTINCT FROM blocked_lock.virtualxid
            AND blocking_lock.transactionid IS NOT DISTINCT FROM blocked_lock.transactionid
            AND blocking_lock.classid IS NOT DISTINCT FROM blocked_lock.classid
            AND blocking_lock.objid IS NOT DISTINCT FROM blocked_lock.objid
            AND blocking_lock.objsubid IS NOT DISTINCT FROM blocked_lock.objsubid
            AND blocking_lock.pid != blocked_lock.pid
        JOIN pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_lock.pid
        LEFT JOIN pg_class c ON c.oid = blocked_lock.relation
        WHERE NOT blocked_lock.granted
        AND blocking_lock.granted
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(blocking
        .into_iter()
        .map(|b| {
            let blocked_duration_ms = b.6.map(|start| {
                let now = Utc::now();
                now.signed_duration_since(start).num_milliseconds()
            });

            BlockingQuery {
                blocking_pid: b.0,
                blocked_pid: b.1,
                blocking_query: b.2,
                blocked_query: b.3,
                lock_type: LockType::from(b.4),
                table_name: b.5,
                blocked_duration_ms,
            }
        })
        .collect())
}

/// Check for potential deadlocks
///
/// Analyzes lock wait chains to identify potential deadlock situations.
pub async fn detect_deadlocks(pool: &PgPool) -> Result<Vec<DeadlockInfo>> {
    let blocking_queries = get_blocking_queries(pool).await?;

    let mut deadlocks = Vec::new();
    let mut checked_pids = std::collections::HashSet::new();

    for query in &blocking_queries {
        if checked_pids.contains(&query.blocked_pid) {
            continue;
        }

        // Check if there's a circular dependency
        let mut chain = vec![query.blocked_pid];
        let mut current_pid = query.blocking_pid;
        let mut cycle_found = false;

        while let Some(blocker) = blocking_queries
            .iter()
            .find(|q| q.blocked_pid == current_pid)
        {
            if chain.contains(&blocker.blocking_pid) {
                // Found a cycle - this is a deadlock
                cycle_found = true;
                chain.push(blocker.blocking_pid);
                break;
            }

            chain.push(blocker.blocking_pid);
            current_pid = blocker.blocking_pid;

            if chain.len() > 100 {
                // Safety check to prevent infinite loops
                break;
            }
        }

        if cycle_found {
            for pid in &chain {
                checked_pids.insert(*pid);
            }

            deadlocks.push(DeadlockInfo {
                involved_pids: chain.clone(),
                cycle_description: format!(
                    "Deadlock cycle detected: {}",
                    chain
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ),
                detected_at: Utc::now(),
            });
        }
    }

    Ok(deadlocks)
}

/// Kill a blocking query by its process ID
///
/// WARNING: This terminates the backend process. Use with caution.
pub async fn kill_blocking_query(pool: &PgPool, pid: i32) -> Result<bool> {
    let result = sqlx::query_scalar::<_, bool>("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(pool)
        .await?;

    Ok(result)
}

/// Get lock wait statistics
///
/// Returns summary statistics about lock waits in the database.
pub async fn get_lock_wait_stats(pool: &PgPool) -> Result<LockWaitStats> {
    let blocking_queries = get_blocking_queries(pool).await?;

    let total_blocked = blocking_queries.len();
    let max_wait_time = blocking_queries
        .iter()
        .filter_map(|q| q.blocked_duration_ms)
        .max()
        .unwrap_or(0);

    let avg_wait_time = if total_blocked > 0 {
        blocking_queries
            .iter()
            .filter_map(|q| q.blocked_duration_ms)
            .sum::<i64>()
            / total_blocked as i64
    } else {
        0
    };

    // Count unique tables involved
    let mut tables = std::collections::HashSet::new();
    for query in &blocking_queries {
        if let Some(ref table) = query.table_name {
            tables.insert(table.clone());
        }
    }

    Ok(LockWaitStats {
        total_blocked_queries: total_blocked,
        max_wait_time_ms: max_wait_time,
        avg_wait_time_ms: avg_wait_time,
        affected_tables: tables.into_iter().collect(),
    })
}

/// Lock wait statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockWaitStats {
    /// Total number of blocked queries
    pub total_blocked_queries: usize,
    /// Maximum wait time in milliseconds
    pub max_wait_time_ms: i64,
    /// Average wait time in milliseconds
    pub avg_wait_time_ms: i64,
    /// List of affected tables
    pub affected_tables: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_type_from_string() {
        assert_eq!(
            LockType::from("AccessShareLock".to_string()),
            LockType::AccessShare
        );
        assert_eq!(
            LockType::from("RowExclusiveLock".to_string()),
            LockType::RowExclusive
        );
        assert_eq!(
            LockType::from("ExclusiveLock".to_string()),
            LockType::Exclusive
        );
    }

    #[test]
    fn test_lock_type_unknown() {
        match LockType::from("CustomLock".to_string()) {
            LockType::Unknown(s) => assert_eq!(s, "CustomLock"),
            _ => panic!("Expected Unknown variant"),
        }
    }

    #[test]
    fn test_lock_info_structure() {
        let info = LockInfo {
            pid: 12345,
            lock_type: LockType::RowExclusive,
            database: "mydb".to_string(),
            relation: Some("users".to_string()),
            granted: true,
            lock_acquired: None,
        };

        assert_eq!(info.pid, 12345);
        assert!(info.granted);
    }

    #[test]
    fn test_blocking_query_structure() {
        let query = BlockingQuery {
            blocking_pid: 100,
            blocked_pid: 200,
            blocking_query: "UPDATE users SET ...".to_string(),
            blocked_query: "SELECT * FROM users".to_string(),
            lock_type: LockType::RowExclusive,
            table_name: Some("users".to_string()),
            blocked_duration_ms: Some(5000),
        };

        assert_eq!(query.blocking_pid, 100);
        assert_eq!(query.blocked_duration_ms, Some(5000));
    }

    #[test]
    fn test_deadlock_info_structure() {
        let info = DeadlockInfo {
            involved_pids: vec![100, 200, 300],
            cycle_description: "100 -> 200 -> 300 -> 100".to_string(),
            detected_at: Utc::now(),
        };

        assert_eq!(info.involved_pids.len(), 3);
    }

    #[test]
    fn test_lock_wait_stats_structure() {
        let stats = LockWaitStats {
            total_blocked_queries: 5,
            max_wait_time_ms: 10000,
            avg_wait_time_ms: 5000,
            affected_tables: vec!["users".to_string(), "orders".to_string()],
        };

        assert_eq!(stats.total_blocked_queries, 5);
        assert_eq!(stats.affected_tables.len(), 2);
    }

    #[test]
    fn test_lock_type_serialization() {
        let lock_type = LockType::Exclusive;
        let json = serde_json::to_string(&lock_type).unwrap();
        let deserialized: LockType = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, lock_type);
    }

    #[test]
    fn test_lock_info_serialization() {
        let info = LockInfo {
            pid: 999,
            lock_type: LockType::Share,
            database: "testdb".to_string(),
            relation: None,
            granted: false,
            lock_acquired: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: LockInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.pid, info.pid);
        assert_eq!(deserialized.granted, info.granted);
    }
}
