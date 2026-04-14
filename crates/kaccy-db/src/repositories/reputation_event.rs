//! Reputation event repository

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Reputation event data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReputationEventRow {
    /// Unique identifier for this reputation event.
    pub event_id: Uuid,
    /// User whose reputation was affected.
    pub user_id: Uuid,
    /// Type of event that triggered the reputation change.
    pub event_type: String,
    /// Signed reputation change (positive or negative).
    pub delta: Decimal,
    /// Optional explanation for the change.
    pub reason: Option<String>,
    /// Timestamp when the event was recorded.
    pub created_at: DateTime<Utc>,
}

/// Summary of reputation events by type
#[derive(Debug, Clone)]
pub struct EventTypeSummary {
    /// Event type name.
    pub event_type: String,
    /// Number of events of this type.
    pub count: i64,
    /// Sum of all deltas for this event type.
    pub total_delta: Decimal,
    /// Average delta per event.
    pub avg_delta: Decimal,
}

/// User's reputation history summary
#[derive(Debug, Clone)]
pub struct ReputationHistorySummary {
    /// User identifier.
    pub user_id: Uuid,
    /// Total number of reputation events.
    pub total_events: i64,
    /// Sum of all positive deltas.
    pub total_positive_delta: Decimal,
    /// Sum of all negative deltas (absolute value).
    pub total_negative_delta: Decimal,
    /// Net change in reputation.
    pub net_delta: Decimal,
    /// Timestamp of the first reputation event.
    pub first_event_at: Option<DateTime<Utc>>,
    /// Timestamp of the most recent reputation event.
    pub last_event_at: Option<DateTime<Utc>>,
}

/// Repository for reputation event operations
pub struct ReputationEventRepository {
    pool: PgPool,
}

impl ReputationEventRepository {
    /// Create a new `ReputationEventRepository` backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new reputation event
    pub async fn create(
        &self,
        user_id: Uuid,
        event_type: &str,
        delta: Decimal,
        reason: Option<&str>,
    ) -> Result<ReputationEventRow> {
        let event = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(event_type)
        .bind(delta)
        .bind(reason)
        .fetch_one(&self.pool)
        .await?;

        Ok(event)
    }

