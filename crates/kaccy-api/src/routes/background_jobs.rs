//! Background jobs monitoring and management routes
//!
//! Provides endpoints for managing and monitoring background jobs (admin only)

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, middleware::Claims, state::AppState};
use kaccy_db::repositories::background_job::{
    BackgroundJob, BackgroundJobRepository, CreateBackgroundJob, JobStatus, JobTypeStats,
};

/// Query parameters for listing background jobs
#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Number of items per page (max 100)
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter by status
    pub status: Option<String>,
    /// Filter by job type
    pub job_type: Option<String>,
    /// Filter by user ID
    pub user_id: Option<Uuid>,
    /// Filter by correlation ID
    pub correlation_id: Option<String>,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

/// Response for jobs list
#[derive(Debug, Serialize)]
pub struct ListJobsResponse {
    pub jobs: Vec<BackgroundJob>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

/// Job statistics response
#[derive(Debug, Serialize)]
pub struct JobStatsResponse {
    pub total_jobs: i64,
    pub pending_jobs: i64,
    pub running_jobs: i64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub type_statistics: Vec<JobTypeStats>,
    pub queue_depth: i64,
}

/// Create job request
#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub job_type: String,
    pub payload: serde_json::Value,
    pub priority: Option<i32>,
    pub max_retries: Option<i32>,
    pub retry_backoff_ms: Option<i32>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub correlation_id: Option<String>,
}

/// List background jobs with pagination and filtering
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn list_jobs(
    _claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Json<ListJobsResponse>, ApiError> {
    let per_page = query.per_page.clamp(1, 100);
    let page = query.page.max(1);

    let repo = BackgroundJobRepository::new(state.pool.clone());

    // Get jobs based on filters
    let jobs = if let Some(ref status_str) = query.status {
        let status = parse_job_status(status_str)?;
        repo.get_by_status(status, page, per_page).await?
    } else if let Some(ref job_type) = query.job_type {
        repo.get_by_type(job_type, page, per_page).await?
    } else if let Some(ref correlation_id) = query.correlation_id {
        repo.get_by_correlation_id(correlation_id).await?
    } else if let Some(user_id) = query.user_id {
        repo.get_user_jobs(user_id, page, per_page).await?
    } else {
        // Default: get pending jobs
        let status = kaccy_db::repositories::background_job::JobStatus::Pending;
        repo.get_by_status(status, page, per_page).await?
    };

    // Get total count for the filter
    let total = if let Some(ref status_str) = query.status {
        let status = parse_job_status(status_str)?;
        repo.count_by_status(status).await?
    } else {
        // If no specific filter, count all jobs
        let pending = repo.count_by_status(JobStatus::Pending).await?;
        let running = repo.count_by_status(JobStatus::Running).await?;
        let completed = repo.count_by_status(JobStatus::Completed).await?;
        let failed = repo.count_by_status(JobStatus::Failed).await?;
        let cancelled = repo.count_by_status(JobStatus::Cancelled).await?;
        pending + running + completed + failed + cancelled
    };

    let total_pages = (total + per_page - 1) / per_page;

    Ok(Json(ListJobsResponse {
        jobs,
        total,
        page,
        per_page,
        total_pages,
    }))
}

/// Parse status string to JobStatus enum
fn parse_job_status(
    status: &str,
) -> Result<kaccy_db::repositories::background_job::JobStatus, ApiError> {
    match status.to_lowercase().as_str() {
        "pending" => Ok(kaccy_db::repositories::background_job::JobStatus::Pending),
        "running" => Ok(kaccy_db::repositories::background_job::JobStatus::Running),
        "completed" => Ok(kaccy_db::repositories::background_job::JobStatus::Completed),
        "failed" => Ok(kaccy_db::repositories::background_job::JobStatus::Failed),
        "cancelled" => Ok(kaccy_db::repositories::background_job::JobStatus::Cancelled),
        _ => Err(ApiError::Validation(format!(
            "Invalid job status: {}",
            status
        ))),
    }
}

/// Get specific background job by ID
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_job(
    _claims: Claims,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    let job = repo
        .find_by_id(job_id)
        .await?
        .ok_or(ApiError::NotFound("Job not found".to_string()))?;

    Ok(Json(job))
}

/// Get job statistics
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_job_stats(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<JobStatsResponse>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    let pending_jobs = repo.count_by_status(JobStatus::Pending).await?;
    let running_jobs = repo.count_by_status(JobStatus::Running).await?;
    let completed_jobs = repo.count_by_status(JobStatus::Completed).await?;
    let failed_jobs = repo.count_by_status(JobStatus::Failed).await?;
    let total_jobs = pending_jobs + running_jobs + completed_jobs + failed_jobs;

    let type_statistics = repo.get_type_statistics().await?;
    let queue_depth = repo.get_queue_depth().await?;

    Ok(Json(JobStatsResponse {
        total_jobs,
        pending_jobs,
        running_jobs,
        completed_jobs,
        failed_jobs,
        type_statistics,
        queue_depth,
    }))
}

/// Retry a failed job
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn retry_job(
    _claims: Claims,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    // Verify job exists and is in a retriable state
    let job = repo
        .find_by_id(job_id)
        .await?
        .ok_or(ApiError::NotFound("Job not found".to_string()))?;

    if job.status != "failed" && job.status != "cancelled" {
        return Err(ApiError::Validation(
            "Only failed or cancelled jobs can be retried".to_string(),
        ));
    }

    let retried_job = repo.retry_job(job_id).await?;

    Ok(Json(retried_job))
}

/// Cancel a pending job
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn cancel_job(
    _claims: Claims,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<BackgroundJob>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    // Verify job exists and is in a cancellable state
    let job = repo
        .find_by_id(job_id)
        .await?
        .ok_or(ApiError::NotFound("Job not found".to_string()))?;

    if job.status != "pending" {
        return Err(ApiError::Validation(
            "Only pending jobs can be cancelled".to_string(),
        ));
    }

    let cancelled_job = repo.cancel_job(job_id).await?;

    Ok(Json(cancelled_job))
}

/// Create a new background job
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn create_job(
    claims: Claims,
    State(state): State<AppState>,
    Json(request): Json<CreateJobRequest>,
) -> Result<Json<BackgroundJob>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    let user_id = Uuid::parse_str(&claims.sub).ok();

    let job = repo
        .create(CreateBackgroundJob {
            job_type: request.job_type,
            payload: request.payload,
            priority: request.priority,
            max_retries: request.max_retries,
            retry_backoff_ms: request.retry_backoff_ms,
            scheduled_at: request.scheduled_at,
            expires_at: request.expires_at,
            created_by: user_id,
            correlation_id: request.correlation_id,
        })
        .await?;

    Ok(Json(job))
}

