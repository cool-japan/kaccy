//! Score history tracking with daily snapshots

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::Result;
use crate::tier::ReputationTier;

/// Daily score snapshot for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreSnapshot {
    /// Unique identifier for this snapshot.
    pub snapshot_id: Uuid,
    /// User this snapshot belongs to.
    pub user_id: Uuid,
    /// Calendar date of the snapshot.
    pub snapshot_date: NaiveDate,
    /// Overall reputation score on this date.
    pub score: Decimal,
    /// Reputation tier on this date.
    pub tier: ReputationTier,
    /// Score change from previous day
    pub delta_from_previous: Decimal,
    /// Components at time of snapshot (serialized JSON)
    pub components_json: Option<String>,
    /// Timestamp when this snapshot was recorded.
    pub created_at: DateTime<Utc>,
}

/// Score trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreTrend {
    /// User this trend belongs to.
    pub user_id: Uuid,
    /// Average score over the period
    pub average_score: Decimal,
    /// Minimum score in period
    pub min_score: Decimal,
    /// Maximum score in period
    pub max_score: Decimal,
    /// Score at start of period
    pub start_score: Decimal,
    /// Score at end of period
    pub end_score: Decimal,
    /// Total change over period
    pub total_change: Decimal,
    /// Direction of trend (positive, negative, stable)
    pub trend_direction: TrendDirection,
    /// Volatility (standard deviation)
    pub volatility: Decimal,
    /// Number of snapshots in period
    pub snapshot_count: i64,
}

/// Direction of a score trend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrendDirection {
    /// Score is increasing over the period.
    Rising,
    /// Score is decreasing over the period.
    Falling,
    /// Score is relatively flat over the period.
    Stable,
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rising => write!(f, "Rising"),
            Self::Falling => write!(f, "Falling"),
            Self::Stable => write!(f, "Stable"),
        }
    }
}

impl std::str::FromStr for TrendDirection {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "rising" | "up" | "upward" | "increasing" => Ok(TrendDirection::Rising),
            "falling" | "down" | "downward" | "decreasing" => Ok(TrendDirection::Falling),
            "stable" | "flat" | "steady" => Ok(TrendDirection::Stable),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid trend direction: {}. Valid values: rising, falling, stable",
                s
            ))),
        }
    }
}

impl TrendDirection {
    /// Classify a score change as Rising, Falling, or Stable based on a threshold.
    pub fn from_change(change: Decimal, threshold: Decimal) -> Self {
        if change > threshold {
            Self::Rising
        } else if change < -threshold {
            Self::Falling
        } else {
            Self::Stable
        }
    }
}

/// Repository for score history operations
pub struct ScoreHistoryRepository {
    pool: PgPool,
}

