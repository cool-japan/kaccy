//! Table statistics and size utilities
//!
//! This module provides utilities for retrieving table statistics, sizes,
//! and usage information from PostgreSQL.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Comprehensive table statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    /// Schema name
    pub schema: String,
    /// Table name
    pub table_name: String,
    /// Total table size including indexes (bytes)
    pub total_size_bytes: i64,
    /// Table size excluding indexes (bytes)
    pub table_size_bytes: i64,
    /// Index size (bytes)
    pub index_size_bytes: i64,
    /// Approximate row count
    pub row_count: i64,
    /// Number of sequential scans
    pub seq_scans: i64,
    /// Number of index scans
    pub index_scans: i64,
    /// Number of live tuples
    pub live_tuples: i64,
    /// Number of dead tuples
    pub dead_tuples: i64,
    /// Last vacuum time
    pub last_vacuum: Option<DateTime<Utc>>,
    /// Last analyze time
    pub last_analyze: Option<DateTime<Utc>>,
}

/// Index usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    /// Schema name
    pub schema: String,
    /// Table name
    pub table_name: String,
    /// Index name
    pub index_name: String,
    /// Index size in bytes
    pub size_bytes: i64,
    /// Number of index scans
    pub scans: i64,
    /// Number of tuples read by index scans
    pub tuples_read: i64,
    /// Number of tuples fetched by index scans
    pub tuples_fetched: i64,
}

/// Database size summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSizeInfo {
    /// Database name
    pub database_name: String,
    /// Total database size in bytes
    pub size_bytes: i64,
    /// Human-readable size
    pub size_formatted: String,
    /// Number of tables
    pub table_count: i64,
    /// Number of indexes
    pub index_count: i64,
}

/// Get comprehensive statistics for a specific table
pub async fn get_table_stats(pool: &PgPool, table_name: &str) -> Result<Option<TableStats>> {
    let stats = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT
            schemaname,
            tablename,
            pg_total_relation_size(schemaname || '.' || tablename) as total_size,
            pg_relation_size(schemaname || '.' || tablename) as table_size,
            pg_total_relation_size(schemaname || '.' || tablename) -
                pg_relation_size(schemaname || '.' || tablename) as index_size,
            n_tup_ins + n_tup_upd + n_tup_del as row_count,
            seq_scan,
            idx_scan,
            n_live_tup,
            n_dead_tup,
            last_vacuum,
            last_analyze
        FROM pg_stat_user_tables
        WHERE tablename = $1
        "#,
    )
    .bind(table_name)
    .fetch_optional(pool)
    .await?;

    Ok(stats.map(|s| TableStats {
        schema: s.0,
        table_name: s.1,
        total_size_bytes: s.2,
        table_size_bytes: s.3,
        index_size_bytes: s.4,
        row_count: s.5,
        seq_scans: s.6,
        index_scans: s.7,
        live_tuples: s.8,
        dead_tuples: s.9,
        last_vacuum: s.10,
        last_analyze: s.11,
    }))
}

/// Get statistics for all tables, ordered by size
pub async fn get_all_table_stats(pool: &PgPool) -> Result<Vec<TableStats>> {
    let stats = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
        ),
    >(
        r#"
        SELECT
            schemaname,
            tablename,
            pg_total_relation_size(schemaname || '.' || tablename) as total_size,
            pg_relation_size(schemaname || '.' || tablename) as table_size,
            pg_total_relation_size(schemaname || '.' || tablename) -
                pg_relation_size(schemaname || '.' || tablename) as index_size,
            n_tup_ins + n_tup_upd + n_tup_del as row_count,
            seq_scan,
            idx_scan,
            n_live_tup,
            n_dead_tup,
            last_vacuum,
            last_analyze
        FROM pg_stat_user_tables
        ORDER BY total_size DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(stats
        .into_iter()
        .map(|s| TableStats {
            schema: s.0,
            table_name: s.1,
            total_size_bytes: s.2,
            table_size_bytes: s.3,
            index_size_bytes: s.4,
            row_count: s.5,
            seq_scans: s.6,
            index_scans: s.7,
            live_tuples: s.8,
            dead_tuples: s.9,
            last_vacuum: s.10,
            last_analyze: s.11,
        })
        .collect())
}