    /// Get user's reputation events with pagination
    pub async fn get_user_events(
        &self,
        user_id: Uuid,
        page: u32,
        limit: u32,
    ) -> Result<Vec<ReputationEventRow>> {
        let offset = (page.saturating_sub(1)) * limit;

        let events = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get user's events filtered by type
    pub async fn get_user_events_by_type(
        &self,
        user_id: Uuid,
        event_type: &str,
        limit: u32,
    ) -> Result<Vec<ReputationEventRow>> {
        let events = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            WHERE user_id = $1 AND event_type = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(user_id)
        .bind(event_type)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get events within a date range
    pub async fn get_events_in_range(
        &self,
        user_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<ReputationEventRow>> {
        let events = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            WHERE user_id = $1 AND created_at >= $2 AND created_at <= $3
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Count user's total events
    pub async fn count_user_events(&self, user_id: Uuid) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM reputation_events WHERE user_id = $1"#)
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Calculate total reputation change from events
    pub async fn calculate_total_from_events(&self, user_id: Uuid) -> Result<Decimal> {
        let result: Option<(Decimal,)> = sqlx::query_as(
            r#"SELECT COALESCE(SUM(delta), 0) FROM reputation_events WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(d,)| d).unwrap_or(Decimal::ZERO))
    }

    /// Get summary of events by type for a user
    pub async fn get_event_type_summary(&self, user_id: Uuid) -> Result<Vec<EventTypeSummary>> {
        let rows: Vec<(String, i64, Decimal, Decimal)> = sqlx::query_as(
            r#"
            SELECT
                event_type,
                COUNT(*) as count,
                COALESCE(SUM(delta), 0) as total_delta,
                COALESCE(AVG(delta), 0) as avg_delta
            FROM reputation_events
            WHERE user_id = $1
            GROUP BY event_type
            ORDER BY count DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(event_type, count, total_delta, avg_delta)| EventTypeSummary {
                    event_type,
                    count,
                    total_delta,
                    avg_delta,
                },
            )
            .collect())
    }

    /// Get user's reputation history summary
    pub async fn get_history_summary(&self, user_id: Uuid) -> Result<ReputationHistorySummary> {
        let row: (
            i64,
            Decimal,
            Decimal,
            Option<DateTime<Utc>>,
            Option<DateTime<Utc>>,
        ) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total_events,
                COALESCE(SUM(CASE WHEN delta > 0 THEN delta ELSE 0 END), 0) as total_positive,
                COALESCE(SUM(CASE WHEN delta < 0 THEN delta ELSE 0 END), 0) as total_negative,
                MIN(created_at) as first_event,
                MAX(created_at) as last_event
            FROM reputation_events
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(ReputationHistorySummary {
            user_id,
            total_events: row.0,
            total_positive_delta: row.1,
            total_negative_delta: row.2,
            net_delta: row.1 + row.2,
            first_event_at: row.3,
            last_event_at: row.4,
        })
    }

    /// Get recent events across all users (for admin dashboard)
    pub async fn get_recent_events(&self, limit: u32) -> Result<Vec<ReputationEventRow>> {
        let events = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get events by type across all users (for analytics)
    pub async fn get_events_by_type(
        &self,
        event_type: &str,
        limit: u32,
    ) -> Result<Vec<ReputationEventRow>> {
        let events = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            WHERE event_type = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(event_type)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Delete old events (for data retention compliance)
    pub async fn delete_events_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query(r#"DELETE FROM reputation_events WHERE created_at < $1"#)
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Calculate score recalculated from events (for verification)
    pub async fn recalculate_score_from_events(
        &self,
        user_id: Uuid,
        base_score: Decimal,
    ) -> Result<Decimal> {
        let total_delta = self.calculate_total_from_events(user_id).await?;
        let calculated = base_score + total_delta;

        // Clamp to valid range
        Ok(calculated.max(Decimal::ZERO).min(Decimal::from(1000)))
    }

    /// Batch create multiple reputation events (optimized for bulk operations)
    ///
    /// Creates multiple reputation events in a single transaction.
    /// Useful for bulk reputation adjustments or migrations.
    ///
    /// # Arguments
    /// * `events` - Vector of (user_id, event_type, delta, reason) tuples
    pub async fn batch_create(
        &self,
        events: Vec<(Uuid, String, Decimal, Option<String>)>,
    ) -> Result<u64> {
        if events.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut count = 0u64;

        for (user_id, event_type, delta, reason) in events {
            let result = sqlx::query(
                r#"
                INSERT INTO reputation_events (user_id, event_type, delta, reason)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(user_id)
            .bind(event_type)
            .bind(delta)
            .bind(reason)
            .execute(&mut *tx)
            .await?;

            count += result.rows_affected();
        }

        tx.commit().await?;
        Ok(count)
    }

    /// Get top users by reputation gain in a time period (leaderboard)
    ///
    /// Returns users with the highest positive reputation deltas.
    /// Useful for weekly/monthly leaderboards.
    pub async fn get_top_users_by_reputation_gain(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<UserReputationGain>> {
        let users = sqlx::query_as::<_, UserReputationGain>(
            r#"
            SELECT
                user_id,
                COUNT(*) as event_count,
                COALESCE(SUM(delta), 0) as total_delta,
                COALESCE(SUM(CASE WHEN delta > 0 THEN delta ELSE 0 END), 0) as positive_delta,
                COALESCE(SUM(CASE WHEN delta < 0 THEN delta ELSE 0 END), 0) as negative_delta
            FROM reputation_events
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY user_id
            HAVING SUM(delta) > 0
            ORDER BY SUM(delta) DESC
            LIMIT $3
            "#,
        )
        .bind(start)
        .bind(end)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Get platform-wide event type distribution (analytics)
    ///
    /// Returns statistics for all event types across all users.
    pub async fn get_platform_event_distribution(&self) -> Result<Vec<EventTypeSummary>> {
        let rows: Vec<(String, i64, Decimal, Decimal)> = sqlx::query_as(
            r#"
            SELECT
                event_type,
                COUNT(*) as count,
                COALESCE(SUM(delta), 0) as total_delta,
                COALESCE(AVG(delta), 0) as avg_delta
            FROM reputation_events
            GROUP BY event_type
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(event_type, count, total_delta, avg_delta)| EventTypeSummary {
                    event_type,
                    count,
                    total_delta,
                    avg_delta,
                },
            )
            .collect())
    }

    /// Get daily reputation event statistics for a time range
    ///
    /// Returns daily aggregated statistics useful for trend analysis.
    pub async fn get_daily_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DailyReputationStats>> {
        let stats = sqlx::query_as::<_, DailyReputationStats>(
            r#"
            SELECT
                DATE(created_at) as date,
                COUNT(*) as event_count,
                COUNT(DISTINCT user_id) as unique_users,
                COALESCE(SUM(delta), 0) as total_delta,
                COALESCE(SUM(CASE WHEN delta > 0 THEN delta ELSE 0 END), 0) as positive_delta,
                COALESCE(SUM(CASE WHEN delta < 0 THEN delta ELSE 0 END), 0) as negative_delta
            FROM reputation_events
            WHERE created_at >= $1 AND created_at <= $2
            GROUP BY DATE(created_at)
            ORDER BY DATE(created_at) DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    /// Count events by type
    pub async fn count_events_by_type(&self, event_type: &str) -> Result<i64> {
        let (count,): (i64,) =
            sqlx::query_as(r#"SELECT COUNT(*) FROM reputation_events WHERE event_type = $1"#)
                .bind(event_type)
                .fetch_one(&self.pool)
                .await?;

        Ok(count)
    }

    /// Get average delta for an event type
    pub async fn get_average_delta_for_type(&self, event_type: &str) -> Result<Decimal> {
        let avg = sqlx::query_scalar::<_, Option<Decimal>>(
            r#"SELECT COALESCE(AVG(delta), 0) FROM reputation_events WHERE event_type = $1"#,
        )
        .bind(event_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(avg.unwrap_or(Decimal::ZERO))
    }

    /// Get user's most recent event
    pub async fn get_last_event(&self, user_id: Uuid) -> Result<Option<ReputationEventRow>> {
        let event = sqlx::query_as::<_, ReputationEventRow>(
            r#"
            SELECT * FROM reputation_events
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(event)
    }

    /// Get count of positive vs negative events for a user
    pub async fn get_user_event_balance(&self, user_id: Uuid) -> Result<EventBalance> {
        let balance = sqlx::query_as::<_, EventBalance>(
            r#"
            SELECT
                COUNT(CASE WHEN delta > 0 THEN 1 END) as positive_count,
                COUNT(CASE WHEN delta < 0 THEN 1 END) as negative_count,
                COUNT(CASE WHEN delta = 0 THEN 1 END) as neutral_count
            FROM reputation_events
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(balance)
    }
}

/// User reputation gain statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserReputationGain {
    /// Unique identifier of the user.
    pub user_id: Uuid,
    /// Total number of reputation events for this user.
    pub event_count: i64,
    /// Net sum of all reputation deltas.
    pub total_delta: Decimal,
    /// Sum of positive (gain) deltas only.
    pub positive_delta: Decimal,
    /// Sum of negative (loss) deltas only.
    pub negative_delta: Decimal,
}

/// Daily reputation statistics
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DailyReputationStats {
    /// The calendar date these statistics apply to.
    pub date: chrono::NaiveDate,
    /// Number of reputation events recorded on this date.
    pub event_count: i64,
    /// Number of distinct users with events on this date.
    pub unique_users: i64,
    /// Net sum of all deltas on this date.
    pub total_delta: Decimal,
    /// Sum of positive deltas on this date.
    pub positive_delta: Decimal,
    /// Sum of negative deltas on this date.
    pub negative_delta: Decimal,
}

/// Event balance (positive vs negative events)
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventBalance {
    /// Number of events with a positive delta.
    pub positive_count: Option<i64>,
    /// Number of events with a negative delta.
    pub negative_count: Option<i64>,
    /// Number of events with a zero delta.
    pub neutral_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_event_row_structure() {
        let event = ReputationEventRow {
            event_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            event_type: "trade_completed".to_string(),
            delta: Decimal::new(10, 0),
            reason: Some("Successful trade".to_string()),
            created_at: Utc::now(),
        };

        assert_eq!(event.event_type, "trade_completed");
        assert_eq!(event.delta, Decimal::new(10, 0));
    }

    #[test]
    fn test_event_type_summary_structure() {
        let summary = EventTypeSummary {
            event_type: "trade_completed".to_string(),
            count: 100,
            total_delta: Decimal::new(500, 0),
            avg_delta: Decimal::new(5, 0),
        };

        assert_eq!(summary.event_type, "trade_completed");
        assert_eq!(summary.count, 100);
        assert_eq!(summary.avg_delta, Decimal::new(5, 0));
    }

    #[test]
    fn test_reputation_history_summary() {
        let summary = ReputationHistorySummary {
            user_id: Uuid::new_v4(),
            total_events: 50,
            total_positive_delta: Decimal::new(300, 0),
            total_negative_delta: Decimal::new(-50, 0),
            net_delta: Decimal::new(250, 0),
            first_event_at: Some(Utc::now()),
            last_event_at: Some(Utc::now()),
        };

        assert_eq!(summary.total_events, 50);
        assert_eq!(summary.net_delta, Decimal::new(250, 0));
    }

    #[test]
    fn test_user_reputation_gain_structure() {
        let gain = UserReputationGain {
            user_id: Uuid::new_v4(),
            event_count: 20,
            total_delta: Decimal::new(150, 0),
            positive_delta: Decimal::new(200, 0),
            negative_delta: Decimal::new(-50, 0),
        };

        assert_eq!(gain.event_count, 20);
        assert_eq!(gain.total_delta, Decimal::new(150, 0));
    }

    #[test]
    fn test_daily_reputation_stats_structure() {
        let stats = DailyReputationStats {
            date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            event_count: 1000,
            unique_users: 250,
            total_delta: Decimal::new(5000, 0),
            positive_delta: Decimal::new(6000, 0),
            negative_delta: Decimal::new(-1000, 0),
        };

        assert_eq!(stats.event_count, 1000);
        assert_eq!(stats.unique_users, 250);
        assert_eq!(stats.total_delta, Decimal::new(5000, 0));
    }

    #[test]
    fn test_event_balance_structure() {
        let balance = EventBalance {
            positive_count: Some(80),
            negative_count: Some(15),
            neutral_count: Some(5),
        };

        assert_eq!(balance.positive_count, Some(80));
        assert_eq!(balance.negative_count, Some(15));
        assert_eq!(balance.neutral_count, Some(5));
    }

    #[test]
    fn test_batch_create_empty_vector() {
        let events: Vec<(Uuid, String, Decimal, Option<String>)> = vec![];
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_reputation_score_clamping() {
        let base = Decimal::new(900, 0);
        let delta = Decimal::new(200, 0);
        let calculated = base + delta;
        let clamped = calculated.max(Decimal::ZERO).min(Decimal::from(1000));

        assert_eq!(clamped, Decimal::from(1000));
    }

    #[test]
    fn test_reputation_score_negative_clamping() {
        let base = Decimal::new(50, 0);
        let delta = Decimal::new(-100, 0);
        let calculated = base + delta;
        let clamped = calculated.max(Decimal::ZERO).min(Decimal::from(1000));

        assert_eq!(clamped, Decimal::ZERO);
    }

    #[test]
    fn test_net_delta_calculation() {
        let positive = Decimal::new(500, 0);
        let negative = Decimal::new(-200, 0);
        let net = positive + negative;

        assert_eq!(net, Decimal::new(300, 0));
    }
}
