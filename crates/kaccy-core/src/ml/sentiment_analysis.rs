//! Sentiment Analysis Module
//!
//! This module provides sentiment analysis capabilities including:
//! - Multi-source sentiment aggregation
//! - Real-time sentiment scores
//! - Sentiment trend analysis
//! - Sentiment divergence detection
//! - Contrarian indicators

use crate::CoreError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sentiment source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SentimentSource {
    /// Social media sentiment (Twitter, Reddit, etc.)
    SocialMedia,
    /// News sentiment
    News,
    /// On-chain metrics
    OnChain,
    /// Market data sentiment
    Market,
    /// Expert analysis
    Expert,
}

/// Sentiment reading from a specific source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentReading {
    /// Source of the sentiment
    pub source: SentimentSource,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Sentiment score (-1.0 to 1.0, where -1 is very bearish, 1 is very bullish)
    pub score: f64,
    /// Confidence in the score (0.0 to 1.0)
    pub confidence: f64,
    /// Volume/weight of the signal
    pub volume: f64,
}

/// Aggregated sentiment score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedSentiment {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Overall sentiment score (-1.0 to 1.0)
    pub score: f64,
    /// Confidence in the score (0.0 to 1.0)
    pub confidence: f64,
    /// Individual source scores
    pub source_scores: HashMap<SentimentSource, f64>,
    /// Sentiment classification
    pub classification: SentimentClassification,
}

/// Sentiment classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentClassification {
    /// Extremely bearish (< -0.6)
    ExtremelyBearish,
    /// Bearish (-0.6 to -0.2)
    Bearish,
    /// Neutral (-0.2 to 0.2)
    Neutral,
    /// Bullish (0.2 to 0.6)
    Bullish,
    /// Extremely bullish (> 0.6)
    ExtremelyBullish,
}

impl SentimentClassification {
    /// Get classification from score
    pub fn from_score(score: f64) -> Self {
        if score < -0.6 {
            Self::ExtremelyBearish
        } else if score < -0.2 {
            Self::Bearish
        } else if score < 0.2 {
            Self::Neutral
        } else if score < 0.6 {
            Self::Bullish
        } else {
            Self::ExtremelyBullish
        }
    }
}

/// Multi-source sentiment aggregator
#[derive(Debug, Clone)]
pub struct SentimentAggregator {
    /// Source weights
    source_weights: HashMap<SentimentSource, f64>,
}

impl Default for SentimentAggregator {
    fn default() -> Self {
        let mut source_weights = HashMap::new();
        source_weights.insert(SentimentSource::SocialMedia, 0.25);
        source_weights.insert(SentimentSource::News, 0.20);
        source_weights.insert(SentimentSource::OnChain, 0.30);
        source_weights.insert(SentimentSource::Market, 0.20);
        source_weights.insert(SentimentSource::Expert, 0.05);

        Self { source_weights }
    }
}

impl SentimentAggregator {
    /// Create a new sentiment aggregator with custom weights
    pub fn new(source_weights: HashMap<SentimentSource, f64>) -> Self {
        Self { source_weights }
    }

    /// Aggregate sentiment from multiple readings
    pub fn aggregate(&self, readings: &[SentimentReading]) -> anyhow::Result<AggregatedSentiment> {
        if readings.is_empty() {
            return Err(CoreError::Validation("No sentiment readings provided".to_string()).into());
        }

        // Group by source and calculate weighted average
        let mut source_scores: HashMap<SentimentSource, Vec<(f64, f64)>> = HashMap::new();

        for reading in readings {
            source_scores
                .entry(reading.source)
                .or_default()
                .push((reading.score, reading.confidence * reading.volume));
        }

        // Calculate average for each source
        let mut source_averages: HashMap<SentimentSource, f64> = HashMap::new();
        for (source, scores) in source_scores.iter() {
            let total_weight: f64 = scores.iter().map(|(_, w)| w).sum();
            let weighted_sum: f64 = scores.iter().map(|(s, w)| s * w).sum();

            if total_weight > 0.0 {
                source_averages.insert(*source, weighted_sum / total_weight);
            }
        }

        // Aggregate across sources using configured weights
        let mut weighted_score = 0.0;
        let mut total_weight = 0.0;

        for (source, score) in source_averages.iter() {
            let weight = self.source_weights.get(source).copied().unwrap_or(0.1);
            weighted_score += score * weight;
            total_weight += weight;
        }

        let final_score = if total_weight > 0.0 {
            (weighted_score / total_weight).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        // Calculate overall confidence
        let avg_confidence =
            readings.iter().map(|r| r.confidence).sum::<f64>() / readings.len() as f64;

        let classification = SentimentClassification::from_score(final_score);

        Ok(AggregatedSentiment {
            timestamp: Utc::now(),
            score: final_score,
            confidence: avg_confidence,
            source_scores: source_averages,
            classification,
        })
    }
}

/// Sentiment trend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentTrend {
    /// Current sentiment
    pub current: f64,
    /// Trend direction
    pub direction: TrendDirection,
    /// Trend strength (0.0 to 1.0)
    pub strength: f64,
    /// Rate of change
    pub rate_of_change: f64,
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Strong downtrend
    StrongDown,
    /// Downtrend
    Down,
    /// Sideways/neutral
    Sideways,
    /// Uptrend
    Up,
    /// Strong uptrend
    StrongUp,
}

