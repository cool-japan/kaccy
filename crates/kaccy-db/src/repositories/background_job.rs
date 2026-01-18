use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Background job status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

/// Background job model
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct BackgroundJob {
    pub id: Uuid,
    pub job_type: String,
    pub status: String,
    pub priority: i32,
    pub payload: JsonValue,
    pub result: Option<JsonValue>,
    pub error_message: Option<String>,
    pub max_retries: i32,
    pub retry_count: i32,
    pub retry_backoff_ms: i32,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub correlation_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Parameters for creating a new background job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackgroundJob {
    pub job_type: String,
    pub payload: JsonValue,
    pub priority: Option<i32>,
    pub max_retries: Option<i32>,
    pub retry_backoff_ms: Option<i32>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub correlation_id: Option<String>,
}

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub success: bool,
    pub data: Option<JsonValue>,
    pub error: Option<String>,
}

/// Job statistics by type
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct JobTypeStats {
    pub job_type: String,
    pub pending_count: i64,
    pub running_count: i64,
    pub completed_count: i64,
    pub failed_count: i64,
    pub average_duration_seconds: Option<f64>,
}

/// Background job repository
pub struct BackgroundJobRepository {
    pool: PgPool,
}

impl BackgroundJobRepository {
    /// Create a new background job repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new background job
    pub async fn create(&self, params: CreateBackgroundJob) -> Result<BackgroundJob> {
        let job = sqlx::query_as::<_, BackgroundJob>(
            r#"
            INSERT INTO background_jobs (
                job_type, payload, priority, max_retries, retry_backoff_ms,
                scheduled_at, expires_at, created_by, correlation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#,
        )
        .bind(&params.job_type)
        .bind(&params.payload)
        .bind(params.priority.unwrap_or(0))
        .bind(params.max_retries.unwrap_or(3))
        .bind(params.retry_backoff_ms.unwrap_or(1000))
        .bind(params.scheduled_at.unwrap_or_else(Utc::now))
        .bind(params.expires_at)
        .bind(params.created_by)
        .bind(params.correlation_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    /// Batch create multiple jobs (optimized for bulk operations)
    pub async fn batch_create(&self, jobs: Vec<CreateBackgroundJob>) -> Result<Vec<BackgroundJob>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO background_jobs (job_type, payload, priority, max_retries, retry_backoff_ms, scheduled_at, expires_at, created_by, correlation_id) "
        );

        query_builder.push_values(jobs.iter(), |mut b, job| {
            b.push_bind(&job.job_type)
                .push_bind(&job.payload)
                .push_bind(job.priority.unwrap_or(0))
                .push_bind(job.max_retries.unwrap_or(3))
                .push_bind(job.retry_backoff_ms.unwrap_or(1000))
                .push_bind(job.scheduled_at.unwrap_or_else(Utc::now))
                .push_bind(job.expires_at)
                .push_bind(job.created_by)
                .push_bind(&job.correlation_id);
        });

        query_builder.push(" RETURNING *");

        let created_jobs = query_builder
            .build_query_as::<BackgroundJob>()
            .fetch_all(&self.pool)
            .await?;

        Ok(created_jobs)
    }

    /// Find job by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<BackgroundJob>> {
        let job = sqlx::query_as::<_, BackgroundJob>("SELECT * FROM background_jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(job)
    }

    /// Poll for next available job (atomic operation)
    /// Returns the next pending job and marks it as running
    pub async fn poll_next_job(
        &self,
        job_types: Option<Vec<String>>,
    ) -> Result<Option<BackgroundJob>> {
        let job = if let Some(types) = job_types {
            sqlx::query_as::<_, BackgroundJob>(
                r#"
                UPDATE background_jobs
                SET status = 'running', started_at = NOW(), updated_at = NOW()
                WHERE id = (
                    SELECT id FROM background_jobs
                    WHERE status IN ('pending', 'failed')
                        AND job_type = ANY($1)
                        AND scheduled_at <= NOW()
                        AND (expires_at IS NULL OR expires_at > NOW())
                        AND retry_count < max_retries
                    ORDER BY priority DESC, scheduled_at ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING *
                "#,
            )
            .bind(types)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, BackgroundJob>(
                r#"
                UPDATE background_jobs
                SET status = 'running', started_at = NOW(), updated_at = NOW()
                WHERE id = (
                    SELECT id FROM background_jobs
                    WHERE status IN ('pending', 'failed')
                        AND scheduled_at <= NOW()
                        AND (expires_at IS NULL OR expires_at > NOW())
                        AND retry_count < max_retries
                    ORDER BY priority DESC, scheduled_at ASC
                    LIMIT 1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING *
                "#,
            )
            .fetch_optional(&self.pool)
            .await?
        };

        Ok(job)
    }

    /// Mark job as completed with optional result data
    pub async fn mark_completed(
        &self,
        id: Uuid,
        result: Option<JsonValue>,
    ) -> Result<BackgroundJob> {
        let job = sqlx::query_as::<_, BackgroundJob>(
            r#"
            UPDATE background_jobs
            SET status = 'completed', result = $2, completed_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(result)
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    /// Mark job as failed with error message
    pub async fn mark_failed(&self, id: Uuid, error_message: String) -> Result<BackgroundJob> {
        let job = sqlx::query_as::<_, BackgroundJob>(
            r#"
            UPDATE background_jobs
            SET status = 'failed',
                error_message = $2,
                retry_count = retry_count + 1,
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(error_message)
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    /// Retry a failed job (resets to pending status)
    pub async fn retry_job(&self, id: Uuid) -> Result<BackgroundJob> {
        let job = sqlx::query_as::<_, BackgroundJob>(
            r#"
            UPDATE background_jobs
            SET status = 'pending',
                started_at = NULL,
                completed_at = NULL,
                scheduled_at = NOW() + (retry_backoff_ms * POW(2, retry_count)::INTEGER || ' milliseconds')::INTERVAL,
                updated_at = NOW()
            WHERE id = $1 AND retry_count < max_retries
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    /// Cancel a job (sets status to cancelled)
    pub async fn cancel_job(&self, id: Uuid) -> Result<BackgroundJob> {
        let job = sqlx::query_as::<_, BackgroundJob>(
            r#"
            UPDATE background_jobs
            SET status = 'cancelled', completed_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND status IN ('pending', 'failed')
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    /// Get jobs by status with pagination
    pub async fn get_by_status(
        &self,
        status: JobStatus,
        page: i64,
        limit: i64,
    ) -> Result<Vec<BackgroundJob>> {
        let offset = crate::helpers::calculate_offset(page as u32, limit as u32);

        let jobs = sqlx::query_as::<_, BackgroundJob>(
            r#"
            SELECT * FROM background_jobs
            WHERE status = $1
            ORDER BY priority DESC, scheduled_at ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(status.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Get jobs by type
    pub async fn get_by_type(
        &self,
        job_type: &str,
        page: i64,
        limit: i64,
    ) -> Result<Vec<BackgroundJob>> {
        let offset = crate::helpers::calculate_offset(page as u32, limit as u32);

        let jobs = sqlx::query_as::<_, BackgroundJob>(
            r#"
            SELECT * FROM background_jobs
            WHERE job_type = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(job_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Get jobs by correlation ID (for distributed tracing)
    pub async fn get_by_correlation_id(&self, correlation_id: &str) -> Result<Vec<BackgroundJob>> {
        let jobs = sqlx::query_as::<_, BackgroundJob>(
            r#"
            SELECT * FROM background_jobs
            WHERE correlation_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(correlation_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Get user's jobs
    pub async fn get_user_jobs(
        &self,
        user_id: Uuid,
        page: i64,
        limit: i64,
    ) -> Result<Vec<BackgroundJob>> {
        let offset = crate::helpers::calculate_offset(page as u32, limit as u32);

        let jobs = sqlx::query_as::<_, BackgroundJob>(
            r#"
            SELECT * FROM background_jobs
            WHERE created_by = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Get stuck jobs (running for longer than threshold)
    pub async fn get_stuck_jobs(&self, threshold_minutes: i64) -> Result<Vec<BackgroundJob>> {
        let jobs = sqlx::query_as::<_, BackgroundJob>(
            r#"
            SELECT * FROM background_jobs
            WHERE status = 'running'
                AND started_at < NOW() - ($1 || ' minutes')::INTERVAL
            ORDER BY started_at ASC
            "#,
        )
        .bind(threshold_minutes)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Reset stuck jobs to pending status
    pub async fn reset_stuck_jobs(&self, threshold_minutes: i64) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'pending',
                started_at = NULL,
                retry_count = retry_count + 1,
                error_message = 'Job stuck - automatically reset',
                updated_at = NOW()
            WHERE status = 'running'
                AND started_at < NOW() - ($1 || ' minutes')::INTERVAL
            "#,
        )
        .bind(threshold_minutes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Delete completed jobs older than specified days
    pub async fn cleanup_old_jobs(&self, days: i64) -> Result<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM background_jobs
            WHERE status = 'completed'
                AND completed_at < NOW() - ($1 || ' days')::INTERVAL
            "#,
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Delete expired jobs
    pub async fn cleanup_expired_jobs(&self) -> Result<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM background_jobs
            WHERE expires_at IS NOT NULL
                AND expires_at < NOW()
                AND status != 'completed'
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    /// Count jobs by status
    pub async fn count_by_status(&self, status: JobStatus) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM background_jobs WHERE status = $1")
            .bind(status.as_str())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("count"))
    }

    /// Get job type statistics
    pub async fn get_type_statistics(&self) -> Result<Vec<JobTypeStats>> {
        let stats = sqlx::query_as::<_, JobTypeStats>(
            r#"
            SELECT
                job_type,
                COUNT(*) FILTER (WHERE status = 'pending') as pending_count,
                COUNT(*) FILTER (WHERE status = 'running') as running_count,
                COUNT(*) FILTER (WHERE status = 'completed') as completed_count,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_count,
                AVG(EXTRACT(EPOCH FROM (completed_at - started_at))) FILTER (WHERE status = 'completed') as average_duration_seconds
            FROM background_jobs
            GROUP BY job_type
            ORDER BY pending_count DESC, running_count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get queue depth (pending + failed jobs that can be retried)
    pub async fn get_queue_depth(&self) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM background_jobs
            WHERE status IN ('pending', 'failed')
                AND scheduled_at <= NOW()
                AND (expires_at IS NULL OR expires_at > NOW())
                AND retry_count < max_retries
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("count"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_as_str() {
        assert_eq!(JobStatus::Pending.as_str(), "pending");
        assert_eq!(JobStatus::Running.as_str(), "running");
        assert_eq!(JobStatus::Completed.as_str(), "completed");
        assert_eq!(JobStatus::Failed.as_str(), "failed");
        assert_eq!(JobStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_job_status_serialization() {
        let status = JobStatus::Pending;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""pending""#);

        let deserialized: JobStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, JobStatus::Pending);
    }

    #[test]
    fn test_create_background_job_defaults() {
        let job = CreateBackgroundJob {
            job_type: "send_email".to_string(),
            payload: serde_json::json!({"to": "user@example.com"}),
            priority: None,
            max_retries: None,
            retry_backoff_ms: None,
            scheduled_at: None,
            expires_at: None,
            created_by: None,
            correlation_id: None,
        };

        assert_eq!(job.job_type, "send_email");
        assert!(job.priority.is_none());
        assert!(job.max_retries.is_none());
    }

    #[test]
    fn test_job_result_serialization() {
        let result = JobResult {
            success: true,
            data: Some(serde_json::json!({"message": "Email sent"})),
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("Email sent"));

        let deserialized: JobResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
    }

    #[test]
    fn test_job_type_stats_structure() {
        let stats = JobTypeStats {
            job_type: "send_email".to_string(),
            pending_count: 10,
            running_count: 2,
            completed_count: 100,
            failed_count: 5,
            average_duration_seconds: Some(2.5),
        };

        assert_eq!(stats.job_type, "send_email");
        assert_eq!(stats.pending_count, 10);
        assert_eq!(stats.average_duration_seconds, Some(2.5));
    }
}
