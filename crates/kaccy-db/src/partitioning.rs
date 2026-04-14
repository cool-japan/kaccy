//! Database partitioning support for PostgreSQL
//!
//! Provides table partitioning strategies (range, list, hash),
//! partition pruning optimization, and automatic partition management.

use crate::error::{DbError, Result};
use chrono::{DateTime, Datelike, Utc};
use sqlx::PgPool;
use std::fmt;

/// Partitioning strategy
#[derive(Debug, Clone, PartialEq)]
pub enum PartitioningStrategy {
    /// Range partitioning (e.g., by date ranges)
    Range {
        /// Column used as the partition key.
        column: String,
    },
    /// List partitioning (e.g., by specific values)
    List {
        /// Column used as the partition key.
        column: String,
    },
    /// Hash partitioning (e.g., for even distribution)
    Hash {
        /// Column used as the partition key.
        column: String,
        /// Number of hash partitions to create.
        num_partitions: usize,
    },
}

/// Time-based partition interval
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartitionInterval {
    /// Partition by day
    Daily,
    /// Partition by week
    Weekly,
    /// Partition by month
    Monthly,
    /// Partition by quarter
    Quarterly,
    /// Partition by year
    Yearly,
}

impl PartitionInterval {
    /// Get partition suffix for a given date
    pub fn partition_suffix(&self, date: DateTime<Utc>) -> String {
        match self {
            Self::Daily => date.format("%Y%m%d").to_string(),
            Self::Weekly => {
                let week = date.iso_week().week();
                format!("{}w{:02}", date.year(), week)
            }
            Self::Monthly => date.format("%Y%m").to_string(),
            Self::Quarterly => {
                let quarter = (date.month() - 1) / 3 + 1;
                format!("{}q{}", date.year(), quarter)
            }
            Self::Yearly => date.format("%Y").to_string(),
        }
    }

    /// Get start and end dates for partition
    pub fn partition_bounds(&self, date: DateTime<Utc>) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
        use chrono::{Datelike, Duration, NaiveDate};

        match self {
            Self::Daily => {
                let start = date.date_naive().and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid daily partition start time".to_string())
                })?;
                let end = start + Duration::days(1);
                Ok((
                    DateTime::from_naive_utc_and_offset(start, Utc),
                    DateTime::from_naive_utc_and_offset(end, Utc),
                ))
            }
            Self::Weekly => {
                let days_from_monday = date.weekday().num_days_from_monday();
                let start_date = date.date_naive() - Duration::days(days_from_monday as i64);
                let start = start_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid weekly partition start time".to_string())
                })?;
                let end = start + Duration::weeks(1);
                Ok((
                    DateTime::from_naive_utc_and_offset(start, Utc),
                    DateTime::from_naive_utc_and_offset(end, Utc),
                ))
            }
            Self::Monthly => {
                let start_date =
                    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).ok_or_else(|| {
                        DbError::Other(format!(
                            "Invalid monthly partition start date: {}-{:02}-01",
                            date.year(),
                            date.month()
                        ))
                    })?;
                let start = start_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid monthly partition start time".to_string())
                })?;
                let next_month = if date.month() == 12 {
                    NaiveDate::from_ymd_opt(date.year() + 1, 1, 1).ok_or_else(|| {
                        DbError::Other(format!(
                            "Invalid monthly partition end date: {}-01-01",
                            date.year() + 1
                        ))
                    })?
                } else {
                    NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1).ok_or_else(|| {
                        DbError::Other(format!(
                            "Invalid monthly partition end date: {}-{:02}-01",
                            date.year(),
                            date.month() + 1
                        ))
                    })?
                };
                let end = next_month.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid monthly partition end time".to_string())
                })?;
                Ok((
                    DateTime::from_naive_utc_and_offset(start, Utc),
                    DateTime::from_naive_utc_and_offset(end, Utc),
                ))
            }
            Self::Quarterly => {
                let quarter = (date.month() - 1) / 3 + 1;
                let start_month = (quarter - 1) * 3 + 1;
                let start_date =
                    NaiveDate::from_ymd_opt(date.year(), start_month, 1).ok_or_else(|| {
                        DbError::Other(format!(
                            "Invalid quarterly partition start date: {}-{:02}-01",
                            date.year(),
                            start_month
                        ))
                    })?;
                let start = start_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid quarterly partition start time".to_string())
                })?;

                let end_month = start_month + 3;
                let (end_year, end_month) = if end_month > 12 {
                    (date.year() + 1, end_month - 12)
                } else {
                    (date.year(), end_month)
                };
                let end_date =
                    NaiveDate::from_ymd_opt(end_year, end_month, 1).ok_or_else(|| {
                        DbError::Other(format!(
                            "Invalid quarterly partition end date: {}-{:02}-01",
                            end_year, end_month
                        ))
                    })?;
                let end = end_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid quarterly partition end time".to_string())
                })?;

                Ok((
                    DateTime::from_naive_utc_and_offset(start, Utc),
                    DateTime::from_naive_utc_and_offset(end, Utc),
                ))
            }
            Self::Yearly => {
                let start_date = NaiveDate::from_ymd_opt(date.year(), 1, 1).ok_or_else(|| {
                    DbError::Other(format!(
                        "Invalid yearly partition start date: {}-01-01",
                        date.year()
                    ))
                })?;
                let start = start_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid yearly partition start time".to_string())
                })?;
                let end_date = NaiveDate::from_ymd_opt(date.year() + 1, 1, 1).ok_or_else(|| {
                    DbError::Other(format!(
                        "Invalid yearly partition end date: {}-01-01",
                        date.year() + 1
                    ))
                })?;
                let end = end_date.and_hms_opt(0, 0, 0).ok_or_else(|| {
                    DbError::Other("Invalid yearly partition end time".to_string())
                })?;
                Ok((
                    DateTime::from_naive_utc_and_offset(start, Utc),
                    DateTime::from_naive_utc_and_offset(end, Utc),
                ))
            }
        }
    }
}