/// Sentiment trend analyzer
#[derive(Debug, Clone)]
pub struct SentimentTrendAnalyzer {
    /// Window size for trend analysis
    window_size: usize,
}

impl Default for SentimentTrendAnalyzer {
    fn default() -> Self {
        Self { window_size: 14 }
    }
}

impl SentimentTrendAnalyzer {
    /// Create a new trend analyzer
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }

    /// Analyze sentiment trend
    pub fn analyze(&self, sentiments: &[AggregatedSentiment]) -> anyhow::Result<SentimentTrend> {
        if sentiments.len() < 2 {
            return Err(
                CoreError::Validation("Need at least 2 sentiment readings".to_string()).into(),
            );
        }

        let window = if sentiments.len() > self.window_size {
            &sentiments[sentiments.len() - self.window_size..]
        } else {
            sentiments
        };

        let current = window.last().expect("window is sentiments slice, non-empty per len check above").score;
        let previous = window.first().expect("window is sentiments slice, non-empty per len check above").score;

        // Calculate rate of change
        let rate_of_change = (current - previous) / window.len() as f64;

        // Calculate trend strength using linear regression
        let n = window.len() as f64;
        let sum_x: f64 = (0..window.len()).map(|i| i as f64).sum();
        let sum_y: f64 = window.iter().map(|s| s.score).sum();
        let sum_xy: f64 = window
            .iter()
            .enumerate()
            .map(|(i, s)| i as f64 * s.score)
            .sum();
        let sum_x2: f64 = (0..window.len()).map(|i| (i as f64).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        let strength = slope.abs().min(1.0);

        // Determine direction
        let direction = if slope < -0.05 {
            if slope < -0.15 {
                TrendDirection::StrongDown
            } else {
                TrendDirection::Down
            }
        } else if slope > 0.05 {
            if slope > 0.15 {
                TrendDirection::StrongUp
            } else {
                TrendDirection::Up
            }
        } else {
            TrendDirection::Sideways
        };

        Ok(SentimentTrend {
            current,
            direction,
            strength,
            rate_of_change,
        })
    }
}

/// Sentiment divergence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentDivergence {
    /// Start timestamp
    pub start: DateTime<Utc>,
    /// End timestamp
    pub end: DateTime<Utc>,
    /// Divergence type
    pub divergence_type: SentimentDivergenceType,
    /// Divergence strength (0.0 to 1.0)
    pub strength: f64,
}

/// Divergence type between sentiment and price
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentDivergenceType {
    /// Price rising, sentiment falling (bearish divergence)
    BearishDivergence,
    /// Price falling, sentiment rising (bullish divergence)
    BullishDivergence,
}

/// Sentiment divergence detector
#[derive(Debug, Clone)]
pub struct SentimentDivergenceDetector {
    /// Window size for divergence detection
    window_size: usize,
    /// Minimum divergence strength threshold
    min_strength: f64,
}

impl Default for SentimentDivergenceDetector {
    fn default() -> Self {
        Self {
            window_size: 20,
            min_strength: 0.5,
        }
    }
}

impl SentimentDivergenceDetector {
    /// Create a new divergence detector
    pub fn new(window_size: usize, min_strength: f64) -> Self {
        Self {
            window_size,
            min_strength,
        }
    }

