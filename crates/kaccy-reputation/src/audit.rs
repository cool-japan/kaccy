//! Audit trail module for comprehensive change tracking
//!
//! Provides detailed logging and tracking of all reputation-related
//! changes for compliance, debugging, and transparency.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ReputationError;

/// Service for audit trail management
pub struct AuditService {
    pool: PgPool,
}

impl AuditService {
    /// Create a new [`AuditService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Log an audit event
    pub async fn log_event(&self, event: AuditEvent) -> Result<Uuid, ReputationError> {
        let event_id = Uuid::new_v4();

        let query = r#"
            INSERT INTO audit_trail (
                event_id, event_type, user_id, actor_id, entity_type,
                entity_id, action, old_value, new_value, metadata,
                ip_address, user_agent, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#;

        sqlx::query(query)
            .bind(event_id)
            .bind(event.event_type.to_string())
            .bind(event.user_id)
            .bind(event.actor_id)
            .bind(event.entity_type.to_string())
            .bind(event.entity_id)
            .bind(&event.action)
            .bind(&event.old_value)
            .bind(&event.new_value)
            .bind(&event.metadata)
            .bind(&event.ip_address)
            .bind(&event.user_agent)
            .bind(Utc::now())
            .execute(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        Ok(event_id)
    }

    /// Get audit trail for a specific user
    pub async fn get_user_audit_trail(
        &self,
        user_id: Uuid,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let query = r#"
            SELECT event_id, event_type, user_id, actor_id, entity_type,
                   entity_id, action, old_value, new_value, metadata,
                   ip_address, user_agent, created_at
            FROM audit_trail
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
        "#;

        let rows = sqlx::query(query)
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        self.rows_to_records(rows).await
    }

    /// Get audit trail for a specific entity
    pub async fn get_entity_audit_trail(
        &self,
        entity_id: Uuid,
        entity_type: EntityType,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let query = r#"
            SELECT event_id, event_type, user_id, actor_id, entity_type,
                   entity_id, action, old_value, new_value, metadata,
                   ip_address, user_agent, created_at
            FROM audit_trail
            WHERE entity_id = $1 AND entity_type = $2
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(entity_id)
            .bind(entity_type.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        self.rows_to_records(rows).await
    }

    /// Get audit trail by event type
    pub async fn get_events_by_type(
        &self,
        event_type: AuditEventType,
        limit: i32,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let query = r#"
            SELECT event_id, event_type, user_id, actor_id, entity_type,
                   entity_id, action, old_value, new_value, metadata,
                   ip_address, user_agent, created_at
            FROM audit_trail
            WHERE event_type = $1
            ORDER BY created_at DESC
            LIMIT $2
        "#;

        let rows = sqlx::query(query)
            .bind(event_type.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        self.rows_to_records(rows).await
    }

    /// Get audit summary for a date range
    pub async fn get_audit_summary(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<AuditSummary, ReputationError> {
        let query = r#"
            SELECT
                event_type,
                COUNT(*) as count
            FROM audit_trail
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY event_type
        "#;

        let rows = sqlx::query(query)
            .bind(start_date)
            .bind(end_date)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut events_by_type = Vec::new();
        let mut total_events = 0;

        for row in rows {
            let event_type_str: String = row
                .try_get("event_type")
                .map_err(ReputationError::Database)?;
            let count: i64 = row.try_get("count").map_err(ReputationError::Database)?;

            total_events += count as i32;
            events_by_type.push(EventTypeCount {
                event_type: event_type_str
                    .parse::<AuditEventType>()
                    .map_err(|e| ReputationError::Validation(e.to_string()))?,
                count: count as i32,
            });
        }

        Ok(AuditSummary {
            start_date,
            end_date,
            total_events,
            events_by_type,
        })
    }

    /// Get recent admin actions
    pub async fn get_admin_actions(
        &self,
        admin_id: Uuid,
        limit: i32,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let query = r#"
            SELECT event_id, event_type, user_id, actor_id, entity_type,
                   entity_id, action, old_value, new_value, metadata,
                   ip_address, user_agent, created_at
            FROM audit_trail
            WHERE actor_id = $1
            ORDER BY created_at DESC
            LIMIT $2
        "#;

        let rows = sqlx::query(query)
            .bind(admin_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        self.rows_to_records(rows).await
    }

    /// Search audit trail
    pub async fn search_audit_trail(
        &self,
        search: AuditSearch,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let mut query = String::from(
            r#"
            SELECT event_id, event_type, user_id, actor_id, entity_type,
                   entity_id, action, old_value, new_value, metadata,
                   ip_address, user_agent, created_at
            FROM audit_trail
            WHERE 1=1
        "#,
        );

        if search.user_id.is_some() {
            query.push_str(" AND user_id = $1");
        }
        if search.event_type.is_some() {
            query.push_str(" AND event_type = $2");
        }
        if search.entity_type.is_some() {
            query.push_str(" AND entity_type = $3");
        }
        if search.start_date.is_some() {
            query.push_str(" AND created_at >= $4");
        }
        if search.end_date.is_some() {
            query.push_str(" AND created_at <= $5");
        }

        query.push_str(" ORDER BY created_at DESC LIMIT 100");

        let mut q = sqlx::query(sqlx::AssertSqlSafe(query));

        if let Some(uid) = search.user_id {
            q = q.bind(uid);
        }
        if let Some(et) = search.event_type {
            q = q.bind(et.to_string());
        }
        if let Some(ent) = search.entity_type {
            q = q.bind(ent.to_string());
        }
        if let Some(sd) = search.start_date {
            q = q.bind(sd);
        }
        if let Some(ed) = search.end_date {
            q = q.bind(ed);
        }

        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        self.rows_to_records(rows).await
    }

    // Helper to convert rows to records
    async fn rows_to_records(
        &self,
        rows: Vec<sqlx::postgres::PgRow>,
    ) -> Result<Vec<AuditRecord>, ReputationError> {
        let mut records = Vec::new();

        for row in rows {
            let event_type_str: String = row
                .try_get("event_type")
                .map_err(ReputationError::Database)?;
            let entity_type_str: String = row
                .try_get("entity_type")
                .map_err(ReputationError::Database)?;

            records.push(AuditRecord {
                event_id: row.try_get("event_id").map_err(ReputationError::Database)?,
                event_type: event_type_str
                    .parse::<AuditEventType>()
                    .map_err(|e| ReputationError::Validation(e.to_string()))?,
                user_id: row.try_get("user_id").map_err(ReputationError::Database)?,
                actor_id: row.try_get("actor_id").map_err(ReputationError::Database)?,
                entity_type: entity_type_str
                    .parse::<EntityType>()
                    .map_err(|e| ReputationError::Validation(e.to_string()))?,
                entity_id: row
                    .try_get("entity_id")
                    .map_err(ReputationError::Database)?,
                action: row.try_get("action").map_err(ReputationError::Database)?,
                old_value: row
                    .try_get("old_value")
                    .map_err(ReputationError::Database)?,
                new_value: row
                    .try_get("new_value")
                    .map_err(ReputationError::Database)?,
                metadata: row.try_get("metadata").map_err(ReputationError::Database)?,
                ip_address: row
                    .try_get("ip_address")
                    .map_err(ReputationError::Database)?,
                user_agent: row
                    .try_get("user_agent")
                    .map_err(ReputationError::Database)?,
                created_at: row
                    .try_get("created_at")
                    .map_err(ReputationError::Database)?,
            });
        }

        Ok(records)
    }
}

/// Audit event to be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Classification of the audit event.
    pub event_type: AuditEventType,
    /// User who was affected by the event.
    pub user_id: Uuid,
    /// User or system that triggered the event, if different from `user_id`.
    pub actor_id: Option<Uuid>,
    /// Type of entity that was modified.
    pub entity_type: EntityType,
    /// Identifier of the entity that was modified.
    pub entity_id: Uuid,
    /// Human-readable description of the action performed.
    pub action: String,
    /// Previous value of the modified field, if applicable.
    pub old_value: Option<JsonValue>,
    /// New value of the modified field, if applicable.
    pub new_value: Option<JsonValue>,
    /// Additional structured context for the event.
    pub metadata: Option<JsonValue>,
    /// IP address of the requester, if available.
    pub ip_address: Option<String>,
    /// User-agent string of the requester, if available.
    pub user_agent: Option<String>,
}

/// Audit record from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Unique identifier for this audit record.
    pub event_id: Uuid,
    /// Classification of the audit event.
    pub event_type: AuditEventType,
    /// User who was affected.
    pub user_id: Uuid,
    /// User or system that triggered the event, if different from `user_id`.
    pub actor_id: Option<Uuid>,
    /// Type of entity that was modified.
    pub entity_type: EntityType,
    /// Identifier of the entity that was modified.
    pub entity_id: Uuid,
    /// Human-readable description of the action.
    pub action: String,
    /// Previous value, if applicable.
    pub old_value: Option<JsonValue>,
    /// New value, if applicable.
    pub new_value: Option<JsonValue>,
    /// Additional structured context.
    pub metadata: Option<JsonValue>,
    /// IP address of the requester, if available.
    pub ip_address: Option<String>,
    /// User-agent string of the requester, if available.
    pub user_agent: Option<String>,
    /// Timestamp when this record was created.
    pub created_at: DateTime<Utc>,
}

/// Type of audit event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditEventType {
    /// User's reputation score changed.
    ScoreChange,
    /// User's reputation tier changed.
    TierChange,
    /// A new commitment was created.
    CommitmentCreated,
    /// A commitment was verified as fulfilled.
    CommitmentVerified,
    /// A commitment was marked as failed.
    CommitmentFailed,
    /// An administrator performed a manual action.
    AdminAction,
    /// A dispute was opened against a commitment.
    DisputeCreated,
    /// A dispute was resolved with an outcome.
    DisputeResolved,
    /// A Soulbound Token was minted for a user.
    SBTMinted,
    /// A Soulbound Token was updated.
    SBTUpdated,
    /// An achievement was awarded to a user.
    AchievementAwarded,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventType::ScoreChange => write!(f, "ScoreChange"),
            AuditEventType::TierChange => write!(f, "TierChange"),
            AuditEventType::CommitmentCreated => write!(f, "CommitmentCreated"),
            AuditEventType::CommitmentVerified => write!(f, "CommitmentVerified"),
            AuditEventType::CommitmentFailed => write!(f, "CommitmentFailed"),
            AuditEventType::AdminAction => write!(f, "AdminAction"),
            AuditEventType::DisputeCreated => write!(f, "DisputeCreated"),
            AuditEventType::DisputeResolved => write!(f, "DisputeResolved"),
            AuditEventType::SBTMinted => write!(f, "SBTMinted"),
            AuditEventType::SBTUpdated => write!(f, "SBTUpdated"),
            AuditEventType::AchievementAwarded => write!(f, "AchievementAwarded"),
        }
    }
}

impl std::str::FromStr for AuditEventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ScoreChange" => Ok(AuditEventType::ScoreChange),
            "TierChange" => Ok(AuditEventType::TierChange),
            "CommitmentCreated" => Ok(AuditEventType::CommitmentCreated),
            "CommitmentVerified" => Ok(AuditEventType::CommitmentVerified),
            "CommitmentFailed" => Ok(AuditEventType::CommitmentFailed),
            "AdminAction" => Ok(AuditEventType::AdminAction),
            "DisputeCreated" => Ok(AuditEventType::DisputeCreated),
            "DisputeResolved" => Ok(AuditEventType::DisputeResolved),
            "SBTMinted" => Ok(AuditEventType::SBTMinted),
            "SBTUpdated" => Ok(AuditEventType::SBTUpdated),
            "AchievementAwarded" => Ok(AuditEventType::AchievementAwarded),
            _ => Err(format!("Unknown audit event type: {}", s)),
        }
    }
}

