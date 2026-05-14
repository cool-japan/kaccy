//! Reputation score calculation

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

use crate::error::{ReputationError, Result};
use crate::tier::ReputationTier;

/// Maximum allowed score delta per update (prevents gaming)
pub const MAX_POSITIVE_DELTA: Decimal = dec!(50);
/// Maximum negative delta per update
pub const MAX_NEGATIVE_DELTA: Decimal = dec!(-100);
/// Minimum score
pub const MIN_SCORE: Decimal = dec!(0);
/// Maximum score
pub const MAX_SCORE: Decimal = dec!(1000);

/// A user's complete reputation score including tier and component breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationScore {
    /// User this score belongs to.
    pub user_id: Uuid,
    /// Weighted overall score in the range 0–1000.
    pub overall_score: Decimal,
    /// Reputation tier derived from `overall_score`.
    pub tier: ReputationTier,
    /// Individual component scores used to compute the overall score.
    pub components: ScoreComponents,
}

/// Result of comparing two reputation scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreComparison {
    /// Current score value
    pub current_score: Decimal,
    /// Previous score value
    pub previous_score: Decimal,
    /// Absolute difference (current - previous)
    pub difference: Decimal,
    /// Percentage change
    pub percentage_change: Decimal,
    /// Whether the score improved
    pub is_improvement: bool,
    /// Whether tier changed
    pub tier_changed: bool,
    /// Whether tier improved (went up)
    pub tier_improved: bool,
    /// Current tier
    pub current_tier: ReputationTier,
    /// Previous tier
    pub previous_tier: ReputationTier,
}

impl ScoreComparison {
    /// Get a human-readable summary of the comparison
    pub fn summary(&self) -> String {
        let direction = if self.is_improvement {
            "increased"
        } else if self.difference < dec!(0) {
            "decreased"
        } else {
            "remained unchanged"
        };

        if self.tier_changed {
            if self.tier_improved {
                format!(
                    "Score {} by {} points ({:.1}%), advancing from {:?} to {:?} tier",
                    direction,
                    self.difference.abs(),
                    self.percentage_change.abs(),
                    self.previous_tier,
                    self.current_tier
                )
            } else {
                format!(
                    "Score {} by {} points ({:.1}%), falling from {:?} to {:?} tier",
                    direction,
                    self.difference.abs(),
                    self.percentage_change.abs(),
                    self.previous_tier,
                    self.current_tier
                )
            }
        } else {
            format!(
                "Score {} by {} points ({:.1}%), remaining in {:?} tier",
                direction,
                self.difference.abs(),
                self.percentage_change.abs(),
                self.current_tier
            )
        }
    }
}

impl ReputationScore {
    /// Check if score is critical (below Bronze tier threshold)
    pub fn is_critical(&self) -> bool {
        self.overall_score < dec!(200)
    }

    /// Check if score is excellent (Diamond tier)
    pub fn is_excellent(&self) -> bool {
        self.overall_score >= dec!(900)
    }

    /// Check if score is good (Gold tier or above)
    pub fn is_good(&self) -> bool {
        self.overall_score >= dec!(600)
    }

    /// Check if score needs attention (below Silver tier)
    pub fn needs_attention(&self) -> bool {
        self.overall_score < dec!(400)
    }

    /// Check if user can issue tokens (Bronze tier or above)
    pub fn can_issue_tokens(&self) -> bool {
        self.tier.is_verified()
    }

    /// Get distance to next tier threshold
    pub fn distance_to_next_tier(&self) -> Option<Decimal> {
        self.tier.next_tier().map(|next_tier| {
            let next_threshold = next_tier.min_score();
            if self.overall_score < next_threshold {
                next_threshold - self.overall_score
            } else {
                dec!(0)
            }
        })
    }

    /// Get percentage progress within current tier
    pub fn tier_progress_percentage(&self) -> Decimal {
        let (min, max) = self.tier.score_range();
        if max == min {
            return dec!(100);
        }
        let progress = (self.overall_score - min) / (max - min) * dec!(100);
        progress.max(dec!(0)).min(dec!(100))
    }

    /// Compare this score with another score
    ///
    /// Returns a comparison with score difference, percentage change, and direction.
    ///
    /// # Example
    ///
    /// ```
    /// use kaccy_reputation::*;
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let current = ReputationScore {
    ///     user_id: Uuid::new_v4(),
    ///     overall_score: dec!(500),
    ///     tier: ReputationTier::Silver,
    ///     components: Default::default(),
    /// };
    ///
    /// let previous = ReputationScore {
    ///     user_id: Uuid::new_v4(),
    ///     overall_score: dec!(400),
    ///     tier: ReputationTier::Bronze,
    ///     components: Default::default(),
    /// };
    ///
    /// let comparison = current.compare_with(&previous);
    /// assert_eq!(comparison.difference, dec!(100));
    /// assert!(comparison.is_improvement);
    /// ```
    pub fn compare_with(&self, other: &ReputationScore) -> ScoreComparison {
        let difference = self.overall_score - other.overall_score;
        let percentage_change = if other.overall_score != dec!(0) {
            (difference / other.overall_score) * dec!(100)
        } else {
            dec!(0)
        };

        let tier_changed = self.tier != other.tier;
        let tier_improved = tier_changed && self.tier.min_score() > other.tier.min_score();

        ScoreComparison {
            current_score: self.overall_score,
            previous_score: other.overall_score,
            difference,
            percentage_change,
            is_improvement: difference > dec!(0),
            tier_changed,
            tier_improved,
            current_tier: self.tier,
            previous_tier: other.tier,
        }
    }

    /// Get a user-friendly status label for UI display
    ///
    /// Returns a string like "Excellent", "Good", "Needs Attention", or "Critical".
    pub fn status_label(&self) -> &'static str {
        if self.is_excellent() {
            "Excellent"
        } else if self.is_good() {
            "Good"
        } else if self.is_critical() {
            "Critical"
        } else if self.needs_attention() {
            "Needs Attention"
        } else {
            "Fair"
        }
    }