impl ScoreHistoryRepository {
    /// Create a new [`ScoreHistoryRepository`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Take a daily snapshot for a user
    pub async fn take_snapshot(&self, user_id: Uuid) -> Result<ScoreSnapshot> {
        let today = Utc::now().date_naive();

        // Get current score and tier
        let row = sqlx::query(r#"SELECT reputation_score FROM users WHERE user_id = $1"#)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let score: Decimal = row.get("reputation_score");
        let tier = ReputationTier::from_score(score);

        // Get previous snapshot for delta calculation
        let previous = self.get_latest_snapshot(user_id).await?;
        let delta = previous
            .as_ref()
            .map(|p| score - p.score)
            .unwrap_or(Decimal::ZERO);

        // Insert or update snapshot for today
        let snapshot_id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO score_snapshots (snapshot_id, user_id, snapshot_date, score, tier, delta_from_previous, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (user_id, snapshot_date)
            DO UPDATE SET score = $4, tier = $5, delta_from_previous = $6
            "#,
        )
        .bind(snapshot_id)
        .bind(user_id)
        .bind(today)
        .bind(score)
        .bind(tier.to_string())
        .bind(delta)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(ScoreSnapshot {
            snapshot_id,
            user_id,
            snapshot_date: today,
            score,
            tier,
            delta_from_previous: delta,
            components_json: None,
            created_at: now,
        })
    }

    /// Get the latest snapshot for a user
    pub async fn get_latest_snapshot(&self, user_id: Uuid) -> Result<Option<ScoreSnapshot>> {
        let row = sqlx::query(
            r#"
            SELECT snapshot_id, user_id, snapshot_date, score, tier, delta_from_previous, components_json, created_at
            FROM score_snapshots
            WHERE user_id = $1
            ORDER BY snapshot_date DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let tier_str: String = row.get("tier");
                Ok(Some(ScoreSnapshot {
                    snapshot_id: row.get("snapshot_id"),
                    user_id: row.get("user_id"),
                    snapshot_date: row.get("snapshot_date"),
                    score: row.get("score"),
                    tier: parse_tier(&tier_str),
                    delta_from_previous: row.get("delta_from_previous"),
                    components_json: row.get("components_json"),
                    created_at: row.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get snapshots within a date range
    pub async fn get_snapshots_in_range(
        &self,
        user_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<ScoreSnapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT snapshot_id, user_id, snapshot_date, score, tier, delta_from_previous, components_json, created_at
            FROM score_snapshots
            WHERE user_id = $1 AND snapshot_date >= $2 AND snapshot_date <= $3
            ORDER BY snapshot_date ASC
            "#,
        )
        .bind(user_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let tier_str: String = row.get("tier");
                ScoreSnapshot {
                    snapshot_id: row.get("snapshot_id"),
                    user_id: row.get("user_id"),
                    snapshot_date: row.get("snapshot_date"),
                    score: row.get("score"),
                    tier: parse_tier(&tier_str),
                    delta_from_previous: row.get("delta_from_previous"),
                    components_json: row.get("components_json"),
                    created_at: row.get("created_at"),
                }
            })
            .collect())
    }

    /// Get last N days of snapshots
    pub async fn get_recent_snapshots(
        &self,
        user_id: Uuid,
        days: u32,
    ) -> Result<Vec<ScoreSnapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT snapshot_id, user_id, snapshot_date, score, tier, delta_from_previous, components_json, created_at
            FROM score_snapshots
            WHERE user_id = $1 AND snapshot_date >= CURRENT_DATE - $2::integer
            ORDER BY snapshot_date ASC
            "#,
        )
        .bind(user_id)
        .bind(days as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let tier_str: String = row.get("tier");
                ScoreSnapshot {
                    snapshot_id: row.get("snapshot_id"),
                    user_id: row.get("user_id"),
                    snapshot_date: row.get("snapshot_date"),
                    score: row.get("score"),
                    tier: parse_tier(&tier_str),
                    delta_from_previous: row.get("delta_from_previous"),
                    components_json: row.get("components_json"),
                    created_at: row.get("created_at"),
                }
            })
            .collect())
    }

    /// Calculate score trend for a period
    pub async fn calculate_trend(&self, user_id: Uuid, days: u32) -> Result<Option<ScoreTrend>> {
        let snapshots = self.get_recent_snapshots(user_id, days).await?;

        if snapshots.is_empty() {
            return Ok(None);
        }

        let count = snapshots.len() as i64;
        let scores: Vec<Decimal> = snapshots.iter().map(|s| s.score).collect();

        let sum: Decimal = scores.iter().sum();
        let average = sum / Decimal::from(count);

        let min_score = scores.iter().min().cloned().unwrap_or(Decimal::ZERO);
        let max_score = scores.iter().max().cloned().unwrap_or(Decimal::ZERO);

        let start_score = snapshots.first().map(|s| s.score).unwrap_or(Decimal::ZERO);
        let end_score = snapshots.last().map(|s| s.score).unwrap_or(Decimal::ZERO);
        let total_change = end_score - start_score;

        // Calculate volatility (standard deviation)
        let variance_sum: Decimal = scores
            .iter()
            .map(|s| {
                let diff = *s - average;
                diff * diff
            })
            .sum();
        let variance = variance_sum / Decimal::from(count.max(1));
        // Approximate sqrt using Newton's method
        let volatility = sqrt_approx(variance);

        // Determine trend direction (threshold is 5 points)
        let trend_direction = TrendDirection::from_change(total_change, dec!(5));

        Ok(Some(ScoreTrend {
            user_id,
            average_score: average,
            min_score,
            max_score,
            start_score,
            end_score,
            total_change,
            trend_direction,
            volatility,
            snapshot_count: count,
        }))
    }

    /// Take snapshots for all users (batch job)
    pub async fn take_all_snapshots(&self) -> Result<u64> {
        let today = Utc::now().date_naive();

        let result = sqlx::query(
            r#"
            INSERT INTO score_snapshots (snapshot_id, user_id, snapshot_date, score, tier, delta_from_previous, created_at)
            SELECT
                gen_random_uuid(),
                u.user_id,
                $1,
                u.reputation_score,
                CASE
                    WHEN u.reputation_score >= 900 THEN 'diamond'
                    WHEN u.reputation_score >= 800 THEN 'platinum'
                    WHEN u.reputation_score >= 600 THEN 'gold'
                    WHEN u.reputation_score >= 400 THEN 'silver'
                    WHEN u.reputation_score >= 200 THEN 'bronze'
                    ELSE 'unverified'
                END,
                u.reputation_score - COALESCE(
                    (SELECT score FROM score_snapshots WHERE user_id = u.user_id ORDER BY snapshot_date DESC LIMIT 1),
                    u.reputation_score
                ),
                NOW()
            FROM users u
            ON CONFLICT (user_id, snapshot_date) DO UPDATE
            SET score = EXCLUDED.score, tier = EXCLUDED.tier, delta_from_previous = EXCLUDED.delta_from_previous
            "#,
        )
        .bind(today)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Delete old snapshots (data retention)
    pub async fn delete_old_snapshots(&self, days_to_keep: u32) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM score_snapshots
            WHERE snapshot_date < CURRENT_DATE - $1::integer
            "#,
        )
        .bind(days_to_keep as i32)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get percentile ranking for a user
    pub async fn get_percentile(&self, user_id: Uuid) -> Result<Option<f64>> {
        let row: Option<(f64,)> = sqlx::query_as(
            r#"
            SELECT
                PERCENT_RANK() OVER (ORDER BY reputation_score) * 100
            FROM users
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(p,)| p))
    }

    /// Get top N users by score
    pub async fn get_top_users(&self, limit: u32) -> Result<Vec<(Uuid, Decimal, ReputationTier)>> {
        let rows = sqlx::query(
            r#"
            SELECT user_id, reputation_score
            FROM users
            ORDER BY reputation_score DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let user_id: Uuid = row.get("user_id");
                let score: Decimal = row.get("reputation_score");
                let tier = ReputationTier::from_score(score);
                (user_id, score, tier)
            })
            .collect())
    }
}

