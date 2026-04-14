//! Market sentiment analysis
//!
//! This module provides tools for analyzing market sentiment based on trading activity,
//! price movements, and order book data.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Market sentiment indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentIndicator {
    /// Overall sentiment score (-100 to +100)
    pub sentiment_score: Decimal,
    /// Sentiment classification
    pub sentiment: Sentiment,
    /// Buy pressure (0-100)
    pub buy_pressure: Decimal,
    /// Sell pressure (0-100)
    pub sell_pressure: Decimal,
    /// Order book imbalance (-1 to +1)
    pub order_book_imbalance: Decimal,
    /// Price momentum score
    pub price_momentum: Decimal,
    /// Calculated at
    pub calculated_at: DateTime<Utc>,
}

impl SentimentIndicator {
    /// Calculate market sentiment from various indicators
    pub fn calculate(
        buy_volume: Decimal,
        sell_volume: Decimal,
        bid_depth: Decimal,
        ask_depth: Decimal,
        price_changes: &[Decimal],
    ) -> Self {
        let total_volume = buy_volume + sell_volume;

        // Buy/sell pressure (0-100)
        let buy_pressure = if total_volume.is_zero() {
            dec!(50)
        } else {
            (buy_volume / total_volume) * dec!(100)
        };

        let sell_pressure = dec!(100) - buy_pressure;

        // Order book imbalance (-1 to +1)
        let total_depth = bid_depth + ask_depth;
        let order_book_imbalance = if total_depth.is_zero() {
            dec!(0)
        } else {
            (bid_depth - ask_depth) / total_depth
        };

        // Price momentum from recent changes
        let price_momentum = Self::calculate_momentum(price_changes);

        // Overall sentiment score (-100 to +100)
        let sentiment_score = (buy_pressure - sell_pressure) * dec!(0.4)
            + (order_book_imbalance * dec!(50))
            + (price_momentum * dec!(0.6));

        let sentiment = Self::classify_sentiment(sentiment_score);

        Self {
            sentiment_score,
            sentiment,
            buy_pressure,
            sell_pressure,
            order_book_imbalance,
            price_momentum,
            calculated_at: Utc::now(),
        }
    }

    fn calculate_momentum(price_changes: &[Decimal]) -> Decimal {
        if price_changes.is_empty() {
            return dec!(0);
        }

        // Recent changes weighted more heavily
        let mut weighted_sum = dec!(0);
        let mut weight_total = dec!(0);

        for (i, &change) in price_changes.iter().enumerate() {
            let weight = Decimal::from(i + 1); // More recent = higher weight
            weighted_sum += change * weight;
            weight_total += weight;
        }

        if weight_total.is_zero() {
            dec!(0)
        } else {
            (weighted_sum / weight_total).max(dec!(-100)).min(dec!(100))
        }
    }

    fn classify_sentiment(score: Decimal) -> Sentiment {
        if score >= dec!(50) {
            Sentiment::StronglyBullish
        } else if score >= dec!(20) {
            Sentiment::Bullish
        } else if score >= dec!(-20) {
            Sentiment::Neutral
        } else if score >= dec!(-50) {
            Sentiment::Bearish
        } else {
            Sentiment::StronglyBearish
        }
    }

    /// Check if sentiment is positive
    pub fn is_bullish(&self) -> bool {
        matches!(
            self.sentiment,
            Sentiment::Bullish | Sentiment::StronglyBullish
        )
    }

    /// Check if sentiment is negative
    pub fn is_bearish(&self) -> bool {
        matches!(
            self.sentiment,
            Sentiment::Bearish | Sentiment::StronglyBearish
        )
    }
}

/// Market sentiment classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Sentiment {
    /// Strongly negative market sentiment (score < −50)
    StronglyBearish,
    /// Moderately negative market sentiment (score −50 to −20)
    Bearish,
    /// Balanced market sentiment (score −20 to 20)
    Neutral,
    /// Moderately positive market sentiment (score 20 to 50)
    Bullish,
    /// Strongly positive market sentiment (score ≥ 50)
    StronglyBullish,
}

impl fmt::Display for Sentiment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sentiment::StronglyBearish => write!(f, "strongly_bearish"),
            Sentiment::Bearish => write!(f, "bearish"),
            Sentiment::Neutral => write!(f, "neutral"),
            Sentiment::Bullish => write!(f, "bullish"),
            Sentiment::StronglyBullish => write!(f, "strongly_bullish"),
        }
    }
}

/// Fear and greed index (0-100)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearGreedIndex {
    /// Index value (0 = extreme fear, 100 = extreme greed)
    pub value: Decimal,
    /// Classification
    pub classification: FearGreedClassification,
    /// Contributing factors
    pub factors: FearGreedFactors,
    /// Calculated at
    pub calculated_at: DateTime<Utc>,
}

impl FearGreedIndex {
    /// Calculate fear and greed index
    pub fn calculate(
        price_momentum: Decimal,
        volume_momentum: Decimal,
        market_volatility: Decimal,
        social_sentiment: Decimal,
    ) -> Self {
        // Normalize inputs to 0-100
        let price_component = ((price_momentum + dec!(100)) / dec!(2))
            .max(dec!(0))
            .min(dec!(100));
        let volume_component = ((volume_momentum + dec!(100)) / dec!(2))
            .max(dec!(0))
            .min(dec!(100));
        let volatility_component = (dec!(100) - market_volatility.min(dec!(100))).max(dec!(0));
        let social_component = social_sentiment.max(dec!(0)).min(dec!(100));

        // Weighted average
        let value = (price_component * dec!(0.25))
            + (volume_component * dec!(0.25))
            + (volatility_component * dec!(0.25))
            + (social_component * dec!(0.25));

        let classification = Self::classify(value);

        Self {
            value,
            classification,
            factors: FearGreedFactors {
                price_momentum: price_component,
                volume_momentum: volume_component,
                market_volatility: volatility_component,
                social_sentiment: social_component,
            },
            calculated_at: Utc::now(),
        }
    }

