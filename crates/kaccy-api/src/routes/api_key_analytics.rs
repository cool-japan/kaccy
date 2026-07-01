//! API Key Usage Analytics Routes
//!
//! Provides endpoints for tracking and analyzing API key usage patterns

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

/// API key usage statistics
#[derive(Debug, Serialize, FromRow)]
pub struct ApiKeyUsageStats {
    /// API key ID
    pub api_key_id: Uuid,
    /// Total number of requests made with this key
    pub total_requests: i64,
    /// Number of successful requests (2xx status codes)
    pub successful_requests: i64,
    /// Number of failed requests (non-2xx status codes)
    pub failed_requests: i64,
    /// Timestamp of most recent use
    pub last_used_at: Option<DateTime<Utc>>,
    /// List of endpoints that have been accessed
    pub endpoints_used: Option<Vec<String>>,
}

/// Detailed usage entry
#[derive(Debug, Serialize, FromRow)]
pub struct UsageEntry {
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// API endpoint path
    pub endpoint: String,
    /// HTTP method
    pub method: String,
    /// HTTP response status code
    pub status_code: i32,
    /// Response time in milliseconds
    pub response_time_ms: Option<i32>,
}

/// Query parameters for usage analytics
#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    /// Start date for analytics (defaults to 7 days ago)
    pub start_date: Option<DateTime<Utc>>,
    /// End date for analytics (defaults to now)
    pub end_date: Option<DateTime<Utc>>,
    /// Group by time interval (hour, day, week)
    #[serde(default)]
    pub group_by: GroupByInterval,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

/// Time interval for grouping
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GroupByInterval {
    /// Group by hour
    Hour,
    /// Group by day (default)
    #[default]
    Day,
    /// Group by week
    Week,
}

/// Aggregated usage data
#[derive(Debug, Serialize, FromRow)]
pub struct AggregatedUsage {
    /// Time bucket start
    pub time_bucket: DateTime<Utc>,
    /// Number of requests in this time bucket
    pub request_count: i64,
    /// Percentage of successful requests (0-100)
    pub success_rate: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: Option<f64>,
}

/// API key usage summary
#[derive(Debug, Serialize)]
pub struct UsageSummary {
    /// API key ID
    pub api_key_id: Uuid,
    /// Start of the reporting period
    pub period_start: DateTime<Utc>,
    /// End of the reporting period
    pub period_end: DateTime<Utc>,
    /// Total requests in the period
    pub total_requests: i64,
    /// Overall success rate (0-100)
    pub success_rate: f64,
    /// Most frequently accessed endpoints
    pub top_endpoints: Vec<EndpointUsage>,
    /// Request distribution over time
    pub hourly_distribution: Vec<AggregatedUsage>,
    /// Distribution of HTTP status codes
    pub status_code_distribution: Vec<StatusCodeCount>,
}

/// Endpoint usage statistics
#[derive(Debug, Serialize, FromRow)]
pub struct EndpointUsage {
    /// Endpoint path
    pub endpoint: String,
    /// Number of requests to this endpoint
    pub request_count: i64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: Option<f64>,
}

/// Status code distribution
#[derive(Debug, Serialize, FromRow)]
pub struct StatusCodeCount {
    /// HTTP status code
    pub status_code: i32,
    /// Number of responses with this status code
    pub count: i64,
}

