//! Recurring commitments module
//!
//! This module provides:
//! - Recurring commitment definitions (monthly, weekly, etc.)
//! - Automatic commitment generation
//! - Recurring commitment tracking

use chrono::{DateTime, Datelike, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{ReputationError, Result};

/// Recurrence frequency
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceFrequency {
    /// Every week
    Weekly,
    /// Every two weeks
    Biweekly,
    /// Every month
    Monthly,
    /// Every quarter (3 months)
    Quarterly,
}

impl RecurrenceFrequency {
    /// Get the database string representation
    pub fn to_db_string(&self) -> &'static str {
        match self {
            RecurrenceFrequency::Weekly => "weekly",
            RecurrenceFrequency::Biweekly => "biweekly",
            RecurrenceFrequency::Monthly => "monthly",
            RecurrenceFrequency::Quarterly => "quarterly",
        }
    }

    /// Parse from database string
    pub fn from_db_string(s: &str) -> Option<Self> {
        match s {
            "weekly" => Some(RecurrenceFrequency::Weekly),
            "biweekly" => Some(RecurrenceFrequency::Biweekly),
            "monthly" => Some(RecurrenceFrequency::Monthly),
            "quarterly" => Some(RecurrenceFrequency::Quarterly),
            _ => None,
        }
    }

    /// Calculate next deadline from current date
    pub fn next_deadline(&self, from: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            RecurrenceFrequency::Weekly => from + Duration::days(7),
            RecurrenceFrequency::Biweekly => from + Duration::days(14),
            RecurrenceFrequency::Monthly => {
                // Add one month
                let next_month = if from.month() == 12 {
                    from.with_year(from.year() + 1)
                        .and_then(|d| d.with_month(1))
                } else {
                    from.with_month(from.month() + 1)
                };
                next_month.unwrap_or(from + Duration::days(30))
            }
            RecurrenceFrequency::Quarterly => {
                // Add three months
                let target_month = from.month() + 3;
                let (year, month) = if target_month > 12 {
                    (from.year() + 1, target_month - 12)
                } else {
                    (from.year(), target_month)
                };
                from.with_year(year)
                    .and_then(|d| d.with_month(month))
                    .unwrap_or(from + Duration::days(90))
            }
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            RecurrenceFrequency::Weekly => "every week",
            RecurrenceFrequency::Biweekly => "every two weeks",
            RecurrenceFrequency::Monthly => "every month",
            RecurrenceFrequency::Quarterly => "every quarter",
        }
    }
}

impl std::fmt::Display for RecurrenceFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_db_string())
    }
}

impl std::str::FromStr for RecurrenceFrequency {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "weekly" => Ok(RecurrenceFrequency::Weekly),
            "biweekly" | "bi-weekly" | "bi_weekly" => Ok(RecurrenceFrequency::Biweekly),
            "monthly" => Ok(RecurrenceFrequency::Monthly),
            "quarterly" => Ok(RecurrenceFrequency::Quarterly),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid recurrence frequency: {}. Valid values: weekly, biweekly, monthly, quarterly",
                s
            ))),
        }
    }
}

/// A recurring commitment definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecurringCommitment {
    /// Unique identifier for this recurring commitment.
    pub recurring_id: Uuid,
    /// User who owns this recurring commitment.
    pub user_id: Uuid,
    /// Token associated with this recurring commitment.
    pub token_id: Uuid,
    /// Short descriptive title.
    pub title: String,
    /// Optional extended description.
    pub description: Option<String>,
    /// String representation of the recurrence frequency.
    pub frequency: String,
    /// Whether this recurring commitment is currently active.
    pub is_active: bool,
    /// Total number of instances generated so far.
    pub instances_created: i32,
    /// Number of instances that have been fulfilled.
    pub instances_fulfilled: i32,
    /// Number of instances that have failed.
    pub instances_failed: i32,
    /// Timestamp when the last instance was generated.
    pub last_generated_at: Option<DateTime<Utc>>,
    /// Deadline for the next generated instance.
    pub next_deadline: DateTime<Utc>,
    /// When this recurring commitment was created.
    pub created_at: DateTime<Utc>,
}