    fn classify(value: Decimal) -> FearGreedClassification {
        if value >= dec!(75) {
            FearGreedClassification::ExtremeGreed
        } else if value >= dec!(55) {
            FearGreedClassification::Greed
        } else if value >= dec!(45) {
            FearGreedClassification::Neutral
        } else if value >= dec!(25) {
            FearGreedClassification::Fear
        } else {
            FearGreedClassification::ExtremeFear
        }
    }
}

/// Fear and greed classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FearGreedClassification {
    /// Index 0–24: extreme fear dominates the market
    ExtremeFear,
    /// Index 25–44: fear is present
    Fear,
    /// Index 45–54: sentiment is balanced
    Neutral,
    /// Index 55–74: greed is present
    Greed,
    /// Index 75–100: extreme greed dominates the market
    ExtremeGreed,
}

impl fmt::Display for FearGreedClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FearGreedClassification::ExtremeFear => write!(f, "extreme_fear"),
            FearGreedClassification::Fear => write!(f, "fear"),
            FearGreedClassification::Neutral => write!(f, "neutral"),
            FearGreedClassification::Greed => write!(f, "greed"),
            FearGreedClassification::ExtremeGreed => write!(f, "extreme_greed"),
        }
    }
}

/// Contributing factors to fear and greed index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearGreedFactors {
    /// Normalized price momentum component (0–100)
    pub price_momentum: Decimal,
    /// Normalized volume momentum component (0–100)
    pub volume_momentum: Decimal,
    /// Normalized market volatility component (0–100, higher = calmer)
    pub market_volatility: Decimal,
    /// Normalized social sentiment component (0–100)
    pub social_sentiment: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_bullish() {
        let sentiment = SentimentIndicator::calculate(
            dec!(8000),                   // High buy volume
            dec!(2000),                   // Low sell volume
            dec!(10000),                  // High bid depth
            dec!(3000),                   // Low ask depth
            &[dec!(2), dec!(3), dec!(5)], // Positive price changes
        );

        assert!(sentiment.is_bullish());
        assert!(!sentiment.is_bearish());
        assert!(sentiment.buy_pressure > dec!(70));
    }

    #[test]
    fn test_sentiment_bearish() {
        let sentiment = SentimentIndicator::calculate(
            dec!(2000),                      // Low buy volume
            dec!(8000),                      // High sell volume
            dec!(3000),                      // Low bid depth
            dec!(10000),                     // High ask depth
            &[dec!(-2), dec!(-3), dec!(-5)], // Negative price changes
        );

        assert!(sentiment.is_bearish());
        assert!(!sentiment.is_bullish());
        assert!(sentiment.sell_pressure > dec!(70));
    }

    #[test]
    fn test_sentiment_neutral() {
        let sentiment = SentimentIndicator::calculate(
            dec!(5000),
            dec!(5000),
            dec!(5000),
            dec!(5000),
            &[dec!(0), dec!(1), dec!(-1)],
        );

        assert_eq!(sentiment.sentiment, Sentiment::Neutral);
    }

    #[test]
    fn test_fear_greed_extreme_greed() {
        let index = FearGreedIndex::calculate(
            dec!(80), // High price momentum
            dec!(75), // High volume momentum
            dec!(10), // Low volatility
            dec!(90), // Positive social sentiment
        );

        assert_eq!(index.classification, FearGreedClassification::ExtremeGreed);
        assert!(index.value > dec!(70));
    }

    #[test]
    fn test_fear_greed_extreme_fear() {
        let index = FearGreedIndex::calculate(
            dec!(-80), // Negative price momentum
            dec!(-75), // Negative volume momentum
            dec!(90),  // High volatility
            dec!(10),  // Negative social sentiment
        );

        assert_eq!(index.classification, FearGreedClassification::ExtremeFear);
        assert!(index.value < dec!(30));
    }

    #[test]
    fn test_fear_greed_neutral() {
        let index = FearGreedIndex::calculate(
            dec!(0),  // Neutral price momentum
            dec!(0),  // Neutral volume momentum
            dec!(50), // Moderate volatility
            dec!(50), // Neutral social sentiment
        );

        assert_eq!(index.classification, FearGreedClassification::Neutral);
    }

    #[test]
    fn test_sentiment_momentum_empty() {
        let sentiment =
            SentimentIndicator::calculate(dec!(5000), dec!(5000), dec!(5000), dec!(5000), &[]);

        assert_eq!(sentiment.price_momentum, dec!(0));
    }

    #[test]
    fn test_sentiment_zero_volume() {
        let sentiment =
            SentimentIndicator::calculate(dec!(0), dec!(0), dec!(5000), dec!(5000), &[dec!(1)]);

        assert_eq!(sentiment.buy_pressure, dec!(50));
        assert_eq!(sentiment.sell_pressure, dec!(50));
    }
}