/// Get usage statistics for a specific API key
///
/// Returns aggregated usage data for the authenticated user's API key
#[allow(dead_code)]
pub async fn get_api_key_usage(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Path(api_key_id): Path<Uuid>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsageSummary>, ApiError> {
    // Verify the API key belongs to the user
    let key_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM api_keys WHERE id = $1 AND user_id = $2)")
            .bind(api_key_id)
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::Database)?;

    if !key_exists {
        return Err(ApiError::NotFound("API key not found".to_string()));
    }

    // Set default date range (last 7 days)
    let end_date = query.end_date.unwrap_or_else(Utc::now);
    let start_date = query
        .start_date
        .unwrap_or_else(|| end_date - Duration::days(7));

    // Validate date range
    if start_date > end_date {
        return Err(ApiError::Validation(
            "start_date must be before end_date".to_string(),
        ));
    }

    // Get total requests and success rate
    let (total_requests, successful_requests): (i64, i64) = sqlx::query_as(
        "SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status_code >= 200 AND status_code < 300) as successful
         FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2
         AND timestamp <= $3",
    )
    .bind(api_key_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    let success_rate = if total_requests > 0 {
        (successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    // Get top endpoints
    let top_endpoints: Vec<EndpointUsage> = sqlx::query_as(
        "SELECT
            endpoint,
            COUNT(*) as request_count,
            AVG(response_time_ms) as avg_response_time_ms
         FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2
         AND timestamp <= $3
         GROUP BY endpoint
         ORDER BY request_count DESC
         LIMIT 10",
    )
    .bind(api_key_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    // Get hourly distribution
    let interval = match query.group_by {
        GroupByInterval::Hour => "1 hour",
        GroupByInterval::Day => "1 day",
        GroupByInterval::Week => "1 week",
    };

    let hourly_distribution: Vec<AggregatedUsage> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT
            DATE_TRUNC('{}', timestamp) as time_bucket,
            COUNT(*) as request_count,
            (COUNT(*) FILTER (WHERE status_code >= 200 AND status_code < 300)::float / COUNT(*)::float * 100) as success_rate,
            AVG(response_time_ms) as avg_response_time_ms
         FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2
         AND timestamp <= $3
         GROUP BY time_bucket
         ORDER BY time_bucket", interval)))
    .bind(api_key_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    // Get status code distribution
    let status_code_distribution: Vec<StatusCodeCount> = sqlx::query_as(
        "SELECT
            status_code,
            COUNT(*) as count
         FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2
         AND timestamp <= $3
         GROUP BY status_code
         ORDER BY count DESC",
    )
    .bind(api_key_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(UsageSummary {
        api_key_id,
        period_start: start_date,
        period_end: end_date,
        total_requests,
        success_rate,
        top_endpoints,
        hourly_distribution,
        status_code_distribution,
    }))
}

/// Get detailed usage logs for an API key
///
/// Returns paginated list of individual requests
#[allow(dead_code)]
pub async fn get_api_key_usage_logs(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Path(api_key_id): Path<Uuid>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<Vec<UsageEntry>>, ApiError> {
    // Verify the API key belongs to the user
    let key_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM api_keys WHERE id = $1 AND user_id = $2)")
            .bind(api_key_id)
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::Database)?;

    if !key_exists {
        return Err(ApiError::NotFound("API key not found".to_string()));
    }

    let end_date = query.end_date.unwrap_or_else(Utc::now);
    let start_date = query
        .start_date
        .unwrap_or_else(|| end_date - Duration::days(7));

    let per_page = query.per_page.clamp(1, 100);
    let page = query.page.max(1);
    let offset = (page - 1) * per_page;

    let logs: Vec<UsageEntry> = sqlx::query_as(
        "SELECT timestamp, endpoint, method, status_code, response_time_ms
         FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2
         AND timestamp <= $3
         ORDER BY timestamp DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(api_key_id)
    .bind(start_date)
    .bind(end_date)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(logs))
}

/// Get usage comparison across all user's API keys
///
/// Returns comparative statistics for all API keys owned by the user
#[allow(dead_code)]
pub async fn get_all_api_keys_usage(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<Vec<ApiKeyUsageStats>>, ApiError> {
    let end_date = query.end_date.unwrap_or_else(Utc::now);
    let start_date = query
        .start_date
        .unwrap_or_else(|| end_date - Duration::days(7));

    let stats: Vec<ApiKeyUsageStats> = sqlx::query_as(
        "SELECT
            aku.api_key_id,
            COUNT(*) as total_requests,
            COUNT(*) FILTER (WHERE aku.status_code >= 200 AND aku.status_code < 300) as successful_requests,
            COUNT(*) FILTER (WHERE aku.status_code < 200 OR aku.status_code >= 300) as failed_requests,
            MAX(aku.timestamp) as last_used_at,
            NULL as endpoints_used
         FROM api_key_usage aku
         JOIN api_keys ak ON aku.api_key_id = ak.id
         WHERE ak.user_id = $1
         AND aku.timestamp >= $2
         AND aku.timestamp <= $3
         GROUP BY aku.api_key_id
         ORDER BY total_requests DESC"
    )
    .bind(user_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(stats))
}

/// API key quota usage information
#[derive(Debug, Serialize)]
pub struct QuotaUsage {
    /// API key ID
    pub api_key_id: Uuid,
    /// Quota period (monthly, daily, etc.)
    pub period: String,
    /// Current usage count in this period
    pub current_usage: i64,
    /// Maximum allowed requests (None if unlimited)
    pub quota_limit: Option<i64>,
    /// Percentage of quota used (0-100)
    pub percentage_used: Option<f64>,
    /// Timestamp when quota resets
    pub reset_at: Option<DateTime<Utc>>,
}

/// Get the current quota usage for a specific API key
#[allow(dead_code)]
pub async fn get_api_key_quota(
    Extension(user_id): Extension<Uuid>,
    State(state): State<AppState>,
    Path(api_key_id): Path<Uuid>,
) -> Result<Json<QuotaUsage>, ApiError> {
    // Verify the API key belongs to the user
    let key_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM api_keys WHERE id = $1 AND user_id = $2)")
            .bind(api_key_id)
            .bind(user_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::Database)?;

    if !key_exists {
        return Err(ApiError::NotFound("API key not found".to_string()));
    }

    // Get current month's usage
    let now = Utc::now();
    let naive_date = now.date_naive();
    let start_of_month_date = naive_date
        .with_day(1)
        .expect("day 1 is always a valid day of month");
    let start_of_month = start_of_month_date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
        .and_utc();

    let current_usage: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM api_key_usage
         WHERE api_key_id = $1
         AND timestamp >= $2",
    )
    .bind(api_key_id)
    .bind(start_of_month)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    // In a real implementation, quota_limit would come from api_keys table
    let quota_limit = Some(100_000i64); // Example: 100k requests per month

    let percentage_used = quota_limit.map(|limit| (current_usage as f64 / limit as f64) * 100.0);

    // Calculate reset date (first day of next month)
    let next_month = if naive_date.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(naive_date.year() + 1, 1, 1)
            .expect("January 1 of next year is always a valid date")
    } else {
        chrono::NaiveDate::from_ymd_opt(naive_date.year(), naive_date.month() + 1, 1)
            .expect("first day of next month derived from existing date is always valid")
    };
    let reset_at = next_month
        .and_hms_opt(0, 0, 0)
        .expect("midnight is a valid time")
        .and_utc();

    Ok(Json(QuotaUsage {
        api_key_id,
        period: "monthly".to_string(),
        current_usage,
        quota_limit,
        percentage_used,
        reset_at: Some(reset_at),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_pagination() {
        assert_eq!(default_page(), 1);
        assert_eq!(default_per_page(), 50);
    }

    #[test]
    fn test_group_by_interval_default() {
        let interval = GroupByInterval::default();
        assert!(matches!(interval, GroupByInterval::Day));
    }

    #[test]
    fn test_success_rate_calculation() {
        let total = 100i64;
        let successful = 95i64;
        let rate = (successful as f64 / total as f64) * 100.0;
        assert!((rate - 95.0).abs() < 0.01);
    }

    #[test]
    fn test_quota_percentage() {
        let current = 75_000i64;
        let limit = 100_000i64;
        let percentage = (current as f64 / limit as f64) * 100.0;
        assert!((percentage - 75.0).abs() < 0.01);
    }
}