    /// Detect divergences between sentiment and price
    pub fn detect(
        &self,
        sentiments: &[AggregatedSentiment],
        prices: &[Decimal],
    ) -> anyhow::Result<Vec<SentimentDivergence>> {
        if sentiments.len() != prices.len() || sentiments.len() < self.window_size {
            return Ok(Vec::new());
        }

        let mut divergences = Vec::new();

        for i in self.window_size..sentiments.len() {
            let sentiment_window = &sentiments[i - self.window_size..i];
            let price_window = &prices[i - self.window_size..i];

            // Calculate sentiment trend
            let sentiment_start = sentiment_window.first().expect("window length >= window_size > 0").score;
            let sentiment_end = sentiment_window.last().expect("window length >= window_size > 0").score;
            let sentiment_change = sentiment_end - sentiment_start;

            // Calculate price trend
            let price_start = price_window.first().expect("window length >= window_size > 0").to_f64().unwrap_or(0.0);
            let price_end = price_window.last().expect("window length >= window_size > 0").to_f64().unwrap_or(0.0);
            let price_change = (price_end - price_start) / price_start.max(0.0001);

            // Detect divergence
            let (divergence_type, strength) = if price_change > 0.05 && sentiment_change < -0.2 {
                (
                    Some(SentimentDivergenceType::BearishDivergence),
                    (price_change - sentiment_change).abs().min(1.0),
                )
            } else if price_change < -0.05 && sentiment_change > 0.2 {
                (
                    Some(SentimentDivergenceType::BullishDivergence),
                    (price_change.abs() + sentiment_change).min(1.0),
                )
            } else {
                (None, 0.0)
            };

            if let Some(div_type) = divergence_type {
                if strength >= self.min_strength {
                    divergences.push(SentimentDivergence {
                        start: sentiment_window.first().expect("window length >= window_size > 0").timestamp,
                        end: sentiment_window.last().expect("window length >= window_size > 0").timestamp,
                        divergence_type: div_type,
                        strength,
                    });
                }
            }
        }

        Ok(divergences)
    }
}

/// Contrarian indicator signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrarianSignal {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Signal type
    pub signal_type: ContrarianSignalType,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Reasoning
    pub reasoning: Vec<String>,
}

/// Contrarian signal type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContrarianSignalType {
    /// Buy signal (extreme bearishness)
    Buy,
    /// Sell signal (extreme bullishness)
    Sell,
    /// No signal
    Neutral,
}

/// Contrarian indicator analyzer
#[derive(Debug, Clone)]
pub struct ContrarianIndicator {
    /// Extreme sentiment threshold
    extreme_threshold: f64,
    /// Minimum duration in extreme territory
    min_duration: usize,
}

impl Default for ContrarianIndicator {
    fn default() -> Self {
        Self {
            extreme_threshold: 0.7,
            min_duration: 3,
        }
    }
}

impl ContrarianIndicator {
    /// Create a new contrarian indicator
    pub fn new(extreme_threshold: f64, min_duration: usize) -> Self {
        Self {
            extreme_threshold,
            min_duration,
        }
    }