    /// Get a status emoji for quick visual indication
    pub fn status_emoji(&self) -> &'static str {
        if self.is_excellent() {
            "💎"
        } else if self.is_good() {
            "⭐"
        } else if self.is_critical() {
            "🚨"
        } else if self.needs_attention() {
            "⚠️"
        } else {
            "✓"
        }
    }

    /// Check if user is eligible for a specific operation based on tier requirements
    pub fn is_eligible_for_operation(&self, required_tier: ReputationTier) -> bool {
        self.tier.min_score() >= required_tier.min_score()
    }
}

/// Breakdown of the individual components that make up a reputation score.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreComponents {
    /// Commitment fulfillment sub-score (35% weight).
    pub commitment_fulfillment: Decimal, // 35% weight
    /// Response-time sub-score (15% weight).
    pub response_time: Decimal, // 15% weight
    /// Quality-rating sub-score (30% weight).
    pub quality_rating: Decimal, // 30% weight
    /// Community-trust sub-score (15% weight).
    pub community_trust: Decimal, // 15% weight
    /// Longevity sub-score (5% weight).
    pub longevity: Decimal, // 5% weight
}

impl std::fmt::Display for ScoreComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "commitment: {:.1}, response: {:.1}, quality: {:.1}, trust: {:.1}, longevity: {:.1} (overall: {:.1})",
            self.commitment_fulfillment,
            self.response_time,
            self.quality_rating,
            self.community_trust,
            self.longevity,
            self.calculate_overall()
        )
    }
}

/// Weights for score components (must sum to 1.0)
pub const COMPONENT_WEIGHTS: ComponentWeights = ComponentWeights {
    commitment_fulfillment: dec!(0.35),
    response_time: dec!(0.15),
    quality_rating: dec!(0.30),
    community_trust: dec!(0.15),
    longevity: dec!(0.05),
};

/// Configurable weights for each score component (must sum to 1.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentWeights {
    /// Weight for the commitment-fulfillment component.
    pub commitment_fulfillment: Decimal,
    /// Weight for the response-time component.
    pub response_time: Decimal,
    /// Weight for the quality-rating component.
    pub quality_rating: Decimal,
    /// Weight for the community-trust component.
    pub community_trust: Decimal,
    /// Weight for the longevity component.
    pub longevity: Decimal,
}

impl Default for ComponentWeights {
    fn default() -> Self {
        COMPONENT_WEIGHTS
    }
}

impl ComponentWeights {
    /// Validate that all weights sum to 1.0 (within tolerance)
    pub fn validate(&self) -> Result<()> {
        let sum = self.commitment_fulfillment
            + self.response_time
            + self.quality_rating
            + self.community_trust
            + self.longevity;

        let tolerance = dec!(0.001); // Allow small floating point errors
        if (sum - dec!(1.0)).abs() > tolerance {
            return Err(ReputationError::Validation(format!(
                "Component weights must sum to 1.0, got {}",
                sum
            )));
        }
        Ok(())
    }

    /// Get the sum of all weights
    pub fn sum(&self) -> Decimal {
        self.commitment_fulfillment
            + self.response_time
            + self.quality_rating
            + self.community_trust
            + self.longevity
    }
}

impl ScoreComponents {
    /// Calculate the weighted overall score from all components.
    pub fn calculate_overall(&self) -> Decimal {
        self.commitment_fulfillment * COMPONENT_WEIGHTS.commitment_fulfillment
            + self.response_time * COMPONENT_WEIGHTS.response_time
            + self.quality_rating * COMPONENT_WEIGHTS.quality_rating
            + self.community_trust * COMPONENT_WEIGHTS.community_trust
            + self.longevity * COMPONENT_WEIGHTS.longevity
    }

    /// Scale components (assumed 0-100) to 0-1000 overall score
    pub fn calculate_scaled_overall(&self) -> Decimal {
        self.calculate_overall() * dec!(10)
    }

    /// Check if all components are above a threshold
    pub fn is_all_above_threshold(&self, threshold: Decimal) -> bool {
        self.commitment_fulfillment >= threshold
            && self.response_time >= threshold
            && self.quality_rating >= threshold
            && self.community_trust >= threshold
            && self.longevity >= threshold
    }

    /// Calculate the average score across all components
    pub fn average_component_score(&self) -> Decimal {
        (self.commitment_fulfillment
            + self.response_time
            + self.quality_rating
            + self.community_trust
            + self.longevity)
            / dec!(5)
    }

    /// Get the minimum component score
    pub fn min_component(&self) -> Decimal {
        [
            self.commitment_fulfillment,
            self.response_time,
            self.quality_rating,
            self.community_trust,
            self.longevity,
        ]
        .into_iter()
        .min()
        .unwrap_or(dec!(0))
    }

    /// Get the maximum component score
    pub fn max_component(&self) -> Decimal {
        [
            self.commitment_fulfillment,
            self.response_time,
            self.quality_rating,
            self.community_trust,
            self.longevity,
        ]
        .into_iter()
        .max()
        .unwrap_or(dec!(0))
    }

    /// Get the component with the lowest score (returns tuple of component name and value)
    pub fn weakest_component(&self) -> (&'static str, Decimal) {
        let components = [
            ("commitment_fulfillment", self.commitment_fulfillment),
            ("response_time", self.response_time),
            ("quality_rating", self.quality_rating),
            ("community_trust", self.community_trust),
            ("longevity", self.longevity),
        ];
        components
            .into_iter()
            .min_by(|a, b| a.1.cmp(&b.1))
            .unwrap_or(("unknown", dec!(0)))
    }

