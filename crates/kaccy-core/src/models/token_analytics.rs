//! Token analytics and metrics
//!
//! This module provides advanced analytics for tokens including health scores,
//! performance metrics, and market analysis.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{CoreError, Result};

/// Token health score and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenHealthScore {
    /// Overall health score (0-100)
    pub overall_score: Decimal,
    /// Liquidity score (0-100)
    pub liquidity_score: Decimal,
    /// Trading activity score (0-100)
    pub activity_score: Decimal,
    /// Holder distribution score (0-100)
    pub distribution_score: Decimal,
    /// Commitment fulfillment score (0-100)
    pub commitment_score: Decimal,
    /// Health level
    pub health_level: HealthLevel,
    /// Calculated at
    pub calculated_at: DateTime<Utc>,
}

impl TokenHealthScore {
    /// Calculate token health score from various metrics
    pub fn calculate(
        liquidity_ratio: Decimal,
        daily_volume: Decimal,
        holder_count: usize,
        commitment_fulfillment_rate: Decimal,
        days_since_launch: i64,
    ) -> Self {
        // Liquidity score (0-100)
        let liquidity_score = Self::calculate_liquidity_score(liquidity_ratio);

        // Activity score based on volume and age
        let activity_score = Self::calculate_activity_score(daily_volume, days_since_launch);

        // Distribution score based on holder count
        let distribution_score = Self::calculate_distribution_score(holder_count);

        // Commitment score (directly from fulfillment rate)
        let commitment_score = commitment_fulfillment_rate.min(dec!(100));

        // Overall score: weighted average
        let overall_score = (liquidity_score * dec!(0.25))
            + (activity_score * dec!(0.25))
            + (distribution_score * dec!(0.20))
            + (commitment_score * dec!(0.30));

        let health_level = Self::determine_health_level(overall_score);

        Self {
            overall_score,
            liquidity_score,
            activity_score,
            distribution_score,
            commitment_score,
            health_level,
            calculated_at: Utc::now(),
        }
    }

    fn calculate_liquidity_score(liquidity_ratio: Decimal) -> Decimal {
        // Liquidity ratio: liquidity / market cap
        // Good: >20%, Excellent: >50%
        if liquidity_ratio >= dec!(0.5) {
            dec!(100)
        } else {
            liquidity_ratio * dec!(200) // Maps 0-0.5 to 0-100
        }
        .min(dec!(100))
    }

    fn calculate_activity_score(daily_volume: Decimal, days_since_launch: i64) -> Decimal {
        // Higher volume = better, but normalize by age
        let volume_score = if daily_volume > dec!(10000) {
            dec!(100)
        } else if daily_volume > dec!(1000) {
            dec!(50) + (daily_volume / dec!(200))
        } else {
            daily_volume / dec!(20)
        };

        // Penalty for very new tokens (< 7 days)
        let age_factor = if days_since_launch < 7 {
            Decimal::from(days_since_launch) / dec!(7)
        } else {
            dec!(1)
        };

        (volume_score * age_factor).min(dec!(100))
    }

    fn calculate_distribution_score(holder_count: usize) -> Decimal {
        // More holders = better distribution
        // Excellent: 100+ holders
        // Good: 50+ holders
        // Fair: 20+ holders
        if holder_count >= 100 {
            dec!(100)
        } else if holder_count >= 50 {
            dec!(70) + Decimal::from(holder_count - 50) * dec!(0.6)
        } else if holder_count >= 20 {
            dec!(40) + Decimal::from(holder_count - 20)
        } else if holder_count >= 10 {
            dec!(20) + Decimal::from(holder_count - 10) * dec!(2)
        } else {
            Decimal::from(holder_count) * dec!(2)
        }
        .min(dec!(100))
    }

    fn determine_health_level(score: Decimal) -> HealthLevel {
        if score >= dec!(80) {
            HealthLevel::Excellent
        } else if score >= dec!(60) {
            HealthLevel::Good
        } else if score >= dec!(40) {
            HealthLevel::Fair
        } else if score >= dec!(20) {
            HealthLevel::Poor
        } else {
            HealthLevel::Critical
        }
    }
}

/// Token health level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum HealthLevel {
    /// Token is in critical condition requiring immediate attention
    Critical,
    /// Token is in poor health
    Poor,
    /// Token health is acceptable but below average
    Fair,
    /// Token is in good health
    Good,
    /// Token is in excellent health
    Excellent,
}

impl fmt::Display for HealthLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthLevel::Critical => write!(f, "critical"),
            HealthLevel::Poor => write!(f, "poor"),
            HealthLevel::Fair => write!(f, "fair"),
            HealthLevel::Good => write!(f, "good"),
            HealthLevel::Excellent => write!(f, "excellent"),
        }
    }
}