/// Partition definition
#[derive(Debug, Clone)]
pub struct PartitionDefinition {
    /// Name of the parent partitioned table.
    pub table_name: String,
    /// Name of this specific partition.
    pub partition_name: String,
    /// Partitioning strategy applied.
    pub strategy: PartitioningStrategy,
}

impl PartitionDefinition {
    /// Generate SQL for creating partition
    pub fn create_sql(&self) -> String {
        match &self.strategy {
            PartitioningStrategy::Range { column: _ } => {
                format!(
                    "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM (...) TO (...)",
                    self.partition_name, self.table_name
                )
            }
            PartitioningStrategy::List { column: _ } => {
                format!(
                    "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES IN (...)",
                    self.partition_name, self.table_name
                )
            }
            PartitioningStrategy::Hash {
                column: _,
                num_partitions,
            } => {
                format!(
                    "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES WITH (MODULUS {}, REMAINDER ...)",
                    self.partition_name, self.table_name, num_partitions
                )
            }
        }
    }

    /// Generate SQL for dropping partition
    pub fn drop_sql(&self) -> String {
        format!("DROP TABLE IF EXISTS {}", self.partition_name)
    }
}

/// Partition manager for automatic partition management
pub struct PartitionManager {
    pool: PgPool,
}

impl PartitionManager {
    /// Create a new partition manager
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create partitioned table
    pub async fn create_partitioned_table(
        &self,
        _table_name: &str,
        strategy: &PartitioningStrategy,
        create_table_sql: &str,
    ) -> Result<()> {
        let partition_clause = match strategy {
            PartitioningStrategy::Range { column } => {
                format!("PARTITION BY RANGE ({})", column)
            }
            PartitioningStrategy::List { column } => {
                format!("PARTITION BY LIST ({})", column)
            }
            PartitioningStrategy::Hash { column, .. } => {
                format!("PARTITION BY HASH ({})", column)
            }
        };

        let sql = format!("{} {}", create_table_sql, partition_clause);

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    /// Create a single partition
    pub async fn create_partition(
        &self,
        table_name: &str,
        partition_name: &str,
        from_value: &str,
        to_value: &str,
    ) -> Result<()> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM ('{}') TO ('{}')",
            partition_name, table_name, from_value, to_value
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    /// Create time-based partition
    pub async fn create_time_partition(
        &self,
        table_name: &str,
        date: DateTime<Utc>,
        interval: PartitionInterval,
    ) -> Result<String> {
        let suffix = interval.partition_suffix(date);
        let partition_name = format!("{}_{}", table_name, suffix);
        let (start, end) = interval.partition_bounds(date)?;

        let from_value = start.format("%Y-%m-%d %H:%M:%S").to_string();
        let to_value = end.format("%Y-%m-%d %H:%M:%S").to_string();

        self.create_partition(table_name, &partition_name, &from_value, &to_value)
            .await?;

        Ok(partition_name)
    }

    /// Create hash partition
    pub async fn create_hash_partition(
        &self,
        table_name: &str,
        partition_index: usize,
        modulus: usize,
    ) -> Result<String> {
        let partition_name = format!("{}_p{}", table_name, partition_index);

        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES WITH (MODULUS {}, REMAINDER {})",
            partition_name, table_name, modulus, partition_index
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(partition_name)
    }

    /// Drop partition
    pub async fn drop_partition(&self, partition_name: &str) -> Result<()> {
        let sql = format!("DROP TABLE IF EXISTS {}", partition_name);

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    /// List all partitions for a table
    pub async fn list_partitions(&self, table_name: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT inhrelid::regclass::text
             FROM pg_inherits
             WHERE inhparent = $1::regclass",
        )
        .bind(table_name)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    /// Get partition statistics
    pub async fn partition_stats(&self, table_name: &str) -> Result<PartitionStats> {
        let partitions = self.list_partitions(table_name).await?;

        let mut total_rows = 0;
        let mut total_size = 0;

        for partition in &partitions {
            // Get row count
            let row = sqlx::query_as::<_, (i64,)>(&format!("SELECT COUNT(*) FROM {}", partition))
                .fetch_one(&self.pool)
                .await
                .map_err(DbError::from)?;
            total_rows += row.0;

            // Get size
            let size_row =
                sqlx::query_as::<_, (i64,)>("SELECT pg_total_relation_size($1::regclass)")
                    .bind(partition)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(DbError::from)?;
            total_size += size_row.0;
        }

        Ok(PartitionStats {
            partition_count: partitions.len(),
            total_rows: total_rows as usize,
            total_size_bytes: total_size as usize,
            partitions,
        })
    }

    /// Detach partition (for archival/deletion)
    pub async fn detach_partition(&self, table_name: &str, partition_name: &str) -> Result<()> {
        let sql = format!(
            "ALTER TABLE {} DETACH PARTITION {}",
            table_name, partition_name
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    /// Attach partition
    pub async fn attach_partition(
        &self,
        table_name: &str,
        partition_name: &str,
        from_value: &str,
        to_value: &str,
    ) -> Result<()> {
        let sql = format!(
            "ALTER TABLE {} ATTACH PARTITION {} FOR VALUES FROM ('{}') TO ('{}')",
            table_name, partition_name, from_value, to_value
        );

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }
}

/// Partition statistics
#[derive(Debug, Clone)]
pub struct PartitionStats {
    /// Number of partitions in the table.
    pub partition_count: usize,
    /// Total row count across all partitions.
    pub total_rows: usize,
    /// Total size in bytes across all partitions.
    pub total_size_bytes: usize,
    /// Names of all individual partitions.
    pub partitions: Vec<String>,
}

impl fmt::Display for PartitionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Partitions: {}, Rows: {}, Size: {} bytes",
            self.partition_count, self.total_rows, self.total_size_bytes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_partition_interval_suffix() {
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 12, 0, 0).unwrap();

        assert_eq!(PartitionInterval::Daily.partition_suffix(date), "20240315");
        assert_eq!(PartitionInterval::Monthly.partition_suffix(date), "202403");
        assert_eq!(PartitionInterval::Yearly.partition_suffix(date), "2024");
    }

    #[test]
    fn test_partition_interval_quarterly() {
        let q1 = Utc.with_ymd_and_hms(2024, 2, 15, 0, 0, 0).unwrap();
        let q2 = Utc.with_ymd_and_hms(2024, 5, 15, 0, 0, 0).unwrap();
        let q3 = Utc.with_ymd_and_hms(2024, 8, 15, 0, 0, 0).unwrap();
        let q4 = Utc.with_ymd_and_hms(2024, 11, 15, 0, 0, 0).unwrap();

        assert_eq!(PartitionInterval::Quarterly.partition_suffix(q1), "2024q1");
        assert_eq!(PartitionInterval::Quarterly.partition_suffix(q2), "2024q2");
        assert_eq!(PartitionInterval::Quarterly.partition_suffix(q3), "2024q3");
        assert_eq!(PartitionInterval::Quarterly.partition_suffix(q4), "2024q4");
    }

    #[test]
    fn test_partition_bounds_daily() -> crate::error::Result<()> {
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 12, 30, 45).unwrap();
        let (start, end) = PartitionInterval::Daily.partition_bounds(date)?;

        assert_eq!(start, Utc.with_ymd_and_hms(2024, 3, 15, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2024, 3, 16, 0, 0, 0).unwrap());
        Ok(())
    }

    #[test]
    fn test_partition_bounds_monthly() -> crate::error::Result<()> {
        let date = Utc.with_ymd_and_hms(2024, 3, 15, 12, 0, 0).unwrap();
        let (start, end) = PartitionInterval::Monthly.partition_bounds(date)?;

        assert_eq!(start, Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2024, 4, 1, 0, 0, 0).unwrap());
        Ok(())
    }