/// Get stuck jobs (jobs that have been running for too long)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_stuck_jobs(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<Vec<BackgroundJob>>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    // Default timeout: 1 hour
    let timeout_minutes = 60;

    let stuck_jobs = repo.get_stuck_jobs(timeout_minutes).await?;

    Ok(Json(stuck_jobs))
}

/// Reset stuck jobs (mark them as failed so they can be retried)
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn reset_stuck_jobs(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    // Default timeout: 1 hour
    let timeout_minutes = 60;

    let reset_count = repo.reset_stuck_jobs(timeout_minutes).await?;

    Ok(Json(serde_json::json!({
        "reset_count": reset_count,
        "message": format!("Reset {} stuck jobs", reset_count)
    })))
}

/// Cleanup old completed jobs
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn cleanup_old_jobs(
    _claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<CleanupJobsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let repo = BackgroundJobRepository::new(state.pool.clone());

    let days_old = query.days_old.unwrap_or(30);

    if !(1..=365).contains(&days_old) {
        return Err(ApiError::Validation(
            "days_old must be between 1 and 365".to_string(),
        ));
    }

    let deleted_count = repo.cleanup_old_jobs(i64::from(days_old)).await?;

    Ok(Json(serde_json::json!({
        "deleted_count": deleted_count,
        "message": format!("Deleted {} old jobs", deleted_count)
    })))
}

/// Query parameters for cleanup
#[derive(Debug, Deserialize)]
pub struct CleanupJobsQuery {
    /// Number of days old (default: 30)
    pub days_old: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_jobs_query_defaults() {
        let query: ListJobsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, 50);
        assert!(query.status.is_none());
        assert!(query.job_type.is_none());
    }

    #[test]
    fn test_cleanup_validation() {
        let query = CleanupJobsQuery {
            days_old: Some(400),
        };
        assert!(query.days_old.unwrap() > 365);
    }

    #[test]
    fn test_parse_job_status() {
        assert!(parse_job_status("pending").is_ok());
        assert!(parse_job_status("running").is_ok());
        assert!(parse_job_status("completed").is_ok());
        assert!(parse_job_status("failed").is_ok());
        assert!(parse_job_status("cancelled").is_ok());
        assert!(parse_job_status("invalid").is_err());
    }
}
