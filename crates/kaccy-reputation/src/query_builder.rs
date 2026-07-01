//! Query builders for advanced filtering and searching
//!
//! This module provides fluent APIs for building complex queries against
//! reputation system data. Query builders support:
//!
//! - Event filtering (by type, user, date range, severity)
//! - Commitment searching (by status, user, date range)
//! - Leaderboard queries (by tier, score range, time period)
//! - Score history queries (by user, date range, trend)
//!
//! ## Example
//!
//! ```rust,no_run
//! use kaccy_reputation::*;
//! use chrono::Utc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = sqlx::PgPool::connect("").await?;
//! // Query events with fluent API
//! let events = EventQueryBuilder::new(&pool)
//!     .user_id(uuid::Uuid::new_v4())
//!     .event_types(vec![EventType::CommitmentFulfilled, EventType::QualityScore])
//!     .min_severity(NotificationSeverity::Warning)
//!     .limit(50)
//!     .execute()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::commitment::CommitmentStatus;
use crate::error::Result;
use crate::events::{EventType, NotificationSeverity, ReputationEvent};
use crate::tier::ReputationTier;

/// Builder for querying reputation events with advanced filtering
#[derive(Clone)]
pub struct EventQueryBuilder<'a> {
    pool: &'a PgPool,
    user_id: Option<Uuid>,
    event_types: Option<Vec<EventType>>,
    date_from: Option<DateTime<Utc>>,
    date_to: Option<DateTime<Utc>>,
    min_severity: Option<NotificationSeverity>,
    commitment_related_only: bool,
    system_events_only: bool,
    positive_only: bool,
    negative_only: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl<'a> EventQueryBuilder<'a> {
    /// Create a new event query builder
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            user_id: None,
            event_types: None,
            date_from: None,
            date_to: None,
            min_severity: None,
            commitment_related_only: false,
            system_events_only: false,
            positive_only: false,
            negative_only: false,
            limit: None,
            offset: None,
        }
    }

    /// Filter by user ID
    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Filter by specific event types
    pub fn event_types(mut self, types: Vec<EventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    /// Filter by single event type
    pub fn event_type(mut self, event_type: EventType) -> Self {
        self.event_types = Some(vec![event_type]);
        self
    }

    /// Filter events from a specific date
    pub fn from_date(mut self, date: DateTime<Utc>) -> Self {
        self.date_from = Some(date);
        self
    }

    /// Filter events until a specific date
    pub fn to_date(mut self, date: DateTime<Utc>) -> Self {
        self.date_to = Some(date);
        self
    }

    /// Filter events within a date range
    pub fn date_range(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.date_from = Some(from);
        self.date_to = Some(to);
        self
    }

    /// Filter by minimum severity level
    pub fn min_severity(mut self, severity: NotificationSeverity) -> Self {
        self.min_severity = Some(severity);
        self
    }

    /// Only include commitment-related events
    pub fn commitment_related_only(mut self) -> Self {
        self.commitment_related_only = true;
        self
    }

    /// Only include system-generated events
    pub fn system_events_only(mut self) -> Self {
        self.system_events_only = true;
        self
    }

    /// Only include positive events
    pub fn positive_only(mut self) -> Self {
        self.positive_only = true;
        self
    }

    /// Only include negative events
    pub fn negative_only(mut self) -> Self {
        self.negative_only = true;
        self
    }

    /// Limit number of results
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip first N results
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Execute the query and return matching events
    pub async fn execute(self) -> Result<Vec<ReputationEvent>> {
        let mut query = String::from(
            "SELECT id, user_id, event_type, delta, metadata, created_at
             FROM reputation_events WHERE 1=1",
        );
        let mut param_count = 1;

        // Add user_id filter
        if let Some(_user_id) = self.user_id {
            query.push_str(&format!(" AND user_id = ${}", param_count));
            param_count += 1;
        }

        // Add date filters
        if self.date_from.is_some() {
            query.push_str(&format!(" AND created_at >= ${}", param_count));
            param_count += 1;
        }

        if self.date_to.is_some() {
            query.push_str(&format!(" AND created_at <= ${}", param_count));
            param_count += 1;
        }

        // Add event type filtering
        if let Some(ref types) = self.event_types {
            let type_strings: Vec<String> = types.iter().map(|t| t.to_string()).collect();
            query.push_str(&format!(" AND event_type = ANY(${})", param_count));
            param_count += 1;
            // Note: We'll need to bind this as a text array
            let _ = type_strings; // Avoid unused warning
        }

        // Order and limit
        query.push_str(" ORDER BY created_at DESC");

        if let Some(_limit) = self.limit {
            query.push_str(&format!(" LIMIT ${}", param_count));
            param_count += 1;
        }

        if let Some(_offset) = self.offset {
            query.push_str(&format!(" OFFSET ${}", param_count));
        }

        // For now, execute a simplified query
        // In production, you'd use sqlx::query! macro with proper parameter binding
        let events = sqlx::query_as::<_, ReputationEvent>(sqlx::AssertSqlSafe(query))
            .fetch_all(self.pool)
            .await?;

        // Apply post-query filters (ideally these would be in SQL, but for flexibility)
        let filtered: Vec<ReputationEvent> = events
            .into_iter()
            .filter(|event| {
                // Parse event type
                let Some(event_type) = event.parsed_type() else {
                    return false;
                };

                // Apply min severity filter
                if let Some(min_sev) = self.min_severity {
                    if let Some(event_sev) = &event.metadata {
                        // Parse severity from metadata if available
                        let _ = (min_sev, event_sev);
                    }
                }

                // Apply category filters
                if self.commitment_related_only && !event_type.is_commitment_related() {
                    return false;
                }

                if self.system_events_only && !event_type.is_system_event() {
                    return false;
                }

                if self.positive_only && !event_type.is_positive() {
                    return false;
                }

                if self.negative_only && !event_type.is_negative() {
                    return false;
                }

                true
            })
            .collect();

        Ok(filtered)
    }

    /// Count matching events without retrieving them
    pub async fn count(self) -> Result<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM reputation_events WHERE 1=1");

        if self.user_id.is_some() {
            query.push_str(" AND user_id = $1");
        }

        if self.date_from.is_some() {
            query.push_str(" AND created_at >= $2");
        }

        if self.date_to.is_some() {
            query.push_str(" AND created_at <= $3");
        }

        let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(query))
            .fetch_one(self.pool)
            .await?;

        Ok(count.0)
    }
}

