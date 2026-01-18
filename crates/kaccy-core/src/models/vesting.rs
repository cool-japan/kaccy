//! Token vesting schedules

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Vesting schedule for token allocations
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VestingSchedule {
    pub schedule_id: Uuid,
    pub token_id: Uuid,
    pub beneficiary_user_id: Uuid,
    /// Total tokens allocated
    pub total_tokens: Decimal,
    /// Tokens already released
    pub released_tokens: Decimal,
    /// Vesting start time
    pub start_time: DateTime<Utc>,
    /// Cliff period (no tokens released before this)
    pub cliff_duration_seconds: i64,
    /// Total vesting duration
    pub vesting_duration_seconds: i64,
    /// Whether tokens are revocable by issuer
    pub revocable: bool,
    /// Whether schedule has been revoked
    pub revoked: bool,
    /// Vesting type
    pub vesting_type: VestingType,
    /// Optional milestone-based conditions
    pub milestone_conditions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum VestingType {
    /// Linear vesting over time
    #[default]
    Linear,
    /// Graded vesting (steps at intervals)
    Graded,
    /// Milestone-based vesting
    Milestone,
    /// Cliff-only (all at once after cliff)
    CliffOnly,
}

impl fmt::Display for VestingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VestingType::Linear => write!(f, "linear"),
            VestingType::Graded => write!(f, "graded"),
            VestingType::Milestone => write!(f, "milestone"),
            VestingType::CliffOnly => write!(f, "cliff_only"),
        }
    }
}

impl VestingSchedule {
    /// Calculate vested amount at a given time
    pub fn vested_amount(&self, at_time: DateTime<Utc>) -> Decimal {
        if self.revoked {
            return self.released_tokens; // No more vesting after revocation
        }

        let elapsed = (at_time - self.start_time).num_seconds();

        // Before cliff, nothing is vested
        if elapsed < self.cliff_duration_seconds {
            return dec!(0);
        }

        match self.vesting_type {
            VestingType::Linear => self.linear_vested_amount(elapsed),
            VestingType::Graded => self.graded_vested_amount(elapsed),
            VestingType::Milestone => self.released_tokens, // Milestone unlocked manually
            VestingType::CliffOnly => {
                if elapsed >= self.cliff_duration_seconds {
                    self.total_tokens
                } else {
                    dec!(0)
                }
            }
        }
    }

    /// Calculate linearly vested amount
    fn linear_vested_amount(&self, elapsed_seconds: i64) -> Decimal {
        if elapsed_seconds >= self.vesting_duration_seconds {
            return self.total_tokens;
        }

        let elapsed = Decimal::from(elapsed_seconds);
        let total_duration = Decimal::from(self.vesting_duration_seconds);

        (self.total_tokens * elapsed) / total_duration
    }

    /// Calculate graded vested amount (e.g., 25% every 3 months)
    fn graded_vested_amount(&self, elapsed_seconds: i64) -> Decimal {
        if elapsed_seconds >= self.vesting_duration_seconds {
            return self.total_tokens;
        }

        // For graded vesting, we divide into 4 equal parts over the duration
        let num_grades = 4;
        let grade_duration = self.vesting_duration_seconds / num_grades;

        let completed_grades = elapsed_seconds / grade_duration;
        let grade_amount = self.total_tokens / Decimal::from(num_grades);

        grade_amount * Decimal::from(completed_grades)
    }

    /// Calculate releasable amount (vested but not yet released)
    pub fn releasable_amount(&self, at_time: DateTime<Utc>) -> Decimal {
        let vested = self.vested_amount(at_time);
        (vested - self.released_tokens).max(dec!(0))
    }

    /// Check if vesting is complete
    pub fn is_vesting_complete(&self, at_time: DateTime<Utc>) -> bool {
        self.vested_amount(at_time) >= self.total_tokens
    }

    /// Calculate vesting progress percentage (0-1)
    pub fn vesting_progress(&self, at_time: DateTime<Utc>) -> Decimal {
        if self.total_tokens == dec!(0) {
            return dec!(1);
        }
        (self.vested_amount(at_time) / self.total_tokens).min(dec!(1))
    }

    /// Get time remaining until fully vested
    pub fn time_remaining(&self, at_time: DateTime<Utc>) -> Option<i64> {
        let end_time = self.start_time + chrono::Duration::seconds(self.vesting_duration_seconds);

        if at_time >= end_time {
            return None;
        }

        Some((end_time - at_time).num_seconds())
    }