    #[test]
    fn test_partition_bounds_yearly() -> crate::error::Result<()> {
        let date = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();
        let (start, end) = PartitionInterval::Yearly.partition_bounds(date)?;

        assert_eq!(start, Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
        assert_eq!(end, Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());
        Ok(())
    }

    #[test]
    fn test_partitioning_strategy_enum() {
        let range = PartitioningStrategy::Range {
            column: "created_at".to_string(),
        };
        assert!(matches!(range, PartitioningStrategy::Range { .. }));

        let list = PartitioningStrategy::List {
            column: "status".to_string(),
        };
        assert!(matches!(list, PartitioningStrategy::List { .. }));

        let hash = PartitioningStrategy::Hash {
            column: "user_id".to_string(),
            num_partitions: 4,
        };
        assert!(matches!(hash, PartitioningStrategy::Hash { .. }));
    }

    #[test]
    fn test_partition_definition_create_sql() {
        let def = PartitionDefinition {
            table_name: "events".to_string(),
            partition_name: "events_202401".to_string(),
            strategy: PartitioningStrategy::Range {
                column: "created_at".to_string(),
            },
        };

        let sql = def.create_sql();
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("PARTITION OF"));
        assert!(sql.contains("events"));
    }

    #[test]
    fn test_partition_definition_drop_sql() {
        let def = PartitionDefinition {
            table_name: "events".to_string(),
            partition_name: "events_202401".to_string(),
            strategy: PartitioningStrategy::Range {
                column: "created_at".to_string(),
            },
        };

        let sql = def.drop_sql();
        assert_eq!(sql, "DROP TABLE IF EXISTS events_202401");
    }

    #[test]
    fn test_partition_stats_display() {
        let stats = PartitionStats {
            partition_count: 3,
            total_rows: 10000,
            total_size_bytes: 1048576,
            partitions: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
        };

        let display = format!("{}", stats);
        assert!(display.contains("Partitions: 3"));
        assert!(display.contains("Rows: 10000"));
        assert!(display.contains("Size: 1048576 bytes"));
    }

    #[test]
    fn test_partition_interval_enum() {
        assert_eq!(PartitionInterval::Daily, PartitionInterval::Daily);
        assert_ne!(PartitionInterval::Daily, PartitionInterval::Monthly);
    }
}