impl RecurringCommitment {
    /// Get parsed frequency
    pub fn parsed_frequency(&self) -> Option<RecurrenceFrequency> {
        RecurrenceFrequency::from_db_string(&self.frequency)
    }

    /// Calculate fulfillment rate
    pub fn fulfillment_rate(&self) -> Decimal {
        let total = self.instances_fulfilled + self.instances_failed;
        if total == 0 {
            dec!(1) // 100% for no instances yet
        } else {
            Decimal::from(self.instances_fulfilled) / Decimal::from(total)
        }
    }
}

/// Generated commitment instance from a recurring commitment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCommitment {
    /// ID of the generated commitment record.
    pub commitment_id: Uuid,
    /// Parent recurring commitment ID.
    pub recurring_id: Uuid,
    /// Sequential instance number (1-based).
    pub instance_number: i32,
    /// Deadline for this specific instance.
    pub deadline: DateTime<Utc>,
}

/// Request to create a recurring commitment
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRecurringRequest {
    /// Token to associate with this recurring commitment.
    pub token_id: Uuid,
    /// Short descriptive title.
    pub title: String,
    /// Optional extended description.
    pub description: Option<String>,
    /// How often instances should be generated.
    pub frequency: RecurrenceFrequency,
    /// When to start the first commitment
    pub start_date: Option<DateTime<Utc>>,
}