/// Token performance metrics over a period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPerformanceMetrics {
    /// Price change percentage
    pub price_change_pct: Decimal,
    /// Volume change percentage
    pub volume_change_pct: Decimal,
    /// Market cap change percentage
    pub market_cap_change_pct: Decimal,
    /// Holder count change
    pub holder_count_change: i32,
    /// Number of trades
    pub trade_count: usize,
    /// Average trade size
    pub avg_trade_size: Decimal,
    /// Price volatility (standard deviation)
    pub price_volatility: Decimal,
    /// Period start
    pub period_start: DateTime<Utc>,
    /// Period end
    pub period_end: DateTime<Utc>,
}

impl TokenPerformanceMetrics {
    /// Create new performance metrics
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        price_start: Decimal,
        price_end: Decimal,
        volume_start: Decimal,
        volume_end: Decimal,
        market_cap_start: Decimal,
        market_cap_end: Decimal,
        holder_count_start: i32,
        holder_count_end: i32,
        trade_count: usize,
        avg_trade_size: Decimal,
        price_volatility: Decimal,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Self {
        let price_change_pct = if price_start.is_zero() {
            dec!(0)
        } else {
            ((price_end - price_start) / price_start) * dec!(100)
        };

        let volume_change_pct = if volume_start.is_zero() {
            dec!(0)
        } else {
            ((volume_end - volume_start) / volume_start) * dec!(100)
        };

        let market_cap_change_pct = if market_cap_start.is_zero() {
            dec!(0)
        } else {
            ((market_cap_end - market_cap_start) / market_cap_start) * dec!(100)
        };

        Self {
            price_change_pct,
            volume_change_pct,
            market_cap_change_pct,
            holder_count_change: holder_count_end - holder_count_start,
            trade_count,
            avg_trade_size,
            price_volatility,
            period_start,
            period_end,
        }
    }

    /// Check if performance is positive
    pub fn is_positive_performance(&self) -> bool {
        self.price_change_pct > dec!(0)
            && self.volume_change_pct > dec!(0)
            && self.holder_count_change > 0
    }

    /// Get performance grade
    pub fn performance_grade(&self) -> PerformanceGrade {
        let score = (self.price_change_pct * dec!(0.4))
            + (self.volume_change_pct * dec!(0.3))
            + (Decimal::from(self.holder_count_change) * dec!(0.3));

        if score >= dec!(50) {
            PerformanceGrade::Excellent
        } else if score >= dec!(20) {
            PerformanceGrade::Good
        } else if score >= dec!(0) {
            PerformanceGrade::Neutral
        } else if score >= dec!(-20) {
            PerformanceGrade::Poor
        } else {
            PerformanceGrade::VeryPoor
        }
    }
}

/// Performance grade
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceGrade {
    /// Significantly underperforming across all metrics
    VeryPoor,
    /// Underperforming
    Poor,
    /// Flat or mixed performance
    Neutral,
    /// Positive performance across key metrics
    Good,
    /// Strongly outperforming across all metrics
    Excellent,
}

impl fmt::Display for PerformanceGrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PerformanceGrade::VeryPoor => write!(f, "very_poor"),
            PerformanceGrade::Poor => write!(f, "poor"),
            PerformanceGrade::Neutral => write!(f, "neutral"),
            PerformanceGrade::Good => write!(f, "good"),
            PerformanceGrade::Excellent => write!(f, "excellent"),
        }
    }
}

/// Token comparison metrics
pub struct TokenComparator;

impl TokenComparator {
    /// Compare two tokens and generate comparison report
    pub fn compare(
        token_a_health: &TokenHealthScore,
        token_b_health: &TokenHealthScore,
    ) -> TokenComparison {
        TokenComparison {
            overall_score_diff: token_a_health.overall_score - token_b_health.overall_score,
            liquidity_score_diff: token_a_health.liquidity_score - token_b_health.liquidity_score,
            activity_score_diff: token_a_health.activity_score - token_b_health.activity_score,
            distribution_score_diff: token_a_health.distribution_score
                - token_b_health.distribution_score,
            commitment_score_diff: token_a_health.commitment_score
                - token_b_health.commitment_score,
            winner: if token_a_health.overall_score > token_b_health.overall_score {
                ComparisonWinner::TokenA
            } else if token_a_health.overall_score < token_b_health.overall_score {
                ComparisonWinner::TokenB
            } else {
                ComparisonWinner::Tie
            },
        }
    }