    /// Validate the vesting schedule
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.total_tokens <= dec!(0) {
            return Err(ValidationError("Total tokens must be positive".to_string()));
        }
        if self.cliff_duration_seconds < 0 {
            return Err(ValidationError(
                "Cliff duration cannot be negative".to_string(),
            ));
        }
        if self.vesting_duration_seconds <= 0 {
            return Err(ValidationError(
                "Vesting duration must be positive".to_string(),
            ));
        }
        if self.cliff_duration_seconds > self.vesting_duration_seconds {
            return Err(ValidationError(
                "Cliff duration cannot exceed vesting duration".to_string(),
            ));
        }
        if self.released_tokens < dec!(0) {
            return Err(ValidationError(
                "Released tokens cannot be negative".to_string(),
            ));
        }
        if self.released_tokens > self.total_tokens {
            return Err(ValidationError(
                "Released tokens cannot exceed total tokens".to_string(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for VestingSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VestingSchedule({}, {}/{} tokens, type={})",
            self.schedule_id, self.released_tokens, self.total_tokens, self.vesting_type
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateVestingScheduleRequest {
    pub token_id: Uuid,
    pub beneficiary_user_id: Uuid,
    pub total_tokens: Decimal,
    pub cliff_duration_seconds: i64,
    pub vesting_duration_seconds: i64,
    pub revocable: bool,
    pub vesting_type: VestingType,
    pub milestone_conditions: Option<String>,
}

impl CreateVestingScheduleRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.total_tokens <= dec!(0) {
            return Err(ValidationError("Total tokens must be positive".to_string()));
        }
        if self.cliff_duration_seconds < 0 {
            return Err(ValidationError(
                "Cliff duration cannot be negative".to_string(),
            ));
        }
        if self.vesting_duration_seconds <= 0 {
            return Err(ValidationError(
                "Vesting duration must be positive".to_string(),
            ));
        }
        if self.cliff_duration_seconds > self.vesting_duration_seconds {
            return Err(ValidationError(
                "Cliff duration cannot exceed vesting duration".to_string(),
            ));
        }
        Ok(())
    }
}

/// Record of a vesting release event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VestingRelease {
    pub release_id: Uuid,
    pub schedule_id: Uuid,
    pub user_id: Uuid,
    pub amount: Decimal,
    pub released_at: DateTime<Utc>,
}

impl fmt::Display for VestingRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VestingRelease({}, amount={}, at={})",
            self.release_id, self.amount, self.released_at
        )
    }
}

/// Summary of vesting schedules for a user
#[derive(Debug, Serialize)]
pub struct VestingSummary {
    pub user_id: Uuid,
    pub total_schedules: i32,
    pub total_vesting_tokens: Decimal,
    pub total_vested_tokens: Decimal,
    pub total_released_tokens: Decimal,
    pub total_releasable_tokens: Decimal,
    pub schedules: Vec<VestingSchedule>,
}

/// Predefined vesting schedule templates
pub struct VestingTemplates;

impl VestingTemplates {
    /// Standard 4-year vesting with 1-year cliff (common for equity/token grants)
    pub fn standard_four_year() -> (i64, i64, VestingType) {
        const ONE_YEAR: i64 = 365 * 24 * 60 * 60;
        const FOUR_YEARS: i64 = 4 * ONE_YEAR;
        (ONE_YEAR, FOUR_YEARS, VestingType::Linear)
    }

    /// Quarterly vesting over 1 year
    pub fn quarterly_one_year() -> (i64, i64, VestingType) {
        const THREE_MONTHS: i64 = 90 * 24 * 60 * 60;
        const ONE_YEAR: i64 = 365 * 24 * 60 * 60;
        (THREE_MONTHS, ONE_YEAR, VestingType::Graded)
    }

    /// Monthly vesting over 2 years
    pub fn monthly_two_years() -> (i64, i64, VestingType) {
        const TWO_YEARS: i64 = 2 * 365 * 24 * 60 * 60;
        (0, TWO_YEARS, VestingType::Linear)
    }

    /// Immediate unlock (no vesting)
    pub fn immediate() -> (i64, i64, VestingType) {
        (0, 1, VestingType::CliffOnly)
    }