/// Helper to parse tier from string
fn parse_tier(s: &str) -> ReputationTier {
    match s.to_lowercase().as_str() {
        "diamond" => ReputationTier::Diamond,
        "platinum" => ReputationTier::Platinum,
        "gold" => ReputationTier::Gold,
        "silver" => ReputationTier::Silver,
        "bronze" => ReputationTier::Bronze,
        _ => ReputationTier::Unverified,
    }
}

/// Approximate square root using Newton's method
fn sqrt_approx(n: Decimal) -> Decimal {
    if n <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    let mut x = n;
    let two = dec!(2);

    // 10 iterations should be sufficient for convergence
    for _ in 0..10 {
        let x_next = (x + n / x) / two;
        if (x_next - x).abs() < dec!(0.0001) {
            return x_next;
        }
        x = x_next;
    }

    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_direction_display() {
        assert_eq!(TrendDirection::Rising.to_string(), "Rising");
        assert_eq!(TrendDirection::Falling.to_string(), "Falling");
        assert_eq!(TrendDirection::Stable.to_string(), "Stable");
    }

    #[test]
    fn test_trend_direction_from_str_valid() {
        assert_eq!(
            "rising".parse::<TrendDirection>().unwrap(),
            TrendDirection::Rising
        );
        assert_eq!(
            "falling".parse::<TrendDirection>().unwrap(),
            TrendDirection::Falling
        );
        assert_eq!(
            "stable".parse::<TrendDirection>().unwrap(),
            TrendDirection::Stable
        );
    }

    #[test]
    fn test_trend_direction_from_str_synonyms() {
        // Rising synonyms
        assert_eq!(
            "up".parse::<TrendDirection>().unwrap(),
            TrendDirection::Rising
        );
        assert_eq!(
            "upward".parse::<TrendDirection>().unwrap(),
            TrendDirection::Rising
        );
        assert_eq!(
            "increasing".parse::<TrendDirection>().unwrap(),
            TrendDirection::Rising
        );

        // Falling synonyms
        assert_eq!(
            "down".parse::<TrendDirection>().unwrap(),
            TrendDirection::Falling
        );
        assert_eq!(
            "downward".parse::<TrendDirection>().unwrap(),
            TrendDirection::Falling
        );
        assert_eq!(
            "decreasing".parse::<TrendDirection>().unwrap(),
            TrendDirection::Falling
        );

        // Stable synonyms
        assert_eq!(
            "flat".parse::<TrendDirection>().unwrap(),
            TrendDirection::Stable
        );
        assert_eq!(
            "steady".parse::<TrendDirection>().unwrap(),
            TrendDirection::Stable
        );
    }

    #[test]
    fn test_trend_direction_from_str_case_insensitive() {
        assert_eq!(
            "RISING".parse::<TrendDirection>().unwrap(),
            TrendDirection::Rising
        );
        assert_eq!(
            "Falling".parse::<TrendDirection>().unwrap(),
            TrendDirection::Falling
        );
        assert_eq!(
            "STABLE".parse::<TrendDirection>().unwrap(),
            TrendDirection::Stable
        );
    }

    #[test]
    fn test_trend_direction_from_str_invalid() {
        assert!("invalid".parse::<TrendDirection>().is_err());
        assert!("unknown".parse::<TrendDirection>().is_err());
    }
}
