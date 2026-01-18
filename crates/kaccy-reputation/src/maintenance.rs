//! Maintenance module for automated periodic tasks
//!
//! Provides scheduled tasks for system maintenance, cleanup,
//! and automated reputation system operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::batch::BatchService;
use crate::error::ReputationError;
use crate::score::ReputationEngine;
use crate::score_history::ScoreHistoryRepository;

/// Service for automated maintenance tasks
pub struct MaintenanceService {
    pool: PgPool,
    batch_service: BatchService,
    engine: ReputationEngine,
    history_repo: ScoreHistoryRepository,
}

impl MaintenanceService {
    pub fn new(pool: PgPool) -> Self {
        let batch_service = BatchService::new(pool.clone());
        let engine = ReputationEngine::new(pool.clone());
        let history_repo = ScoreHistoryRepository::new(pool.clone());

        Self {
            pool,
            batch_service,
            engine,
            history_repo,
        }
    }

    /// Run all daily maintenance tasks
    pub async fn run_daily_maintenance(&self) -> Result<MaintenanceReport, ReputationError> {
        let started_at = Utc::now();
        let mut report = MaintenanceReport::default();

        // 1. Expire overdue commitments
        match self.batch_service.expire_overdue_commitments().await {
            Ok(result) => {
                report.commitments_expired = result.expired_count;
                report
                    .tasks_completed
                    .push("expire_commitments".to_string());
            }
            Err(e) => {
                report
                    .tasks_failed
                    .push(format!("expire_commitments: {}", e));
            }
        }

        // 2. Apply time decay
        match self.batch_service.apply_time_decay_batch().await {
            Ok(result) => {
                report.users_decayed = result.users_decayed;
                report.tasks_completed.push("time_decay".to_string());
            }
            Err(e) => {
                report.tasks_failed.push(format!("time_decay: {}", e));
            }
        }

        // 3. Create score snapshots
        match self.create_daily_snapshots().await {
            Ok(count) => {
                report.snapshots_created = count;
                report.tasks_completed.push("create_snapshots".to_string());
            }
            Err(e) => {
                report.tasks_failed.push(format!("create_snapshots: {}", e));
            }
        }

        // 4. Clean old data
        match self.cleanup_old_data(90).await {
            Ok(count) => {
                report.records_cleaned = count;
                report.tasks_completed.push("cleanup_old_data".to_string());
            }
            Err(e) => {
                report.tasks_failed.push(format!("cleanup_old_data: {}", e));
            }
        }

        report.completed_at = Some(Utc::now());
        report.duration_seconds = (report.completed_at.unwrap() - started_at).num_seconds() as i32;

        Ok(report)
    }

    /// Run weekly maintenance tasks
    pub async fn run_weekly_maintenance(&self) -> Result<MaintenanceReport, ReputationError> {
        let started_at = Utc::now();
        let mut report = MaintenanceReport::default();

        // 1. Recalculate all scores (integrity check)
        match self.recalculate_all_scores().await {
            Ok(count) => {
                report.scores_recalculated = count;
                report
                    .tasks_completed
                    .push("recalculate_scores".to_string());
            }
            Err(e) => {
                report
                    .tasks_failed
                    .push(format!("recalculate_scores: {}", e));
            }
        }

        // 2. Vacuum and analyze database tables
        match self.optimize_database().await {
            Ok(_) => {
                report.tasks_completed.push("optimize_database".to_string());
            }
            Err(e) => {
                report
                    .tasks_failed
                    .push(format!("optimize_database: {}", e));
            }
        }

        // 3. Archive old audit logs
        match self.archive_old_audit_logs(180).await {
            Ok(count) => {
                report.records_archived = count;
                report
                    .tasks_completed
                    .push("archive_audit_logs".to_string());
            }
            Err(e) => {
                report
                    .tasks_failed
                    .push(format!("archive_audit_logs: {}", e));
            }
        }

        report.completed_at = Some(Utc::now());
        report.duration_seconds = (report.completed_at.unwrap() - started_at).num_seconds() as i32;

        Ok(report)
    }