    /// Analyze sentiment for contrarian signals
    pub fn analyze(&self, sentiments: &[AggregatedSentiment]) -> anyhow::Result<ContrarianSignal> {
        if sentiments.len() < self.min_duration {
            return Ok(ContrarianSignal {
                timestamp: Utc::now(),
                signal_type: ContrarianSignalType::Neutral,
                confidence: 0.0,
                reasoning: vec!["Insufficient data".to_string()],
            });
        }

        let recent = &sentiments[sentiments.len() - self.min_duration..];
        let current = recent.last().expect("recent slice has min_duration elements, which is > 0");

        let mut reasoning = Vec::new();

        // Check for extreme bullishness (contrarian sell signal)
        if current.score > self.extreme_threshold {
            let extreme_count = recent
                .iter()
                .filter(|s| s.score > self.extreme_threshold)
                .count();

            if extreme_count >= self.min_duration {
                reasoning.push(format!(
                    "Extreme bullishness: score {:.2} > {:.2}",
                    current.score, self.extreme_threshold
                ));
                reasoning.push(format!("Sustained for {} periods", extreme_count));

                let confidence = ((current.score - self.extreme_threshold)
                    / (1.0 - self.extreme_threshold))
                    .min(1.0)
                    * current.confidence;

                return Ok(ContrarianSignal {
                    timestamp: current.timestamp,
                    signal_type: ContrarianSignalType::Sell,
                    confidence,
                    reasoning,
                });
            }
        }

        // Check for extreme bearishness (contrarian buy signal)
        if current.score < -self.extreme_threshold {
            let extreme_count = recent
                .iter()
                .filter(|s| s.score < -self.extreme_threshold)
                .count();

            if extreme_count >= self.min_duration {
                reasoning.push(format!(
                    "Extreme bearishness: score {:.2} < -{:.2}",
                    current.score, self.extreme_threshold
                ));
                reasoning.push(format!("Sustained for {} periods", extreme_count));

                let confidence = ((current.score.abs() - self.extreme_threshold)
                    / (1.0 - self.extreme_threshold))
                    .min(1.0)
                    * current.confidence;

                return Ok(ContrarianSignal {
                    timestamp: current.timestamp,
                    signal_type: ContrarianSignalType::Buy,
                    confidence,
                    reasoning,
                });
            }
        }

        Ok(ContrarianSignal {
            timestamp: current.timestamp,
            signal_type: ContrarianSignalType::Neutral,
            confidence: 0.0,
            reasoning: vec!["No extreme sentiment detected".to_string()],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_readings() -> Vec<SentimentReading> {
        vec![
            SentimentReading {
                source: SentimentSource::SocialMedia,
                timestamp: Utc::now(),
                score: 0.5,
                confidence: 0.8,
                volume: 1000.0,
            },
            SentimentReading {
                source: SentimentSource::News,
                timestamp: Utc::now(),
                score: 0.3,
                confidence: 0.9,
                volume: 500.0,
            },
            SentimentReading {
                source: SentimentSource::OnChain,
                timestamp: Utc::now(),
                score: 0.6,
                confidence: 0.95,
                volume: 2000.0,
            },
        ]
    }

    #[test]
    fn test_sentiment_aggregator() {
        let readings = create_test_readings();
        let aggregator = SentimentAggregator::default();

        let result = aggregator.aggregate(&readings).unwrap();

        assert!(result.score >= -1.0 && result.score <= 1.0);
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        assert!(result.source_scores.len() >= 2);
        assert!(matches!(
            result.classification,
            SentimentClassification::Bullish | SentimentClassification::Neutral
        ));
    }

    #[test]
    fn test_sentiment_classification() {
        assert_eq!(
            SentimentClassification::from_score(-0.8),
            SentimentClassification::ExtremelyBearish
        );
        assert_eq!(
            SentimentClassification::from_score(-0.4),
            SentimentClassification::Bearish
        );
        assert_eq!(
            SentimentClassification::from_score(0.0),
            SentimentClassification::Neutral
        );
        assert_eq!(
            SentimentClassification::from_score(0.4),
            SentimentClassification::Bullish
        );
        assert_eq!(
            SentimentClassification::from_score(0.8),
            SentimentClassification::ExtremelyBullish
        );
    }

    #[test]
    fn test_sentiment_trend_analyzer() {
        let sentiments: Vec<AggregatedSentiment> = (0..20)
            .map(|i| AggregatedSentiment {
                timestamp: Utc::now(),
                score: -0.5 + (i as f64 * 0.05), // Uptrend from -0.5 to 0.45
                confidence: 0.8,
                source_scores: HashMap::new(),
                classification: SentimentClassification::Neutral,
            })
            .collect();

        let analyzer = SentimentTrendAnalyzer::default();
        let trend = analyzer.analyze(&sentiments).unwrap();

        assert!(matches!(
            trend.direction,
            TrendDirection::Up | TrendDirection::StrongUp
        ));
        assert!(trend.strength > 0.0);
        assert!(trend.rate_of_change > 0.0);
    }

    #[test]
    fn test_sentiment_divergence_detector() {
        let sentiments: Vec<AggregatedSentiment> = (0..30)
            .map(|i| AggregatedSentiment {
                timestamp: Utc::now(),
                score: 0.5 - (i as f64 * 0.03), // Declining sentiment
                confidence: 0.8,
                source_scores: HashMap::new(),
                classification: SentimentClassification::Neutral,
            })
            .collect();

        let prices: Vec<Decimal> = (0..30)
            .map(|i| dec!(100) + Decimal::from(i * 2)) // Rising prices
            .collect();

        let detector = SentimentDivergenceDetector::default();
        let divergences = detector.detect(&sentiments, &prices).unwrap();

        assert!(!divergences.is_empty());
        assert!(
            divergences
                .iter()
                .any(|d| d.divergence_type == SentimentDivergenceType::BearishDivergence)
        );
    }

    #[test]
    fn test_contrarian_indicator() {
        // Extreme bullish sentiment
        let extreme_bullish: Vec<AggregatedSentiment> = (0..5)
            .map(|_| AggregatedSentiment {
                timestamp: Utc::now(),
                score: 0.85,
                confidence: 0.9,
                source_scores: HashMap::new(),
                classification: SentimentClassification::ExtremelyBullish,
            })
            .collect();

        let indicator = ContrarianIndicator::default();
        let signal = indicator.analyze(&extreme_bullish).unwrap();

        assert_eq!(signal.signal_type, ContrarianSignalType::Sell);
        assert!(signal.confidence > 0.0);
        assert!(!signal.reasoning.is_empty());

        // Extreme bearish sentiment
        let extreme_bearish: Vec<AggregatedSentiment> = (0..5)
            .map(|_| AggregatedSentiment {
                timestamp: Utc::now(),
                score: -0.85,
                confidence: 0.9,
                source_scores: HashMap::new(),
                classification: SentimentClassification::ExtremelyBearish,
            })
            .collect();

        let signal2 = indicator.analyze(&extreme_bearish).unwrap();

        assert_eq!(signal2.signal_type, ContrarianSignalType::Buy);
        assert!(signal2.confidence > 0.0);
    }
}
