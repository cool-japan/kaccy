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
    pub recurring_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub frequency: String,
    pub is_active: bool,
    pub instances_created: i32,
    pub instances_fulfilled: i32,
    pub instances_failed: i32,
    pub last_generated_at: Option<DateTime<Utc>>,
    pub next_deadline: DateTime<Utc>,
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
    pub commitment_id: Uuid,
    pub recurring_id: Uuid,
    pub instance_number: i32,
    pub deadline: DateTime<Utc>,
}

/// Request to create a recurring commitment
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRecurringRequest {
    pub token_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub frequency: RecurrenceFrequency,
    /// When to start the first commitment
    pub start_date: Option<DateTime<Utc>>,
}

impl CreateRecurringRequest {
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
    pub total_active: i64,
    pub total_instances: i64,
    pub fulfilled_instances: i64,
    pub failed_instances: i64,
    pub average_fulfillment_rate: Decimal,
    pub upcoming_deadlines: Vec<UpcomingDeadline>,
}

/// Upcoming deadline info
#[derive(Debug, Clone, Serialize)]
pub struct UpcomingDeadline {
    pub recurring_id: Uuid,
    pub title: String,
    pub deadline: DateTime<Utc>,
    pub days_until: i64,
}

/// Service for managing recurring commitments
pub struct RecurringCommitmentService {
    pool: PgPool,
}

impl RecurringCommitmentService {
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
