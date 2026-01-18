//! Time-based reputation decay
//!
//! Applies decay to reputation scores based on inactivity.
//! Recent events are weighted more heavily, and scores decay
//! after periods of inactivity.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::Result;
use crate::tier::ReputationTier;

/// Configuration for time-based decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayConfig {
    /// Days of inactivity before decay starts
    pub inactivity_threshold_days: i64,
    /// Daily decay rate (percentage of score to decay per day)
    pub daily_decay_rate: Decimal,
    /// Minimum score that decay can reduce to
    pub decay_floor: Decimal,
    /// Maximum daily decay amount
    pub max_daily_decay: Decimal,
    /// Grace period for new users (no decay during this period)
    pub new_user_grace_days: i64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            inactivity_threshold_days: 90, // 3 months of inactivity
            daily_decay_rate: dec!(0.1),   // 0.1% per day
            decay_floor: dec!(200),        // Bronze tier minimum
            max_daily_decay: dec!(5),      // Max 5 points per day
            new_user_grace_days: 30,       // 1 month grace for new users
        }
    }
}

impl DecayConfig {
    /// Strict decay configuration
    pub fn strict() -> Self {
        Self {
            inactivity_threshold_days: 60,
            daily_decay_rate: dec!(0.2),
            decay_floor: dec!(100),
            max_daily_decay: dec!(10),
            new_user_grace_days: 14,
        }
    }

    /// Lenient decay configuration
    pub fn lenient() -> Self {
        Self {
            inactivity_threshold_days: 180,
            daily_decay_rate: dec!(0.05),
            decay_floor: dec!(300),
            max_daily_decay: dec!(2),
            new_user_grace_days: 60,
        }
    }

    /// Validate decay configuration parameters
    pub fn validate(&self) -> Result<()> {
        use crate::error::ReputationError;

        if self.inactivity_threshold_days <= 0 {
            return Err(ReputationError::Validation(
                "Inactivity threshold days must be positive".to_string(),
            ));
        }

        if self.daily_decay_rate < dec!(0) || self.daily_decay_rate > dec!(1) {
            return Err(ReputationError::Validation(
                "Daily decay rate must be between 0 and 1".to_string(),
            ));
        }

        if self.decay_floor < dec!(0) || self.decay_floor > dec!(1000) {
            return Err(ReputationError::Validation(
                "Decay floor must be between 0 and 1000".to_string(),
            ));
        }

        if self.max_daily_decay <= dec!(0) {
            return Err(ReputationError::Validation(
                "Max daily decay must be positive".to_string(),
            ));
        }

        if self.new_user_grace_days < 0 {
            return Err(ReputationError::Validation(
                "New user grace days cannot be negative".to_string(),
            ));
        }

        Ok(())
    }
}

/// User activity status for decay calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivity {
    pub user_id: Uuid,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub account_created_at: DateTime<Utc>,
    pub current_score: Decimal,
    pub days_inactive: i64,
    pub is_in_grace_period: bool,
    pub should_decay: bool,
}

/// Result of applying decay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayResult {
    pub user_id: Uuid,
    pub previous_score: Decimal,
    pub new_score: Decimal,
    pub decay_applied: Decimal,
    pub previous_tier: ReputationTier,
    pub new_tier: ReputationTier,
    pub tier_changed: bool,
}

/// Service for applying time-based decay
pub struct TimeDecayService {
    pool: PgPool,
    config: DecayConfig,
}

impl TimeDecayService {
    pub fn new(pool: PgPool, config: DecayConfig) -> Self {
        Self { pool, config }
    }

    pub fn with_default_config(pool: PgPool) -> Self {
        Self::new(pool, DecayConfig::default())
    }

