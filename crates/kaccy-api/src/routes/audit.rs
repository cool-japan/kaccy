//! Audit log query routes
//!
//! Provides endpoints for querying audit events (admin only)

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{error::ApiError, middleware::Claims, state::AppState};

/// Audit event from database
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    #[sqlx(try_from = "chrono::DateTime<chrono::Utc>")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub source: String,
    pub correlation_id: Option<String>,
    pub user_id: Option<Uuid>,
}

/// Query parameters for listing audit events
#[derive(Debug, Deserialize)]
pub struct AuditEventsQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,
    /// Number of items per page (max 100)
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by user ID
    pub user_id: Option<Uuid>,
    /// Filter by correlation ID
    pub correlation_id: Option<String>,
    /// Filter by source
    pub source: Option<String>,
    /// Start date filter (ISO 8601)
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    /// End date filter (ISO 8601)
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    50
}

/// Response for audit events list
#[derive(Debug, Serialize)]
pub struct AuditEventsResponse {
    pub events: Vec<AuditEvent>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

/// Audit statistics response
#[derive(Debug, Serialize)]
pub struct AuditStatsResponse {
    pub total_events: i64,
    pub event_types: Vec<EventTypeCount>,
    pub events_last_24h: i64,
    pub events_last_7d: i64,
    pub events_last_30d: i64,
}

/// Event type count
#[derive(Debug, Serialize, FromRow)]
pub struct EventTypeCount {
    pub event_type: String,
    pub count: i64,
}

/// List audit events with pagination and filtering
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn list_audit_events(
    _claims: Claims,
    State(state): State<AppState>,
    Query(query): Query<AuditEventsQuery>,
) -> Result<Json<AuditEventsResponse>, ApiError> {
    // Validate date range
    if let (Some(start), Some(end)) = (&query.start_date, &query.end_date) {
        if start > end {
            return Err(ApiError::Validation(
                "start_date must be before end_date".to_string(),
            ));
        }
    }

    let per_page = query.per_page.clamp(1, 100);
    let page = query.page.max(1);
    let offset = (page - 1) * per_page;

    // Build dynamic query using QueryBuilder for proper parameterization
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id FROM audit_events WHERE 1=1",
    );

    // Apply filters dynamically
    if let Some(ref event_type) = query.event_type {
        query_builder.push(" AND event_type = ");
        query_builder.push_bind(event_type);
    }
    if let Some(ref user_id) = query.user_id {
        query_builder.push(" AND user_id = ");
        query_builder.push_bind(user_id);
    }
    if let Some(ref correlation_id) = query.correlation_id {
        query_builder.push(" AND correlation_id = ");
        query_builder.push_bind(correlation_id);
    }
    if let Some(ref source) = query.source {
        query_builder.push(" AND source = ");
        query_builder.push_bind(source);
    }
    if let Some(ref start_date) = query.start_date {
        query_builder.push(" AND timestamp >= ");
        query_builder.push_bind(start_date);
    }
    if let Some(ref end_date) = query.end_date {
        query_builder.push(" AND timestamp <= ");
        query_builder.push_bind(end_date);
    }

    query_builder.push(" ORDER BY timestamp DESC LIMIT ");
    query_builder.push_bind(per_page);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    // Execute main query
    let events: Vec<AuditEvent> = query_builder
        .build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::Database)?;

    // Build count query with same filters
    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM audit_events WHERE 1=1");

    if let Some(ref event_type) = query.event_type {
        count_builder.push(" AND event_type = ");
        count_builder.push_bind(event_type);
    }
    if let Some(ref user_id) = query.user_id {
        count_builder.push(" AND user_id = ");
        count_builder.push_bind(user_id);
    }
    if let Some(ref correlation_id) = query.correlation_id {
        count_builder.push(" AND correlation_id = ");
        count_builder.push_bind(correlation_id);
    }
    if let Some(ref source) = query.source {
        count_builder.push(" AND source = ");
        count_builder.push_bind(source);
    }
    if let Some(ref start_date) = query.start_date {
        count_builder.push(" AND timestamp >= ");
        count_builder.push_bind(start_date);
    }
    if let Some(ref end_date) = query.end_date {
        count_builder.push(" AND timestamp <= ");
        count_builder.push_bind(end_date);
    }

    // Execute count query
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::Database)?;

    let total_pages = (total + per_page - 1) / per_page;

    Ok(Json(AuditEventsResponse {
        events,
        total,
        page,
        per_page,
        total_pages,
    }))
}

/// Get a specific audit event by ID
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_audit_event(
    _claims: Claims,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<AuditEvent>, ApiError> {
    let event: AuditEvent = sqlx::query_as(
        "SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id
         FROM audit_events
         WHERE id = $1",
    )
    .bind(event_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => ApiError::NotFound("Audit event not found".to_string()),
        _ => ApiError::Database(e),
    })?;

    Ok(Json(event))
}

/// Get audit events for a specific user
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_user_audit_events(
    _claims: Claims,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<AuditEventsQuery>,
) -> Result<Json<AuditEventsResponse>, ApiError> {
    let per_page = query.per_page.clamp(1, 100);
    let page = query.page.max(1);
    let offset = (page - 1) * per_page;

    let events: Vec<AuditEvent> = sqlx::query_as(
        "SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id
         FROM audit_events
         WHERE user_id = $1
         ORDER BY timestamp DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::Database)?;

    let total_pages = (total + per_page - 1) / per_page;

    Ok(Json(AuditEventsResponse {
        events,
        total,
        page,
        per_page,
        total_pages,
    }))
}

/// Get audit statistics
///
/// Admin only endpoint
#[allow(dead_code)]
pub async fn get_audit_stats(
    _claims: Claims,
    State(state): State<AppState>,
) -> Result<Json<AuditStatsResponse>, ApiError> {
    let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events")
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::Database)?;

    let event_types: Vec<EventTypeCount> = sqlx::query_as(
        "SELECT event_type, COUNT(*) as count
         FROM audit_events
         GROUP BY event_type
         ORDER BY count DESC
         LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    let events_last_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events
         WHERE timestamp >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    let events_last_7d: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events
         WHERE timestamp >= NOW() - INTERVAL '7 days'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    let events_last_30d: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_events
         WHERE timestamp >= NOW() - INTERVAL '30 days'",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(AuditStatsResponse {
        total_events,
        event_types,
        events_last_24h,
        events_last_7d,
        events_last_30d,
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
}
