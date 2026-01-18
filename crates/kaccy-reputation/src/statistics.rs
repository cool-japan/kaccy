//! Statistical analysis module for reputation data
//!
//! This module provides advanced statistical methods for analyzing reputation
//! patterns, trends, and distributions. It includes:
//!
//! - Trend analysis (moving averages, momentum, volatility)
//! - Statistical aggregations (mean, median, mode, variance, standard deviation)
//! - Percentile and quartile calculations
//! - Correlation analysis between score components
//! - Outlier detection using z-scores and IQR
//!
//! ## Example
//!
//! ```rust
//! use kaccy_reputation::*;
//! use rust_decimal_macros::dec;
//!
//! // Calculate statistical measures
//! let scores = vec![dec!(100), dec!(200), dec!(300), dec!(400), dec!(500)];
//!
//! let mean = Statistics::mean(&scores);
//! let median = Statistics::median(&scores);
//! let std_dev = Statistics::std_dev(&scores);
//!
//! println!("Mean: {}, Median: {}, StdDev: {}", mean, median, std_dev);
//!
//! // Check for outliers
//! let is_outlier = Statistics::is_outlier_zscore(dec!(1000), mean, std_dev);
//! println!("Is 1000 an outlier? {}", is_outlier);
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Statistical distribution of reputation scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreStatistics {
    /// Mean (average) score
    pub mean: Decimal,
    /// Median score
    pub median: Decimal,
    /// Mode (most common score range)
    pub mode: Option<Decimal>,
    /// Standard deviation
    pub std_dev: Decimal,
    /// Variance
    pub variance: Decimal,
    /// Minimum score
    pub min: Decimal,
    /// Maximum score
    pub max: Decimal,
    /// Total count
    pub count: i64,
}

impl ScoreStatistics {
    /// Calculate coefficient of variation (relative standard deviation)
    pub fn coefficient_of_variation(&self) -> Decimal {
        if self.mean == dec!(0) {
            dec!(0)
        } else {
            (self.std_dev / self.mean) * dec!(100)
        }
    }

    /// Get the range (max - min)
    pub fn range(&self) -> Decimal {
        self.max - self.min
    }

    /// Check if distribution is highly variable (CV > 30%)
    pub fn is_highly_variable(&self) -> bool {
        self.coefficient_of_variation() > dec!(30)
    }
}

/// Quartile values for score distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quartiles {
    /// First quartile (25th percentile)
    pub q1: Decimal,
    /// Second quartile (50th percentile - median)
    pub q2: Decimal,
    /// Third quartile (75th percentile)
    pub q3: Decimal,
    /// Interquartile range (Q3 - Q1)
    pub iqr: Decimal,
}

impl Quartiles {
    /// Calculate lower fence for outlier detection (Q1 - 1.5 * IQR)
    pub fn lower_fence(&self) -> Decimal {
        self.q1 - (dec!(1.5) * self.iqr)
    }

    /// Calculate upper fence for outlier detection (Q3 + 1.5 * IQR)
    pub fn upper_fence(&self) -> Decimal {
        self.q3 + (dec!(1.5) * self.iqr)
    }

    /// Check if a score is an outlier
    pub fn is_outlier(&self, score: Decimal) -> bool {
        score < self.lower_fence() || score > self.upper_fence()
    }
}

/// Percentile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Percentiles {
    /// 10th percentile
    pub p10: Decimal,
    /// 25th percentile
    pub p25: Decimal,
    /// 50th percentile (median)
    pub p50: Decimal,
    /// 75th percentile
    pub p75: Decimal,
    /// 90th percentile
    pub p90: Decimal,
    /// 95th percentile
    pub p95: Decimal,
    /// 99th percentile
    pub p99: Decimal,
}

/// Trend analysis for score changes over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// User ID
    pub user_id: Uuid,
    /// Current score
    pub current_score: Decimal,
    /// Score 7 days ago
    pub score_7d_ago: Option<Decimal>,
    /// Score 30 days ago
    pub score_30d_ago: Option<Decimal>,
    /// Score 90 days ago
    pub score_90d_ago: Option<Decimal>,
    /// 7-day change
    pub change_7d: Option<Decimal>,
    /// 30-day change
    pub change_30d: Option<Decimal>,
    /// 90-day change
    pub change_90d: Option<Decimal>,
    /// Average daily change (30-day)
    pub avg_daily_change: Option<Decimal>,
    /// Momentum (positive, negative, or neutral)
    pub momentum: TrendMomentum,
}

/// Trend momentum indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TrendMomentum {
    /// Strong positive momentum (>10 points/week)
    StrongPositive,
    /// Positive momentum (1-10 points/week)
    Positive,
    /// Neutral momentum (-1 to 1 points/week)
    Neutral,
    /// Negative momentum (-10 to -1 points/week)
    Negative,
    /// Strong negative momentum (<-10 points/week)
    StrongNegative,
}

