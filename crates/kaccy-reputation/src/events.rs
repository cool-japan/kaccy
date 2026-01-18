//! Reputation events module - event sourcing and notifications
//!
//! This module provides:
//! - Structured event types for all reputation changes
//! - Event sourcing (replay events to recalculate score)
//! - Event notifications for score changes

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::Result;

/// All possible reputation event types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Commitment fulfilled successfully
    CommitmentFulfilled,
    /// Commitment failed verification
    CommitmentFailed,
    /// Commitment expired without evidence
    CommitmentExpired,
    /// Milestone completed
    MilestoneCompleted,
    /// Quality score from verification
    QualityScore,
    /// Holder rating received
    HolderRating,
    /// Admin manual adjustment
    AdminAdjustment,
    /// Account created (initial bonus)
    AccountCreated,
    /// First token issued bonus
    FirstTokenIssued,
    /// Time decay applied
    TimeDecay,
    /// Circuit breaker triggered
    CircuitBreakerTriggered,
    /// Circuit breaker recovered
    CircuitBreakerRecovered,
}

impl EventType {
    /// Get all event types
    pub fn all() -> Vec<EventType> {
        vec![
            EventType::CommitmentFulfilled,
            EventType::CommitmentFailed,
            EventType::CommitmentExpired,
            EventType::MilestoneCompleted,
            EventType::QualityScore,
            EventType::HolderRating,
            EventType::AdminAdjustment,
            EventType::AccountCreated,
            EventType::FirstTokenIssued,
            EventType::TimeDecay,
            EventType::CircuitBreakerTriggered,
            EventType::CircuitBreakerRecovered,
        ]
    }

    /// Convert to database string representation
    pub fn to_db_string(&self) -> &'static str {
        match self {
            EventType::CommitmentFulfilled => "commitment_fulfilled",
            EventType::CommitmentFailed => "commitment_failed",
            EventType::CommitmentExpired => "commitment_expired",
            EventType::MilestoneCompleted => "milestone_completed",
            EventType::QualityScore => "quality_score",
            EventType::HolderRating => "holder_rating",
            EventType::AdminAdjustment => "admin_adjustment",
            EventType::AccountCreated => "account_created",
            EventType::FirstTokenIssued => "first_token_issued",
            EventType::TimeDecay => "time_decay",
            EventType::CircuitBreakerTriggered => "circuit_breaker_triggered",
            EventType::CircuitBreakerRecovered => "circuit_breaker_recovered",
        }
    }

    /// Parse from database string
    pub fn from_db_string(s: &str) -> Option<EventType> {
        match s {
            "commitment_fulfilled" => Some(EventType::CommitmentFulfilled),
            "commitment_failed" => Some(EventType::CommitmentFailed),
            "commitment_expired" => Some(EventType::CommitmentExpired),
            "milestone_completed" => Some(EventType::MilestoneCompleted),
            "quality_score" => Some(EventType::QualityScore),
            "holder_rating" => Some(EventType::HolderRating),
            "admin_adjustment" => Some(EventType::AdminAdjustment),
            "account_created" => Some(EventType::AccountCreated),
            "first_token_issued" => Some(EventType::FirstTokenIssued),
            "time_decay" => Some(EventType::TimeDecay),
            "circuit_breaker_triggered" => Some(EventType::CircuitBreakerTriggered),
            "circuit_breaker_recovered" => Some(EventType::CircuitBreakerRecovered),
            _ => None,
        }
    }

    /// Default delta for this event type
    pub fn default_delta(&self) -> Decimal {
        match self {
            EventType::CommitmentFulfilled => dec!(10),
            EventType::CommitmentFailed => dec!(-20),
            EventType::CommitmentExpired => dec!(-15),
            EventType::MilestoneCompleted => dec!(5),
            EventType::QualityScore => dec!(0),     // Varies
            EventType::HolderRating => dec!(0),     // Varies
            EventType::AdminAdjustment => dec!(0),  // Varies
            EventType::AccountCreated => dec!(500), // Starting score
            EventType::FirstTokenIssued => dec!(20),
            EventType::TimeDecay => dec!(0), // Varies (always negative)
            EventType::CircuitBreakerTriggered => dec!(-50),
            EventType::CircuitBreakerRecovered => dec!(25),
        }
    }

    /// Check if this is a positive event type (typically increases reputation)
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            EventType::CommitmentFulfilled
                | EventType::MilestoneCompleted
                | EventType::AccountCreated
                | EventType::FirstTokenIssued
                | EventType::CircuitBreakerRecovered
        )
    }

    /// Check if this is a negative event type (typically decreases reputation)
    pub fn is_negative(&self) -> bool {
        matches!(
            self,
            EventType::CommitmentFailed
                | EventType::CommitmentExpired
                | EventType::TimeDecay
                | EventType::CircuitBreakerTriggered
        )
    }

    /// Check if this is a neutral event type (variable delta based on context)
    pub fn is_neutral(&self) -> bool {
        matches!(
            self,
            EventType::QualityScore | EventType::HolderRating | EventType::AdminAdjustment
        )
    }

    /// Check if this event is related to commitments or milestones
    pub fn is_commitment_related(&self) -> bool {
        matches!(
            self,
            EventType::CommitmentFulfilled
                | EventType::CommitmentFailed
                | EventType::CommitmentExpired
                | EventType::MilestoneCompleted
        )
    }

    /// Check if this is a system-generated event (not directly user-initiated)
    pub fn is_system_event(&self) -> bool {
        matches!(
            self,
            EventType::TimeDecay
                | EventType::CircuitBreakerTriggered
                | EventType::CircuitBreakerRecovered
                | EventType::AccountCreated
        )
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_db_string())
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_db_string(s).ok_or_else(|| format!("Invalid event type: {}", s))
    }
}

