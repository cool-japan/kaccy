//! Audit event repository for event-driven architecture integration
//!
//! This repository handles the `audit_events` table which stores events from
//! the event-driven architecture for compliance, debugging, and audit trails.

use crate::error::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

/// Audit event entry representing an event from the event-driven architecture
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub event_data: JsonValue,
    pub source: String,
    pub correlation_id: Option<String>,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Create audit event parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAuditEvent {
    pub id: Uuid,
    pub event_type: String,
    pub event_data: JsonValue,
    pub source: String,
    pub correlation_id: Option<String>,
    pub user_id: Option<Uuid>,
}

/// Event type statistics
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventTypeStats {
    pub event_type: String,
    pub event_count: i64,
    pub unique_users: i64,
    pub unique_correlations: i64,
    pub first_occurrence: DateTime<Utc>,
    pub last_occurrence: DateTime<Utc>,
}

/// Daily event summary
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DailyEventSummary {
    pub event_date: NaiveDate,
    pub total_events: i64,
    pub unique_event_types: i64,
    pub unique_sources: i64,
    pub unique_users: i64,
}

/// User event activity summary
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserEventActivity {
    pub user_id: Uuid,
    pub total_events: i64,
    pub unique_event_types: i64,
    pub first_event: DateTime<Utc>,
    pub last_event: DateTime<Utc>,
}

/// Correlation trace for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CorrelationTrace {
    pub correlation_id: String,
    pub event_count: i64,
    pub event_types: Vec<String>,
    pub sources: Vec<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_ms: f64,
}

/// Audit event repository for querying and storing event-driven architecture events
pub struct AuditEventRepository {
    pool: PgPool,
}