    /// Get the component with the highest score (returns tuple of component name and value)
    pub fn strongest_component(&self) -> (&'static str, Decimal) {
        let components = [
            ("commitment_fulfillment", self.commitment_fulfillment),
            ("response_time", self.response_time),
            ("quality_rating", self.quality_rating),
            ("community_trust", self.community_trust),
            ("longevity", self.longevity),
        ];
        components
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1))
            .unwrap_or(("unknown", dec!(0)))
    }

    /// Calculate component balance score (0-100)
    ///
    /// Measures how evenly distributed the component scores are.
    /// Higher values indicate better balance. Returns 100 for perfect balance,
    /// lower values indicate some components are lagging behind.
    pub fn component_balance_score(&self) -> Decimal {
        let avg = self.average_component_score();
        if avg == dec!(0) {
            return dec!(100);
        }

        let variance = [
            self.commitment_fulfillment,
            self.response_time,
            self.quality_rating,
            self.community_trust,
            self.longevity,
        ]
        .into_iter()
        .map(|component| {
            let diff = component - avg;
            diff * diff
        })
        .sum::<Decimal>()
            / dec!(5);

        // Convert variance to balance score (lower variance = higher balance)
        // Use coefficient of variation approach
        if avg == dec!(0) {
            dec!(100)
        } else {
            let std_dev = crate::statistics::Statistics::sqrt(variance);
            let cv = (std_dev / avg) * dec!(100);
            // Invert so higher is better, cap at 100
            (dec!(100) - cv).max(dec!(0))
        }
    }

    /// Get improvement priority list (sorted by potential impact)
    ///
    /// Returns components ordered by how much improving them would impact overall score.
    /// Components with higher weights and lower current values are prioritized.
    pub fn improvement_priorities(&self) -> Vec<(&'static str, Decimal, Decimal)> {
        let mut priorities = vec![
            (
                "commitment_fulfillment",
                self.commitment_fulfillment,
                COMPONENT_WEIGHTS.commitment_fulfillment,
            ),
            (
                "response_time",
                self.response_time,
                COMPONENT_WEIGHTS.response_time,
            ),
            (
                "quality_rating",
                self.quality_rating,
                COMPONENT_WEIGHTS.quality_rating,
            ),
            (
                "community_trust",
                self.community_trust,
                COMPONENT_WEIGHTS.community_trust,
            ),
            ("longevity", self.longevity, COMPONENT_WEIGHTS.longevity),
        ];

        // Sort by potential impact: (weight * (100 - current_value))
        // Higher values = more room for improvement with significant weight
        priorities.sort_by(|a, b| {
            let impact_a = a.2 * (dec!(100) - a.1);
            let impact_b = b.2 * (dec!(100) - b.1);
            impact_b.cmp(&impact_a) // Descending order
        });

        priorities
    }

    /// Get the bottleneck component (lowest performance relative to weight)
    ///
    /// Returns the component that's underperforming most relative to its importance.
    pub fn bottleneck_component(&self) -> (&'static str, Decimal, Decimal) {
        let components = vec![
            (
                "commitment_fulfillment",
                self.commitment_fulfillment,
                COMPONENT_WEIGHTS.commitment_fulfillment,
            ),
            (
                "response_time",
                self.response_time,
                COMPONENT_WEIGHTS.response_time,
            ),
            (
                "quality_rating",
                self.quality_rating,
                COMPONENT_WEIGHTS.quality_rating,
            ),
            (
                "community_trust",
                self.community_trust,
                COMPONENT_WEIGHTS.community_trust,
            ),
            ("longevity", self.longevity, COMPONENT_WEIGHTS.longevity),
        ];

        components
            .into_iter()
            .min_by(|a, b| {
                // Compare weighted performance
                let performance_a = a.1 / a.2;
                let performance_b = b.1 / b.2;
                performance_a.partial_cmp(&performance_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(("unknown", dec!(0), dec!(0)))
    }

    /// Calculate component as percentage of its weighted contribution
    ///
    /// Shows how well each component performs relative to its potential impact.
    /// Returns a value between 0-100 for the specified component.
    pub fn component_as_percentile(&self, component_name: &str) -> Option<Decimal> {
        match component_name {
            "commitment_fulfillment" => Some(self.commitment_fulfillment),
            "response_time" => Some(self.response_time),
            "quality_rating" => Some(self.quality_rating),
            "community_trust" => Some(self.community_trust),
            "longevity" => Some(self.longevity),
            _ => None,
        }
    }
}

/// Reputation event types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum ReputationEventType {
    /// A commitment was successfully fulfilled.
    CommitmentFulfilled,
    /// A commitment was not fulfilled and is marked as failed.
    CommitmentFailed,
    /// A commitment passed its deadline without being fulfilled.
    CommitmentExpired,
    /// A quality score was received for a delivered commitment.
    QualityScore,
    /// A token holder submitted a rating for the issuer.
    HolderRating,
    /// An administrator manually adjusted the reputation score.
    AdminAdjustment,
    /// The user's account was created for the first time.
    AccountCreated,
    /// The user issued their first token on the platform.
    FirstTokenIssued,
}

impl std::fmt::Display for ReputationEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommitmentFulfilled => write!(f, "commitment_fulfilled"),
            Self::CommitmentFailed => write!(f, "commitment_failed"),
            Self::CommitmentExpired => write!(f, "commitment_expired"),
            Self::QualityScore => write!(f, "quality_score"),
            Self::HolderRating => write!(f, "holder_rating"),
            Self::AdminAdjustment => write!(f, "admin_adjustment"),
            Self::AccountCreated => write!(f, "account_created"),
            Self::FirstTokenIssued => write!(f, "first_token_issued"),
        }
    }
}

impl std::str::FromStr for ReputationEventType {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().replace([' ', '-'], "_").as_str() {
            "commitment_fulfilled" | "commitmentfulfilled" => {
                Ok(ReputationEventType::CommitmentFulfilled)
            }
            "commitment_failed" | "commitmentfailed" => Ok(ReputationEventType::CommitmentFailed),
            "commitment_expired" | "commitmentexpired" => {
                Ok(ReputationEventType::CommitmentExpired)
            }
            "quality_score" | "qualityscore" => Ok(ReputationEventType::QualityScore),
            "holder_rating" | "holderrating" => Ok(ReputationEventType::HolderRating),
            "admin_adjustment" | "adminadjustment" => Ok(ReputationEventType::AdminAdjustment),
            "account_created" | "accountcreated" => Ok(ReputationEventType::AccountCreated),
            "first_token_issued" | "firsttokenissued" => Ok(ReputationEventType::FirstTokenIssued),
            _ => Err(ReputationError::Validation(format!(
                "Invalid reputation event type: {}. Valid values: commitment_fulfilled, commitment_failed, commitment_expired, quality_score, holder_rating, admin_adjustment, account_created, first_token_issued",
                s
            ))),
        }
    }
}

/// Row from reputation_events table (for future direct event queries)
#[allow(dead_code)]
#[derive(Debug, FromRow)]
struct ReputationEventRow {
    event_id: Uuid,
    user_id: Uuid,
    event_type: String,
    delta: Decimal,
    reason: Option<String>,
    created_at: DateTime<Utc>,
}