impl TryFrom<String> for EventType {
    type Error = String;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<&str> for EventType {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

/// A reputation event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReputationEvent {
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub event_type: String,
    pub delta: Decimal,
    pub reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl ReputationEvent {
    /// Get parsed event type
    pub fn parsed_type(&self) -> Option<EventType> {
        EventType::from_db_string(&self.event_type)
    }
}

/// Request to create a new event
#[derive(Debug, Clone)]
pub struct CreateEventRequest {
    pub user_id: Uuid,
    pub event_type: EventType,
    pub delta: Option<Decimal>,
    pub reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Event summary for analytics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventSummary {
    pub total_events: i64,
    pub positive_events: i64,
    pub negative_events: i64,
    pub total_positive_delta: Decimal,
    pub total_negative_delta: Decimal,
    pub net_delta: Decimal,
    pub events_by_type: HashMap<String, i64>,
}

/// User score recalculated from events
#[derive(Debug, Clone, Serialize)]
pub struct RecalculatedScore {
    pub user_id: Uuid,
    pub calculated_score: Decimal,
    pub current_score: Decimal,
    pub difference: Decimal,
    pub event_count: i64,
}

/// Notification for score changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreNotification {
    pub user_id: Uuid,
    pub event_type: String,
    pub old_score: Decimal,
    pub new_score: Decimal,
    pub delta: Decimal,
    pub message: String,
    pub severity: NotificationSeverity,
    pub created_at: DateTime<Utc>,
}

/// Notification severity level
///
/// Ordered from least to most severe: Info < Warning < Critical
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum NotificationSeverity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for NotificationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "Info"),
            Self::Warning => write!(f, "Warning"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

impl std::str::FromStr for NotificationSeverity {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "info" => Ok(NotificationSeverity::Info),
            "warning" => Ok(NotificationSeverity::Warning),
            "critical" => Ok(NotificationSeverity::Critical),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid notification severity: {}. Valid values: info, warning, critical",
                s
            ))),
        }
    }
}

impl NotificationSeverity {
    fn from_delta(delta: Decimal) -> Self {
        if delta < dec!(-30) {
            NotificationSeverity::Critical
        } else if delta < dec!(0) {
            NotificationSeverity::Warning
        } else {
            NotificationSeverity::Info
        }
    }

    /// Get all severity variants in order from lowest to highest
    pub fn all_variants() -> [Self; 3] {
        [Self::Info, Self::Warning, Self::Critical]
    }

    /// Get severity as string (lowercase)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    /// Check if this severity is info level
    pub fn is_info(&self) -> bool {
        matches!(self, Self::Info)
    }

    /// Check if this severity is warning level
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::Warning)
    }

    /// Check if this severity is critical level
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }
}

/// Service for managing reputation events
pub struct EventService {
    pool: PgPool,
    notification_handlers: Vec<Box<dyn NotificationHandler + Send + Sync>>,
}