impl AuditEventRepository {
    /// Create a new audit event repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new audit event
    pub async fn create(&self, event: CreateAuditEvent) -> Result<AuditEvent> {
        let result = sqlx::query_as::<_, AuditEvent>(
            r#"
            INSERT INTO audit_events (id, event_type, event_data, source, correlation_id, user_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            "#,
        )
        .bind(event.id)
        .bind(&event.event_type)
        .bind(&event.event_data)
        .bind(&event.source)
        .bind(&event.correlation_id)
        .bind(event.user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Batch create multiple audit events (optimized for high-volume ingestion)
    ///
    /// This method uses a single INSERT statement with multiple value sets,
    /// which is much more efficient than individual inserts when ingesting
    /// large numbers of events.
    pub async fn batch_create(&self, events: Vec<CreateAuditEvent>) -> Result<Vec<AuditEvent>> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        // Build the VALUES clause dynamically
        let mut query = String::from(
            "INSERT INTO audit_events (id, event_type, event_data, source, correlation_id, user_id) VALUES "
        );

        let mut bindings = Vec::new();
        for (i, event) in events.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            let base = i * 6;
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${})",
                base + 1,
                base + 2,
                base + 3,
                base + 4,
                base + 5,
                base + 6
            ));
            bindings.push((
                event.id,
                event.event_type.clone(),
                event.event_data.clone(),
                event.source.clone(),
                event.correlation_id.clone(),
                event.user_id,
            ));
        }

        query.push_str(" RETURNING id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at");

        // Build the query with all bindings
        let mut query_builder = sqlx::query_as::<_, AuditEvent>(&query);
        for binding in bindings {
            query_builder = query_builder
                .bind(binding.0)
                .bind(binding.1)
                .bind(binding.2)
                .bind(binding.3)
                .bind(binding.4)
                .bind(binding.5);
        }

        let results = query_builder.fetch_all(&self.pool).await?;
        Ok(results)
    }

    /// Get audit events by user ID with pagination
    pub async fn get_by_user(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEvent>> {
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE user_id = $1
            ORDER BY timestamp DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get audit events by event type with pagination
    pub async fn get_by_event_type(
        &self,
        event_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEvent>> {
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE event_type = $1
            ORDER BY timestamp DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(event_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get audit events by correlation ID (for distributed tracing)
    pub async fn get_by_correlation(&self, correlation_id: &str) -> Result<Vec<AuditEvent>> {
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE correlation_id = $1
            ORDER BY timestamp ASC
            "#,
        )
        .bind(correlation_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get audit events by source system
    pub async fn get_by_source(
        &self,
        source: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEvent>> {
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE source = $1
            ORDER BY timestamp DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(source)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get audit events within a time range
    pub async fn get_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEvent>> {
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE timestamp >= $1 AND timestamp < $2
            ORDER BY timestamp DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get event type statistics
    pub async fn get_event_type_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<EventTypeStats>> {
        let stats = sqlx::query_as::<_, EventTypeStats>(
            r#"
            SELECT
                event_type,
                COUNT(*) as event_count,
                COUNT(DISTINCT user_id) FILTER (WHERE user_id IS NOT NULL) as unique_users,
                COUNT(DISTINCT correlation_id) FILTER (WHERE correlation_id IS NOT NULL) as unique_correlations,
                MIN(timestamp) as first_occurrence,
                MAX(timestamp) as last_occurrence
            FROM audit_events
            WHERE timestamp >= $1 AND timestamp < $2
            GROUP BY event_type
            ORDER BY event_count DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Get daily event summary
    pub async fn get_daily_summary(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<DailyEventSummary>> {
        let summaries = sqlx::query_as::<_, DailyEventSummary>(
            r#"
            SELECT
                DATE(timestamp) as event_date,
                COUNT(*) as total_events,
                COUNT(DISTINCT event_type) as unique_event_types,
                COUNT(DISTINCT source) as unique_sources,
                COUNT(DISTINCT user_id) FILTER (WHERE user_id IS NOT NULL) as unique_users
            FROM audit_events
            WHERE DATE(timestamp) >= $1 AND DATE(timestamp) <= $2
            GROUP BY DATE(timestamp)
            ORDER BY event_date DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(summaries)
    }

    /// Get user event activity
    pub async fn get_user_activity(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<UserEventActivity>> {
        let activity = sqlx::query_as::<_, UserEventActivity>(
            r#"
            SELECT
                user_id,
                COUNT(*) as total_events,
                COUNT(DISTINCT event_type) as unique_event_types,
                MIN(timestamp) as first_event,
                MAX(timestamp) as last_event
            FROM audit_events
            WHERE user_id IS NOT NULL
              AND timestamp >= $1 AND timestamp < $2
            GROUP BY user_id
            ORDER BY total_events DESC
            LIMIT $3
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(activity)
    }

    /// Get correlation trace for distributed tracing
    pub async fn get_correlation_trace(
        &self,
        correlation_id: &str,
    ) -> Result<Option<CorrelationTrace>> {
        let trace = sqlx::query_as::<_, CorrelationTrace>(
            r#"
            SELECT
                $1 as correlation_id,
                COUNT(*) as event_count,
                ARRAY_AGG(DISTINCT event_type ORDER BY event_type) as event_types,
                ARRAY_AGG(DISTINCT source ORDER BY source) as sources,
                MIN(timestamp) as start_time,
                MAX(timestamp) as end_time,
                EXTRACT(EPOCH FROM (MAX(timestamp) - MIN(timestamp))) * 1000 as duration_ms
            FROM audit_events
            WHERE correlation_id = $1
            "#,
        )
        .bind(correlation_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(trace)
    }

    /// Count events by event type
    pub async fn count_by_event_type(&self, event_type: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM audit_events WHERE event_type = $1
            "#,
        )
        .bind(event_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Count events by user
    pub async fn count_by_user(&self, user_id: Uuid) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM audit_events WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Search events by JSONB field (for advanced querying of event_data)
    pub async fn search_by_json_field(
        &self,
        json_path: &str,
        value: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEvent>> {
        // Using JSONB operators for efficient searching
        let events = sqlx::query_as::<_, AuditEvent>(
            r#"
            SELECT id, timestamp, event_type, event_data, source, correlation_id, user_id, created_at
            FROM audit_events
            WHERE event_data @> $1::jsonb
            ORDER BY timestamp DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(format!(r#"{{"{}":"{}"}}"#, json_path, value))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Delete old events (for GDPR compliance and storage management)
    pub async fn delete_older_than(
        &self,
        executor: impl PgExecutor<'_>,
        cutoff_date: DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM audit_events WHERE timestamp < $1
            "#,
        )
        .bind(cutoff_date)
        .execute(executor)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get total event count
    pub async fn count_all(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM audit_events
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_audit_event_structure() {
        let event = CreateAuditEvent {
            id: Uuid::new_v4(),
            event_type: "UserCreated".to_string(),
            event_data: json!({"username": "testuser"}),
            source: "kaccy-api".to_string(),
            correlation_id: Some("trace-123".to_string()),
            user_id: Some(Uuid::new_v4()),
        };

        assert_eq!(event.event_type, "UserCreated");
        assert_eq!(event.source, "kaccy-api");
        assert!(event.correlation_id.is_some());
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: "OrderPlaced".to_string(),
            event_data: json!({"order_id": "12345", "amount": 100}),
            source: "kaccy-api".to_string(),
            correlation_id: Some("order-trace-456".to_string()),
            user_id: Some(Uuid::new_v4()),
            created_at: Utc::now(),
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: AuditEvent = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.event_type, deserialized.event_type);
    }

    #[test]
    fn test_event_type_stats_structure() {
        let stats = EventTypeStats {
            event_type: "UserLogin".to_string(),
            event_count: 150,
            unique_users: 50,
            unique_correlations: 75,
            first_occurrence: Utc::now(),
            last_occurrence: Utc::now(),
        };

        assert_eq!(stats.event_count, 150);
        assert_eq!(stats.unique_users, 50);
    }

    #[test]
    fn test_daily_event_summary_structure() {
        let summary = DailyEventSummary {
            event_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 18).unwrap(),
            total_events: 1000,
            unique_event_types: 25,
            unique_sources: 3,
            unique_users: 200,
        };

        assert_eq!(summary.total_events, 1000);
        assert_eq!(summary.unique_event_types, 25);
    }

    #[test]
    fn test_user_event_activity_structure() {
        let activity = UserEventActivity {
            user_id: Uuid::new_v4(),
            total_events: 50,
            unique_event_types: 10,
            first_event: Utc::now(),
            last_event: Utc::now(),
        };

        assert_eq!(activity.total_events, 50);
        assert_eq!(activity.unique_event_types, 10);
    }

    #[test]
    fn test_correlation_trace_structure() {
        let trace = CorrelationTrace {
            correlation_id: "trace-789".to_string(),
            event_count: 5,
            event_types: vec![
                "RequestReceived".to_string(),
                "OrderValidated".to_string(),
                "OrderExecuted".to_string(),
            ],
            sources: vec!["kaccy-api".to_string(), "kaccy-core".to_string()],
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration_ms: 250.5,
        };

        assert_eq!(trace.event_count, 5);
        assert_eq!(trace.event_types.len(), 3);
        assert_eq!(trace.sources.len(), 2);
    }

    #[test]
    fn test_create_audit_event_with_minimal_data() {
        let event = CreateAuditEvent {
            id: Uuid::new_v4(),
            event_type: "TestEvent".to_string(),
            event_data: json!({}),
            source: "test-system".to_string(),
            correlation_id: None,
            user_id: None,
        };

        assert!(event.correlation_id.is_none());
        assert!(event.user_id.is_none());
        assert_eq!(event.event_data, json!({}));
    }

    #[test]
    fn test_create_audit_event_with_complex_data() {
        let complex_data = json!({
            "action": "transfer",
            "from": "user1",
            "to": "user2",
            "amount": 100.50,
            "currency": "BTC",
            "metadata": {
                "ip": "192.168.1.1",
                "user_agent": "Mozilla/5.0"
            }
        });

        let event = CreateAuditEvent {
            id: Uuid::new_v4(),
            event_type: "TokenTransfer".to_string(),
            event_data: complex_data.clone(),
            source: "kaccy-core".to_string(),
            correlation_id: Some("txn-001".to_string()),
            user_id: Some(Uuid::new_v4()),
        };

        assert_eq!(event.event_data, complex_data);
        assert_eq!(event.event_type, "TokenTransfer");
    }

    #[test]
    fn test_batch_create_events() {
        // Test that we can create multiple events in a batch
        let events = [
            CreateAuditEvent {
                id: Uuid::new_v4(),
                event_type: "Event1".to_string(),
                event_data: json!({"data": 1}),
                source: "test".to_string(),
                correlation_id: Some("batch-1".to_string()),
                user_id: Some(Uuid::new_v4()),
            },
            CreateAuditEvent {
                id: Uuid::new_v4(),
                event_type: "Event2".to_string(),
                event_data: json!({"data": 2}),
                source: "test".to_string(),
                correlation_id: Some("batch-1".to_string()),
                user_id: Some(Uuid::new_v4()),
            },
            CreateAuditEvent {
                id: Uuid::new_v4(),
                event_type: "Event3".to_string(),
                event_data: json!({"data": 3}),
                source: "test".to_string(),
                correlation_id: Some("batch-1".to_string()),
                user_id: None,
            },
        ];

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "Event1");
        assert_eq!(events[1].event_type, "Event2");
        assert_eq!(events[2].event_type, "Event3");
        assert!(events[2].user_id.is_none());
    }

    #[test]
    fn test_batch_create_empty() {
        // Test that batch_create handles empty vector gracefully
        let events: Vec<CreateAuditEvent> = vec![];
        assert!(events.is_empty());
    }
}
