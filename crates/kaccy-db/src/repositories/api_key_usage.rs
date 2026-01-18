//! API Key Usage Repository
//!
//! Tracks API key usage statistics and analytics for monitoring and rate limiting.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// API key usage record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiKeyUsage {
    pub id: Uuid,
    pub api_key_id: Uuid,
    pub endpoint: String,
    pub method: String,
    pub status_code: i32,
    pub response_time_ms: i32,
    pub created_at: DateTime<Utc>,
}

/// Parameters for recording API usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordUsage {
    pub api_key_id: Uuid,
    pub endpoint: String,
    pub method: String,
    pub status_code: i32,
    pub response_time_ms: i32,
}

/// Usage statistics summary
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UsageStats {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub avg_response_time_ms: Option<f64>,
    pub min_response_time_ms: Option<i32>,
    pub max_response_time_ms: Option<i32>,
}

/// Per-endpoint usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EndpointStats {
    pub endpoint: String,
    pub method: String,
    pub request_count: i64,
    pub avg_response_time_ms: Option<f64>,
    pub success_rate: Option<f64>,
}

/// Hourly usage count for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HourlyUsage {
    pub hour: DateTime<Utc>,
    pub request_count: i64,
}

/// Repository for API key usage operations
pub struct ApiKeyUsageRepository {
    pool: PgPool,
}