/// Aggregated event stats for component calculation
#[derive(Debug, Default)]
struct EventStats {
    total_commitments: i64,
    fulfilled_commitments: i64,
    #[allow(dead_code)]
    failed_commitments: i64, // Reserved for failed commitment tracking
    total_quality_score: Decimal,
    quality_score_count: i64,
    account_age_days: i64,
}

/// Engine for computing and updating reputation scores.
pub struct ReputationEngine {
    pool: PgPool,
}

impl ReputationEngine {
    /// Create a new [`ReputationEngine`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reputation score with components calculated from events
    pub async fn get_score(&self, user_id: Uuid) -> Result<ReputationScore> {
        let (overall_score,): (Decimal,) = sqlx::query_as(
            r#"
            SELECT reputation_score FROM users WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ReputationError::UserNotFound(user_id.to_string()))?;

        let tier = ReputationTier::from_score(overall_score);
        let components = self.calculate_components(user_id).await?;

        Ok(ReputationScore {
            user_id,
            overall_score,
            tier,
            components,
        })
    }

    /// Calculate score components from reputation events
    pub async fn calculate_components(&self, user_id: Uuid) -> Result<ScoreComponents> {
        let stats = self.fetch_event_stats(user_id).await?;

        // Commitment fulfillment: % of successful commitments (0-100)
        let commitment_fulfillment = if stats.total_commitments > 0 {
            Decimal::from(stats.fulfilled_commitments * 100)
                / Decimal::from(stats.total_commitments)
        } else {
            dec!(50) // Default for new users
        };

        // Response time: placeholder (would need timing data)
        // Default to 70 (good) for now
        let response_time = dec!(70);

        // Quality rating: average quality score from events
        let quality_rating = if stats.quality_score_count > 0 {
            stats.total_quality_score / Decimal::from(stats.quality_score_count)
        } else {
            dec!(50) // Default
        };

        // Community trust: from holder ratings
        let community_trust = self.calculate_community_trust_score(user_id).await?;

        // Longevity: based on account age (max 100 at 365 days)
        let longevity = (Decimal::from(stats.account_age_days) / dec!(365) * dec!(100))
            .min(dec!(100))
            .max(dec!(0));

        Ok(ScoreComponents {
            commitment_fulfillment,
            response_time,
            quality_rating,
            community_trust,
            longevity,
        })
    }

    /// Calculate community trust score (0-100) from holder ratings
    ///
    /// Uses the same algorithm as HolderRatingService::calculate_community_trust:
    /// - With 0 ratings: Returns default neutral score (50)
    /// - With < 10 ratings: Blends weighted average with default based on confidence
    /// - With >= 10 ratings: Uses weighted average with high confidence
    async fn calculate_community_trust_score(&self, issuer_user_id: Uuid) -> Result<Decimal> {
        // Get weighted average rating from holder_ratings table
        let rating_row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_ratings,
                COALESCE(
                    SUM(rating * holder_balance) / NULLIF(SUM(holder_balance), 0),
                    0
                ) as weighted_avg
            FROM holder_ratings
            WHERE issuer_user_id = $1
            "#,
        )
        .bind(issuer_user_id)
        .fetch_one(&self.pool)
        .await?;

        let total_ratings: i64 = rating_row.get("total_ratings");
        let weighted_avg: f64 = rating_row.get("weighted_avg");

        if total_ratings == 0 {
            return Ok(dec!(50)); // Default neutral score
        }

        // Convert 1-5 star rating to 0-100 score
        let rating_score = Decimal::from_f64_retain(weighted_avg)
            .unwrap_or(dec!(2.5)) // Default to 2.5 if conversion fails
            * dec!(20); // Scale: 1 star = 20, 5 stars = 100