impl TrendMomentum {
    /// Calculate momentum from weekly change
    pub fn from_weekly_change(change: Decimal) -> Self {
        if change > dec!(10) {
            TrendMomentum::StrongPositive
        } else if change > dec!(1) {
            TrendMomentum::Positive
        } else if change >= dec!(-1) {
            TrendMomentum::Neutral
        } else if change >= dec!(-10) {
            TrendMomentum::Negative
        } else {
            TrendMomentum::StrongNegative
        }
    }

    /// Check if momentum is positive
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            TrendMomentum::Positive | TrendMomentum::StrongPositive
        )
    }

    /// Check if momentum is negative
    pub fn is_negative(&self) -> bool {
        matches!(
            self,
            TrendMomentum::Negative | TrendMomentum::StrongNegative
        )
    }

    /// Check if momentum is neutral
    pub fn is_neutral(&self) -> bool {
        matches!(self, TrendMomentum::Neutral)
    }
}

impl std::fmt::Display for TrendMomentum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendMomentum::StrongPositive => write!(f, "strong_positive"),
            TrendMomentum::Positive => write!(f, "positive"),
            TrendMomentum::Neutral => write!(f, "neutral"),
            TrendMomentum::Negative => write!(f, "negative"),
            TrendMomentum::StrongNegative => write!(f, "strong_negative"),
        }
    }
}

/// Component correlation analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentCorrelation {
    /// Component A name
    pub component_a: String,
    /// Component B name
    pub component_b: String,
    /// Correlation coefficient (-1 to 1)
    pub correlation: Decimal,
    /// Sample size
    pub sample_size: i64,
}

impl ComponentCorrelation {
    /// Check if correlation is strong (|r| > 0.7)
    pub fn is_strong(&self) -> bool {
        self.correlation.abs() > dec!(0.7)
    }

    /// Check if correlation is moderate (0.3 < |r| <= 0.7)
    pub fn is_moderate(&self) -> bool {
        let abs_corr = self.correlation.abs();
        abs_corr > dec!(0.3) && abs_corr <= dec!(0.7)
    }

    /// Check if correlation is weak (|r| <= 0.3)
    pub fn is_weak(&self) -> bool {
        self.correlation.abs() <= dec!(0.3)
    }

    /// Get correlation strength description
    pub fn strength(&self) -> &'static str {
        if self.is_strong() {
            "strong"
        } else if self.is_moderate() {
            "moderate"
        } else {
            "weak"
        }
    }
}

/// Moving average calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAverage {
    /// User ID
    pub user_id: Uuid,
    /// Simple moving average (30-day)
    pub sma_30d: Decimal,
    /// Exponential moving average (30-day)
    pub ema_30d: Decimal,
    /// Current score
    pub current_score: Decimal,
    /// Timestamp
    pub calculated_at: DateTime<Utc>,
}

impl MovingAverage {
    /// Check if current score is above SMA (bullish signal)
    pub fn is_above_sma(&self) -> bool {
        self.current_score > self.sma_30d
    }

    /// Check if current score is below SMA (bearish signal)
    pub fn is_below_sma(&self) -> bool {
        self.current_score < self.sma_30d
    }

    /// Calculate distance from SMA as percentage
    pub fn distance_from_sma_pct(&self) -> Decimal {
        if self.sma_30d == dec!(0) {
            dec!(0)
        } else {
            ((self.current_score - self.sma_30d) / self.sma_30d) * dec!(100)
        }
    }
}

/// Volatility metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreVolatility {
    /// User ID
    pub user_id: Uuid,
    /// Standard deviation of daily changes (30-day)
    pub daily_volatility: Decimal,
    /// Average absolute daily change
    pub avg_abs_change: Decimal,
    /// Maximum positive daily change
    pub max_gain: Decimal,
    /// Maximum negative daily change
    pub max_loss: Decimal,
    /// Number of positive days
    pub positive_days: i32,
    /// Number of negative days
    pub negative_days: i32,
    /// Period (days)
    pub period_days: i32,
}

impl ScoreVolatility {
    /// Check if score is highly volatile (daily volatility > 5)
    pub fn is_highly_volatile(&self) -> bool {
        self.daily_volatility > dec!(5)
    }

    /// Calculate win rate (positive days / total days)
    pub fn win_rate(&self) -> Decimal {
        let total_days = self.positive_days + self.negative_days;
        if total_days == 0 {
            dec!(0)
        } else {
            (Decimal::from(self.positive_days) / Decimal::from(total_days)) * dec!(100)
        }
    }

