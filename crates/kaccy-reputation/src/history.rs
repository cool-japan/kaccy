//! Tier history tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::Result;
use crate::tier::{ReputationTier, TierChangeDirection, TierChangeEvent};

/// A record of a user's tier history
#[derive(Debug, Clone, Serialize)]
pub struct TierHistoryRecord {
    /// Unique record ID
    pub id: Uuid,
    /// User ID
    pub user_id: Uuid,
    /// Previous tier
    pub previous_tier: ReputationTier,
    /// New tier
    pub new_tier: ReputationTier,
    /// Direction of change
    pub direction: TierChangeDirection,
    /// Previous score
    pub previous_score: Decimal,
    /// New score
    pub new_score: Decimal,
    /// When the change occurred
    pub changed_at: DateTime<Utc>,
    /// Optional reason for the change
    pub reason: Option<String>,
}

/// Tier history repository
pub struct TierHistoryRepository {
    pool: PgPool,
}

impl TierHistoryRepository {
    /// Create a new tier history repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record a tier change event
    pub async fn record_change(
        &self,
        event: &TierChangeEvent,
        reason: Option<&str>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let direction_str = match event.direction {
            TierChangeDirection::Promotion => "promotion",
            TierChangeDirection::Demotion => "demotion",
        };

        sqlx::query(
            r#"
            INSERT INTO tier_history (id, user_id, previous_tier, new_tier, direction, previous_score, new_score, changed_at, reason)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(event.user_id)
        .bind(tier_to_string(&event.previous_tier))
        .bind(tier_to_string(&event.new_tier))
        .bind(direction_str)
        .bind(event.previous_score)
        .bind(event.new_score)
        .bind(event.changed_at)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            user_id = %event.user_id,
            from_tier = %event.previous_tier,
            to_tier = %event.new_tier,
            direction = direction_str,
            "Tier change recorded"
        );

        Ok(id)
    }