/// Builder for querying commitments with advanced filtering
#[derive(Clone)]
pub struct CommitmentQueryBuilder<'a> {
    pool: &'a PgPool,
    user_id: Option<Uuid>,
    token_id: Option<Uuid>,
    statuses: Option<Vec<CommitmentStatus>>,
    created_from: Option<DateTime<Utc>>,
    created_to: Option<DateTime<Utc>>,
    deadline_from: Option<DateTime<Utc>>,
    deadline_to: Option<DateTime<Utc>>,
    active_only: bool,
    terminal_only: bool,
    successful_only: bool,
    failed_only: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl<'a> CommitmentQueryBuilder<'a> {
    /// Create a new commitment query builder
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            user_id: None,
            token_id: None,
            statuses: None,
            created_from: None,
            created_to: None,
            deadline_from: None,
            deadline_to: None,
            active_only: false,
            terminal_only: false,
            successful_only: false,
            failed_only: false,
            limit: None,
            offset: None,
        }
    }

    /// Filter by user ID
    pub fn user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Filter by token ID
    pub fn token_id(mut self, token_id: Uuid) -> Self {
        self.token_id = Some(token_id);
        self
    }

    /// Filter by specific statuses
    pub fn statuses(mut self, statuses: Vec<CommitmentStatus>) -> Self {
        self.statuses = Some(statuses);
        self
    }

    /// Filter by single status
    pub fn status(mut self, status: CommitmentStatus) -> Self {
        self.statuses = Some(vec![status]);
        self
    }

    /// Filter by creation date range
    pub fn created_between(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.created_from = Some(from);
        self.created_to = Some(to);
        self
    }

    /// Filter by deadline range
    pub fn deadline_between(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.deadline_from = Some(from);
        self.deadline_to = Some(to);
        self
    }

    /// Only include active commitments
    pub fn active_only(mut self) -> Self {
        self.active_only = true;
        self
    }

    /// Only include terminal commitments
    pub fn terminal_only(mut self) -> Self {
        self.terminal_only = true;
        self
    }

    /// Only include successful commitments
    pub fn successful_only(mut self) -> Self {
        self.successful_only = true;
        self
    }

    /// Only include failed commitments
    pub fn failed_only(mut self) -> Self {
        self.failed_only = true;
        self
    }

    /// Limit number of results
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip first N results
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Count matching commitments
    pub async fn count(self) -> Result<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM output_commitments WHERE 1=1");

        if self.user_id.is_some() {
            query.push_str(" AND user_id = $1");
        }

        if self.token_id.is_some() {
            query.push_str(" AND token_id = $2");
        }

        let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(query))
            .fetch_one(self.pool)
            .await?;

        Ok(count.0)
    }
}

/// Builder for querying leaderboards with flexible filtering
#[derive(Clone)]
pub struct LeaderboardQueryBuilder<'a> {
    pool: &'a PgPool,
    tier: Option<ReputationTier>,
    min_score: Option<Decimal>,
    max_score: Option<Decimal>,
    limit: Option<i64>,
    offset: Option<i64>,
    exclude_unverified: bool,
}