/// Trait for handling notifications
pub trait NotificationHandler: Send + Sync {
    fn handle(&self, notification: &ScoreNotification);
}

/// Default log-based notification handler
#[derive(Default)]
pub struct LogNotificationHandler;

impl NotificationHandler for LogNotificationHandler {
    fn handle(&self, notification: &ScoreNotification) {
        match notification.severity {
            NotificationSeverity::Critical => {
                tracing::error!(
                    user_id = %notification.user_id,
                    event = %notification.event_type,
                    delta = %notification.delta,
                    new_score = %notification.new_score,
                    "{}",
                    notification.message
                );
            }
            NotificationSeverity::Warning => {
                tracing::warn!(
                    user_id = %notification.user_id,
                    event = %notification.event_type,
                    delta = %notification.delta,
                    new_score = %notification.new_score,
                    "{}",
                    notification.message
                );
            }
            NotificationSeverity::Info => {
                tracing::info!(
                    user_id = %notification.user_id,
                    event = %notification.event_type,
                    delta = %notification.delta,
                    new_score = %notification.new_score,
                    "{}",
                    notification.message
                );
            }
        }
    }
}

impl EventService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            notification_handlers: vec![Box::new(LogNotificationHandler)],
        }
    }

    /// Add a notification handler
    pub fn add_notification_handler(
        &mut self,
        handler: Box<dyn NotificationHandler + Send + Sync>,
    ) {
        self.notification_handlers.push(handler);
    }

    /// Create a new reputation event and update score
    pub async fn create_event(&self, request: &CreateEventRequest) -> Result<ReputationEvent> {
        let delta = request
            .delta
            .unwrap_or_else(|| request.event_type.default_delta());

        // Get current score for notification
        let old_score = self.get_user_score(request.user_id).await?;

        let mut tx = self.pool.begin().await?;

        // Insert event
        let event: ReputationEvent = sqlx::query_as(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING event_id, user_id, event_type, delta, reason,
                      metadata::jsonb as metadata, created_at
            "#,
        )
        .bind(request.user_id)
        .bind(request.event_type.to_db_string())
        .bind(delta)
        .bind(&request.reason)
        .bind(&request.metadata)
        .fetch_one(&mut *tx)
        .await?;

        // Update user score
        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST(0, LEAST(1000, reputation_score + $2))
            WHERE user_id = $1
            RETURNING reputation_score
            "#,
        )
        .bind(request.user_id)
        .bind(delta)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        // Send notification
        let notification = self.create_notification(
            request.user_id,
            &request.event_type,
            old_score,
            new_score,
            delta,
        );

        for handler in &self.notification_handlers {
            handler.handle(&notification);
        }

        Ok(event)
    }

    /// Get user's current score
    async fn get_user_score(&self, user_id: Uuid) -> Result<Decimal> {
        let result: Option<(Decimal,)> =
            sqlx::query_as("SELECT reputation_score FROM users WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|(s,)| s).unwrap_or(dec!(500)))
    }

    /// Create notification from event
    fn create_notification(
        &self,
        user_id: Uuid,
        event_type: &EventType,
        old_score: Decimal,
        new_score: Decimal,
        delta: Decimal,
    ) -> ScoreNotification {
        let message = match event_type {
            EventType::CommitmentFulfilled => {
                format!("Commitment fulfilled! +{} reputation", delta)
            }
            EventType::CommitmentFailed => {
                format!("Commitment failed. {} reputation", delta)
            }
            EventType::CommitmentExpired => {
                format!("Commitment expired. {} reputation", delta)
            }
            EventType::MilestoneCompleted => {
                format!("Milestone completed! +{} reputation", delta)
            }
            EventType::HolderRating => {
                if delta >= dec!(0) {
                    format!("Positive holder rating! +{} reputation", delta)
                } else {
                    format!("Negative holder rating. {} reputation", delta)
                }
            }
            EventType::AdminAdjustment => {
                format!("Admin adjustment: {} reputation", delta)
            }
            EventType::AccountCreated => "Welcome! Your account has been created.".to_string(),
            EventType::FirstTokenIssued => {
                format!("First token issued! +{} reputation bonus", delta)
            }
            EventType::TimeDecay => {
                format!("Inactivity decay: {} reputation", delta)
            }
            EventType::CircuitBreakerTriggered => {
                "Warning: Reputation too low. Token issuance disabled.".to_string()
            }
            EventType::CircuitBreakerRecovered => {
                "Reputation recovered! Token issuance re-enabled.".to_string()
            }
            EventType::QualityScore => {
                format!("Quality score: {} reputation", delta)
            }
        };

        ScoreNotification {
            user_id,
            event_type: event_type.to_string(),
            old_score,
            new_score,
            delta,
            message,
            severity: NotificationSeverity::from_delta(delta),
            created_at: Utc::now(),
        }
    }

    /// Get user's event history
    pub async fn get_user_events(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReputationEvent>> {
        let events = sqlx::query_as::<_, ReputationEvent>(
            r#"
            SELECT event_id, user_id, event_type, delta, reason,
                   metadata::jsonb as metadata, created_at
            FROM reputation_events
            WHERE user_id = $1
            ORDER BY created_at DESC
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

    /// Get event summary for a user
    pub async fn get_event_summary(&self, user_id: Uuid) -> Result<EventSummary> {
        // Get counts by type
        let type_counts: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT event_type, COUNT(*) as count
            FROM reputation_events
            WHERE user_id = $1
            GROUP BY event_type
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        // Get aggregate stats
        let stats: Option<(i64, Decimal, Decimal)> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN delta > 0 THEN delta ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN delta < 0 THEN delta ELSE 0 END), 0)
            FROM reputation_events
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (total_events, total_positive, total_negative) = stats.unwrap_or((0, dec!(0), dec!(0)));

        let mut events_by_type = HashMap::new();
        let mut positive_events = 0i64;
        let mut negative_events = 0i64;

        for (event_type, count) in type_counts {
            if let Some(et) = EventType::from_db_string(&event_type) {
                if et.is_positive() {
                    positive_events += count;
                } else {
                    negative_events += count;
                }
            }
            events_by_type.insert(event_type, count);
        }

        Ok(EventSummary {
            total_events,
            positive_events,
            negative_events,
            total_positive_delta: total_positive,
            total_negative_delta: total_negative,
            net_delta: total_positive + total_negative,
            events_by_type,
        })
    }

    /// Replay all events to recalculate score (event sourcing)
    pub async fn replay_events(&self, user_id: Uuid) -> Result<RecalculatedScore> {
        // Get all events in chronological order
        let events: Vec<(Decimal,)> = sqlx::query_as(
            r#"
            SELECT delta
            FROM reputation_events
            WHERE user_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        // Calculate score from events
        let mut calculated_score = dec!(500); // Starting score
        for (delta,) in &events {
            calculated_score = (calculated_score + delta).max(dec!(0)).min(dec!(1000));
        }

        // Get current score
        let current_score = self.get_user_score(user_id).await?;

        Ok(RecalculatedScore {
            user_id,
            calculated_score,
            current_score,
            difference: calculated_score - current_score,
            event_count: events.len() as i64,
        })
    }

    /// Sync score with events (fix any drift)
    pub async fn sync_score_with_events(&self, user_id: Uuid) -> Result<RecalculatedScore> {
        let recalculated = self.replay_events(user_id).await?;

        if recalculated.difference.abs() > dec!(0.01) {
            sqlx::query(
                r#"
                UPDATE users
                SET reputation_score = $2
                WHERE user_id = $1
                "#,
            )
            .bind(user_id)
            .bind(recalculated.calculated_score)
            .execute(&self.pool)
            .await?;

            tracing::warn!(
                user_id = %user_id,
                old_score = %recalculated.current_score,
                new_score = %recalculated.calculated_score,
                difference = %recalculated.difference,
                "Score synced with events - drift corrected"
            );
        }

        Ok(recalculated)
    }

    /// Get recent events for all users (admin view)
    pub async fn get_recent_events(&self, limit: i64) -> Result<Vec<ReputationEvent>> {
        let events = sqlx::query_as::<_, ReputationEvent>(
            r#"
            SELECT event_id, user_id, event_type, delta, reason,
                   metadata::jsonb as metadata, created_at
            FROM reputation_events
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get events by type
    pub async fn get_events_by_type(
        &self,
        event_type: EventType,
        limit: i64,
    ) -> Result<Vec<ReputationEvent>> {
        let events = sqlx::query_as::<_, ReputationEvent>(
            r#"
            SELECT event_id, user_id, event_type, delta, reason,
                   metadata::jsonb as metadata, created_at
            FROM reputation_events
            WHERE event_type = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(event_type.to_db_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Batch check all users for event/score drift
    pub async fn audit_all_scores(&self) -> Result<Vec<RecalculatedScore>> {
        let user_ids: Vec<(Uuid,)> = sqlx::query_as("SELECT user_id FROM users")
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for (user_id,) in user_ids {
            let recalc = self.replay_events(user_id).await?;
            if recalc.difference.abs() > dec!(0.01) {
                results.push(recalc);
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_severity_display() {
        assert_eq!(NotificationSeverity::Info.to_string(), "Info");
        assert_eq!(NotificationSeverity::Warning.to_string(), "Warning");
        assert_eq!(NotificationSeverity::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_notification_severity_from_str_valid() {
        assert_eq!(
            "info".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Info
        );
        assert_eq!(
            "warning".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Warning
        );
        assert_eq!(
            "critical".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Critical
        );
    }

    #[test]
    fn test_notification_severity_from_str_case_insensitive() {
        assert_eq!(
            "INFO".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Info
        );
        assert_eq!(
            "Warning".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Warning
        );
        assert_eq!(
            "CRITICAL".parse::<NotificationSeverity>().unwrap(),
            NotificationSeverity::Critical
        );
    }

    #[test]
    fn test_notification_severity_from_str_invalid() {
        assert!("invalid".parse::<NotificationSeverity>().is_err());
        assert!("error".parse::<NotificationSeverity>().is_err());
        assert!("".parse::<NotificationSeverity>().is_err());
    }

    #[test]
    fn test_notification_severity_ordering() {
        // Test basic ordering: Info < Warning < Critical
        assert!(NotificationSeverity::Info < NotificationSeverity::Warning);
        assert!(NotificationSeverity::Warning < NotificationSeverity::Critical);
        assert!(NotificationSeverity::Info < NotificationSeverity::Critical);
    }

    #[test]
    fn test_notification_severity_ordering_transitive() {
        // Test transitivity
        assert!(NotificationSeverity::Info <= NotificationSeverity::Info);
        assert!(NotificationSeverity::Warning <= NotificationSeverity::Warning);
        assert!(NotificationSeverity::Critical <= NotificationSeverity::Critical);

        assert!(NotificationSeverity::Info <= NotificationSeverity::Warning);
        assert!(NotificationSeverity::Warning <= NotificationSeverity::Critical);
    }

    #[test]
    fn test_notification_severity_max_min() {
        use std::cmp::{max, min};

        // Test max/min operations
        assert_eq!(
            max(NotificationSeverity::Info, NotificationSeverity::Warning),
            NotificationSeverity::Warning
        );
        assert_eq!(
            max(
                NotificationSeverity::Warning,
                NotificationSeverity::Critical
            ),
            NotificationSeverity::Critical
        );
        assert_eq!(
            max(NotificationSeverity::Info, NotificationSeverity::Critical),
            NotificationSeverity::Critical
        );

        assert_eq!(
            min(NotificationSeverity::Info, NotificationSeverity::Warning),
            NotificationSeverity::Info
        );
        assert_eq!(
            min(
                NotificationSeverity::Warning,
                NotificationSeverity::Critical
            ),
            NotificationSeverity::Warning
        );
        assert_eq!(
            min(NotificationSeverity::Info, NotificationSeverity::Critical),
            NotificationSeverity::Info
        );
    }

    #[test]
    fn test_notification_severity_sorting() {
        let mut severities = vec![
            NotificationSeverity::Critical,
            NotificationSeverity::Info,
            NotificationSeverity::Warning,
            NotificationSeverity::Critical,
            NotificationSeverity::Info,
        ];

        severities.sort();

        assert_eq!(
            severities,
            vec![
                NotificationSeverity::Info,
                NotificationSeverity::Info,
                NotificationSeverity::Warning,
                NotificationSeverity::Critical,
                NotificationSeverity::Critical,
            ]
        );
    }

    #[test]
    fn test_event_type_is_positive() {
        // Positive events
        assert!(EventType::CommitmentFulfilled.is_positive());
        assert!(EventType::MilestoneCompleted.is_positive());
        assert!(EventType::AccountCreated.is_positive());
        assert!(EventType::FirstTokenIssued.is_positive());
        assert!(EventType::CircuitBreakerRecovered.is_positive());

        // Not positive
        assert!(!EventType::CommitmentFailed.is_positive());
        assert!(!EventType::TimeDecay.is_positive());
        assert!(!EventType::QualityScore.is_positive());
    }

    #[test]
    fn test_event_type_is_negative() {
        // Negative events
        assert!(EventType::CommitmentFailed.is_negative());
        assert!(EventType::CommitmentExpired.is_negative());
        assert!(EventType::TimeDecay.is_negative());
        assert!(EventType::CircuitBreakerTriggered.is_negative());

        // Not negative
        assert!(!EventType::CommitmentFulfilled.is_negative());
        assert!(!EventType::AccountCreated.is_negative());
        assert!(!EventType::QualityScore.is_negative());
    }

    #[test]
    fn test_event_type_is_neutral() {
        // Neutral events (variable delta)
        assert!(EventType::QualityScore.is_neutral());
        assert!(EventType::HolderRating.is_neutral());
        assert!(EventType::AdminAdjustment.is_neutral());

        // Not neutral
        assert!(!EventType::CommitmentFulfilled.is_neutral());
        assert!(!EventType::CommitmentFailed.is_neutral());
        assert!(!EventType::TimeDecay.is_neutral());
    }

    #[test]
    fn test_event_type_is_commitment_related() {
        // Commitment-related events
        assert!(EventType::CommitmentFulfilled.is_commitment_related());
        assert!(EventType::CommitmentFailed.is_commitment_related());
        assert!(EventType::CommitmentExpired.is_commitment_related());
        assert!(EventType::MilestoneCompleted.is_commitment_related());

        // Not commitment-related
        assert!(!EventType::QualityScore.is_commitment_related());
        assert!(!EventType::TimeDecay.is_commitment_related());
        assert!(!EventType::AccountCreated.is_commitment_related());
    }

    #[test]
    fn test_event_type_is_system_event() {
        // System events
        assert!(EventType::TimeDecay.is_system_event());
        assert!(EventType::CircuitBreakerTriggered.is_system_event());
        assert!(EventType::CircuitBreakerRecovered.is_system_event());
        assert!(EventType::AccountCreated.is_system_event());

        // Not system events (user-initiated)
        assert!(!EventType::CommitmentFulfilled.is_system_event());
        assert!(!EventType::HolderRating.is_system_event());
        assert!(!EventType::AdminAdjustment.is_system_event());
    }

    #[test]
    fn test_event_type_categorization_complete() {
        // Every event should be exactly one of: positive, negative, or neutral
        for event_type in EventType::all() {
            let positive = event_type.is_positive();
            let negative = event_type.is_negative();
            let neutral = event_type.is_neutral();

            // Count how many categories this event belongs to
            let count = [positive, negative, neutral].iter().filter(|&&x| x).count();

            // Each event should belong to exactly one category
            assert_eq!(
                count, 1,
                "Event {:?} should be exactly one of positive, negative, or neutral (got {} categories)",
                event_type, count
            );
        }
    }

    // Phase 18 tests - New functionality
    #[test]
    fn test_notification_severity_all_variants() {
        let variants = NotificationSeverity::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], NotificationSeverity::Info);
        assert_eq!(variants[1], NotificationSeverity::Warning);
        assert_eq!(variants[2], NotificationSeverity::Critical);
    }

    #[test]
    fn test_notification_severity_as_str() {
        assert_eq!(NotificationSeverity::Info.as_str(), "info");
        assert_eq!(NotificationSeverity::Warning.as_str(), "warning");
        assert_eq!(NotificationSeverity::Critical.as_str(), "critical");
    }

    #[test]
    fn test_notification_severity_is_methods() {
        assert!(NotificationSeverity::Info.is_info());
        assert!(!NotificationSeverity::Info.is_warning());
        assert!(!NotificationSeverity::Info.is_critical());

        assert!(!NotificationSeverity::Warning.is_info());
        assert!(NotificationSeverity::Warning.is_warning());
        assert!(!NotificationSeverity::Warning.is_critical());

        assert!(!NotificationSeverity::Critical.is_info());
        assert!(!NotificationSeverity::Critical.is_warning());
        assert!(NotificationSeverity::Critical.is_critical());
    }
}