/// Type of entity being audited
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    /// A platform user.
    User,
    /// A commitment record.
    Commitment,
    /// A dispute record.
    Dispute,
    /// A Soulbound Token.
    SBT,
    /// An achievement record.
    Achievement,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::User => write!(f, "User"),
            EntityType::Commitment => write!(f, "Commitment"),
            EntityType::Dispute => write!(f, "Dispute"),
            EntityType::SBT => write!(f, "SBT"),
            EntityType::Achievement => write!(f, "Achievement"),
        }
    }
}

impl std::str::FromStr for EntityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "User" => Ok(EntityType::User),
            "Commitment" => Ok(EntityType::Commitment),
            "Dispute" => Ok(EntityType::Dispute),
            "SBT" => Ok(EntityType::SBT),
            "Achievement" => Ok(EntityType::Achievement),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

/// Audit summary for a date range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    /// Start of the summary period.
    pub start_date: DateTime<Utc>,
    /// End of the summary period.
    pub end_date: DateTime<Utc>,
    /// Total number of audit events in this period.
    pub total_events: i32,
    /// Breakdown of event counts by type.
    pub events_by_type: Vec<EventTypeCount>,
}

/// Event count by type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeCount {
    /// The audit event type.
    pub event_type: AuditEventType,
    /// Number of events of this type in the period.
    pub count: i32,
}