    /// Get tier history for a user
    pub async fn get_user_history(
        &self,
        user_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<TierHistoryRecord>> {
        let limit = limit.unwrap_or(50);

        let rows = sqlx::query(
            r#"
            SELECT id, user_id, previous_tier, new_tier, direction, previous_score, new_score, changed_at, reason
            FROM tier_history
            WHERE user_id = $1
            ORDER BY changed_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let records = rows
            .into_iter()
            .map(|row| {
                let previous_tier_str: String = row.get("previous_tier");
                let new_tier_str: String = row.get("new_tier");
                let direction_str: String = row.get("direction");

                TierHistoryRecord {
                    id: row.get("id"),
                    user_id: row.get("user_id"),
                    previous_tier: string_to_tier(&previous_tier_str),
                    new_tier: string_to_tier(&new_tier_str),
                    direction: match direction_str.as_str() {
                        "promotion" => TierChangeDirection::Promotion,
                        _ => TierChangeDirection::Demotion,
                    },
                    previous_score: row.get("previous_score"),
                    new_score: row.get("new_score"),
                    changed_at: row.get("changed_at"),
                    reason: row.get("reason"),
                }
            })
            .collect();

        Ok(records)
    }

    /// Get the latest tier change for a user
    pub async fn get_latest_change(&self, user_id: Uuid) -> Result<Option<TierHistoryRecord>> {
        let history = self.get_user_history(user_id, Some(1)).await?;
        Ok(history.into_iter().next())
    }

    /// Get tier promotion count for a user
    pub async fn count_promotions(&self, user_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM tier_history
            WHERE user_id = $1 AND direction = 'promotion'
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get("count");
        Ok(count)
    }

    /// Get tier demotion count for a user
    pub async fn count_demotions(&self, user_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM tier_history
            WHERE user_id = $1 AND direction = 'demotion'
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get("count");
        Ok(count)
    }

    /// Get time at current tier
    pub async fn time_at_current_tier(&self, user_id: Uuid) -> Result<Option<chrono::Duration>> {
        if let Some(latest) = self.get_latest_change(user_id).await? {
            let duration = Utc::now() - latest.changed_at;
            Ok(Some(duration))
        } else {
            Ok(None)
        }
    }
}

/// Tier history summary for a user
#[derive(Debug, Clone, Serialize)]
pub struct TierHistorySummary {
    /// User ID
    pub user_id: Uuid,
    /// Current tier
    pub current_tier: ReputationTier,
    /// Highest tier ever achieved
    pub highest_tier: ReputationTier,
    /// Total promotions
    pub total_promotions: i64,
    /// Total demotions
    pub total_demotions: i64,
    /// Days at current tier
    pub days_at_current_tier: Option<i64>,
    /// Recent changes
    pub recent_changes: Vec<TierHistoryRecord>,
}

impl TierHistoryRepository {
    /// Get a summary of tier history for a user
    pub async fn get_summary(
        &self,
        user_id: Uuid,
        current_score: Decimal,
    ) -> Result<TierHistorySummary> {
        let current_tier = ReputationTier::from_score(current_score);
        let recent_changes = self.get_user_history(user_id, Some(10)).await?;
        let total_promotions = self.count_promotions(user_id).await?;
        let total_demotions = self.count_demotions(user_id).await?;

        // Find highest tier from history
        let highest_tier = recent_changes
            .iter()
            .map(|r| r.new_tier)
            .max()
            .unwrap_or(current_tier);

        let days_at_current_tier = self
            .time_at_current_tier(user_id)
            .await?
            .map(|d| d.num_days());

        Ok(TierHistorySummary {
            user_id,
            current_tier,
            highest_tier,
            total_promotions,
            total_demotions,
            days_at_current_tier,
            recent_changes,
        })
    }
}

/// Convert tier enum to string for database storage
fn tier_to_string(tier: &ReputationTier) -> String {
    match tier {
        ReputationTier::Unverified => "unverified".to_string(),
        ReputationTier::Bronze => "bronze".to_string(),
        ReputationTier::Silver => "silver".to_string(),
        ReputationTier::Gold => "gold".to_string(),
        ReputationTier::Platinum => "platinum".to_string(),
        ReputationTier::Diamond => "diamond".to_string(),
    }
}

/// Convert string from database to tier enum
fn string_to_tier(s: &str) -> ReputationTier {
    match s.to_lowercase().as_str() {
        "bronze" => ReputationTier::Bronze,
        "silver" => ReputationTier::Silver,
        "gold" => ReputationTier::Gold,
        "platinum" => ReputationTier::Platinum,
        "diamond" => ReputationTier::Diamond,
        _ => ReputationTier::Unverified,
    }
}

/// Integration with ReputationEngine for automatic tier tracking
pub struct TierTracker {
    history_repo: TierHistoryRepository,
}

impl TierTracker {
    /// Create a new tier tracker
    pub fn new(pool: PgPool) -> Self {
        Self {
            history_repo: TierHistoryRepository::new(pool),
        }
    }

    /// Track a potential tier change when score is updated
    ///
    /// Returns the tier change event if a change occurred
    pub async fn track_score_change(
        &self,
        user_id: Uuid,
        old_score: Decimal,
        new_score: Decimal,
        reason: Option<&str>,
    ) -> Result<Option<TierChangeEvent>> {
        if let Some(event) = TierChangeEvent::detect(user_id, old_score, new_score) {
            self.history_repo.record_change(&event, reason).await?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// Get tier history for a user
    pub async fn get_history(
        &self,
        user_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<TierHistoryRecord>> {
        self.history_repo.get_user_history(user_id, limit).await
    }

    /// Get summary for a user
    pub async fn get_summary(
        &self,
        user_id: Uuid,
        current_score: Decimal,
    ) -> Result<TierHistorySummary> {
        self.history_repo.get_summary(user_id, current_score).await
    }
}