    /// Get volatility level description
    pub fn volatility_level(&self) -> &'static str {
        if self.daily_volatility < dec!(2) {
            "low"
        } else if self.daily_volatility < dec!(5) {
            "moderate"
        } else {
            "high"
        }
    }
}

/// Statistical helper functions
pub struct Statistics;

impl Statistics {
    /// Calculate mean of decimal values
    pub fn mean(values: &[Decimal]) -> Decimal {
        if values.is_empty() {
            return dec!(0);
        }
        let sum: Decimal = values.iter().sum();
        sum / Decimal::from(values.len())
    }

    /// Calculate median of decimal values
    pub fn median(values: &[Decimal]) -> Decimal {
        if values.is_empty() {
            return dec!(0);
        }
        let mut sorted = values.to_vec();
        sorted.sort();
        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / dec!(2)
        } else {
            sorted[mid]
        }
    }

    /// Calculate variance of decimal values
    pub fn variance(values: &[Decimal]) -> Decimal {
        if values.len() < 2 {
            return dec!(0);
        }
        let mean = Self::mean(values);
        let sum_squared_diff: Decimal = values
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum();
        sum_squared_diff / Decimal::from(values.len() - 1)
    }

    /// Calculate standard deviation of decimal values
    pub fn std_dev(values: &[Decimal]) -> Decimal {
        let variance = Self::variance(values);
        // Approximate square root using Newton's method
        Self::sqrt(variance)
    }

    /// Calculate approximate square root using Newton's method
    pub fn sqrt(value: Decimal) -> Decimal {
        if value <= dec!(0) {
            return dec!(0);
        }

        let mut x = value / dec!(2);
        let epsilon = dec!(0.0001);

        for _ in 0..20 {
            let next = (x + value / x) / dec!(2);
            if (next - x).abs() < epsilon {
                return next;
            }
            x = next;
        }
        x
    }

    /// Calculate percentile (0-100)
    pub fn percentile(values: &[Decimal], p: i32) -> Decimal {
        if values.is_empty() {
            return dec!(0);
        }
        let mut sorted = values.to_vec();
        sorted.sort();

        let index = ((Decimal::from(p) / dec!(100)) * Decimal::from(sorted.len() - 1))
            .round()
            .to_string()
            .parse::<usize>()
            .unwrap_or(0);

        sorted[index.min(sorted.len() - 1)]
    }

    /// Calculate z-score for a value
    pub fn z_score(value: Decimal, mean: Decimal, std_dev: Decimal) -> Decimal {
        if std_dev == dec!(0) {
            dec!(0)
        } else {
            (value - mean) / std_dev
        }
    }

    /// Check if value is an outlier using z-score (|z| > 3)
    pub fn is_outlier_zscore(value: Decimal, mean: Decimal, std_dev: Decimal) -> bool {
        let z = Self::z_score(value, mean, std_dev);
        z.abs() > dec!(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_statistics_mean() {
        let values = vec![dec!(10), dec!(20), dec!(30), dec!(40), dec!(50)];
        assert_eq!(Statistics::mean(&values), dec!(30));
    }

    #[test]
    fn test_statistics_median_odd() {
        let values = vec![dec!(10), dec!(30), dec!(20)];
        assert_eq!(Statistics::median(&values), dec!(20));
    }

    #[test]
    fn test_statistics_median_even() {
        let values = vec![dec!(10), dec!(20), dec!(30), dec!(40)];
        assert_eq!(Statistics::median(&values), dec!(25));
    }

    #[test]
    fn test_statistics_variance() {
        let values = vec![dec!(2), dec!(4), dec!(6), dec!(8), dec!(10)];
        let variance = Statistics::variance(&values);
        assert!(variance > dec!(0));
    }

    #[test]
    fn test_statistics_std_dev() {
        let values = vec![dec!(2), dec!(4), dec!(6), dec!(8), dec!(10)];
        let std_dev = Statistics::std_dev(&values);
        assert!(std_dev > dec!(0));
    }

    #[test]
    fn test_statistics_sqrt() {
        assert_eq!(Statistics::sqrt(dec!(0)), dec!(0));

        let sqrt1 = Statistics::sqrt(dec!(1));
        assert!((sqrt1 - dec!(1)).abs() < dec!(0.001));

        let sqrt4 = Statistics::sqrt(dec!(4));
        assert!((sqrt4 - dec!(2)).abs() < dec!(0.01));

        let sqrt9 = Statistics::sqrt(dec!(9));
        assert!((sqrt9 - dec!(3)).abs() < dec!(0.01));
    }

    #[test]
    fn test_statistics_percentile() {
        let values = vec![
            dec!(10),
            dec!(20),
            dec!(30),
            dec!(40),
            dec!(50),
            dec!(60),
            dec!(70),
            dec!(80),
            dec!(90),
            dec!(100),
        ];

        assert_eq!(Statistics::percentile(&values, 50), dec!(50));
        assert!(Statistics::percentile(&values, 25) <= dec!(30));
        assert!(Statistics::percentile(&values, 75) >= dec!(70));
    }

    #[test]
    fn test_score_statistics_coefficient_of_variation() {
        let dist = ScoreStatistics {
            mean: dec!(100),
            median: dec!(95),
            mode: Some(dec!(90)),
            std_dev: dec!(20),
            variance: dec!(400),
            min: dec!(50),
            max: dec!(150),
            count: 100,
        };

        assert_eq!(dist.coefficient_of_variation(), dec!(20));
        assert!(!dist.is_highly_variable());
    }

    #[test]
    fn test_score_statistics_range() {
        let dist = ScoreStatistics {
            mean: dec!(100),
            median: dec!(95),
            mode: Some(dec!(90)),
            std_dev: dec!(20),
            variance: dec!(400),
            min: dec!(50),
            max: dec!(150),
            count: 100,
        };

        assert_eq!(dist.range(), dec!(100));
    }

    #[test]
    fn test_quartiles_fences() {
        let quartiles = Quartiles {
            q1: dec!(25),
            q2: dec!(50),
            q3: dec!(75),
            iqr: dec!(50),
        };

        assert_eq!(quartiles.lower_fence(), dec!(-50));
        assert_eq!(quartiles.upper_fence(), dec!(150));
        assert!(quartiles.is_outlier(dec!(200)));
        assert!(!quartiles.is_outlier(dec!(60)));
    }

    #[test]
    fn test_trend_momentum_calculation() {
        assert_eq!(
            TrendMomentum::from_weekly_change(dec!(15)),
            TrendMomentum::StrongPositive
        );
        assert_eq!(
            TrendMomentum::from_weekly_change(dec!(5)),
            TrendMomentum::Positive
        );
        assert_eq!(
            TrendMomentum::from_weekly_change(dec!(0)),
            TrendMomentum::Neutral
        );
        assert_eq!(
            TrendMomentum::from_weekly_change(dec!(-5)),
            TrendMomentum::Negative
        );
        assert_eq!(
            TrendMomentum::from_weekly_change(dec!(-15)),
            TrendMomentum::StrongNegative
        );
    }

    #[test]
    fn test_trend_momentum_checks() {
        assert!(TrendMomentum::StrongPositive.is_positive());
        assert!(!TrendMomentum::StrongPositive.is_negative());
        assert!(TrendMomentum::Neutral.is_neutral());
        assert!(TrendMomentum::Negative.is_negative());
    }

    #[test]
    fn test_component_correlation_strength() {
        let strong_corr = ComponentCorrelation {
            component_a: "commitment".to_string(),
            component_b: "quality".to_string(),
            correlation: dec!(0.85),
            sample_size: 100,
        };

        assert!(strong_corr.is_strong());
        assert!(!strong_corr.is_moderate());
        assert_eq!(strong_corr.strength(), "strong");
    }

    #[test]
    fn test_moving_average_signals() {
        let ma = MovingAverage {
            user_id: Uuid::new_v4(),
            sma_30d: dec!(500),
            ema_30d: dec!(510),
            current_score: dec!(550),
            calculated_at: Utc::now(),
        };

        assert!(ma.is_above_sma());
        assert!(!ma.is_below_sma());
        assert_eq!(ma.distance_from_sma_pct(), dec!(10));
    }

    #[test]
    fn test_score_volatility_metrics() {
        let volatility = ScoreVolatility {
            user_id: Uuid::new_v4(),
            daily_volatility: dec!(3.5),
            avg_abs_change: dec!(2.5),
            max_gain: dec!(15),
            max_loss: dec!(-10),
            positive_days: 18,
            negative_days: 12,
            period_days: 30,
        };

        assert!(!volatility.is_highly_volatile());
        assert_eq!(volatility.volatility_level(), "moderate");
        assert_eq!(volatility.win_rate(), dec!(60));
    }

    #[test]
    fn test_statistics_empty_values() {
        let empty: Vec<Decimal> = vec![];
        assert_eq!(Statistics::mean(&empty), dec!(0));
        assert_eq!(Statistics::median(&empty), dec!(0));
        assert_eq!(Statistics::percentile(&empty, 50), dec!(0));
    }

    #[test]
    fn test_statistics_z_score() {
        let z = Statistics::z_score(dec!(110), dec!(100), dec!(10));
        assert_eq!(z, dec!(1));

        assert!(!Statistics::is_outlier_zscore(
            dec!(110),
            dec!(100),
            dec!(10)
        ));
        assert!(Statistics::is_outlier_zscore(
            dec!(140),
            dec!(100),
            dec!(10)
        ));
    }
}