    /// Calculate relative strength index (simplified)
    pub fn calculate_rsi(price_changes: &[Decimal]) -> Result<Decimal> {
        if price_changes.is_empty() {
            return Err(CoreError::Validation(
                "Cannot calculate RSI from empty data".to_string(),
            ));
        }

        let mut gains = dec!(0);
        let mut losses = dec!(0);

        for &change in price_changes {
            if change > dec!(0) {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / Decimal::from(price_changes.len());
        let avg_loss = losses / Decimal::from(price_changes.len());

        if avg_loss.is_zero() {
            return Ok(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (dec!(1) + rs));

        Ok(rsi)
    }
}

/// Token comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenComparison {
    /// Difference in overall score (positive means Token A is better)
    pub overall_score_diff: Decimal,
    /// Difference in liquidity score
    pub liquidity_score_diff: Decimal,
    /// Difference in activity score
    pub activity_score_diff: Decimal,
    /// Difference in distribution score
    pub distribution_score_diff: Decimal,
    /// Difference in commitment score
    pub commitment_score_diff: Decimal,
    /// Which token came out ahead overall
    pub winner: ComparisonWinner,
}

/// Comparison winner
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonWinner {
    /// Token A has a higher overall score
    TokenA,
    /// Token B has a higher overall score
    TokenB,
    /// Both tokens have the same overall score
    Tie,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_health_score_excellent() {
        let health = TokenHealthScore::calculate(
            dec!(0.6),   // High liquidity
            dec!(15000), // High volume
            150,         // Many holders
            dec!(95),    // High commitment fulfillment
            30,          // 30 days old
        );

        assert_eq!(health.health_level, HealthLevel::Excellent);
        assert!(health.overall_score >= dec!(80));
    }

    #[test]
    fn test_token_health_score_poor() {
        let health = TokenHealthScore::calculate(
            dec!(0.05), // Low liquidity
            dec!(100),  // Low volume
            5,          // Few holders
            dec!(20),   // Low commitment fulfillment
            2,          // Very new
        );

        assert!(health.overall_score < dec!(40));
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = TokenPerformanceMetrics::new(
            dec!(100),   // start price
            dec!(150),   // end price (+50%)
            dec!(1000),  // start volume
            dec!(2000),  // end volume (+100%)
            dec!(10000), // start market cap
            dec!(15000), // end market cap (+50%)
            10,          // start holders
            25,          // end holders (+15)
            100,         // trades
            dec!(50),    // avg trade size
            dec!(5),     // volatility
            Utc::now(),
            Utc::now(),
        );

        assert_eq!(metrics.price_change_pct, dec!(50));
        assert_eq!(metrics.volume_change_pct, dec!(100));
        assert_eq!(metrics.holder_count_change, 15);
        assert!(metrics.is_positive_performance());
    }

    #[test]
    fn test_performance_grade() {
        let good_metrics = TokenPerformanceMetrics::new(
            dec!(100),
            dec!(130), // +30%
            dec!(1000),
            dec!(1500), // +50%
            dec!(10000),
            dec!(12000),
            10,
            20, // +10
            100,
            dec!(50),
            dec!(5),
            Utc::now(),
            Utc::now(),
        );

        assert_eq!(good_metrics.performance_grade(), PerformanceGrade::Good);
    }

    #[test]
    fn test_token_comparison() {
        let health_a = TokenHealthScore::calculate(dec!(0.5), dec!(10000), 100, dec!(90), 30);

        let health_b = TokenHealthScore::calculate(dec!(0.3), dec!(5000), 50, dec!(70), 30);

        let comparison = TokenComparator::compare(&health_a, &health_b);

        assert_eq!(comparison.winner, ComparisonWinner::TokenA);
        assert!(comparison.overall_score_diff > dec!(0));
    }

    #[test]
    fn test_calculate_rsi() {
        let price_changes = vec![
            dec!(2),
            dec!(3),
            dec!(-1),
            dec!(4),
            dec!(-2),
            dec!(5),
            dec!(1),
            dec!(-3),
        ];

        let rsi = TokenComparator::calculate_rsi(&price_changes).unwrap();

        // RSI should be between 0 and 100
        assert!(rsi >= dec!(0) && rsi <= dec!(100));
    }

    #[test]
    fn test_calculate_rsi_all_gains() {
        let price_changes = vec![dec!(1), dec!(2), dec!(3), dec!(4)];

        let rsi = TokenComparator::calculate_rsi(&price_changes).unwrap();

        // All gains should result in RSI of 100
        assert_eq!(rsi, dec!(100));
    }

    #[test]
    fn test_calculate_rsi_empty() {
        let price_changes = vec![];
        let result = TokenComparator::calculate_rsi(&price_changes);

        assert!(result.is_err());
    }
}
