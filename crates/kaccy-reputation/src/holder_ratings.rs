//! Holder ratings system
//!
//! Allows token holders to rate issuers, contributing to the
//! community_trust component of reputation scores.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::Result;

/// Minimum number of ratings for high confidence community trust score
/// Below this threshold, scores are blended with default neutral score (50)
pub const MIN_RATINGS_FOR_CONFIDENCE: i64 = 10;

/// Minimum rating value (1 star)
pub const MIN_RATING: u8 = 1;

/// Maximum rating value (5 stars)
pub const MAX_RATING: u8 = 5;

/// Default neutral community trust score when no ratings exist
pub const DEFAULT_TRUST_SCORE: Decimal = dec!(50);

/// A rating from a token holder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderRating {
    pub rating_id: Uuid,
    pub issuer_user_id: Uuid,
    pub holder_user_id: Uuid,
    pub token_id: Uuid,
    /// Rating from 1-5 stars
    pub rating: u8,
    /// Optional text feedback
    pub feedback: Option<String>,
    /// Holder's token balance at time of rating (for weighting)
    pub holder_balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Aggregated rating statistics for an issuer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingStats {
    pub issuer_user_id: Uuid,
    pub total_ratings: i64,
    pub average_rating: f64,
    pub weighted_average: f64,
    pub rating_distribution: RatingDistribution,
    pub total_feedback_count: i64,
    pub unique_raters: i64,
}

/// Distribution of ratings (1-5 stars)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RatingDistribution {
    pub one_star: i64,
    pub two_star: i64,
    pub three_star: i64,
    pub four_star: i64,
    pub five_star: i64,
}

impl RatingDistribution {
    pub fn total(&self) -> i64 {
        self.one_star + self.two_star + self.three_star + self.four_star + self.five_star
    }

    pub fn percentage(&self, stars: u8) -> f64 {
        let total = self.total() as f64;
        if total == 0.0 {
            return 0.0;
        }
        let count = match stars {
            1 => self.one_star,
            2 => self.two_star,
            3 => self.three_star,
            4 => self.four_star,
            5 => self.five_star,
            _ => 0,
        } as f64;
        (count / total) * 100.0
    }
}

/// Request to create or update a rating
#[derive(Debug, Deserialize)]
pub struct RatingRequest {
    pub token_id: Uuid,
    pub rating: u8,
    pub feedback: Option<String>,
}