/// Search criteria for audit trail
#[derive(Debug, Clone, Default)]
pub struct AuditSearch {
    /// Filter by user ID, or `None` to match all users.
    pub user_id: Option<Uuid>,
    /// Filter by event type, or `None` to match all types.
    pub event_type: Option<AuditEventType>,
    /// Filter by entity type, or `None` to match all entity types.
    pub entity_type: Option<EntityType>,
    /// Restrict to events at or after this timestamp.
    pub start_date: Option<DateTime<Utc>>,
    /// Restrict to events at or before this timestamp.
    pub end_date: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_type_display() {
        assert_eq!(AuditEventType::ScoreChange.to_string(), "ScoreChange");
        assert_eq!(AuditEventType::TierChange.to_string(), "TierChange");
    }

    #[test]
    fn test_audit_event_type_from_str() {
        assert_eq!(
            "ScoreChange".parse::<AuditEventType>().unwrap(),
            AuditEventType::ScoreChange
        );
        assert_eq!(
            "CommitmentVerified".parse::<AuditEventType>().unwrap(),
            AuditEventType::CommitmentVerified
        );
    }

    #[test]
    fn test_entity_type_display() {
        assert_eq!(EntityType::User.to_string(), "User");
        assert_eq!(EntityType::Commitment.to_string(), "Commitment");
    }

    #[test]
    fn test_entity_type_from_str() {
        assert_eq!("User".parse::<EntityType>().unwrap(), EntityType::User);
        assert_eq!(
            "Dispute".parse::<EntityType>().unwrap(),
            EntityType::Dispute
        );
    }

    #[test]
    fn test_audit_event_structure() {
        let event = AuditEvent {
            event_type: AuditEventType::ScoreChange,
            user_id: Uuid::new_v4(),
            actor_id: None,
            entity_type: EntityType::User,
            entity_id: Uuid::new_v4(),
            action: "score_updated".to_string(),
            old_value: Some(serde_json::json!({"score": 500})),
            new_value: Some(serde_json::json!({"score": 550})),
            metadata: None,
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: None,
        };

        assert_eq!(event.event_type, AuditEventType::ScoreChange);
        assert_eq!(event.entity_type, EntityType::User);
    }

    #[test]
    fn test_audit_search_default() {
        let search = AuditSearch::default();

        assert!(search.user_id.is_none());
        assert!(search.event_type.is_none());
        assert!(search.entity_type.is_none());
    }
}