/// Get the largest tables in the database
pub async fn get_largest_tables(pool: &PgPool, limit: i32) -> Result<Vec<TableStats>> {
    let stats = get_all_table_stats(pool).await?;
    Ok(stats.into_iter().take(limit as usize).collect())
}

/// Get index statistics for a table
pub async fn get_table_index_stats(pool: &PgPool, table_name: &str) -> Result<Vec<IndexStats>> {
    let stats = sqlx::query_as::<_, (String, String, String, i64, i64, i64, i64)>(
        r#"
        SELECT
            schemaname,
            tablename,
            indexname,
            pg_relation_size(schemaname || '.' || indexname) as size,
            idx_scan,
            idx_tup_read,
            idx_tup_fetch
        FROM pg_stat_user_indexes
        WHERE tablename = $1
        ORDER BY idx_scan DESC
        "#,
    )
    .bind(table_name)
    .fetch_all(pool)
    .await?;

    Ok(stats
        .into_iter()
        .map(|s| IndexStats {
            schema: s.0,
            table_name: s.1,
            index_name: s.2,
            size_bytes: s.3,
            scans: s.4,
            tuples_read: s.5,
            tuples_fetched: s.6,
        })
        .collect())
}

/// Get unused indexes (indexes with zero scans)
pub async fn get_unused_indexes(pool: &PgPool) -> Result<Vec<IndexStats>> {
    let stats = sqlx::query_as::<_, (String, String, String, i64, i64, i64, i64)>(
        r#"
        SELECT
            schemaname,
            tablename,
            indexname,
            pg_relation_size(schemaname || '.' || indexname) as size,
            idx_scan,
            idx_tup_read,
            idx_tup_fetch
        FROM pg_stat_user_indexes
        WHERE idx_scan = 0
        AND indexname NOT LIKE '%_pkey'
        ORDER BY size DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(stats
        .into_iter()
        .map(|s| IndexStats {
            schema: s.0,
            table_name: s.1,
            index_name: s.2,
            size_bytes: s.3,
            scans: s.4,
            tuples_read: s.5,
            tuples_fetched: s.6,
        })
        .collect())
}

/// Get database size information
pub async fn get_database_size(pool: &PgPool) -> Result<DatabaseSizeInfo> {
    let (db_name, size_bytes): (String, i64) = sqlx::query_as(
        r#"
        SELECT
            current_database(),
            pg_database_size(current_database())
        "#,
    )
    .fetch_one(pool)
    .await?;

    let table_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_tables
        WHERE schemaname = 'public'
        "#,
    )
    .fetch_one(pool)
    .await?;

    let index_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_indexes
        WHERE schemaname = 'public'
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(DatabaseSizeInfo {
        database_name: db_name,
        size_bytes,
        size_formatted: crate::helpers::format_bytes(size_bytes as u64),
        table_count,
        index_count,
    })
}

/// Get tables with highest sequential scan ratio
///
/// These tables might benefit from adding indexes.
pub async fn get_high_seq_scan_tables(pool: &PgPool, min_scans: i64) -> Result<Vec<SeqScanInfo>> {
    let tables = sqlx::query_as::<_, (String, i64, i64, f64)>(
        r#"
        SELECT
            tablename,
            seq_scan,
            idx_scan,
            CASE
                WHEN (seq_scan + idx_scan) > 0
                THEN (seq_scan::float / (seq_scan + idx_scan)::float) * 100
                ELSE 0
            END as seq_scan_percent
        FROM pg_stat_user_tables
        WHERE seq_scan > $1
        ORDER BY seq_scan_percent DESC, seq_scan DESC
        "#,
    )
    .bind(min_scans)
    .fetch_all(pool)
    .await?;

    Ok(tables
        .into_iter()
        .map(|t| SeqScanInfo {
            table_name: t.0,
            seq_scans: t.1,
            index_scans: t.2,
            seq_scan_percent: t.3,
        })
        .collect())
}