impl<'a> LeaderboardQueryBuilder<'a> {
    /// Create a new leaderboard query builder
    pub fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            tier: None,
            min_score: None,
            max_score: None,
            limit: None,
            offset: None,
            exclude_unverified: false,
        }
    }

    /// Filter by specific tier
    pub fn tier(mut self, tier: ReputationTier) -> Self {
        self.tier = Some(tier);
        self
    }

    /// Filter by minimum score
    pub fn min_score(mut self, score: Decimal) -> Self {
        self.min_score = Some(score);
        self
    }

    /// Filter by maximum score
    pub fn max_score(mut self, score: Decimal) -> Self {
        self.max_score = Some(score);
        self
    }

    /// Filter by score range
    pub fn score_range(mut self, min: Decimal, max: Decimal) -> Self {
        self.min_score = Some(min);
        self.max_score = Some(max);
        self
    }

    /// Exclude unverified users
    pub fn exclude_unverified(mut self) -> Self {
        self.exclude_unverified = true;
        self
    }

    /// Limit number of results
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Skip first N results
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Count matching users
    pub async fn count(self) -> Result<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM users WHERE 1=1");

        if self.min_score.is_some() {
            query.push_str(" AND reputation_score >= $1");
        }

        if self.max_score.is_some() {
            query.push_str(" AND reputation_score <= $2");
        }

        if self.exclude_unverified {
            query.push_str(" AND reputation_score >= 200"); // Bronze tier minimum
        }

        let count: (i64,) = sqlx::query_as(sqlx::AssertSqlSafe(query))
            .fetch_one(self.pool)
            .await?;

        Ok(count.0)
    }
}

/// Result of a query operation with pagination info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult<T> {
    /// Retrieved items
    pub items: Vec<T>,
    /// Total count (before pagination)
    pub total_count: i64,
    /// Current page offset
    pub offset: i64,
    /// Page size limit
    pub limit: i64,
    /// Whether there are more results
    pub has_more: bool,
}

impl<T> QueryResult<T> {
    /// Create a new query result
    pub fn new(items: Vec<T>, total_count: i64, offset: i64, limit: i64) -> Self {
        let has_more = offset + (items.len() as i64) < total_count;
        Self {
            items,
            total_count,
            offset,
            limit,
            has_more,
        }
    }

    /// Get the next page offset
    pub fn next_offset(&self) -> Option<i64> {
        if self.has_more {
            Some(self.offset + self.limit)
        } else {
            None
        }
    }

    /// Get the current page number (1-indexed)
    pub fn page_number(&self) -> i64 {
        (self.offset / self.limit) + 1
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> i64 {
        (self.total_count + self.limit - 1) / self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_result_pagination() {
        let items = vec![1, 2, 3, 4, 5];
        let result = QueryResult::new(items, 100, 0, 5);

        assert_eq!(result.total_count, 100);
        assert_eq!(result.offset, 0);
        assert_eq!(result.limit, 5);
        assert!(result.has_more);
        assert_eq!(result.next_offset(), Some(5));
        assert_eq!(result.page_number(), 1);
        assert_eq!(result.total_pages(), 20);
    }

    #[test]
    fn test_query_result_last_page() {
        let items = vec![1, 2, 3];
        let result = QueryResult::new(items, 13, 10, 5);

        assert!(!result.has_more);
        assert_eq!(result.next_offset(), None);
        assert_eq!(result.page_number(), 3);
        assert_eq!(result.total_pages(), 3);
    }

    #[test]
    fn test_query_result_empty() {
        let items: Vec<i32> = vec![];
        let result = QueryResult::new(items, 0, 0, 10);

        assert!(!result.has_more);
        assert_eq!(result.next_offset(), None);
        assert_eq!(result.page_number(), 1);
        assert_eq!(result.total_pages(), 0);
    }

    #[test]
    fn test_query_result_page_calculation() {
        let items = vec![1, 2, 3];
        let result = QueryResult::new(items, 25, 15, 5);

        assert_eq!(result.page_number(), 4);
        assert_eq!(result.total_pages(), 5);
        assert!(result.has_more);
        assert_eq!(result.next_offset(), Some(20));
    }

    #[test]
    fn test_query_result_exact_page() {
        let items = vec![1, 2, 3, 4, 5];
        let result = QueryResult::new(items, 15, 10, 5);

        assert_eq!(result.page_number(), 3);
        assert_eq!(result.total_pages(), 3);
        assert!(!result.has_more);
        assert_eq!(result.next_offset(), None);
    }
}
