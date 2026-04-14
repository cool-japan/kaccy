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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    // -----------------------------------------------------------------------
    // tier_to_string
    // -----------------------------------------------------------------------

    #[test]
    fn tier_to_string_all_variants() {
        assert_eq!(tier_to_string(&ReputationTier::Unverified), "unverified");
        assert_eq!(tier_to_string(&ReputationTier::Bronze), "bronze");
        assert_eq!(tier_to_string(&ReputationTier::Silver), "silver");
        assert_eq!(tier_to_string(&ReputationTier::Gold), "gold");
        assert_eq!(tier_to_string(&ReputationTier::Platinum), "platinum");
        assert_eq!(tier_to_string(&ReputationTier::Diamond), "diamond");
    }

    // -----------------------------------------------------------------------
    // string_to_tier
    // -----------------------------------------------------------------------

    #[test]
    fn string_to_tier_all_valid_lowercase() {
        assert_eq!(string_to_tier("unverified"), ReputationTier::Unverified);
        assert_eq!(string_to_tier("bronze"), ReputationTier::Bronze);
        assert_eq!(string_to_tier("silver"), ReputationTier::Silver);
        assert_eq!(string_to_tier("gold"), ReputationTier::Gold);
        assert_eq!(string_to_tier("platinum"), ReputationTier::Platinum);
        assert_eq!(string_to_tier("diamond"), ReputationTier::Diamond);
    }

    #[test]
    fn string_to_tier_case_insensitive() {
        assert_eq!(string_to_tier("BRONZE"), ReputationTier::Bronze);
        assert_eq!(string_to_tier("Silver"), ReputationTier::Silver);
        assert_eq!(string_to_tier("GOLD"), ReputationTier::Gold);
        assert_eq!(string_to_tier("Platinum"), ReputationTier::Platinum);
        assert_eq!(string_to_tier("DIAMOND"), ReputationTier::Diamond);
    }

    #[test]
    fn string_to_tier_unknown_falls_back_to_unverified() {
        assert_eq!(string_to_tier(""), ReputationTier::Unverified);
        assert_eq!(string_to_tier("unknown"), ReputationTier::Unverified);
        assert_eq!(string_to_tier("garbage_value"), ReputationTier::Unverified);
        assert_eq!(string_to_tier("123"), ReputationTier::Unverified);
    }

    // -----------------------------------------------------------------------
    // Roundtrip: tier → string → tier
    // -----------------------------------------------------------------------

    #[test]
    fn tier_string_roundtrip_all_variants() {
        for tier in ReputationTier::all_variants() {
            let s = tier_to_string(&tier);
            let recovered = string_to_tier(&s);
            assert_eq!(
                recovered, tier,
                "Roundtrip failed for tier {:?}: got string '{}', recovered {:?}",
                tier, s, recovered
            );
        }
    }

    // -----------------------------------------------------------------------
    // TierHistoryRecord construction and field access
    // -----------------------------------------------------------------------

    fn make_record(
        previous_tier: ReputationTier,
        new_tier: ReputationTier,
        direction: TierChangeDirection,
        previous_score: Decimal,
        new_score: Decimal,
        reason: Option<&str>,
    ) -> TierHistoryRecord {
        TierHistoryRecord {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            previous_tier,
            new_tier,
            direction,
            previous_score,
            new_score,
            changed_at: Utc::now(),
            reason: reason.map(|s| s.to_string()),
        }
    }

    #[test]
    fn tier_history_record_promotion_fields() {
        let rec = make_record(
            ReputationTier::Bronze,
            ReputationTier::Silver,
            TierChangeDirection::Promotion,
            dec!(350),
            dec!(420),
            Some("verified KYC"),
        );

        assert_eq!(rec.previous_tier, ReputationTier::Bronze);
        assert_eq!(rec.new_tier, ReputationTier::Silver);
        assert_eq!(rec.direction, TierChangeDirection::Promotion);
        assert_eq!(rec.previous_score, dec!(350));
        assert_eq!(rec.new_score, dec!(420));
        assert_eq!(
            rec.reason.as_deref().expect("reason should be Some"),
            "verified KYC"
        );
    }

    #[test]
    fn tier_history_record_demotion_fields() {
        let rec = make_record(
            ReputationTier::Gold,
            ReputationTier::Silver,
            TierChangeDirection::Demotion,
            dec!(620),
            dec!(395),
            None,
        );

        assert_eq!(rec.previous_tier, ReputationTier::Gold);
        assert_eq!(rec.new_tier, ReputationTier::Silver);
        assert!(rec.direction.is_negative());
        assert!(rec.reason.is_none());
    }

    // -----------------------------------------------------------------------
    // Score delta computation on TierHistoryRecord
    // -----------------------------------------------------------------------

    #[test]
    fn score_delta_positive_for_promotion() {
        let rec = make_record(
            ReputationTier::Bronze,
            ReputationTier::Silver,
            TierChangeDirection::Promotion,
            dec!(350),
            dec!(420),
            None,
        );
        let delta = rec.new_score - rec.previous_score;
        assert!(delta > dec!(0), "Promotion should yield a positive delta");
        assert_eq!(delta, dec!(70));
    }

    #[test]
    fn score_delta_negative_for_demotion() {
        let rec = make_record(
            ReputationTier::Gold,
            ReputationTier::Silver,
            TierChangeDirection::Demotion,
            dec!(620),
            dec!(395),
            None,
        );
        let delta = rec.new_score - rec.previous_score;
        assert!(delta < dec!(0), "Demotion should yield a negative delta");
        assert_eq!(delta, dec!(-225));
    }

    #[test]
    fn score_delta_zero_for_identical_scores() {
        let score = dec!(500);
        let rec = make_record(
            ReputationTier::Silver,
            ReputationTier::Silver,
            // In practice detect() would return None, but we can still construct the record
            TierChangeDirection::Promotion,
            score,
            score,
            None,
        );
        assert_eq!(rec.new_score - rec.previous_score, dec!(0));
    }

    // -----------------------------------------------------------------------
    // TierHistorySummary construction
    // -----------------------------------------------------------------------

    #[test]
    fn tier_history_summary_empty_recent_changes() {
        let user_id = Uuid::new_v4();
        let summary = TierHistorySummary {
            user_id,
            current_tier: ReputationTier::Unverified,
            highest_tier: ReputationTier::Unverified,
            total_promotions: 0,
            total_demotions: 0,
            days_at_current_tier: None,
            recent_changes: vec![],
        };

        assert_eq!(summary.user_id, user_id);
        assert_eq!(summary.current_tier, ReputationTier::Unverified);
        assert_eq!(summary.total_promotions, 0);
        assert_eq!(summary.total_demotions, 0);
        assert!(summary.days_at_current_tier.is_none());
        assert!(summary.recent_changes.is_empty());
    }

    #[test]
    fn tier_history_summary_with_records() {
        let user_id = Uuid::new_v4();
        let rec1 = make_record(
            ReputationTier::Bronze,
            ReputationTier::Silver,
            TierChangeDirection::Promotion,
            dec!(350),
            dec!(420),
            None,
        );
        let rec2 = make_record(
            ReputationTier::Silver,
            ReputationTier::Gold,
            TierChangeDirection::Promotion,
            dec!(420),
            dec!(610),
            None,
        );

        let summary = TierHistorySummary {
            user_id,
            current_tier: ReputationTier::Gold,
            highest_tier: ReputationTier::Gold,
            total_promotions: 2,
            total_demotions: 0,
            days_at_current_tier: Some(30),
            recent_changes: vec![rec1, rec2],
        };

        assert_eq!(summary.total_promotions, 2);
        assert_eq!(summary.total_demotions, 0);
        assert_eq!(
            summary.days_at_current_tier.expect("days should be set"),
            30
        );
        assert_eq!(summary.recent_changes.len(), 2);

        // highest_tier should be the max of all new_tier values in history
        let derived_highest = summary
            .recent_changes
            .iter()
            .map(|r| r.new_tier)
            .max()
            .unwrap_or(summary.current_tier);
        assert_eq!(derived_highest, ReputationTier::Gold);
    }

    // -----------------------------------------------------------------------
    // Direction classification via direction string mapping
    // -----------------------------------------------------------------------

    #[test]
    fn direction_string_mapping_promotion() {
        let direction = TierChangeDirection::Promotion;
        let s = match direction {
            TierChangeDirection::Promotion => "promotion",
            TierChangeDirection::Demotion => "demotion",
        };
        assert_eq!(s, "promotion");

        // Reconstruct as done in get_user_history
        let reconstructed = match s {
            "promotion" => TierChangeDirection::Promotion,
            _ => TierChangeDirection::Demotion,
        };
        assert_eq!(reconstructed, TierChangeDirection::Promotion);
        assert!(reconstructed.is_positive());
    }

    #[test]
    fn direction_string_mapping_demotion() {
        let direction = TierChangeDirection::Demotion;
        let s = match direction {
            TierChangeDirection::Promotion => "promotion",
            TierChangeDirection::Demotion => "demotion",
        };
        assert_eq!(s, "demotion");

        let reconstructed = match s {
            "promotion" => TierChangeDirection::Promotion,
            _ => TierChangeDirection::Demotion,
        };
        assert_eq!(reconstructed, TierChangeDirection::Demotion);
        assert!(reconstructed.is_negative());
    }

    // -----------------------------------------------------------------------
    // JSON serialization roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn tier_history_record_serializes_to_json() {
        let user_id = Uuid::new_v4();
        let record_id = Uuid::new_v4();
        let rec = TierHistoryRecord {
            id: record_id,
            user_id,
            previous_tier: ReputationTier::Silver,
            new_tier: ReputationTier::Gold,
            direction: TierChangeDirection::Promotion,
            previous_score: dec!(580),
            new_score: dec!(610),
            changed_at: Utc::now(),
            reason: Some("completed first trade".to_string()),
        };

        let json = serde_json::to_string(&rec).expect("TierHistoryRecord must serialize");
        // ReputationTier derives serde with no rename, so variants serialize as PascalCase
        assert!(
            json.contains("Silver"),
            "JSON should contain previous_tier variant"
        );
        assert!(
            json.contains("Gold"),
            "JSON should contain new_tier variant"
        );
        assert!(
            json.contains("completed first trade"),
            "JSON should contain reason"
        );
    }

    #[test]
    fn tier_history_summary_serializes_to_json() {
        let user_id = Uuid::new_v4();
        let summary = TierHistorySummary {
            user_id,
            current_tier: ReputationTier::Platinum,
            highest_tier: ReputationTier::Diamond,
            total_promotions: 5,
            total_demotions: 1,
            days_at_current_tier: Some(14),
            recent_changes: vec![],
        };

        let json = serde_json::to_string(&summary).expect("TierHistorySummary must serialize");
        assert!(json.contains("\"total_promotions\":5"));
        assert!(json.contains("\"total_demotions\":1"));
    }

    // -----------------------------------------------------------------------
    // Single-entry history — min/max/average are all the same entry
    // -----------------------------------------------------------------------

    #[test]
    fn single_entry_history_min_max_same() {
        let user_id = Uuid::new_v4();
        let rec = make_record(
            ReputationTier::Bronze,
            ReputationTier::Silver,
            TierChangeDirection::Promotion,
            dec!(350),
            dec!(420),
            None,
        );
        let records = vec![rec];

        let max_score = records
            .iter()
            .map(|r| r.new_score)
            .max()
            .expect("non-empty iterator must yield a max");
        let min_score = records
            .iter()
            .map(|r| r.new_score)
            .min()
            .expect("non-empty iterator must yield a min");

        assert_eq!(max_score, dec!(420));
        assert_eq!(min_score, dec!(420));

        let summary = TierHistorySummary {
            user_id,
            current_tier: ReputationTier::Silver,
            highest_tier: ReputationTier::Silver,
            total_promotions: 1,
            total_demotions: 0,
            days_at_current_tier: Some(1),
            recent_changes: records,
        };
        assert_eq!(summary.recent_changes.len(), 1);
    }
}
