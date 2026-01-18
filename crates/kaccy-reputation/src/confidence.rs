//! Component confidence tracking
//!
//! Tracks confidence levels for reputation score components based on
//! the amount of data available. Components with more data points
//! have higher confidence and are weighted more heavily.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::Result;

// Type aliases to reduce complexity
type CommitmentStats = (i64, i64, Option<DateTime<Utc>>, Option<DateTime<Utc>>);
type QualityStats = (i64, Decimal, Option<DateTime<Utc>>, Option<DateTime<Utc>>);

/// Confidence level for a score component
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfidenceLevel {
    /// Very low confidence (< 3 data points)
    VeryLow,
    /// Low confidence (3-9 data points)
    Low,
    /// Medium confidence (10-29 data points)
    Medium,
    /// High confidence (30-99 data points)
    High,
    /// Very high confidence (100+ data points)
    VeryHigh,
}

impl ConfidenceLevel {
    /// Get confidence from number of data points
    pub fn from_data_points(count: i64) -> Self {
        if count >= 100 {
            ConfidenceLevel::VeryHigh
        } else if count >= 30 {
            ConfidenceLevel::High
        } else if count >= 10 {
            ConfidenceLevel::Medium
        } else if count >= 3 {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::VeryLow
        }
    }

    /// Get the weight multiplier for this confidence level
    /// Higher confidence = closer to 1.0
    pub fn weight_multiplier(&self) -> Decimal {
        match self {
            ConfidenceLevel::VeryLow => dec!(0.5),  // 50% weight
            ConfidenceLevel::Low => dec!(0.7),      // 70% weight
            ConfidenceLevel::Medium => dec!(0.85),  // 85% weight
            ConfidenceLevel::High => dec!(0.95),    // 95% weight
            ConfidenceLevel::VeryHigh => dec!(1.0), // 100% weight
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ConfidenceLevel::VeryLow => "Very Low (needs more data)",
            ConfidenceLevel::Low => "Low",
            ConfidenceLevel::Medium => "Medium",
            ConfidenceLevel::High => "High",
            ConfidenceLevel::VeryHigh => "Very High (reliable)",
        }
    }
}

impl std::fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::str::FromStr for ConfidenceLevel {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().trim() {
            "very low" | "verylow" | "very_low" => Ok(ConfidenceLevel::VeryLow),
            "low" => Ok(ConfidenceLevel::Low),
            "medium" | "med" => Ok(ConfidenceLevel::Medium),
            "high" => Ok(ConfidenceLevel::High),
            "very high" | "veryhigh" | "very_high" => Ok(ConfidenceLevel::VeryHigh),
            _ => Err(crate::error::ReputationError::Validation(format!(
                "Invalid confidence level: {}. Valid values: very low, low, medium, high, very high",
                s
            ))),
        }
    }
}

/// Confidence data for a single component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfidence {
    /// Component name
    pub component: String,
    /// Raw score value
    pub raw_score: Decimal,
    /// Confidence-adjusted score
    pub adjusted_score: Decimal,
    /// Number of data points used
    pub data_points: i64,
    /// Confidence level
    pub confidence: ConfidenceLevel,
    /// Weight multiplier applied
    pub weight_multiplier: Decimal,
    /// Oldest data point timestamp
    pub oldest_data: Option<DateTime<Utc>>,
    /// Newest data point timestamp
    pub newest_data: Option<DateTime<Utc>>,
}

impl ComponentConfidence {
    pub fn new(component: impl Into<String>, raw_score: Decimal, data_points: i64) -> Self {
        let confidence = ConfidenceLevel::from_data_points(data_points);
        let weight_multiplier = confidence.weight_multiplier();
        let adjusted_score = raw_score * weight_multiplier;

        Self {
            component: component.into(),
            raw_score,
            adjusted_score,
            data_points,
            confidence,
            weight_multiplier,
            oldest_data: None,
            newest_data: None,
        }
    }

    pub fn with_timestamps(
        mut self,
        oldest: Option<DateTime<Utc>>,
        newest: Option<DateTime<Utc>>,
    ) -> Self {
        self.oldest_data = oldest;
        self.newest_data = newest;
        self
    }
}

/// Full confidence breakdown for all components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    pub user_id: Uuid,
    pub components: Vec<ComponentConfidence>,
    pub overall_confidence: ConfidenceLevel,
    pub overall_data_points: i64,
    /// Confidence-weighted overall score
    pub weighted_score: Decimal,
    /// Raw overall score (without confidence weighting)
    pub raw_score: Decimal,
    pub calculated_at: DateTime<Utc>,
}