impl RatingRequest {
    /// Validate rating request
    ///
    /// # Validation Rules
    /// - Rating must be between MIN_RATING (1) and MAX_RATING (5)
    /// - Feedback must be at most 2000 characters if provided
    ///
    /// # Errors
    /// Returns `ReputationError::Validation` if validation fails
    pub fn validate(&self) -> Result<()> {
        if self.rating < MIN_RATING || self.rating > MAX_RATING {
            return Err(crate::error::ReputationError::Validation(format!(
                "Rating must be between {} and {}",
                MIN_RATING, MAX_RATING
            )));
        }
        if let Some(ref feedback) = self.feedback {
            if feedback.len() > 2000 {
                return Err(crate::error::ReputationError::Validation(
                    "Feedback must be at most 2000 characters".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Service for managing holder ratings
pub struct HolderRatingService {
    pool: PgPool,
    /// Minimum token balance required to rate
    min_balance_to_rate: Decimal,
    /// Minimum holding duration in hours before rating allowed
    min_holding_hours: i64,
}

impl HolderRatingService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            min_balance_to_rate: dec!(1),
            min_holding_hours: 24,
        }
    }

    pub fn with_requirements(pool: PgPool, min_balance: Decimal, min_holding_hours: i64) -> Self {
        Self {
            pool,
            min_balance_to_rate: min_balance,
            min_holding_hours,
        }
    }

    /// Submit or update a rating
    pub async fn submit_rating(
        &self,
        holder_user_id: Uuid,
        request: &RatingRequest,
    ) -> Result<HolderRating> {
        request.validate()?;

        // Get the token's issuer
        let issuer_row = sqlx::query(r#"SELECT issuer_user_id FROM tokens WHERE token_id = $1"#)
            .bind(request.token_id)
            .fetch_optional(&self.pool)
            .await?;

        let issuer_user_id: Uuid = issuer_row
            .ok_or_else(|| {
                crate::error::ReputationError::Validation("Token not found".to_string())
            })?
            .get("issuer_user_id");

        // Can't rate yourself
        if issuer_user_id == holder_user_id {
            return Err(crate::error::ReputationError::Validation(
                "Cannot rate your own token".to_string(),
            ));
        }

        // Check holder's balance
        let balance_row = sqlx::query(
            r#"
            SELECT amount, updated_at
            FROM balances
            WHERE user_id = $1 AND token_id = $2
            "#,
        )
        .bind(holder_user_id)
        .bind(request.token_id)
        .fetch_optional(&self.pool)
        .await?;

        let (balance, holding_since): (Decimal, DateTime<Utc>) = match balance_row {
            Some(row) => (row.get("amount"), row.get("updated_at")),
            None => {
                return Err(crate::error::ReputationError::Validation(
                    "You must hold tokens to rate".to_string(),
                ));
            }
        };

        if balance < self.min_balance_to_rate {
            return Err(crate::error::ReputationError::Validation(format!(
                "Minimum balance of {} required to rate",
                self.min_balance_to_rate
            )));
        }

        let holding_hours = (Utc::now() - holding_since).num_hours();
        if holding_hours < self.min_holding_hours {
            return Err(crate::error::ReputationError::Validation(format!(
                "Must hold tokens for at least {} hours before rating",
                self.min_holding_hours
            )));
        }

        let rating_id = Uuid::new_v4();
        let now = Utc::now();

        // Insert or update rating
        sqlx::query(
            r#"
            INSERT INTO holder_ratings (rating_id, issuer_user_id, holder_user_id, token_id, rating, feedback, holder_balance, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            ON CONFLICT (holder_user_id, token_id)
            DO UPDATE SET rating = $5, feedback = $6, holder_balance = $7, updated_at = $8
            "#,
        )
        .bind(rating_id)
        .bind(issuer_user_id)
        .bind(holder_user_id)
        .bind(request.token_id)
        .bind(request.rating as i16)
        .bind(&request.feedback)
        .bind(balance)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(HolderRating {
            rating_id,
            issuer_user_id,
            holder_user_id,
            token_id: request.token_id,
            rating: request.rating,
            feedback: request.feedback.clone(),
            holder_balance: balance,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get rating stats for an issuer
    pub async fn get_issuer_stats(&self, issuer_user_id: Uuid) -> Result<RatingStats> {
        let stats_row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(DISTINCT holder_user_id) as unique_raters,
                COUNT(feedback) as feedback_count,
                COALESCE(AVG(rating), 0) as avg_rating,
                COALESCE(
                    SUM(rating * holder_balance) / NULLIF(SUM(holder_balance), 0),
                    0
                ) as weighted_avg,
                COUNT(*) FILTER (WHERE rating = 1) as one_star,
                COUNT(*) FILTER (WHERE rating = 2) as two_star,
                COUNT(*) FILTER (WHERE rating = 3) as three_star,
                COUNT(*) FILTER (WHERE rating = 4) as four_star,
                COUNT(*) FILTER (WHERE rating = 5) as five_star
            FROM holder_ratings
            WHERE issuer_user_id = $1
            "#,
        )
        .bind(issuer_user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(RatingStats {
            issuer_user_id,
            total_ratings: stats_row.get("total"),
            average_rating: stats_row.get::<f64, _>("avg_rating"),
            weighted_average: stats_row.get::<f64, _>("weighted_avg"),
            rating_distribution: RatingDistribution {
                one_star: stats_row.get("one_star"),
                two_star: stats_row.get("two_star"),
                three_star: stats_row.get("three_star"),
                four_star: stats_row.get("four_star"),
                five_star: stats_row.get("five_star"),
            },
            total_feedback_count: stats_row.get("feedback_count"),
            unique_raters: stats_row.get("unique_raters"),
        })
    }

    /// Get rating stats for a specific token
    pub async fn get_token_stats(&self, token_id: Uuid) -> Result<RatingStats> {
        // Get issuer first
        let issuer_row = sqlx::query(r#"SELECT issuer_user_id FROM tokens WHERE token_id = $1"#)
            .bind(token_id)
            .fetch_optional(&self.pool)
            .await?;

        let issuer_user_id: Uuid = issuer_row
            .ok_or_else(|| {
                crate::error::ReputationError::Validation("Token not found".to_string())
            })?
            .get("issuer_user_id");

        let stats_row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(DISTINCT holder_user_id) as unique_raters,
                COUNT(feedback) as feedback_count,
                COALESCE(AVG(rating), 0) as avg_rating,
                COALESCE(
                    SUM(rating * holder_balance) / NULLIF(SUM(holder_balance), 0),
                    0
                ) as weighted_avg,
                COUNT(*) FILTER (WHERE rating = 1) as one_star,
                COUNT(*) FILTER (WHERE rating = 2) as two_star,
                COUNT(*) FILTER (WHERE rating = 3) as three_star,
                COUNT(*) FILTER (WHERE rating = 4) as four_star,
                COUNT(*) FILTER (WHERE rating = 5) as five_star
            FROM holder_ratings
            WHERE token_id = $1
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(RatingStats {
            issuer_user_id,
            total_ratings: stats_row.get("total"),
            average_rating: stats_row.get::<f64, _>("avg_rating"),
            weighted_average: stats_row.get::<f64, _>("weighted_avg"),
            rating_distribution: RatingDistribution {
                one_star: stats_row.get("one_star"),
                two_star: stats_row.get("two_star"),
                three_star: stats_row.get("three_star"),
                four_star: stats_row.get("four_star"),
                five_star: stats_row.get("five_star"),
            },
            total_feedback_count: stats_row.get("feedback_count"),
            unique_raters: stats_row.get("unique_raters"),
        })
    }

    /// Get all ratings for an issuer with pagination
    pub async fn get_issuer_ratings(
        &self,
        issuer_user_id: Uuid,
        page: u32,
        limit: u32,
    ) -> Result<Vec<HolderRating>> {
        let offset = (page.saturating_sub(1)) * limit;

        let rows = sqlx::query(
            r#"
            SELECT rating_id, issuer_user_id, holder_user_id, token_id, rating, feedback, holder_balance, created_at, updated_at
            FROM holder_ratings
            WHERE issuer_user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(issuer_user_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| HolderRating {
                rating_id: row.get("rating_id"),
                issuer_user_id: row.get("issuer_user_id"),
                holder_user_id: row.get("holder_user_id"),
                token_id: row.get("token_id"),
                rating: row.get::<i16, _>("rating") as u8,
                feedback: row.get("feedback"),
                holder_balance: row.get("holder_balance"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    /// Get a user's rating for a specific token
    pub async fn get_user_rating(
        &self,
        holder_user_id: Uuid,
        token_id: Uuid,
    ) -> Result<Option<HolderRating>> {
        let row = sqlx::query(
            r#"
            SELECT rating_id, issuer_user_id, holder_user_id, token_id, rating, feedback, holder_balance, created_at, updated_at
            FROM holder_ratings
            WHERE holder_user_id = $1 AND token_id = $2
            "#,
        )
        .bind(holder_user_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| HolderRating {
            rating_id: r.get("rating_id"),
            issuer_user_id: r.get("issuer_user_id"),
            holder_user_id: r.get("holder_user_id"),
            token_id: r.get("token_id"),
            rating: r.get::<i16, _>("rating") as u8,
            feedback: r.get("feedback"),
            holder_balance: r.get("holder_balance"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// Delete a rating
    pub async fn delete_rating(&self, holder_user_id: Uuid, token_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"DELETE FROM holder_ratings WHERE holder_user_id = $1 AND token_id = $2"#,
        )
        .bind(holder_user_id)
        .bind(token_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Calculate community trust score (0-100) from ratings
    ///
    /// # Algorithm
    /// - With 0 ratings: Returns default neutral score (50)
    /// - With < MIN_RATINGS_FOR_CONFIDENCE: Blends weighted average with default based on confidence
    /// - With >= MIN_RATINGS_FOR_CONFIDENCE: Uses weighted average with high confidence
    ///
    /// # Confidence Calculation
    /// Confidence increases with number of ratings, approaching 1.0 asymptotically.
    /// Formula: `1.0 - (1.0 / (1.0 + ratings / MIN_RATINGS_FOR_CONFIDENCE))`
    ///
    /// # Examples
    /// - 0 ratings → 50 (default)
    /// - 1 rating (5 stars) → ~59 (low confidence, blended with default)
    /// - 10 ratings (5 stars avg) → ~87 (medium confidence)
    /// - 50+ ratings (5 stars avg) → ~100 (high confidence)
    pub async fn calculate_community_trust(&self, issuer_user_id: Uuid) -> Result<Decimal> {
        let stats = self.get_issuer_stats(issuer_user_id).await?;

        if stats.total_ratings == 0 {
            return Ok(DEFAULT_TRUST_SCORE);
        }

        // Weighted average (1-5) converted to 0-100 scale
        // Also factor in number of ratings (more ratings = more confidence)
        let base_score = (stats.weighted_average - 1.0) / 4.0 * 100.0;

        // Confidence factor: approaches 1.0 as ratings increase
        // Uses MIN_RATINGS_FOR_CONFIDENCE as the scaling factor
        let confidence =
            1.0 - (1.0 / (1.0 + stats.total_ratings as f64 / MIN_RATINGS_FOR_CONFIDENCE as f64));

        // Blend base score with default based on confidence
        // Low ratings → more weight on default (50), high ratings → more weight on actual score
        let final_score = (base_score * confidence) + (50.0 * (1.0 - confidence));

        Ok(Decimal::from_f64_retain(final_score).unwrap_or(DEFAULT_TRUST_SCORE))
    }
}