/// Sequential scan information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqScanInfo {
    /// Table name
    pub table_name: String,
    /// Number of sequential scans
    pub seq_scans: i64,
    /// Number of index scans
    pub index_scans: i64,
    /// Percentage of scans that were sequential
    pub seq_scan_percent: f64,
}

/// Get cache hit ratio for tables
pub async fn get_table_cache_hit_ratio(pool: &PgPool) -> Result<Vec<CacheHitInfo>> {
    let tables = sqlx::query_as::<_, (String, i64, i64, f64)>(
        r#"
        SELECT
            tablename,
            heap_blks_read,
            heap_blks_hit,
            CASE
                WHEN (heap_blks_read + heap_blks_hit) > 0
                THEN (heap_blks_hit::float / (heap_blks_read + heap_blks_hit)::float) * 100
                ELSE 0
            END as cache_hit_ratio
        FROM pg_statio_user_tables
        WHERE (heap_blks_read + heap_blks_hit) > 0
        ORDER BY cache_hit_ratio ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(tables
        .into_iter()
        .map(|t| CacheHitInfo {
            table_name: t.0,
            blocks_read: t.1,
            blocks_hit: t.2,
            hit_ratio_percent: t.3,
        })
        .collect())
}

/// Cache hit ratio information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHitInfo {
    /// Table name
    pub table_name: String,
    /// Number of blocks read from disk
    pub blocks_read: i64,
    /// Number of blocks found in cache
    pub blocks_hit: i64,
    /// Cache hit ratio percentage
    pub hit_ratio_percent: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_stats_structure() {
        let stats = TableStats {
            schema: "public".to_string(),
            table_name: "users".to_string(),
            total_size_bytes: 1_048_576,
            table_size_bytes: 524_288,
            index_size_bytes: 524_288,
            row_count: 10000,
            seq_scans: 100,
            index_scans: 1000,
            live_tuples: 9500,
            dead_tuples: 500,
            last_vacuum: None,
            last_analyze: None,
        };

        assert_eq!(stats.table_name, "users");
        assert_eq!(stats.row_count, 10000);
    }

    #[test]
    fn test_index_stats_structure() {
        let stats = IndexStats {
            schema: "public".to_string(),
            table_name: "users".to_string(),
            index_name: "users_email_idx".to_string(),
            size_bytes: 262_144,
            scans: 500,
            tuples_read: 5000,
            tuples_fetched: 4500,
        };

        assert_eq!(stats.index_name, "users_email_idx");
        assert_eq!(stats.scans, 500);
    }

    #[test]
    fn test_database_size_info_structure() {
        let info = DatabaseSizeInfo {
            database_name: "mydb".to_string(),
            size_bytes: 10_485_760,
            size_formatted: "10.00 MB".to_string(),
            table_count: 15,
            index_count: 25,
        };

        assert_eq!(info.database_name, "mydb");
        assert_eq!(info.table_count, 15);
    }

    #[test]
    fn test_seq_scan_info_structure() {
        let info = SeqScanInfo {
            table_name: "orders".to_string(),
            seq_scans: 1000,
            index_scans: 100,
            seq_scan_percent: 90.9,
        };

        assert_eq!(info.seq_scans, 1000);
        assert!(info.seq_scan_percent > 90.0);
    }

    #[test]
    fn test_cache_hit_info_structure() {
        let info = CacheHitInfo {
            table_name: "products".to_string(),
            blocks_read: 100,
            blocks_hit: 900,
            hit_ratio_percent: 90.0,
        };

        assert_eq!(info.hit_ratio_percent, 90.0);
    }

    #[test]
    fn test_table_stats_serialization() {
        let stats = TableStats {
            schema: "public".to_string(),
            table_name: "test".to_string(),
            total_size_bytes: 1024,
            table_size_bytes: 512,
            index_size_bytes: 512,
            row_count: 100,
            seq_scans: 10,
            index_scans: 50,
            live_tuples: 95,
            dead_tuples: 5,
            last_vacuum: None,
            last_analyze: None,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: TableStats = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.table_name, stats.table_name);
        assert_eq!(deserialized.row_count, stats.row_count);
    }
}