        // Apply confidence weighting
        const MIN_RATINGS: i64 = 10;
        if total_ratings < MIN_RATINGS {
            // Blend with default score based on confidence
            // Confidence increases asymptotically: 1 - (1 / (1 + ratings/MIN))
            let confidence = dec!(1)
                - (dec!(1) / (dec!(1) + Decimal::from(total_ratings) / Decimal::from(MIN_RATINGS)));
            let default_score = dec!(50);
            Ok(rating_score * confidence + default_score * (dec!(1) - confidence))
        } else {
            // High confidence, use weighted average directly
            Ok(rating_score)
        }
    }

    /// Fetch aggregated event statistics for a user
    async fn fetch_event_stats(&self, user_id: Uuid) -> Result<EventStats> {
        // Get commitment stats
        let commitment_stats: Option<(i64, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE event_type = 'commitment_fulfilled') as fulfilled,
                COUNT(*) FILTER (WHERE event_type = 'commitment_failed') as failed
            FROM reputation_events
            WHERE user_id = $1
            AND event_type IN ('commitment_fulfilled', 'commitment_failed', 'commitment_expired')
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (total_commitments, fulfilled_commitments, failed_commitments) =
            commitment_stats.unwrap_or((0, 0, 0));

        // Get quality score stats
        let quality_stats: Option<(Decimal, i64)> = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(delta), 0), COUNT(*)
            FROM reputation_events
            WHERE user_id = $1 AND event_type = 'quality_score'
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (total_quality_score, quality_score_count) = quality_stats.unwrap_or((dec!(0), 0));

        // Get account age
        let account_age: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT EXTRACT(DAY FROM NOW() - created_at)::bigint
            FROM users WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let account_age_days = account_age.map(|(d,)| d).unwrap_or(0);

        Ok(EventStats {
            total_commitments,
            fulfilled_commitments,
            failed_commitments,
            total_quality_score,
            quality_score_count,
            account_age_days,
        })
    }

    /// Validate a score delta before applying
    pub fn validate_delta(delta: Decimal) -> Result<Decimal> {
        if delta > MAX_POSITIVE_DELTA {
            tracing::warn!(
                delta = %delta,
                max = %MAX_POSITIVE_DELTA,
                "Score delta exceeded maximum positive, clamping"
            );
            return Ok(MAX_POSITIVE_DELTA);
        }

        if delta < MAX_NEGATIVE_DELTA {
            tracing::warn!(
                delta = %delta,
                min = %MAX_NEGATIVE_DELTA,
                "Score delta exceeded maximum negative, clamping"
            );
            return Ok(MAX_NEGATIVE_DELTA);
        }

        Ok(delta)
    }

    /// Update score with delta validation
    pub async fn update_score(&self, user_id: Uuid, delta: Decimal) -> Result<ReputationScore> {
        // Validate and clamp delta
        let validated_delta = Self::validate_delta(delta)?;

        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST($3, LEAST($4, reputation_score + $1))
            WHERE user_id = $2
            RETURNING reputation_score
            "#,
        )
        .bind(validated_delta)
        .bind(user_id)
        .bind(MIN_SCORE)
        .bind(MAX_SCORE)
        .fetch_one(&self.pool)
        .await?;

        let tier = ReputationTier::from_score(new_score);
        let components = self.calculate_components(user_id).await?;

        // Check for circuit breaker
        if tier == ReputationTier::Unverified {
            tracing::warn!(user_id = %user_id, score = %new_score, "Circuit breaker triggered - reputation too low");
        }

        Ok(ReputationScore {
            user_id,
            overall_score: new_score,
            tier,
            components,
        })
    }

    /// Update score with validated delta and record event
    pub async fn update_score_with_event(
        &self,
        user_id: Uuid,
        delta: Decimal,
        event_type: ReputationEventType,
        reason: Option<&str>,
    ) -> Result<ReputationScore> {
        let validated_delta = Self::validate_delta(delta)?;

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Record event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(format!("{:?}", event_type).to_lowercase())
        .bind(validated_delta)
        .bind(reason)
        .execute(&mut *tx)
        .await?;

        // Update score
        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST($3, LEAST($4, reputation_score + $1))
            WHERE user_id = $2
            RETURNING reputation_score
            "#,
        )
        .bind(validated_delta)
        .bind(user_id)
        .bind(MIN_SCORE)
        .bind(MAX_SCORE)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        let tier = ReputationTier::from_score(new_score);
        let components = self.calculate_components(user_id).await?;

        tracing::info!(
            user_id = %user_id,
            event_type = ?event_type,
            delta = %validated_delta,
            new_score = %new_score,
            tier = ?tier,
            "Reputation updated"
        );

        Ok(ReputationScore {
            user_id,
            overall_score: new_score,
            tier,
            components,
        })
    }

    /// Recalculate score from components (useful for auditing)
    pub async fn recalculate_score(&self, user_id: Uuid) -> Result<ReputationScore> {
        let components = self.calculate_components(user_id).await?;
        let calculated_score = components.calculate_scaled_overall();

        // Update the user's score to match calculated
        let (new_score,): (Decimal,) = sqlx::query_as(
            r#"
            UPDATE users
            SET reputation_score = GREATEST($2, LEAST($3, $1))
            WHERE user_id = $4
            RETURNING reputation_score
            "#,
        )
        .bind(calculated_score)
        .bind(MIN_SCORE)
        .bind(MAX_SCORE)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let tier = ReputationTier::from_score(new_score);

        Ok(ReputationScore {
            user_id,
            overall_score: new_score,
            tier,
            components,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_score_constants() {
        assert_eq!(MAX_POSITIVE_DELTA, dec!(50));
        assert_eq!(MAX_NEGATIVE_DELTA, dec!(-100));
        assert_eq!(MIN_SCORE, dec!(0));
        assert_eq!(MAX_SCORE, dec!(1000));
    }

    #[test]
    fn test_component_weights_sum_to_one() {
        let sum = COMPONENT_WEIGHTS.commitment_fulfillment
            + COMPONENT_WEIGHTS.response_time
            + COMPONENT_WEIGHTS.quality_rating
            + COMPONENT_WEIGHTS.community_trust
            + COMPONENT_WEIGHTS.longevity;
        assert_eq!(sum, dec!(1.0));
    }

    #[test]
    fn test_component_weights_values() {
        assert_eq!(COMPONENT_WEIGHTS.commitment_fulfillment, dec!(0.35));
        assert_eq!(COMPONENT_WEIGHTS.response_time, dec!(0.15));
        assert_eq!(COMPONENT_WEIGHTS.quality_rating, dec!(0.30));
        assert_eq!(COMPONENT_WEIGHTS.community_trust, dec!(0.15));
        assert_eq!(COMPONENT_WEIGHTS.longevity, dec!(0.05));
    }

    #[test]
    fn test_score_components_calculate_overall() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(90),
            quality_rating: dec!(85),
            community_trust: dec!(75),
            longevity: dec!(60),
        };

        let overall = components.calculate_overall();
        // 80*0.35 + 90*0.15 + 85*0.30 + 75*0.15 + 60*0.05
        // = 28 + 13.5 + 25.5 + 11.25 + 3
        // = 81.25
        assert_eq!(overall, dec!(81.25));
    }

    #[test]
    fn test_score_components_calculate_overall_all_zeros() {
        let components = ScoreComponents::default();
        let overall = components.calculate_overall();
        assert_eq!(overall, dec!(0));
    }

    #[test]
    fn test_score_components_calculate_overall_all_hundreds() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(100),
            response_time: dec!(100),
            quality_rating: dec!(100),
            community_trust: dec!(100),
            longevity: dec!(100),
        };

        let overall = components.calculate_overall();
        assert_eq!(overall, dec!(100));
    }

    #[test]
    fn test_score_components_calculate_scaled_overall() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(90),
            quality_rating: dec!(85),
            community_trust: dec!(75),
            longevity: dec!(60),
        };

        let scaled = components.calculate_scaled_overall();
        // 81.25 * 10 = 812.5
        assert_eq!(scaled, dec!(812.5));
    }

    #[test]
    fn test_score_components_calculate_scaled_overall_max() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(100),
            response_time: dec!(100),
            quality_rating: dec!(100),
            community_trust: dec!(100),
            longevity: dec!(100),
        };

        let scaled = components.calculate_scaled_overall();
        assert_eq!(scaled, dec!(1000)); // 100 * 10
    }

    #[test]
    fn test_validate_delta_normal() {
        let delta = dec!(25);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(25));
    }

    #[test]
    fn test_validate_delta_max_positive() {
        let delta = dec!(50);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(50));
    }

    #[test]
    fn test_validate_delta_exceeds_positive() {
        let delta = dec!(100);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, MAX_POSITIVE_DELTA);
    }

    #[test]
    fn test_validate_delta_negative() {
        let delta = dec!(-50);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(-50));
    }

    #[test]
    fn test_validate_delta_max_negative() {
        let delta = dec!(-100);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(-100));
    }

    #[test]
    fn test_validate_delta_exceeds_negative() {
        let delta = dec!(-200);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, MAX_NEGATIVE_DELTA);
    }

    #[test]
    fn test_validate_delta_zero() {
        let delta = dec!(0);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(0));
    }

    #[test]
    fn test_validate_delta_just_below_max_positive() {
        let delta = dec!(49.9);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(49.9));
    }

    #[test]
    fn test_validate_delta_just_above_max_negative() {
        let delta = dec!(-99.9);
        let result = ReputationEngine::validate_delta(delta).unwrap();
        assert_eq!(result, dec!(-99.9));
    }

    #[test]
    fn test_reputation_event_type_display() {
        assert_eq!(
            ReputationEventType::CommitmentFulfilled.to_string(),
            "commitment_fulfilled"
        );
        assert_eq!(
            ReputationEventType::CommitmentFailed.to_string(),
            "commitment_failed"
        );
        assert_eq!(
            ReputationEventType::CommitmentExpired.to_string(),
            "commitment_expired"
        );
        assert_eq!(
            ReputationEventType::QualityScore.to_string(),
            "quality_score"
        );
        assert_eq!(
            ReputationEventType::HolderRating.to_string(),
            "holder_rating"
        );
        assert_eq!(
            ReputationEventType::AdminAdjustment.to_string(),
            "admin_adjustment"
        );
        assert_eq!(
            ReputationEventType::AccountCreated.to_string(),
            "account_created"
        );
        assert_eq!(
            ReputationEventType::FirstTokenIssued.to_string(),
            "first_token_issued"
        );
    }

    #[test]
    fn test_reputation_event_type_from_str_valid() {
        assert_eq!(
            "commitment_fulfilled"
                .parse::<ReputationEventType>()
                .unwrap(),
            ReputationEventType::CommitmentFulfilled
        );
        assert_eq!(
            "commitment_failed".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::CommitmentFailed
        );
        assert_eq!(
            "commitment_expired".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::CommitmentExpired
        );
        assert_eq!(
            "quality_score".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::QualityScore
        );
        assert_eq!(
            "holder_rating".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::HolderRating
        );
        assert_eq!(
            "admin_adjustment".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::AdminAdjustment
        );
        assert_eq!(
            "account_created".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::AccountCreated
        );
        assert_eq!(
            "first_token_issued".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::FirstTokenIssued
        );
    }

    #[test]
    fn test_reputation_event_type_from_str_variants() {
        assert_eq!(
            "commitment fulfilled"
                .parse::<ReputationEventType>()
                .unwrap(),
            ReputationEventType::CommitmentFulfilled
        );
        assert_eq!(
            "commitmentfulfilled"
                .parse::<ReputationEventType>()
                .unwrap(),
            ReputationEventType::CommitmentFulfilled
        );
        assert_eq!(
            "quality score".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::QualityScore
        );
        assert_eq!(
            "qualityscore".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::QualityScore
        );
    }

    #[test]
    fn test_reputation_event_type_from_str_case_insensitive() {
        assert_eq!(
            "COMMITMENT_FULFILLED"
                .parse::<ReputationEventType>()
                .unwrap(),
            ReputationEventType::CommitmentFulfilled
        );
        assert_eq!(
            "Quality_Score".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::QualityScore
        );
        assert_eq!(
            "HOLDER_RATING".parse::<ReputationEventType>().unwrap(),
            ReputationEventType::HolderRating
        );
    }

    #[test]
    fn test_reputation_event_type_from_str_invalid() {
        assert!("invalid".parse::<ReputationEventType>().is_err());
        assert!("unknown_event".parse::<ReputationEventType>().is_err());
        assert!("".parse::<ReputationEventType>().is_err());
    }

    #[test]
    fn test_component_weights_validate_valid() {
        // Default weights should be valid
        assert!(COMPONENT_WEIGHTS.validate().is_ok());
        assert_eq!(COMPONENT_WEIGHTS.sum(), dec!(1.0));
    }

    #[test]
    fn test_component_weights_validate_invalid() {
        let invalid_weights = ComponentWeights {
            commitment_fulfillment: dec!(0.5),
            response_time: dec!(0.2),
            quality_rating: dec!(0.2),
            community_trust: dec!(0.2),
            longevity: dec!(0.1),
        };
        assert!(invalid_weights.validate().is_err());
        assert_eq!(invalid_weights.sum(), dec!(1.2));
    }

    #[test]
    fn test_component_weights_validate_within_tolerance() {
        // Slight rounding error within tolerance should be valid
        let weights = ComponentWeights {
            commitment_fulfillment: dec!(0.35),
            response_time: dec!(0.15),
            quality_rating: dec!(0.30),
            community_trust: dec!(0.15),
            longevity: dec!(0.0501), // Slightly off due to rounding
        };
        // Sum is 1.0001, within tolerance of 0.001
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_component_weights_sum() {
        let weights = ComponentWeights {
            commitment_fulfillment: dec!(0.25),
            response_time: dec!(0.25),
            quality_rating: dec!(0.25),
            community_trust: dec!(0.15),
            longevity: dec!(0.10),
        };
        assert_eq!(weights.sum(), dec!(1.0));
    }

    // ========================================================================
    // Property-Based Tests
    // ========================================================================

    mod proptest_score {
        use super::*;
        use proptest::prelude::*;

        // Strategy for generating valid component scores (0-100)
        fn component_score() -> impl Strategy<Value = i64> {
            0i64..=100i64
        }

        proptest! {
            /// Property: Overall score should always be between 0 and 100
            #[test]
            fn prop_overall_score_in_bounds(
                cf in component_score(),
                rt in component_score(),
                qr in component_score(),
                ct in component_score(),
                lg in component_score(),
            ) {
                let components = ScoreComponents {
                    commitment_fulfillment: Decimal::new(cf, 0),
                    response_time: Decimal::new(rt, 0),
                    quality_rating: Decimal::new(qr, 0),
                    community_trust: Decimal::new(ct, 0),
                    longevity: Decimal::new(lg, 0),
                };

                let overall = components.calculate_overall();
                prop_assert!(overall >= dec!(0));
                prop_assert!(overall <= dec!(100));
            }

            /// Property: Scaled overall score should always be between 0 and 1000
            #[test]
            fn prop_scaled_overall_in_bounds(
                cf in component_score(),
                rt in component_score(),
                qr in component_score(),
                ct in component_score(),
                lg in component_score(),
            ) {
                let components = ScoreComponents {
                    commitment_fulfillment: Decimal::new(cf, 0),
                    response_time: Decimal::new(rt, 0),
                    quality_rating: Decimal::new(qr, 0),
                    community_trust: Decimal::new(ct, 0),
                    longevity: Decimal::new(lg, 0),
                };

                let scaled = components.calculate_scaled_overall();
                prop_assert!(scaled >= MIN_SCORE);
                prop_assert!(scaled <= MAX_SCORE);
            }

            /// Property: Positive deltas should be clamped to MAX_POSITIVE_DELTA
            #[test]
            fn prop_positive_delta_clamped(delta in 0i64..=1000i64) {
                let delta_dec = Decimal::new(delta, 0);
                let result = ReputationEngine::validate_delta(delta_dec).unwrap();
                prop_assert!(result <= MAX_POSITIVE_DELTA);
                prop_assert!(result >= dec!(0));
            }

            /// Property: Negative deltas should be clamped to MAX_NEGATIVE_DELTA
            #[test]
            fn prop_negative_delta_clamped(delta in -1000i64..=0i64) {
                let delta_dec = Decimal::new(delta, 0);
                let result = ReputationEngine::validate_delta(delta_dec).unwrap();
                prop_assert!(result >= MAX_NEGATIVE_DELTA);
                prop_assert!(result <= dec!(0));
            }

            /// Property: Doubling all components should double the overall score
            #[test]
            fn prop_score_linearity(
                cf in 0i64..=50i64,
                rt in 0i64..=50i64,
                qr in 0i64..=50i64,
                ct in 0i64..=50i64,
                lg in 0i64..=50i64,
            ) {
                let components1 = ScoreComponents {
                    commitment_fulfillment: Decimal::new(cf, 0),
                    response_time: Decimal::new(rt, 0),
                    quality_rating: Decimal::new(qr, 0),
                    community_trust: Decimal::new(ct, 0),
                    longevity: Decimal::new(lg, 0),
                };

                let components2 = ScoreComponents {
                    commitment_fulfillment: Decimal::new(cf * 2, 0),
                    response_time: Decimal::new(rt * 2, 0),
                    quality_rating: Decimal::new(qr * 2, 0),
                    community_trust: Decimal::new(ct * 2, 0),
                    longevity: Decimal::new(lg * 2, 0),
                };

                let score1 = components1.calculate_overall();
                let score2 = components2.calculate_overall();

                // Due to decimal precision, check approximate equality
                let expected = score1 * dec!(2);
                let diff = (score2 - expected).abs();
                prop_assert!(diff < dec!(0.01));
            }
        }
    }

    // Phase 17 tests - New functionality
    #[test]
    fn test_component_weights_default() {
        let default_weights = ComponentWeights::default();
        assert_eq!(default_weights.commitment_fulfillment, dec!(0.35));
        assert_eq!(default_weights.response_time, dec!(0.15));
        assert_eq!(default_weights.quality_rating, dec!(0.30));
        assert_eq!(default_weights.community_trust, dec!(0.15));
        assert_eq!(default_weights.longevity, dec!(0.05));
    }

    #[test]
    fn test_score_components_is_all_above_threshold() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(75),
            quality_rating: dec!(85),
            community_trust: dec!(70),
            longevity: dec!(65),
        };

        assert!(components.is_all_above_threshold(dec!(60)));
        assert!(!components.is_all_above_threshold(dec!(80)));
        assert!(components.is_all_above_threshold(dec!(0)));
    }

    #[test]
    fn test_score_components_average() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(70),
            quality_rating: dec!(90),
            community_trust: dec!(60),
            longevity: dec!(50),
        };

        let avg = components.average_component_score();
        assert_eq!(avg, dec!(70)); // (80+70+90+60+50)/5 = 70
    }

    #[test]
    fn test_score_components_min_max() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(70),
            quality_rating: dec!(90),
            community_trust: dec!(60),
            longevity: dec!(50),
        };

        assert_eq!(components.min_component(), dec!(50));
        assert_eq!(components.max_component(), dec!(90));
    }

    #[test]
    fn test_score_components_weakest_strongest() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(70),
            quality_rating: dec!(90),
            community_trust: dec!(60),
            longevity: dec!(50),
        };

        let (weakest_name, weakest_val) = components.weakest_component();
        assert_eq!(weakest_name, "longevity");
        assert_eq!(weakest_val, dec!(50));

        let (strongest_name, strongest_val) = components.strongest_component();
        assert_eq!(strongest_name, "quality_rating");
        assert_eq!(strongest_val, dec!(90));
    }

    #[test]
    fn test_score_components_display() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(90),
            quality_rating: dec!(85),
            community_trust: dec!(75),
            longevity: dec!(60),
        };

        let display_string = format!("{}", components);
        assert!(display_string.contains("commitment: 80.0"));
        assert!(display_string.contains("response: 90.0"));
        assert!(display_string.contains("quality: 85.0"));
        assert!(display_string.contains("trust: 75.0"));
        assert!(display_string.contains("longevity: 60.0"));
        assert!(
            display_string.contains("overall: 81.3") || display_string.contains("overall: 81.2")
        );
    }

    #[test]
    fn test_reputation_score_is_critical() {
        let score_critical = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(150),
            tier: ReputationTier::Unverified,
            components: ScoreComponents::default(),
        };
        assert!(score_critical.is_critical());

        let score_not_critical = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(250),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };
        assert!(!score_not_critical.is_critical());
    }

    #[test]
    fn test_reputation_score_is_excellent() {
        let score_excellent = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(950),
            tier: ReputationTier::Diamond,
            components: ScoreComponents::default(),
        };
        assert!(score_excellent.is_excellent());

        let score_not_excellent = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(850),
            tier: ReputationTier::Platinum,
            components: ScoreComponents::default(),
        };
        assert!(!score_not_excellent.is_excellent());
    }

    #[test]
    fn test_reputation_score_is_good() {
        let score_good = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(700),
            tier: ReputationTier::Gold,
            components: ScoreComponents::default(),
        };
        assert!(score_good.is_good());

        let score_not_good = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(500),
            tier: ReputationTier::Silver,
            components: ScoreComponents::default(),
        };
        assert!(!score_not_good.is_good());
    }

    #[test]
    fn test_reputation_score_needs_attention() {
        let score_needs_attention = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(350),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };
        assert!(score_needs_attention.needs_attention());

        let score_fine = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(500),
            tier: ReputationTier::Silver,
            components: ScoreComponents::default(),
        };
        assert!(!score_fine.needs_attention());
    }

    #[test]
    fn test_reputation_score_distance_to_next_tier() {
        let score = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(350),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };

        let distance = score.distance_to_next_tier();
        assert_eq!(distance, Some(dec!(50))); // 400 - 350 = 50

        let score_diamond = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(950),
            tier: ReputationTier::Diamond,
            components: ScoreComponents::default(),
        };

        assert_eq!(score_diamond.distance_to_next_tier(), None);
    }

    #[test]
    fn test_reputation_score_tier_progress() {
        let score = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(300),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };

        let progress = score.tier_progress_percentage();
        // Bronze: 200-399, score 300
        // (300 - 200) / (399 - 200) = 100 / 199 = ~50.25%
        assert!(progress > dec!(50) && progress < dec!(51));
    }

    // Phase 23 Tests

    #[test]
    fn test_score_comparison() {
        let current = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(500),
            tier: ReputationTier::Silver,
            components: ScoreComponents::default(),
        };

        let previous = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(400),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };

        let comparison = current.compare_with(&previous);
        assert_eq!(comparison.difference, dec!(100));
        assert_eq!(comparison.percentage_change, dec!(25)); // 100/400 * 100
        assert!(comparison.is_improvement);
        assert!(comparison.tier_changed);
        assert!(comparison.tier_improved);
    }

    #[test]
    fn test_score_comparison_decline() {
        let current = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(300),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };

        let previous = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(450),
            tier: ReputationTier::Silver,
            components: ScoreComponents::default(),
        };

        let comparison = current.compare_with(&previous);
        assert_eq!(comparison.difference, dec!(-150));
        assert!(!comparison.is_improvement);
        assert!(comparison.tier_changed);
        assert!(!comparison.tier_improved);

        let summary = comparison.summary();
        assert!(summary.contains("decreased"));
        assert!(summary.contains("falling"));
    }

    #[test]
    fn test_score_status_labels() {
        let excellent = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(950),
            tier: ReputationTier::Diamond,
            components: ScoreComponents::default(),
        };
        assert_eq!(excellent.status_label(), "Excellent");
        assert_eq!(excellent.status_emoji(), "💎");

        let good = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(700),
            tier: ReputationTier::Gold,
            components: ScoreComponents::default(),
        };
        assert_eq!(good.status_label(), "Good");
        assert_eq!(good.status_emoji(), "⭐");

        let needs_attention = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(300),
            tier: ReputationTier::Bronze,
            components: ScoreComponents::default(),
        };
        assert_eq!(needs_attention.status_label(), "Needs Attention");
        assert_eq!(needs_attention.status_emoji(), "⚠️");

        let critical = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(100),
            tier: ReputationTier::Unverified,
            components: ScoreComponents::default(),
        };
        assert_eq!(critical.status_label(), "Critical");
        assert_eq!(critical.status_emoji(), "🚨");
    }

    #[test]
    fn test_is_eligible_for_operation() {
        let gold_score = ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: dec!(700),
            tier: ReputationTier::Gold,
            components: ScoreComponents::default(),
        };

        assert!(gold_score.is_eligible_for_operation(ReputationTier::Bronze));
        assert!(gold_score.is_eligible_for_operation(ReputationTier::Silver));
        assert!(gold_score.is_eligible_for_operation(ReputationTier::Gold));
        assert!(!gold_score.is_eligible_for_operation(ReputationTier::Platinum));
        assert!(!gold_score.is_eligible_for_operation(ReputationTier::Diamond));
    }

    #[test]
    fn test_component_balance_score() {
        // Perfectly balanced components
        let balanced = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(80),
            quality_rating: dec!(80),
            community_trust: dec!(80),
            longevity: dec!(80),
        };
        let balance = balanced.component_balance_score();
        assert_eq!(balance, dec!(100)); // Perfect balance

        // Unbalanced components
        let unbalanced = ScoreComponents {
            commitment_fulfillment: dec!(90),
            response_time: dec!(30),
            quality_rating: dec!(85),
            community_trust: dec!(40),
            longevity: dec!(75),
        };
        let balance = unbalanced.component_balance_score();
        assert!(balance < dec!(100)); // Less than perfect
    }

    #[test]
    fn test_improvement_priorities() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(50), // Low, high weight (35%)
            response_time: dec!(90),          // High, medium weight (15%)
            quality_rating: dec!(60),         // Medium, high weight (30%)
            community_trust: dec!(85),        // High, medium weight (15%)
            longevity: dec!(40),              // Low, low weight (5%)
        };

        let priorities = components.improvement_priorities();
        // commitment_fulfillment should be top priority (low score, high weight)
        assert_eq!(priorities[0].0, "commitment_fulfillment");
        // quality_rating should be second (medium score, high weight)
        assert_eq!(priorities[1].0, "quality_rating");
    }

    #[test]
    fn test_bottleneck_component() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(70), // 70 / 0.35 = 200
            response_time: dec!(30),          // 30 / 0.15 = 200
            quality_rating: dec!(90),         // 90 / 0.30 = 300
            community_trust: dec!(45),        // 45 / 0.15 = 300
            longevity: dec!(5),               // 5 / 0.05 = 100 <- bottleneck
        };

        let (name, _value, _weight) = components.bottleneck_component();
        assert_eq!(name, "longevity");
    }

    #[test]
    fn test_component_as_percentile() {
        let components = ScoreComponents {
            commitment_fulfillment: dec!(75),
            response_time: dec!(80),
            quality_rating: dec!(85),
            community_trust: dec!(70),
            longevity: dec!(60),
        };

        assert_eq!(
            components.component_as_percentile("commitment_fulfillment"),
            Some(dec!(75))
        );
        assert_eq!(
            components.component_as_percentile("quality_rating"),
            Some(dec!(85))
        );
        assert_eq!(components.component_as_percentile("invalid"), None);
    }
}