impl ConfidenceBreakdown {
    /// Calculate overall confidence from components
    pub fn calculate_overall(components: &[ComponentConfidence]) -> ConfidenceLevel {
        let total_points: i64 = components.iter().map(|c| c.data_points).sum();
        ConfidenceLevel::from_data_points(total_points / components.len().max(1) as i64)
    }
}

/// Stored confidence record
#[derive(Debug, Clone, FromRow)]
pub struct ConfidenceRecord {
    pub confidence_id: Uuid,
    pub user_id: Uuid,
    pub component: String,
    pub data_points: i64,
    pub confidence_level: String,
    pub last_updated: DateTime<Utc>,
}

/// Service for tracking component confidence
pub struct ConfidenceTracker {
    pool: PgPool,
}

impl ConfidenceTracker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Calculate confidence breakdown for a user
    pub async fn get_confidence_breakdown(&self, user_id: Uuid) -> Result<ConfidenceBreakdown> {
        // Get commitment fulfillment confidence
        let commitment_conf = self.get_commitment_confidence(user_id).await?;

        // Get quality rating confidence
        let quality_conf = self.get_quality_confidence(user_id).await?;

        // Get community trust (holder ratings) confidence
        let trust_conf = self.get_community_trust_confidence(user_id).await?;

        // Get response time confidence (placeholder - would need timing data)
        let response_conf = ComponentConfidence::new("response_time", dec!(70), 0);

        // Get longevity confidence (always high - based on account age)
        let longevity_conf = self.get_longevity_confidence(user_id).await?;

        let components = vec![
            commitment_conf,
            quality_conf,
            trust_conf,
            response_conf,
            longevity_conf,
        ];

        let overall_data_points: i64 = components.iter().map(|c| c.data_points).sum();
        let overall_confidence = ConfidenceBreakdown::calculate_overall(&components);

        // Calculate raw and weighted scores
        let raw_score: Decimal = components.iter().map(|c| c.raw_score).sum::<Decimal>()
            / Decimal::from(components.len());
        let weighted_score: Decimal = components.iter().map(|c| c.adjusted_score).sum::<Decimal>()
            / Decimal::from(components.len());

        Ok(ConfidenceBreakdown {
            user_id,
            components,
            overall_confidence,
            overall_data_points,
            weighted_score,
            raw_score,
            calculated_at: Utc::now(),
        })
    }

    /// Get commitment fulfillment confidence
    async fn get_commitment_confidence(&self, user_id: Uuid) -> Result<ComponentConfidence> {
        let stats: Option<CommitmentStats> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'verified') as fulfilled,
                MIN(created_at) as oldest,
                MAX(created_at) as newest
            FROM output_commitments
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (total, fulfilled, oldest, newest) = stats.unwrap_or((0, 0, None, None));

        let score = if total > 0 {
            Decimal::from(fulfilled * 100) / Decimal::from(total)
        } else {
            dec!(50) // Default score for new users
        };

        Ok(
            ComponentConfidence::new("commitment_fulfillment", score, total)
                .with_timestamps(oldest, newest),
        )
    }

    /// Get quality rating confidence
    async fn get_quality_confidence(&self, user_id: Uuid) -> Result<ComponentConfidence> {
        let stats: Option<QualityStats> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as count,
                COALESCE(AVG(delta), 0) as avg_score,
                MIN(created_at) as oldest,
                MAX(created_at) as newest
            FROM reputation_events
            WHERE user_id = $1 AND event_type = 'quality_score'
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (count, avg_score, oldest, newest) = stats.unwrap_or((0, dec!(0), None, None));

        // Normalize to 0-100 scale (quality scores are typically 0-100)
        let score = if count > 0 { avg_score } else { dec!(50) };

        Ok(
            ComponentConfidence::new("quality_rating", score, count)
                .with_timestamps(oldest, newest),
        )
    }

    /// Get community trust confidence from holder ratings
    async fn get_community_trust_confidence(&self, user_id: Uuid) -> Result<ComponentConfidence> {
        let stats: Option<QualityStats> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as count,
                COALESCE(AVG(rating), 0) as avg_rating,
                MIN(created_at) as oldest,
                MAX(created_at) as newest
            FROM holder_ratings
            WHERE issuer_user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (count, avg_rating, oldest, newest) = stats.unwrap_or((0, dec!(0), None, None));

        // Convert 1-5 star rating to 0-100 scale
        let score = if count > 0 {
            (avg_rating - dec!(1)) / dec!(4) * dec!(100)
        } else {
            dec!(50)
        };

        Ok(ComponentConfidence::new("community_trust", score, count)
            .with_timestamps(oldest, newest))
    }

    /// Get longevity confidence (based on account age)
    async fn get_longevity_confidence(&self, user_id: Uuid) -> Result<ComponentConfidence> {
        let result: Option<(i64, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT
                EXTRACT(DAY FROM NOW() - created_at)::bigint as days,
                created_at
            FROM users
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        let (days, created_at) = result.unwrap_or((0, Utc::now()));

        // Score based on account age (max 100 at 365 days)
        let score = (Decimal::from(days) / dec!(365) * dec!(100))
            .min(dec!(100))
            .max(dec!(0));

        // Longevity always has high confidence (it's a fact)
        let mut conf = ComponentConfidence::new("longevity", score, 100);
        conf.oldest_data = Some(created_at);
        conf.newest_data = Some(Utc::now());
        conf.confidence = ConfidenceLevel::VeryHigh;
        conf.weight_multiplier = dec!(1.0);
        conf.adjusted_score = score;

        Ok(conf)
    }

    /// Store/update confidence record
    pub async fn update_confidence(&self, user_id: Uuid, component: &str) -> Result<()> {
        let breakdown = self.get_confidence_breakdown(user_id).await?;

        let comp = breakdown
            .components
            .iter()
            .find(|c| c.component == component);

        if let Some(comp) = comp {
            sqlx::query(
                r#"
                INSERT INTO component_confidence (user_id, component, data_points, confidence_level, last_updated)
                VALUES ($1, $2, $3, $4, NOW())
                ON CONFLICT (user_id, component) DO UPDATE
                SET data_points = EXCLUDED.data_points,
                    confidence_level = EXCLUDED.confidence_level,
                    last_updated = NOW()
                "#,
            )
            .bind(user_id)
            .bind(component)
            .bind(comp.data_points)
            .bind(format!("{:?}", comp.confidence))
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Get users with low confidence scores (for attention)
    pub async fn get_low_confidence_users(&self, limit: i64) -> Result<Vec<(Uuid, String)>> {
        let users: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT user_id, confidence_level
            FROM component_confidence
            WHERE confidence_level IN ('VeryLow', 'Low')
            ORDER BY last_updated DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(users)
    }

    /// Calculate confidence-weighted score for a user
    pub async fn get_weighted_score(&self, user_id: Uuid) -> Result<WeightedScore> {
        let breakdown = self.get_confidence_breakdown(user_id).await?;

        // Apply component weights
        let weights = ComponentWeights::default();
        let mut total_weight = dec!(0);
        let mut weighted_sum = dec!(0);

        for comp in &breakdown.components {
            let base_weight = match comp.component.as_str() {
                "commitment_fulfillment" => weights.commitment_fulfillment,
                "response_time" => weights.response_time,
                "quality_rating" => weights.quality_rating,
                "community_trust" => weights.community_trust,
                "longevity" => weights.longevity,
                _ => dec!(0),
            };

            // Apply confidence multiplier to weight
            let adjusted_weight = base_weight * comp.weight_multiplier;
            total_weight += adjusted_weight;
            weighted_sum += comp.raw_score * adjusted_weight;
        }

        let final_score = if total_weight > dec!(0) {
            (weighted_sum / total_weight).min(dec!(100)).max(dec!(0))
        } else {
            dec!(50)
        };

        Ok(WeightedScore {
            user_id,
            final_score: final_score * dec!(10), // Scale to 0-1000
            raw_score: breakdown.raw_score * dec!(10),
            confidence: breakdown.overall_confidence,
            components: breakdown.components,
        })
    }
}

/// Component weights for score calculation
#[derive(Debug, Clone)]
pub struct ComponentWeights {
    pub commitment_fulfillment: Decimal,
    pub response_time: Decimal,
    pub quality_rating: Decimal,
    pub community_trust: Decimal,
    pub longevity: Decimal,
}

impl Default for ComponentWeights {
    fn default() -> Self {
        Self {
            commitment_fulfillment: dec!(0.35),
            response_time: dec!(0.15),
            quality_rating: dec!(0.30),
            community_trust: dec!(0.15),
            longevity: dec!(0.05),
        }
    }
}

/// Final weighted score with confidence info
#[derive(Debug, Clone, Serialize)]
pub struct WeightedScore {
    pub user_id: Uuid,
    /// Final score (0-1000, confidence-weighted)
    pub final_score: Decimal,
    /// Raw score without confidence weighting
    pub raw_score: Decimal,
    /// Overall confidence level
    pub confidence: ConfidenceLevel,
    /// Component breakdown
    pub components: Vec<ComponentConfidence>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_confidence_level_from_data_points() {
        assert_eq!(
            ConfidenceLevel::from_data_points(0),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            ConfidenceLevel::from_data_points(1),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            ConfidenceLevel::from_data_points(2),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(ConfidenceLevel::from_data_points(3), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_data_points(9), ConfidenceLevel::Low);
        assert_eq!(
            ConfidenceLevel::from_data_points(10),
            ConfidenceLevel::Medium
        );
        assert_eq!(
            ConfidenceLevel::from_data_points(29),
            ConfidenceLevel::Medium
        );
        assert_eq!(ConfidenceLevel::from_data_points(30), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_data_points(99), ConfidenceLevel::High);
        assert_eq!(
            ConfidenceLevel::from_data_points(100),
            ConfidenceLevel::VeryHigh
        );
        assert_eq!(
            ConfidenceLevel::from_data_points(1000),
            ConfidenceLevel::VeryHigh
        );
    }

    #[test]
    fn test_confidence_level_weight_multiplier() {
        assert_eq!(ConfidenceLevel::VeryLow.weight_multiplier(), dec!(0.5));
        assert_eq!(ConfidenceLevel::Low.weight_multiplier(), dec!(0.7));
        assert_eq!(ConfidenceLevel::Medium.weight_multiplier(), dec!(0.85));
        assert_eq!(ConfidenceLevel::High.weight_multiplier(), dec!(0.95));
        assert_eq!(ConfidenceLevel::VeryHigh.weight_multiplier(), dec!(1.0));
    }

    #[test]
    fn test_confidence_level_description() {
        assert_eq!(
            ConfidenceLevel::VeryLow.description(),
            "Very Low (needs more data)"
        );
        assert_eq!(ConfidenceLevel::Low.description(), "Low");
        assert_eq!(ConfidenceLevel::Medium.description(), "Medium");
        assert_eq!(ConfidenceLevel::High.description(), "High");
        assert_eq!(
            ConfidenceLevel::VeryHigh.description(),
            "Very High (reliable)"
        );
    }

    #[test]
    fn test_confidence_level_display() {
        assert_eq!(
            ConfidenceLevel::VeryLow.to_string(),
            "Very Low (needs more data)"
        );
        assert_eq!(ConfidenceLevel::Low.to_string(), "Low");
        assert_eq!(ConfidenceLevel::Medium.to_string(), "Medium");
        assert_eq!(ConfidenceLevel::High.to_string(), "High");
        assert_eq!(
            ConfidenceLevel::VeryHigh.to_string(),
            "Very High (reliable)"
        );
    }

    #[test]
    fn test_confidence_level_ordering() {
        assert!(ConfidenceLevel::VeryHigh > ConfidenceLevel::High);
        assert!(ConfidenceLevel::High > ConfidenceLevel::Medium);
        assert!(ConfidenceLevel::Medium > ConfidenceLevel::Low);
        assert!(ConfidenceLevel::Low > ConfidenceLevel::VeryLow);
    }

    #[test]
    fn test_component_confidence_new_very_low() {
        let conf = ComponentConfidence::new("test", dec!(100), 2);
        assert_eq!(conf.component, "test");
        assert_eq!(conf.raw_score, dec!(100));
        assert_eq!(conf.data_points, 2);
        assert_eq!(conf.confidence, ConfidenceLevel::VeryLow);
        assert_eq!(conf.weight_multiplier, dec!(0.5));
        assert_eq!(conf.adjusted_score, dec!(50)); // 100 * 0.5
        assert!(conf.oldest_data.is_none());
        assert!(conf.newest_data.is_none());
    }

    #[test]
    fn test_component_confidence_new_high() {
        let conf = ComponentConfidence::new("test", dec!(80), 50);
        assert_eq!(conf.raw_score, dec!(80));
        assert_eq!(conf.data_points, 50);
        assert_eq!(conf.confidence, ConfidenceLevel::High);
        assert_eq!(conf.weight_multiplier, dec!(0.95));
        assert_eq!(conf.adjusted_score, dec!(76)); // 80 * 0.95
    }

    #[test]
    fn test_component_confidence_with_timestamps() {
        let oldest = Utc::now() - chrono::Duration::days(30);
        let newest = Utc::now();

        let conf = ComponentConfidence::new("test", dec!(100), 10)
            .with_timestamps(Some(oldest), Some(newest));

        assert!(conf.oldest_data.is_some());
        assert!(conf.newest_data.is_some());
        assert_eq!(conf.oldest_data.unwrap(), oldest);
        assert_eq!(conf.newest_data.unwrap(), newest);
    }

    #[test]
    fn test_confidence_breakdown_calculate_overall_single() {
        let components = vec![ComponentConfidence::new("commitment", dec!(80), 50)];

        let overall = ConfidenceBreakdown::calculate_overall(&components);
        assert_eq!(overall, ConfidenceLevel::High); // 50 data points
    }

    #[test]
    fn test_confidence_breakdown_calculate_overall_multiple() {
        let components = vec![
            ComponentConfidence::new("commitment", dec!(80), 100), // VeryHigh
            ComponentConfidence::new("quality", dec!(70), 50),     // High
            ComponentConfidence::new("community", dec!(90), 20),   // Medium
        ];

        // Average: (100 + 50 + 20) / 3 = 56.67 -> High
        let overall = ConfidenceBreakdown::calculate_overall(&components);
        assert_eq!(overall, ConfidenceLevel::High);
    }

    #[test]
    fn test_confidence_breakdown_calculate_overall_empty() {
        let components: Vec<ComponentConfidence> = vec![];

        let overall = ConfidenceBreakdown::calculate_overall(&components);
        assert_eq!(overall, ConfidenceLevel::VeryLow); // 0 / 1 = 0
    }

    #[test]
    fn test_component_weights_default() {
        let weights = ComponentWeights::default();
        assert_eq!(weights.commitment_fulfillment, dec!(0.35));
        assert_eq!(weights.response_time, dec!(0.15));
        assert_eq!(weights.quality_rating, dec!(0.30));
        assert_eq!(weights.community_trust, dec!(0.15));
        assert_eq!(weights.longevity, dec!(0.05));

        // Verify weights sum to 1.0
        let sum = weights.commitment_fulfillment
            + weights.response_time
            + weights.quality_rating
            + weights.community_trust
            + weights.longevity;
        assert_eq!(sum, dec!(1.0));
    }

    #[test]
    fn test_confidence_adjustment_reduces_score() {
        // With very low confidence, score should be cut in half
        let conf = ComponentConfidence::new("test", dec!(100), 1);
        assert!(conf.adjusted_score < conf.raw_score);
        assert_eq!(conf.adjusted_score, dec!(50)); // 100 * 0.5

        // With very high confidence, score should remain the same
        let conf_high = ComponentConfidence::new("test", dec!(100), 100);
        assert_eq!(conf_high.adjusted_score, conf_high.raw_score);
    }

    #[test]
    fn test_confidence_level_from_str_valid() {
        assert_eq!(
            "very low".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            "low".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::Low
        );
        assert_eq!(
            "medium".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::Medium
        );
        assert_eq!(
            "high".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::High
        );
        assert_eq!(
            "very high".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryHigh
        );
    }

    #[test]
    fn test_confidence_level_from_str_variants() {
        // Test different format variations
        assert_eq!(
            "verylow".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            "very_low".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            "med".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::Medium
        );
        assert_eq!(
            "veryhigh".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryHigh
        );
        assert_eq!(
            "very_high".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryHigh
        );
    }

    #[test]
    fn test_confidence_level_from_str_case_insensitive() {
        assert_eq!(
            "VERY LOW".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryLow
        );
        assert_eq!(
            "Low".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::Low
        );
        assert_eq!(
            "MEDIUM".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::Medium
        );
        assert_eq!(
            "High".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::High
        );
        assert_eq!(
            "VERY HIGH".parse::<ConfidenceLevel>().unwrap(),
            ConfidenceLevel::VeryHigh
        );
    }

    #[test]
    fn test_confidence_level_from_str_invalid() {
        assert!("invalid".parse::<ConfidenceLevel>().is_err());
        assert!("moderate".parse::<ConfidenceLevel>().is_err());
        assert!("".parse::<ConfidenceLevel>().is_err());
    }

    // ========================================================================
    // Property-Based Tests
    // ========================================================================

    mod proptest_confidence {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Property: Confidence level should increase with more data points
            #[test]
            fn prop_confidence_increases_with_data(
                data_points1 in 0i64..=50i64,
                data_points2 in 51i64..=200i64,
            ) {
                let level1 = ConfidenceLevel::from_data_points(data_points1);
                let level2 = ConfidenceLevel::from_data_points(data_points2);

                // More data points should give same or higher confidence
                prop_assert!(level2 >= level1);
            }

            /// Property: Weight multiplier should be between 0.5 and 1.0
            #[test]
            fn prop_weight_multiplier_bounds(data_points in 0i64..=200i64) {
                let level = ConfidenceLevel::from_data_points(data_points);
                let weight = level.weight_multiplier();

                prop_assert!(weight >= dec!(0.5));
                prop_assert!(weight <= dec!(1.0));
            }

            /// Property: Higher confidence should give higher weight multiplier
            #[test]
            fn prop_higher_confidence_higher_weight(
                data_low in 0i64..=5i64,
                data_high in 100i64..=200i64,
            ) {
                let level_low = ConfidenceLevel::from_data_points(data_low);
                let level_high = ConfidenceLevel::from_data_points(data_high);

                let weight_low = level_low.weight_multiplier();
                let weight_high = level_high.weight_multiplier();

                prop_assert!(weight_high >= weight_low);
            }

            /// Property: Adjusted score should never exceed raw score
            #[test]
            fn prop_adjusted_score_bounded(
                raw_score in 0i64..=100i64,
                data_points in 0i64..=200i64,
            ) {
                let conf = ComponentConfidence::new(
                    "test",
                    Decimal::new(raw_score, 0),
                    data_points,
                );

                prop_assert!(conf.adjusted_score <= conf.raw_score);
                prop_assert!(conf.adjusted_score >= dec!(0));
            }

            /// Property: With very high confidence (100+ points), adjusted equals raw
            #[test]
            fn prop_very_high_confidence_no_adjustment(
                raw_score in 0i64..=100i64,
                data_points in 100i64..=200i64,
            ) {
                let conf = ComponentConfidence::new(
                    "test",
                    Decimal::new(raw_score, 0),
                    data_points,
                );

                prop_assert_eq!(conf.adjusted_score, conf.raw_score);
            }

            /// Property: Overall confidence from multiple components should be bounded
            #[test]
            fn prop_overall_confidence_bounded(
                count in 1usize..=10usize,
                data_points in 0i64..=200i64,
            ) {
                let components: Vec<ComponentConfidence> = (0..count)
                    .map(|i| ComponentConfidence::new(format!("comp{}", i), dec!(50), data_points))
                    .collect();

                let overall = ConfidenceBreakdown::calculate_overall(&components);

                // Overall should be one of the valid confidence levels
                prop_assert!(matches!(
                    overall,
                    ConfidenceLevel::VeryLow | ConfidenceLevel::Low | ConfidenceLevel::Medium | ConfidenceLevel::High | ConfidenceLevel::VeryHigh
                ));
            }

            /// Property: More components with same confidence should give same overall
            #[test]
            fn prop_same_confidence_same_overall(
                count1 in 1usize..=5usize,
                count2 in 1usize..=5usize,
                data_points in 50i64..=100i64,
            ) {
                let components1: Vec<ComponentConfidence> = (0..count1)
                    .map(|i| ComponentConfidence::new(format!("comp{}", i), dec!(50), data_points))
                    .collect();

                let components2: Vec<ComponentConfidence> = (0..count2)
                    .map(|i| ComponentConfidence::new(format!("comp{}", i), dec!(50), data_points))
                    .collect();

                let overall1 = ConfidenceBreakdown::calculate_overall(&components1);
                let overall2 = ConfidenceBreakdown::calculate_overall(&components2);

                // Same data points should give same overall confidence
                prop_assert_eq!(overall1, overall2);
            }

            /// Property: Confidence level ordering should be consistent
            #[test]
            fn prop_confidence_level_ordering(
                data1 in 0i64..=50i64,
                data2 in 51i64..=100i64,
                data3 in 101i64..=200i64,
            ) {
                let level1 = ConfidenceLevel::from_data_points(data1);
                let level2 = ConfidenceLevel::from_data_points(data2);
                let level3 = ConfidenceLevel::from_data_points(data3);

                // Ordering should be consistent
                prop_assert!(level1 <= level2);
                prop_assert!(level2 <= level3);
                prop_assert!(level1 <= level3);
            }
        }
    }
}