    /// Get activity status for a user
    pub async fn get_user_activity(&self, user_id: Uuid) -> Result<Option<UserActivity>> {
        let row = sqlx::query(
            r#"
            SELECT
                u.user_id,
                u.reputation_score,
                u.created_at as account_created,
                (SELECT MAX(created_at) FROM reputation_events WHERE user_id = u.user_id) as last_event,
                (SELECT MAX(created_at) FROM output_commitments WHERE user_id = u.user_id) as last_commitment
            FROM users u
            WHERE u.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let user_id: Uuid = row.get("user_id");
                let current_score: Decimal = row.get("reputation_score");
                let account_created: DateTime<Utc> = row.get("account_created");
                let last_event: Option<DateTime<Utc>> = row.get("last_event");
                let last_commitment: Option<DateTime<Utc>> = row.get("last_commitment");

                // Last activity is the most recent of last_event or last_commitment
                let last_activity = match (last_event, last_commitment) {
                    (Some(e), Some(c)) => Some(e.max(c)),
                    (Some(e), None) => Some(e),
                    (None, Some(c)) => Some(c),
                    (None, None) => None,
                };

                let now = Utc::now();
                let account_age_days = (now - account_created).num_days();
                let is_in_grace_period = account_age_days < self.config.new_user_grace_days;

                let days_inactive = last_activity
                    .map(|la| (now - la).num_days())
                    .unwrap_or(account_age_days);

                let should_decay = !is_in_grace_period
                    && days_inactive > self.config.inactivity_threshold_days
                    && current_score > self.config.decay_floor;

                Ok(Some(UserActivity {
                    user_id,
                    last_activity_at: last_activity,
                    account_created_at: account_created,
                    current_score,
                    days_inactive,
                    is_in_grace_period,
                    should_decay,
                }))
            }
            None => Ok(None),
        }
    }

    /// Calculate decay amount for a user
    pub fn calculate_decay(&self, activity: &UserActivity) -> Decimal {
        if !activity.should_decay {
            return Decimal::ZERO;
        }

        // Days past the threshold
        let excess_inactive_days = activity.days_inactive - self.config.inactivity_threshold_days;
        if excess_inactive_days <= 0 {
            return Decimal::ZERO;
        }

        // Calculate decay: score * rate * excess_days, capped by max_daily_decay * days
        let percentage_decay = activity.current_score
            * (self.config.daily_decay_rate / dec!(100))
            * Decimal::from(excess_inactive_days);

        let max_decay = self.config.max_daily_decay * Decimal::from(excess_inactive_days);

        let decay = percentage_decay.min(max_decay);

        // Don't decay below floor
        let score_above_floor = activity.current_score - self.config.decay_floor;
        decay.min(score_above_floor.max(Decimal::ZERO))
    }

    /// Apply decay to a single user
    pub async fn apply_decay(&self, user_id: Uuid) -> Result<Option<DecayResult>> {
        let activity = match self.get_user_activity(user_id).await? {
            Some(a) => a,
            None => return Ok(None),
        };

        let decay_amount = self.calculate_decay(&activity);
        if decay_amount <= Decimal::ZERO {
            return Ok(None);
        }

        let previous_score = activity.current_score;
        let previous_tier = ReputationTier::from_score(previous_score);

        // Apply decay in transaction
        let mut tx = self.pool.begin().await?;

        // Update score
        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST($2, reputation_score - $1)
            WHERE user_id = $3
            RETURNING reputation_score
            "#,
        )
        .bind(decay_amount)
        .bind(self.config.decay_floor)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        // Record decay event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, 'time_decay', $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(-decay_amount)
        .bind(format!(
            "Inactivity decay: {} days since last activity",
            activity.days_inactive
        ))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let new_tier = ReputationTier::from_score(new_score);
        let tier_changed = new_tier != previous_tier;

        Ok(Some(DecayResult {
            user_id,
            previous_score,
            new_score,
            decay_applied: decay_amount,
            previous_tier,
            new_tier,
            tier_changed,
        }))
    }

    /// Apply decay to all inactive users (batch job)
    pub async fn apply_decay_to_all(&self) -> Result<Vec<DecayResult>> {
        // Find users who should have decay applied
        let inactive_users: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT u.user_id
            FROM users u
            WHERE u.reputation_score > $1
              AND u.created_at < NOW() - INTERVAL '1 day' * $2
              AND (
                NOT EXISTS (
                    SELECT 1 FROM reputation_events re
                    WHERE re.user_id = u.user_id
                      AND re.created_at > NOW() - INTERVAL '1 day' * $3
                )
                AND NOT EXISTS (
                    SELECT 1 FROM output_commitments oc
                    WHERE oc.user_id = u.user_id
                      AND oc.created_at > NOW() - INTERVAL '1 day' * $3
                )
              )
            "#,
        )
        .bind(self.config.decay_floor)
        .bind(self.config.new_user_grace_days)
        .bind(self.config.inactivity_threshold_days)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for (user_id,) in inactive_users {
            if let Some(result) = self.apply_decay(user_id).await? {
                results.push(result);
            }
        }

        if !results.is_empty() {
            tracing::info!(
                count = results.len(),
                "Applied time decay to inactive users"
            );
        }

        Ok(results)
    }

    /// Get users approaching decay threshold (for notifications)
    pub async fn get_users_approaching_decay(
        &self,
        days_warning: i64,
    ) -> Result<Vec<UserActivity>> {
        let threshold_days = self.config.inactivity_threshold_days - days_warning;

        let rows = sqlx::query(
            r#"
            SELECT
                u.user_id,
                u.reputation_score,
                u.created_at as account_created,
                COALESCE(
                    (SELECT MAX(created_at) FROM reputation_events WHERE user_id = u.user_id),
                    u.created_at
                ) as last_activity
            FROM users u
            WHERE u.reputation_score > $1
              AND u.created_at < NOW() - INTERVAL '1 day' * $2
            "#,
        )
        .bind(self.config.decay_floor)
        .bind(self.config.new_user_grace_days)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut approaching = Vec::new();

        for row in rows {
            let user_id: Uuid = row.get("user_id");
            let current_score: Decimal = row.get("reputation_score");
            let account_created: DateTime<Utc> = row.get("account_created");
            let last_activity: DateTime<Utc> = row.get("last_activity");

            let days_inactive = (now - last_activity).num_days();

            if days_inactive >= threshold_days
                && days_inactive < self.config.inactivity_threshold_days
            {
                approaching.push(UserActivity {
                    user_id,
                    last_activity_at: Some(last_activity),
                    account_created_at: account_created,
                    current_score,
                    days_inactive,
                    is_in_grace_period: false,
                    should_decay: false,
                });
            }
        }

        Ok(approaching)
    }

    /// Preview decay for a user without applying
    pub async fn preview_decay(&self, user_id: Uuid) -> Result<Option<(Decimal, Decimal)>> {
        let activity = match self.get_user_activity(user_id).await? {
            Some(a) => a,
            None => return Ok(None),
        };

        let decay = self.calculate_decay(&activity);
        let projected_score = (activity.current_score - decay).max(self.config.decay_floor);

        Ok(Some((decay, projected_score)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_decay_config_default() {
        let config = DecayConfig::default();
        assert_eq!(config.inactivity_threshold_days, 90);
        assert_eq!(config.daily_decay_rate, dec!(0.1));
        assert_eq!(config.decay_floor, dec!(200));
        assert_eq!(config.max_daily_decay, dec!(5));
        assert_eq!(config.new_user_grace_days, 30);
    }

    #[test]
    fn test_decay_config_strict() {
        let config = DecayConfig::strict();
        assert_eq!(config.inactivity_threshold_days, 60);
        assert_eq!(config.daily_decay_rate, dec!(0.2));
        assert_eq!(config.decay_floor, dec!(100));
        assert_eq!(config.max_daily_decay, dec!(10));
        assert_eq!(config.new_user_grace_days, 14);
    }

    #[test]
    fn test_decay_config_lenient() {
        let config = DecayConfig::lenient();
        assert_eq!(config.inactivity_threshold_days, 180);
        assert_eq!(config.daily_decay_rate, dec!(0.05));
        assert_eq!(config.decay_floor, dec!(300));
        assert_eq!(config.max_daily_decay, dec!(2));
        assert_eq!(config.new_user_grace_days, 60);
    }

    // Phase 17 tests - New functionality
    #[test]
    fn test_decay_config_validation_valid() {
        let config = DecayConfig::default();
        assert!(config.validate().is_ok());

        let strict = DecayConfig::strict();
        assert!(strict.validate().is_ok());

        let lenient = DecayConfig::lenient();
        assert!(lenient.validate().is_ok());
    }

    #[test]
    fn test_decay_config_validation_invalid_threshold() {
        let config = DecayConfig {
            inactivity_threshold_days: 0,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config.validate().is_err());

        let config2 = DecayConfig {
            inactivity_threshold_days: -10,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_decay_config_validation_invalid_decay_rate() {
        let config = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(-0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config.validate().is_err());

        let config2 = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(1.5),
            decay_floor: dec!(200),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_decay_config_validation_invalid_decay_floor() {
        let config = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(-100),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config.validate().is_err());

        let config2 = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(1500),
            max_daily_decay: dec!(5),
            new_user_grace_days: 30,
        };
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_decay_config_validation_invalid_max_daily_decay() {
        let config = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(0),
            new_user_grace_days: 30,
        };
        assert!(config.validate().is_err());

        let config2 = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(-5),
            new_user_grace_days: 30,
        };
        assert!(config2.validate().is_err());
    }

    #[test]
    fn test_decay_config_validation_invalid_grace_period() {
        let config = DecayConfig {
            inactivity_threshold_days: 90,
            daily_decay_rate: dec!(0.1),
            decay_floor: dec!(200),
            max_daily_decay: dec!(5),
            new_user_grace_days: -10,
        };
        assert!(config.validate().is_err());
    }
}