impl CreateRecurringRequest {
    /// Validate the request, returning an error if any field is invalid.
    pub fn validate(&self) -> Result<()> {
        if self.title.is_empty() {
            return Err(ReputationError::Validation("Title is required".to_string()));
        }
        if self.title.len() > 200 {
            return Err(ReputationError::Validation(
                "Title must be at most 200 characters".to_string(),
            ));
        }
        if let Some(ref desc) = self.description {
            if desc.len() > 2000 {
                return Err(ReputationError::Validation(
                    "Description must be at most 2000 characters".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Summary of recurring commitments for a user
#[derive(Debug, Clone, Serialize, Default)]
pub struct RecurringSummary {
    /// Number of currently active recurring commitments.
    pub total_active: i64,
    /// Total number of generated instances across all recurring commitments.
    pub total_instances: i64,
    /// Number of instances that were fulfilled.
    pub fulfilled_instances: i64,
    /// Number of instances that failed.
    pub failed_instances: i64,
    /// Average fulfillment rate across all instances (0.0–1.0).
    pub average_fulfillment_rate: Decimal,
    /// Upcoming deadlines sorted by proximity.
    pub upcoming_deadlines: Vec<UpcomingDeadline>,
}

/// Upcoming deadline info
#[derive(Debug, Clone, Serialize)]
pub struct UpcomingDeadline {
    /// Parent recurring commitment ID.
    pub recurring_id: Uuid,
    /// Title of the recurring commitment.
    pub title: String,
    /// Absolute deadline timestamp.
    pub deadline: DateTime<Utc>,
    /// Number of days until the deadline.
    pub days_until: i64,
}

/// Service for managing recurring commitments
pub struct RecurringCommitmentService {
    pool: PgPool,
}

impl RecurringCommitmentService {
    /// Create a new [`RecurringCommitmentService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new recurring commitment
    pub async fn create(
        &self,
        user_id: Uuid,
        request: &CreateRecurringRequest,
    ) -> Result<RecurringCommitment> {
        request.validate()?;

        let start = request.start_date.unwrap_or_else(Utc::now);
        let next_deadline = request.frequency.next_deadline(start);

        let recurring: RecurringCommitment = sqlx::query_as(
            r#"
            INSERT INTO recurring_commitments
                (user_id, token_id, title, description, frequency, next_deadline)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(request.token_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(request.frequency.to_db_string())
        .bind(next_deadline)
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            recurring_id = %recurring.recurring_id,
            frequency = %request.frequency,
            "Recurring commitment created"
        );

        Ok(recurring)
    }

    /// Generate the next commitment instance
    pub async fn generate_instance(
        &self,
        recurring_id: Uuid,
    ) -> Result<Option<GeneratedCommitment>> {
        let mut tx = self.pool.begin().await?;

        // Get recurring commitment
        let recurring: Option<RecurringCommitment> = sqlx::query_as(
            r#"
            SELECT * FROM recurring_commitments
            WHERE recurring_id = $1 AND is_active = true
            FOR UPDATE
            "#,
        )
        .bind(recurring_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(recurring) = recurring else {
            return Ok(None);
        };

        let now = Utc::now();
        let frequency = recurring.parsed_frequency().ok_or_else(|| {
            ReputationError::Validation(format!("Invalid frequency: {}", recurring.frequency))
        })?;

        // Check if it's time to generate
        if recurring.next_deadline > now {
            return Ok(None);
        }

        let instance_number = recurring.instances_created + 1;

        // Create the actual commitment
        let (commitment_id,): (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO output_commitments (user_id, token_id, title, description, deadline)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING commitment_id
            "#,
        )
        .bind(recurring.user_id)
        .bind(recurring.token_id)
        .bind(format!("{} #{}", recurring.title, instance_number))
        .bind(&recurring.description)
        .bind(recurring.next_deadline)
        .fetch_one(&mut *tx)
        .await?;

        // Link to recurring
        sqlx::query(
            r#"
            INSERT INTO recurring_instances (recurring_id, commitment_id, instance_number)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(recurring_id)
        .bind(commitment_id)
        .bind(instance_number)
        .execute(&mut *tx)
        .await?;

        // Update recurring commitment
        let next_deadline = frequency.next_deadline(recurring.next_deadline);
        sqlx::query(
            r#"
            UPDATE recurring_commitments
            SET instances_created = instances_created + 1,
                last_generated_at = NOW(),
                next_deadline = $2
            WHERE recurring_id = $1
            "#,
        )
        .bind(recurring_id)
        .bind(next_deadline)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            recurring_id = %recurring_id,
            commitment_id = %commitment_id,
            instance = instance_number,
            "Generated recurring commitment instance"
        );

        Ok(Some(GeneratedCommitment {
            commitment_id,
            recurring_id,
            instance_number,
            deadline: recurring.next_deadline,
        }))
    }

    /// Generate all due commitment instances (batch job)
    pub async fn generate_all_due(&self) -> Result<Vec<GeneratedCommitment>> {
        // Get all active recurring commitments that are due
        let due: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT recurring_id FROM recurring_commitments
            WHERE is_active = true AND next_deadline <= NOW()
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut generated = Vec::new();
        for (recurring_id,) in due {
            if let Some(instance) = self.generate_instance(recurring_id).await? {
                generated.push(instance);
            }
        }

        if !generated.is_empty() {
            tracing::info!(
                count = generated.len(),
                "Generated recurring commitment instances"
            );
        }

        Ok(generated)
    }

    /// Update recurring stats when a commitment is verified
    pub async fn update_instance_result(&self, commitment_id: Uuid, fulfilled: bool) -> Result<()> {
        let column = if fulfilled {
            "instances_fulfilled"
        } else {
            "instances_failed"
        };

        sqlx::query(&format!(
            r#"
            UPDATE recurring_commitments rc
            SET {} = {} + 1
            FROM recurring_instances ri
            WHERE ri.recurring_id = rc.recurring_id
            AND ri.commitment_id = $1
            "#,
            column, column
        ))
        .bind(commitment_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get user's recurring commitments
    pub async fn get_user_recurring(&self, user_id: Uuid) -> Result<Vec<RecurringCommitment>> {
        let recurring = sqlx::query_as::<_, RecurringCommitment>(
            r#"
            SELECT * FROM recurring_commitments
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(recurring)
    }

    /// Get recurring commitment by ID
    pub async fn get_by_id(&self, recurring_id: Uuid) -> Result<Option<RecurringCommitment>> {
        let recurring = sqlx::query_as::<_, RecurringCommitment>(
            "SELECT * FROM recurring_commitments WHERE recurring_id = $1",
        )
        .bind(recurring_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(recurring)
    }

    /// Pause a recurring commitment
    pub async fn pause(&self, recurring_id: Uuid, user_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE recurring_commitments
            SET is_active = false
            WHERE recurring_id = $1 AND user_id = $2
            "#,
        )
        .bind(recurring_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Resume a recurring commitment
    pub async fn resume(&self, recurring_id: Uuid, user_id: Uuid) -> Result<bool> {
        let mut tx = self.pool.begin().await?;

        // Get current recurring
        let recurring: Option<RecurringCommitment> = sqlx::query_as(
            "SELECT * FROM recurring_commitments WHERE recurring_id = $1 AND user_id = $2",
        )
        .bind(recurring_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(recurring) = recurring else {
            return Ok(false);
        };

        // Calculate next deadline from now if it's in the past
        let now = Utc::now();
        let next_deadline = if recurring.next_deadline < now {
            let frequency = recurring.parsed_frequency().ok_or_else(|| {
                ReputationError::Validation(format!("Invalid frequency: {}", recurring.frequency))
            })?;
            frequency.next_deadline(now)
        } else {
            recurring.next_deadline
        };

        sqlx::query(
            r#"
            UPDATE recurring_commitments
            SET is_active = true, next_deadline = $3
            WHERE recurring_id = $1 AND user_id = $2
            "#,
        )
        .bind(recurring_id)
        .bind(user_id)
        .bind(next_deadline)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(true)
    }

    /// Get summary for a user
    pub async fn get_summary(&self, user_id: Uuid) -> Result<RecurringSummary> {
        // Get aggregate stats
        let stats: Option<(i64, i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE is_active) as active,
                COALESCE(SUM(instances_created), 0) as total,
                COALESCE(SUM(instances_fulfilled), 0) as fulfilled,
                COALESCE(SUM(instances_failed), 0) as failed
            FROM recurring_commitments
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (total_active, total_instances, fulfilled_instances, failed_instances) =
            stats.unwrap_or((0, 0, 0, 0));

        let average_fulfillment_rate = if fulfilled_instances + failed_instances > 0 {
            Decimal::from(fulfilled_instances)
                / Decimal::from(fulfilled_instances + failed_instances)
        } else {
            dec!(1)
        };

        // Get upcoming deadlines
        let upcoming: Vec<(Uuid, String, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT recurring_id, title, next_deadline
            FROM recurring_commitments
            WHERE user_id = $1 AND is_active = true
            ORDER BY next_deadline ASC
            LIMIT 5
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let upcoming_deadlines = upcoming
            .into_iter()
            .map(|(recurring_id, title, deadline)| {
                let days_until = (deadline - now).num_days();
                UpcomingDeadline {
                    recurring_id,
                    title,
                    deadline,
                    days_until,
                }
            })
            .collect();

        Ok(RecurringSummary {
            total_active,
            total_instances,
            fulfilled_instances,
            failed_instances,
            average_fulfillment_rate,
            upcoming_deadlines,
        })
    }

    /// Delete a recurring commitment (only if no instances)
    pub async fn delete(&self, recurring_id: Uuid, user_id: Uuid) -> Result<bool> {
        // Check if there are any instances
        let instance_count: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM recurring_instances
            WHERE recurring_id = $1
            "#,
        )
        .bind(recurring_id)
        .fetch_optional(&self.pool)
        .await?;

        if instance_count.map(|(c,)| c).unwrap_or(0) > 0 {
            // Can't delete, just deactivate
            return self.pause(recurring_id, user_id).await;
        }

        let result = sqlx::query(
            r#"
            DELETE FROM recurring_commitments
            WHERE recurring_id = $1 AND user_id = $2
            "#,
        )
        .bind(recurring_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn fixed_dt(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 12, 0, 0)
            .single()
            .expect("valid fixed date")
    }

    // ── RecurrenceFrequency::to_db_string / from_db_string ────────────────────

    #[test]
    fn test_frequency_to_db_string_all_variants() {
        assert_eq!(RecurrenceFrequency::Weekly.to_db_string(), "weekly");
        assert_eq!(RecurrenceFrequency::Biweekly.to_db_string(), "biweekly");
        assert_eq!(RecurrenceFrequency::Monthly.to_db_string(), "monthly");
        assert_eq!(RecurrenceFrequency::Quarterly.to_db_string(), "quarterly");
    }

    #[test]
    fn test_frequency_from_db_string_roundtrip() {
        for freq in [
            RecurrenceFrequency::Weekly,
            RecurrenceFrequency::Biweekly,
            RecurrenceFrequency::Monthly,
            RecurrenceFrequency::Quarterly,
        ] {
            let s = freq.to_db_string();
            let parsed = RecurrenceFrequency::from_db_string(s)
                .unwrap_or_else(|| panic!("from_db_string failed for {s}"));
            assert_eq!(parsed, freq);
        }
    }

    #[test]
    fn test_frequency_from_db_string_unknown_returns_none() {
        assert!(RecurrenceFrequency::from_db_string("daily").is_none());
        assert!(RecurrenceFrequency::from_db_string("").is_none());
    }

    #[test]
    fn test_frequency_from_str_biweekly_aliases() {
        use std::str::FromStr;
        assert_eq!(
            RecurrenceFrequency::from_str("bi-weekly").expect("bi-weekly alias"),
            RecurrenceFrequency::Biweekly
        );
        assert_eq!(
            RecurrenceFrequency::from_str("bi_weekly").expect("bi_weekly alias"),
            RecurrenceFrequency::Biweekly
        );
    }

    #[test]
    fn test_frequency_from_str_invalid_returns_error() {
        use std::str::FromStr;
        assert!(RecurrenceFrequency::from_str("daily").is_err());
    }

    // ── RecurrenceFrequency::next_deadline ────────────────────────────────────

    #[test]
    fn test_next_deadline_weekly_adds_7_days() {
        let base = fixed_dt(2026, 1, 15);
        let next = RecurrenceFrequency::Weekly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 1, 22));
    }

    #[test]
    fn test_next_deadline_biweekly_adds_14_days() {
        let base = fixed_dt(2026, 1, 15);
        let next = RecurrenceFrequency::Biweekly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 1, 29));
    }

    #[test]
    fn test_next_deadline_monthly_normal_month() {
        let base = fixed_dt(2026, 3, 15);
        let next = RecurrenceFrequency::Monthly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 4, 15));
    }

    #[test]
    fn test_next_deadline_monthly_december_rolls_to_january() {
        let base = fixed_dt(2025, 12, 15);
        let next = RecurrenceFrequency::Monthly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 1, 15));
    }

    #[test]
    fn test_next_deadline_quarterly_normal() {
        let base = fixed_dt(2026, 1, 15);
        let next = RecurrenceFrequency::Quarterly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 4, 15));
    }

    #[test]
    fn test_next_deadline_quarterly_rollover_to_next_year() {
        // October + 3 months = January next year
        let base = fixed_dt(2025, 10, 15);
        let next = RecurrenceFrequency::Quarterly.next_deadline(base);
        assert_eq!(next, fixed_dt(2026, 1, 15));
    }

    #[test]
    fn test_frequency_description_strings() {
        assert_eq!(RecurrenceFrequency::Weekly.description(), "every week");
        assert_eq!(
            RecurrenceFrequency::Biweekly.description(),
            "every two weeks"
        );
        assert_eq!(RecurrenceFrequency::Monthly.description(), "every month");
        assert_eq!(
            RecurrenceFrequency::Quarterly.description(),
            "every quarter"
        );
    }

    // ── RecurringCommitment::fulfillment_rate ─────────────────────────────────

    fn make_recurring(fulfilled: i32, failed: i32) -> RecurringCommitment {
        RecurringCommitment {
            recurring_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            title: "Test Recurring".to_string(),
            description: None,
            frequency: "monthly".to_string(),
            is_active: true,
            instances_created: fulfilled + failed,
            instances_fulfilled: fulfilled,
            instances_failed: failed,
            last_generated_at: None,
            next_deadline: Utc::now() + Duration::days(30),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_fulfillment_rate_no_instances_returns_one() {
        let r = make_recurring(0, 0);
        assert_eq!(r.fulfillment_rate(), dec!(1));
    }

    #[test]
    fn test_fulfillment_rate_all_fulfilled() {
        let r = make_recurring(5, 0);
        assert_eq!(r.fulfillment_rate(), dec!(1));
    }

    #[test]
    fn test_fulfillment_rate_half_fulfilled() {
        let r = make_recurring(5, 5);
        assert_eq!(r.fulfillment_rate(), dec!(0.5));
    }

    #[test]
    fn test_fulfillment_rate_none_fulfilled() {
        let r = make_recurring(0, 4);
        assert_eq!(r.fulfillment_rate(), dec!(0));
    }

    #[test]
    fn test_fulfillment_rate_partial() {
        let r = make_recurring(3, 1);
        // 3/4 = 0.75
        assert_eq!(r.fulfillment_rate(), dec!(0.75));
    }

    // ── CreateRecurringRequest::validate ──────────────────────────────────────

    fn valid_recurring_request() -> CreateRecurringRequest {
        CreateRecurringRequest {
            token_id: Uuid::new_v4(),
            title: "Weekly report".to_string(),
            description: None,
            frequency: RecurrenceFrequency::Weekly,
            start_date: None,
        }
    }

    #[test]
    fn test_create_recurring_validate_valid() {
        valid_recurring_request()
            .validate()
            .expect("valid request should pass");
    }

    #[test]
    fn test_create_recurring_validate_empty_title_rejected() {
        let mut req = valid_recurring_request();
        req.title = String::new();
        assert!(req.validate().is_err(), "empty title should fail");
    }

    #[test]
    fn test_create_recurring_validate_title_too_long_rejected() {
        let mut req = valid_recurring_request();
        req.title = "t".repeat(201);
        assert!(req.validate().is_err(), "title >200 chars should fail");
    }

    #[test]
    fn test_create_recurring_validate_title_exactly_200_accepted() {
        let mut req = valid_recurring_request();
        req.title = "t".repeat(200);
        req.validate()
            .expect("title of exactly 200 chars should pass");
    }

    #[test]
    fn test_create_recurring_validate_description_too_long_rejected() {
        let mut req = valid_recurring_request();
        req.description = Some("d".repeat(2001));
        assert!(
            req.validate().is_err(),
            "description >2000 chars should fail"
        );
    }

    #[test]
    fn test_create_recurring_validate_description_exactly_2000_accepted() {
        let mut req = valid_recurring_request();
        req.description = Some("d".repeat(2000));
        req.validate()
            .expect("description of exactly 2000 chars should pass");
    }

    // ── RecurringSummary default ───────────────────────────────────────────────

    #[test]
    fn test_recurring_summary_default_zeroed() {
        let s = RecurringSummary::default();
        assert_eq!(s.total_active, 0);
        assert_eq!(s.total_instances, 0);
        assert_eq!(s.fulfilled_instances, 0);
        assert_eq!(s.failed_instances, 0);
        assert!(s.upcoming_deadlines.is_empty());
    }

    // ── parsed_frequency ──────────────────────────────────────────────────────

    #[test]
    fn test_parsed_frequency_valid() {
        let r = make_recurring(0, 0);
        // make_recurring sets frequency = "monthly"
        assert_eq!(r.parsed_frequency(), Some(RecurrenceFrequency::Monthly));
    }

    #[test]
    fn test_parsed_frequency_invalid_returns_none() {
        let mut r = make_recurring(0, 0);
        r.frequency = "daily".to_string();
        assert!(r.parsed_frequency().is_none());
    }
}