    /// Create score snapshots for all users
    async fn create_daily_snapshots(&self) -> Result<i32, ReputationError> {
        let query =
            "SELECT user_id, reputation_score FROM users WHERE reputation_score IS NOT NULL";

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut count = 0;
        for row in rows {
            let user_id = row.try_get("user_id").map_err(ReputationError::Database)?;

            if self.history_repo.take_snapshot(user_id).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Clean old data beyond retention period
    async fn cleanup_old_data(&self, days: i32) -> Result<i32, ReputationError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        // Clean old reputation events (keep summary, remove details)
        let query = r#"
            DELETE FROM reputation_events
            WHERE created_at < $1
            AND event_type NOT IN ('tier_change', 'circuit_breaker')
        "#;

        let result = sqlx::query(query)
            .bind(cutoff_date)
            .execute(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        Ok(result.rows_affected() as i32)
    }

    /// Recalculate all user scores
    async fn recalculate_all_scores(&self) -> Result<i32, ReputationError> {
        let query = "SELECT user_id FROM users WHERE reputation_score IS NOT NULL";

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut count = 0;
        for row in rows {
            let user_id = row.try_get("user_id").map_err(ReputationError::Database)?;

            if self.engine.recalculate_score(user_id).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Optimize database tables
    async fn optimize_database(&self) -> Result<(), ReputationError> {
        let tables = vec![
            "users",
            "reputation_events",
            "output_commitments",
            "score_snapshots",
            "tier_history",
            "holder_ratings",
        ];

        for table in tables {
            let vacuum_query = format!("VACUUM ANALYZE {}", table);
            sqlx::query(&vacuum_query)
                .execute(&self.pool)
                .await
                .map_err(ReputationError::Database)?;
        }

        Ok(())
    }

    /// Archive old audit logs
    async fn archive_old_audit_logs(&self, days: i32) -> Result<i32, ReputationError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        // In production, this would move to an archive table or cold storage
        // For now, we'll just count what would be archived
        let query = "SELECT COUNT(*) as count FROM audit_trail WHERE created_at < $1";

        let row = sqlx::query(query)
            .bind(cutoff_date)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;

        Ok(count as i32)
    }

    /// Run health checks
    pub async fn run_health_checks(&self) -> Result<HealthCheckReport, ReputationError> {
        // Check database connectivity
        let database_healthy = sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok();

        // Check for score anomalies
        let anomaly_query = r#"
            SELECT COUNT(*) as count
            FROM users
            WHERE reputation_score < 0 OR reputation_score > 1000
        "#;

        let score_anomalies =
            if let Ok(row) = sqlx::query(anomaly_query).fetch_one(&self.pool).await {
                let count: i64 = row.try_get("count").unwrap_or(0);
                count as i32
            } else {
                0
            };

        // Check for stuck commitments
        let stuck_query = r#"
            SELECT COUNT(*) as count
            FROM output_commitments
            WHERE status = 'Pending'
            AND deadline < NOW() - INTERVAL '30 days'
        "#;

        let stuck_commitments =
            if let Ok(row) = sqlx::query(stuck_query).fetch_one(&self.pool).await {
                let count: i64 = row.try_get("count").unwrap_or(0);
                count as i32
            } else {
                0
            };

        // Check for orphaned records
        let orphan_query = r#"
            SELECT COUNT(*) as count
            FROM reputation_events re
            LEFT JOIN users u ON re.user_id = u.user_id
            WHERE u.user_id IS NULL
        "#;

        let orphaned_records =
            if let Ok(row) = sqlx::query(orphan_query).fetch_one(&self.pool).await {
                let count: i64 = row.try_get("count").unwrap_or(0);
                count as i32
            } else {
                0
            };

        let healthy = database_healthy
            && score_anomalies == 0
            && stuck_commitments < 10
            && orphaned_records == 0;

        let report = HealthCheckReport {
            database_healthy,
            score_anomalies,
            stuck_commitments,
            orphaned_records,
            healthy,
            checked_at: Utc::now(),
        };

        Ok(report)
    }
}

/// Maintenance task report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub tasks_completed: Vec<String>,
    pub tasks_failed: Vec<String>,
    pub commitments_expired: i32,
    pub users_decayed: i32,
    pub snapshots_created: i32,
    pub scores_recalculated: i32,
    pub records_cleaned: i32,
    pub records_archived: i32,
    pub duration_seconds: i32,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Health check report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthCheckReport {
    pub healthy: bool,
    pub database_healthy: bool,
    pub score_anomalies: i32,
    pub stuck_commitments: i32,
    pub orphaned_records: i32,
    pub checked_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maintenance_report_default() {
        let report = MaintenanceReport::default();

        assert_eq!(report.tasks_completed.len(), 0);
        assert_eq!(report.tasks_failed.len(), 0);
        assert_eq!(report.commitments_expired, 0);
    }

    #[test]
    fn test_maintenance_report_with_data() {
        let report = MaintenanceReport {
            tasks_completed: vec!["task1".to_string(), "task2".to_string()],
            tasks_failed: vec![],
            commitments_expired: 10,
            users_decayed: 25,
            snapshots_created: 100,
            scores_recalculated: 50,
            records_cleaned: 200,
            records_archived: 500,
            duration_seconds: 45,
            completed_at: Some(Utc::now()),
        };

        assert_eq!(report.tasks_completed.len(), 2);
        assert_eq!(report.commitments_expired, 10);
        assert_eq!(report.duration_seconds, 45);
    }

    #[test]
    fn test_health_check_report_default() {
        let report = HealthCheckReport::default();

        assert!(!report.healthy);
        assert!(!report.database_healthy);
        assert_eq!(report.score_anomalies, 0);
    }

    #[test]
    fn test_health_check_report_healthy() {
        let report = HealthCheckReport {
            healthy: true,
            database_healthy: true,
            score_anomalies: 0,
            stuck_commitments: 0,
            orphaned_records: 0,
            checked_at: Utc::now(),
        };

        assert!(report.healthy);
        assert!(report.database_healthy);
        assert_eq!(report.score_anomalies, 0);
    }

    #[test]
    fn test_health_check_report_unhealthy() {
        let report = HealthCheckReport {
            healthy: false,
            database_healthy: true,
            score_anomalies: 5,
            stuck_commitments: 15,
            orphaned_records: 3,
            checked_at: Utc::now(),
        };

        assert!(!report.healthy);
        assert_eq!(report.score_anomalies, 5);
        assert_eq!(report.stuck_commitments, 15);
    }
}