    /// Cliff-only after 6 months
    pub fn six_month_cliff() -> (i64, i64, VestingType) {
        const SIX_MONTHS: i64 = 180 * 24 * 60 * 60;
        (SIX_MONTHS, SIX_MONTHS, VestingType::CliffOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_vesting() {
        let now = Utc::now();
        let schedule = VestingSchedule {
            schedule_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            beneficiary_user_id: Uuid::new_v4(),
            total_tokens: dec!(1000),
            released_tokens: dec!(0),
            start_time: now,
            cliff_duration_seconds: 0,
            vesting_duration_seconds: 100,
            revocable: false,
            revoked: false,
            vesting_type: VestingType::Linear,
            milestone_conditions: None,
            created_at: now,
            updated_at: now,
        };

        // At start: 0 vested
        assert_eq!(schedule.vested_amount(now), dec!(0));

        // At 50% through: 500 vested
        let halfway = now + chrono::Duration::seconds(50);
        assert_eq!(schedule.vested_amount(halfway), dec!(500));

        // At 100%: all vested
        let end = now + chrono::Duration::seconds(100);
        assert_eq!(schedule.vested_amount(end), dec!(1000));

        // After end: still all vested
        let after = now + chrono::Duration::seconds(200);
        assert_eq!(schedule.vested_amount(after), dec!(1000));
    }

    #[test]
    fn test_cliff_vesting() {
        let now = Utc::now();
        let schedule = VestingSchedule {
            schedule_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            beneficiary_user_id: Uuid::new_v4(),
            total_tokens: dec!(1000),
            released_tokens: dec!(0),
            start_time: now,
            cliff_duration_seconds: 50,
            vesting_duration_seconds: 100,
            revocable: false,
            revoked: false,
            vesting_type: VestingType::Linear,
            milestone_conditions: None,
            created_at: now,
            updated_at: now,
        };

        // Before cliff: 0 vested
        let before_cliff = now + chrono::Duration::seconds(25);
        assert_eq!(schedule.vested_amount(before_cliff), dec!(0));

        // After cliff: normal vesting
        let after_cliff = now + chrono::Duration::seconds(75);
        assert_eq!(schedule.vested_amount(after_cliff), dec!(750));
    }

    #[test]
    fn test_graded_vesting() {
        let now = Utc::now();
        let schedule = VestingSchedule {
            schedule_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            beneficiary_user_id: Uuid::new_v4(),
            total_tokens: dec!(1000),
            released_tokens: dec!(0),
            start_time: now,
            cliff_duration_seconds: 0,
            vesting_duration_seconds: 100,
            revocable: false,
            revoked: false,
            vesting_type: VestingType::Graded,
            milestone_conditions: None,
            created_at: now,
            updated_at: now,
        };

        // At 0%: 0 vested
        assert_eq!(schedule.vested_amount(now), dec!(0));

        // At 25%: 250 vested (1st grade complete)
        let quarter = now + chrono::Duration::seconds(25);
        assert_eq!(schedule.vested_amount(quarter), dec!(250));

        // At 50%: 500 vested (2nd grade complete)
        let half = now + chrono::Duration::seconds(50);
        assert_eq!(schedule.vested_amount(half), dec!(500));
    }

    #[test]
    fn test_releasable_amount() {
        let now = Utc::now();
        let mut schedule = VestingSchedule {
            schedule_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            beneficiary_user_id: Uuid::new_v4(),
            total_tokens: dec!(1000),
            released_tokens: dec!(300),
            start_time: now,
            cliff_duration_seconds: 0,
            vesting_duration_seconds: 100,
            revocable: false,
            revoked: false,
            vesting_type: VestingType::Linear,
            milestone_conditions: None,
            created_at: now,
            updated_at: now,
        };

        // At 50%: 500 vested, 300 released, 200 releasable
        let halfway = now + chrono::Duration::seconds(50);
        assert_eq!(schedule.releasable_amount(halfway), dec!(200));

        // Release the tokens
        schedule.released_tokens = dec!(500);

        // Now 0 releasable
        assert_eq!(schedule.releasable_amount(halfway), dec!(0));
    }

    #[test]
    fn test_vesting_templates() {
        let (cliff, duration, vtype) = VestingTemplates::standard_four_year();
        assert_eq!(cliff, 365 * 24 * 60 * 60);
        assert_eq!(duration, 4 * 365 * 24 * 60 * 60);
        assert_eq!(vtype, VestingType::Linear);

        let (cliff, _duration, vtype) = VestingTemplates::immediate();
        assert_eq!(cliff, 0);
        assert_eq!(vtype, VestingType::CliffOnly);
    }
}