impl ApiKeyUsageRepository {
    /// Create a new API key usage repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record an API key usage event
    pub async fn record(&self, usage: RecordUsage) -> Result<ApiKeyUsage> {
        let record = sqlx::query_as::<_, ApiKeyUsage>(
            r#"
            INSERT INTO api_key_usage (
                api_key_id, endpoint, method, status_code, response_time_ms
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(usage.api_key_id)
        .bind(&usage.endpoint)
        .bind(&usage.method)
        .bind(usage.status_code)
        .bind(usage.response_time_ms)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    /// Batch record multiple API key usage events (optimized for high-volume usage tracking)
    ///
    /// This method is much more efficient than calling `record()` multiple times,
    /// as it inserts all records in a single database round-trip.
    ///
    /// # Arguments
    /// * `usages` - Vector of usage records to insert
    ///
    /// # Returns
    /// Number of records inserted
    pub async fn batch_record(&self, usages: Vec<RecordUsage>) -> Result<u64> {
        if usages.is_empty() {
            return Ok(0);
        }

        // Build the VALUES clause dynamically
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO api_key_usage (api_key_id, endpoint, method, status_code, response_time_ms) "
        );

        query_builder.push_values(usages.iter(), |mut b, usage| {
            b.push_bind(usage.api_key_id)
                .push_bind(&usage.endpoint)
                .push_bind(&usage.method)
                .push_bind(usage.status_code)
                .push_bind(usage.response_time_ms);
        });

        let result = query_builder.build().execute(&self.pool).await?;

        Ok(result.rows_affected())
    }

    /// Get usage statistics for an API key
    pub async fn get_stats(
        &self,
        api_key_id: Uuid,
        since: Option<DateTime<Utc>>,
    ) -> Result<UsageStats> {
        let since = since.unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

        let stats = sqlx::query_as::<_, UsageStats>(
            r#"
            SELECT
                COUNT(*) as total_requests,
                COUNT(*) FILTER (WHERE status_code < 400) as successful_requests,
                COUNT(*) FILTER (WHERE status_code >= 400) as failed_requests,
                AVG(response_time_ms) as avg_response_time_ms,
                MIN(response_time_ms) as min_response_time_ms,
                MAX(response_time_ms) as max_response_time_ms
            FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= $2
            "#,
        )
        .bind(api_key_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get per-endpoint statistics
    pub async fn get_endpoint_stats(
        &self,
        api_key_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<EndpointStats>> {
        let limit = limit.unwrap_or(20);

        let stats = sqlx::query_as::<_, EndpointStats>(
            r#"
            SELECT
                endpoint,
                method,
                COUNT(*) as request_count,
                AVG(response_time_ms) as avg_response_time_ms,
                (COUNT(*) FILTER (WHERE status_code < 400)::float / COUNT(*)::float * 100) as success_rate
            FROM api_key_usage
            WHERE api_key_id = $1
            GROUP BY endpoint, method
            ORDER BY request_count DESC
            LIMIT $2
            "#,
        )
        .bind(api_key_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get hourly usage for rate limiting
    pub async fn get_hourly_usage(&self, api_key_id: Uuid, hours: i32) -> Result<Vec<HourlyUsage>> {
        let usage = sqlx::query_as::<_, HourlyUsage>(
            r#"
            SELECT
                date_trunc('hour', created_at) as hour,
                COUNT(*) as request_count
            FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= NOW() - INTERVAL '1 hour' * $2
            GROUP BY date_trunc('hour', created_at)
            ORDER BY hour DESC
            "#,
        )
        .bind(api_key_id)
        .bind(hours)
        .fetch_all(&self.pool)
        .await?;

        Ok(usage)
    }

    /// Get current hour's request count for rate limiting
    pub async fn get_current_hour_count(&self, api_key_id: Uuid) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= date_trunc('hour', NOW())
            "#,
        )
        .bind(api_key_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Get recent usage records
    pub async fn get_recent(
        &self,
        api_key_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<ApiKeyUsage>> {
        let limit = limit.unwrap_or(100);

        let records = sqlx::query_as::<_, ApiKeyUsage>(
            r#"
            SELECT * FROM api_key_usage
            WHERE api_key_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(api_key_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Get usage by date range
    pub async fn get_by_date_range(
        &self,
        api_key_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ApiKeyUsage>> {
        let records = sqlx::query_as::<_, ApiKeyUsage>(
            r#"
            SELECT * FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= $2
              AND created_at <= $3
            ORDER BY created_at DESC
            "#,
        )
        .bind(api_key_id)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Get error rate for an API key
    pub async fn get_error_rate(&self, api_key_id: Uuid, hours: i32) -> Result<f64> {
        let rate = sqlx::query_scalar::<_, Option<f64>>(
            r#"
            SELECT
                (COUNT(*) FILTER (WHERE status_code >= 400)::float / COUNT(*)::float * 100)
            FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= NOW() - INTERVAL '1 hour' * $2
            "#,
        )
        .bind(api_key_id)
        .bind(hours)
        .fetch_one(&self.pool)
        .await?;

        Ok(rate.unwrap_or(0.0))
    }

    /// Clean up old usage records (for GDPR compliance / storage management)
    pub async fn cleanup_old_records(&self, older_than_days: i32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM api_key_usage
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
            "#,
        )
        .bind(older_than_days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get top users by request count
    pub async fn get_top_keys(&self, limit: Option<i64>) -> Result<Vec<(Uuid, i64)>> {
        let limit = limit.unwrap_or(10);

        let top_keys = sqlx::query_as::<_, (Uuid, i64)>(
            r#"
            SELECT api_key_id, COUNT(*) as request_count
            FROM api_key_usage
            WHERE created_at >= NOW() - INTERVAL '24 hours'
            GROUP BY api_key_id
            ORDER BY request_count DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(top_keys)
    }

    /// Get average response time trend
    pub async fn get_response_time_trend(
        &self,
        api_key_id: Uuid,
        hours: i32,
    ) -> Result<Vec<(DateTime<Utc>, f64)>> {
        let trend = sqlx::query_as::<_, (DateTime<Utc>, Option<f64>)>(
            r#"
            SELECT
                date_trunc('hour', created_at) as hour,
                AVG(response_time_ms) as avg_response_time
            FROM api_key_usage
            WHERE api_key_id = $1
              AND created_at >= NOW() - INTERVAL '1 hour' * $2
            GROUP BY date_trunc('hour', created_at)
            ORDER BY hour ASC
            "#,
        )
        .bind(api_key_id)
        .bind(hours)
        .fetch_all(&self.pool)
        .await?;

        Ok(trend
            .into_iter()
            .map(|(hour, avg)| (hour, avg.unwrap_or(0.0)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_usage_creation() {
        let usage = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/tokens".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            response_time_ms: 45,
        };

        assert_eq!(usage.endpoint, "/api/v1/tokens");
        assert_eq!(usage.status_code, 200);
    }

    #[test]
    fn test_usage_stats_defaults() {
        let stats = UsageStats {
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            avg_response_time_ms: Some(42.5),
            min_response_time_ms: Some(10),
            max_response_time_ms: Some(150),
        };

        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.successful_requests, 95);
        assert!(stats.avg_response_time_ms.is_some());
    }

    #[test]
    fn test_endpoint_stats_success_rate() {
        let stats = EndpointStats {
            endpoint: "/api/v1/tokens".to_string(),
            method: "GET".to_string(),
            request_count: 100,
            avg_response_time_ms: Some(45.0),
            success_rate: Some(95.0),
        };

        assert!(stats.success_rate.unwrap() > 90.0);
    }

    #[test]
    fn test_hourly_usage_structure() {
        let usage = HourlyUsage {
            hour: Utc::now(),
            request_count: 150,
        };

        assert!(usage.request_count > 0);
    }

    #[test]
    fn test_usage_stats_with_zero_requests() {
        let stats = UsageStats {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_response_time_ms: None,
            min_response_time_ms: None,
            max_response_time_ms: None,
        };

        assert_eq!(stats.total_requests, 0);
        assert!(stats.avg_response_time_ms.is_none());
    }

    #[test]
    fn test_usage_stats_all_successful() {
        let stats = UsageStats {
            total_requests: 100,
            successful_requests: 100,
            failed_requests: 0,
            avg_response_time_ms: Some(50.0),
            min_response_time_ms: Some(20),
            max_response_time_ms: Some(100),
        };

        assert_eq!(stats.failed_requests, 0);
        assert_eq!(stats.successful_requests, stats.total_requests);
    }

    #[test]
    fn test_usage_stats_high_failure_rate() {
        let stats = UsageStats {
            total_requests: 100,
            successful_requests: 10,
            failed_requests: 90,
            avg_response_time_ms: Some(25.0),
            min_response_time_ms: Some(10),
            max_response_time_ms: Some(50),
        };

        assert!(stats.failed_requests > stats.successful_requests);
        let failure_rate = (stats.failed_requests as f64 / stats.total_requests as f64) * 100.0;
        assert_eq!(failure_rate, 90.0);
    }

    #[test]
    fn test_endpoint_stats_perfect_success_rate() {
        let stats = EndpointStats {
            endpoint: "/api/v1/health".to_string(),
            method: "GET".to_string(),
            request_count: 1000,
            avg_response_time_ms: Some(15.0),
            success_rate: Some(100.0),
        };

        assert_eq!(stats.success_rate.unwrap(), 100.0);
        assert!(stats.avg_response_time_ms.unwrap() < 20.0);
    }

    #[test]
    fn test_endpoint_stats_zero_success_rate() {
        let stats = EndpointStats {
            endpoint: "/api/v1/broken".to_string(),
            method: "GET".to_string(),
            request_count: 50,
            avg_response_time_ms: Some(10.0),
            success_rate: Some(0.0),
        };

        assert_eq!(stats.success_rate.unwrap(), 0.0);
    }

    #[test]
    fn test_record_usage_with_different_status_codes() {
        let success_usage = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/tokens".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            response_time_ms: 45,
        };

        let not_found_usage = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/tokens/xyz".to_string(),
            method: "GET".to_string(),
            status_code: 404,
            response_time_ms: 12,
        };

        let server_error_usage = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/orders".to_string(),
            method: "POST".to_string(),
            status_code: 500,
            response_time_ms: 250,
        };

        assert!(success_usage.status_code < 400);
        assert!(not_found_usage.status_code >= 400);
        assert!(server_error_usage.status_code >= 500);
    }

    #[test]
    fn test_hourly_usage_multiple_hours() {
        let hour1 = HourlyUsage {
            hour: Utc::now(),
            request_count: 100,
        };

        let hour2 = HourlyUsage {
            hour: Utc::now() - chrono::Duration::hours(1),
            request_count: 150,
        };

        assert_eq!(hour1.request_count, 100);
        assert_eq!(hour2.request_count, 150);
    }

    #[test]
    fn test_response_time_extremes() {
        let fast_request = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/ping".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            response_time_ms: 1,
        };

        let slow_request = RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/heavy_query".to_string(),
            method: "POST".to_string(),
            status_code: 200,
            response_time_ms: 5000,
        };

        assert!(fast_request.response_time_ms < 10);
        assert!(slow_request.response_time_ms > 1000);
    }

    #[test]
    fn test_endpoint_stats_various_methods() {
        let get_stats = EndpointStats {
            endpoint: "/api/v1/tokens".to_string(),
            method: "GET".to_string(),
            request_count: 100,
            avg_response_time_ms: Some(30.0),
            success_rate: Some(98.0),
        };

        let post_stats = EndpointStats {
            endpoint: "/api/v1/tokens".to_string(),
            method: "POST".to_string(),
            request_count: 50,
            avg_response_time_ms: Some(120.0),
            success_rate: Some(95.0),
        };

        assert_eq!(get_stats.method, "GET");
        assert_eq!(post_stats.method, "POST");
        assert!(post_stats.avg_response_time_ms.unwrap() > get_stats.avg_response_time_ms.unwrap());
    }

    #[test]
    fn test_batch_record_empty_vector() {
        let usages: Vec<RecordUsage> = vec![];
        assert_eq!(usages.len(), 0);
    }

    #[test]
    fn test_batch_record_single_item() {
        let usages = [RecordUsage {
            api_key_id: Uuid::new_v4(),
            endpoint: "/api/v1/tokens".to_string(),
            method: "GET".to_string(),
            status_code: 200,
            response_time_ms: 45,
        }];
        assert_eq!(usages.len(), 1);
    }

    #[test]
    fn test_batch_record_multiple_items() {
        let api_key_id = Uuid::new_v4();
        let usages = [
            RecordUsage {
                api_key_id,
                endpoint: "/api/v1/tokens".to_string(),
                method: "GET".to_string(),
                status_code: 200,
                response_time_ms: 45,
            },
            RecordUsage {
                api_key_id,
                endpoint: "/api/v1/orders".to_string(),
                method: "POST".to_string(),
                status_code: 201,
                response_time_ms: 120,
            },
            RecordUsage {
                api_key_id,
                endpoint: "/api/v1/trades".to_string(),
                method: "GET".to_string(),
                status_code: 200,
                response_time_ms: 60,
            },
        ];
        assert_eq!(usages.len(), 3);
        assert!(usages.iter().all(|u| u.api_key_id == api_key_id));
    }
}
