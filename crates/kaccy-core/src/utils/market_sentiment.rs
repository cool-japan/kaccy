//! Market sentiment analysis system
//!
//! This module provides market sentiment analysis including:
//! - Social sentiment indicators
//! - Fear and greed index
//! - Funding rate analysis
//! - Long/short ratio tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Social sentiment indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialSentiment {
    /// Token symbol
    pub token_symbol: String,

    /// Sentiment score (-100 to 100)
    /// Negative = bearish, Positive = bullish
    pub score: i32,

    /// Mention count across platforms
    pub mention_count: u64,

    /// Positive mention ratio (0.0 to 1.0)
    pub positive_ratio: f64,

    /// Negative mention ratio (0.0 to 1.0)
    pub negative_ratio: f64,

    /// Neutral mention ratio (0.0 to 1.0)
    pub neutral_ratio: f64,

    /// Trending score (0-100)
    pub trending_score: u32,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl SocialSentiment {
    /// Create new social sentiment
    pub fn new(token_symbol: String) -> Self {
        Self {
            token_symbol,
            score: 0,
            mention_count: 0,
            positive_ratio: 0.0,
            negative_ratio: 0.0,
            neutral_ratio: 0.0,
            trending_score: 0,
            updated_at: Utc::now(),
        }
    }

    /// Update sentiment with new mentions
    pub fn update(&mut self, positive: u64, negative: u64, neutral: u64) {
        let total = positive + negative + neutral;

        if total == 0 {
            return;
        }

        self.mention_count += total;
        self.positive_ratio = positive as f64 / total as f64;
        self.negative_ratio = negative as f64 / total as f64;
        self.neutral_ratio = neutral as f64 / total as f64;

        // Calculate score (-100 to 100)
        self.score = ((self.positive_ratio - self.negative_ratio) * 100.0) as i32;

        // Update trending score based on mention growth
        self.trending_score = ((total as f64 / 1000.0) * 100.0).min(100.0) as u32;

        self.updated_at = Utc::now();
    }

    /// Check if sentiment is bullish
    pub fn is_bullish(&self) -> bool {
        self.score > 20
    }

    /// Check if sentiment is bearish
    pub fn is_bearish(&self) -> bool {
        self.score < -20
    }

    /// Get sentiment category
    pub fn category(&self) -> SentimentCategory {
        match self.score {
            s if s > 50 => SentimentCategory::ExtremelyBullish,
            s if s > 20 => SentimentCategory::Bullish,
            s if s > -20 => SentimentCategory::Neutral,
            s if s > -50 => SentimentCategory::Bearish,
            _ => SentimentCategory::ExtremelyBearish,
        }
    }
}

/// Sentiment category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentCategory {
    /// Strongly bullish sentiment (score > 50)
    ExtremelyBullish,
    /// Moderately bullish sentiment (score 20–50)
    Bullish,
    /// Neutral sentiment (score −20 to 20)
    Neutral,
    /// Moderately bearish sentiment (score −50 to −20)
    Bearish,
    /// Strongly bearish sentiment (score < −50)
    ExtremelyBearish,
}

/// Fear and Greed Index (0-100)
/// 0-24: Extreme Fear
/// 25-49: Fear
/// 50: Neutral
/// 51-75: Greed
/// 76-100: Extreme Greed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearAndGreedIndex {
    /// Current index value (0-100)
    pub value: u32,

    /// Individual component scores
    pub components: FearGreedComponents,

    /// Index category
    pub category: FearGreedCategory,

    /// 24h change
    pub change_24h: i32,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Fear and Greed components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FearGreedComponents {
    /// Market volatility (0-100)
    pub volatility: u32,

    /// Market volume (0-100)
    pub volume: u32,

    /// Social media sentiment (0-100)
    pub social: u32,

    /// Market dominance (0-100)
    pub dominance: u32,

    /// Trends (0-100)
    pub trends: u32,
}

/// Fear and Greed category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FearGreedCategory {
    /// Index 0–24: extreme fear in the market
    ExtremeFear,
    /// Index 25–49: fear in the market
    Fear,
    /// Index 50: neutral sentiment
    Neutral,
    /// Index 51–75: greed in the market
    Greed,
    /// Index 76–100: extreme greed in the market
    ExtremeGreed,
}

impl FearAndGreedIndex {
    /// Create new Fear and Greed Index
    pub fn new() -> Self {
        Self {
            value: 50,
            components: FearGreedComponents {
                volatility: 50,
                volume: 50,
                social: 50,
                dominance: 50,
                trends: 50,
            },
            category: FearGreedCategory::Neutral,
            change_24h: 0,
            updated_at: Utc::now(),
        }
    }

    /// Calculate index from components
    pub fn calculate(&mut self) {
        // Weighted average of components
        let weighted = (self.components.volatility * 25
            + self.components.volume * 25
            + self.components.social * 20
            + self.components.dominance * 15
            + self.components.trends * 15)
            / 100;

        let old_value = self.value;
        self.value = weighted;

        self.change_24h = self.value as i32 - old_value as i32;

        // Determine category
        self.category = match self.value {
            0..=24 => FearGreedCategory::ExtremeFear,
            25..=49 => FearGreedCategory::Fear,
            50 => FearGreedCategory::Neutral,
            51..=75 => FearGreedCategory::Greed,
            76..=100 => FearGreedCategory::ExtremeGreed,
            _ => FearGreedCategory::Neutral,
        };

        self.updated_at = Utc::now();
    }

    /// Update volatility component
    pub fn update_volatility(&mut self, volatility_pct: Decimal) {
        // Higher volatility = more fear
        // Normalize volatility to 0-100 scale (assume 0-200% volatility range)
        let normalized = (dec!(100) - (volatility_pct / dec!(2)).min(dec!(100)))
            .to_u32()
            .unwrap_or(50);
        self.components.volatility = normalized;
    }

    /// Update volume component
    pub fn update_volume(&mut self, volume_change_pct: Decimal) {
        // Higher volume increase = more greed
        let normalized = (dec!(50) + volume_change_pct / dec!(2))
            .clamp(dec!(0), dec!(100))
            .to_u32()
            .unwrap_or(50);
        self.components.volume = normalized;
    }

    /// Update social component
    pub fn update_social(&mut self, social_score: i32) {
        // Convert -100 to 100 social score to 0-100 scale
        self.components.social = ((social_score + 100) / 2).clamp(0, 100) as u32;
    }

    /// Check if market is in extreme fear
    pub fn is_extreme_fear(&self) -> bool {
        matches!(self.category, FearGreedCategory::ExtremeFear)
    }

    /// Check if market is in extreme greed
    pub fn is_extreme_greed(&self) -> bool {
        matches!(self.category, FearGreedCategory::ExtremeGreed)
    }
}

impl Default for FearAndGreedIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Funding rate analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateAnalysis {
    /// Token symbol
    pub token_symbol: String,

    /// Current funding rate (%)
    pub current_rate: Decimal,

    /// 24h average funding rate (%)
    pub avg_24h: Decimal,

    /// 7d average funding rate (%)
    pub avg_7d: Decimal,

    /// Historical rates (timestamp, rate)
    pub history: Vec<(DateTime<Utc>, Decimal)>,

    /// Funding rate trend
    pub trend: FundingTrend,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Funding rate trend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingTrend {
    /// Funding rate increasing (more longs)
    Increasing,

    /// Funding rate decreasing (more shorts)
    Decreasing,

    /// Funding rate stable
    Stable,
}

impl FundingRateAnalysis {
    /// Create new funding rate analysis
    pub fn new(token_symbol: String) -> Self {
        Self {
            token_symbol,
            current_rate: Decimal::ZERO,
            avg_24h: Decimal::ZERO,
            avg_7d: Decimal::ZERO,
            history: Vec::new(),
            trend: FundingTrend::Stable,
            updated_at: Utc::now(),
        }
    }

    /// Add funding rate data point
    pub fn add_rate(&mut self, rate: Decimal) {
        self.current_rate = rate;
        self.history.push((Utc::now(), rate));

        // Keep only last 7 days of history
        let cutoff = Utc::now() - chrono::Duration::days(7);
        self.history.retain(|(timestamp, _)| *timestamp > cutoff);

        // Calculate averages
        self.calculate_averages();

        // Determine trend
        self.determine_trend();

        self.updated_at = Utc::now();
    }

    fn calculate_averages(&mut self) {
        let now = Utc::now();
        let day_ago = now - chrono::Duration::hours(24);
        let week_ago = now - chrono::Duration::days(7);

        // 24h average
        let rates_24h: Vec<_> = self
            .history
            .iter()
            .filter(|(timestamp, _)| *timestamp > day_ago)
            .map(|(_, rate)| rate)
            .collect();

        if !rates_24h.is_empty() {
            self.avg_24h =
                rates_24h.iter().copied().sum::<Decimal>() / Decimal::from(rates_24h.len());
        }

        // 7d average
        let rates_7d: Vec<_> = self
            .history
            .iter()
            .filter(|(timestamp, _)| *timestamp > week_ago)
            .map(|(_, rate)| rate)
            .collect();

        if !rates_7d.is_empty() {
            self.avg_7d = rates_7d.iter().copied().sum::<Decimal>() / Decimal::from(rates_7d.len());
        }
    }

    fn determine_trend(&mut self) {
        if self.history.len() < 10 {
            self.trend = FundingTrend::Stable;
            return;
        }

        // Compare recent rates with older rates
        let recent_avg: Decimal = self
            .history
            .iter()
            .rev()
            .take(5)
            .map(|(_, rate)| rate)
            .sum::<Decimal>()
            / dec!(5);

        let older_avg: Decimal = self
            .history
            .iter()
            .rev()
            .skip(5)
            .take(5)
            .map(|(_, rate)| rate)
            .sum::<Decimal>()
            / dec!(5);

        let diff = recent_avg - older_avg;

        self.trend = if diff > dec!(0.0001) {
            FundingTrend::Increasing
        } else if diff < dec!(-0.0001) {
            FundingTrend::Decreasing
        } else {
            FundingTrend::Stable
        };
    }

    /// Check if funding rate is extremely positive (bullish)
    pub fn is_extremely_positive(&self) -> bool {
        self.current_rate > dec!(0.001) // > 0.1% per funding interval
    }

    /// Check if funding rate is extremely negative (bearish)
    pub fn is_extremely_negative(&self) -> bool {
        self.current_rate < dec!(-0.001) // < -0.1% per funding interval
    }
}

/// Long/Short ratio tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongShortRatio {
    /// Token symbol
    pub token_symbol: String,

    /// Current long/short ratio
    pub ratio: Decimal,

    /// Long percentage
    pub long_percentage: Decimal,

    /// Short percentage
    pub short_percentage: Decimal,

    /// Historical ratios
    pub history: Vec<(DateTime<Utc>, Decimal)>,

    /// Sentiment based on ratio
    pub sentiment: RatioSentiment,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Long/Short ratio sentiment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RatioSentiment {
    /// Extremely bullish (> 2.0)
    ExtremelyBullish,

    /// Bullish (> 1.3)
    Bullish,

    /// Neutral (0.7 - 1.3)
    Neutral,

    /// Bearish (< 0.7)
    Bearish,

    /// Extremely bearish (< 0.5)
    ExtremelyBearish,
}

impl LongShortRatio {
    /// Create new long/short ratio tracker
    pub fn new(token_symbol: String) -> Self {
        Self {
            token_symbol,
            ratio: dec!(1.0),
            long_percentage: dec!(50),
            short_percentage: dec!(50),
            history: Vec::new(),
            sentiment: RatioSentiment::Neutral,
            updated_at: Utc::now(),
        }
    }

    /// Update ratio with new data
    pub fn update(&mut self, long_count: u64, short_count: u64) {
        if short_count == 0 {
            self.ratio = dec!(10); // Cap at 10:1
        } else {
            self.ratio = (Decimal::from(long_count) / Decimal::from(short_count)).min(dec!(10));
        }

        let total = long_count + short_count;
        if total > 0 {
            self.long_percentage = Decimal::from(long_count) / Decimal::from(total) * dec!(100);
            self.short_percentage = Decimal::from(short_count) / Decimal::from(total) * dec!(100);
        }

        self.history.push((Utc::now(), self.ratio));

        // Keep only last 7 days
        let cutoff = Utc::now() - chrono::Duration::days(7);
        self.history.retain(|(timestamp, _)| *timestamp > cutoff);

        // Determine sentiment
        self.sentiment = if self.ratio > dec!(2.0) {
            RatioSentiment::ExtremelyBullish
        } else if self.ratio > dec!(1.3) {
            RatioSentiment::Bullish
        } else if self.ratio > dec!(0.7) {
            RatioSentiment::Neutral
        } else if self.ratio > dec!(0.5) {
            RatioSentiment::Bearish
        } else {
            RatioSentiment::ExtremelyBearish
        };

        self.updated_at = Utc::now();
    }

    /// Check if longs are dominant
    pub fn longs_dominant(&self) -> bool {
        self.ratio > dec!(1.3)
    }

    /// Check if shorts are dominant
    pub fn shorts_dominant(&self) -> bool {
        self.ratio < dec!(0.7)
    }
}

/// Market sentiment aggregator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSentimentAggregator {
    /// Social sentiments by token
    pub social_sentiments: HashMap<String, SocialSentiment>,

    /// Fear and Greed Index
    pub fear_greed: FearAndGreedIndex,

    /// Funding rates by token
    pub funding_rates: HashMap<String, FundingRateAnalysis>,

    /// Long/Short ratios by token
    pub long_short_ratios: HashMap<String, LongShortRatio>,
}

impl MarketSentimentAggregator {
    /// Create new market sentiment aggregator
    pub fn new() -> Self {
        Self {
            social_sentiments: HashMap::new(),
            fear_greed: FearAndGreedIndex::new(),
            funding_rates: HashMap::new(),
            long_short_ratios: HashMap::new(),
        }
    }

    /// Get overall sentiment for a token
    pub fn get_overall_sentiment(&self, token: &str) -> OverallSentiment {
        let mut bullish_signals = 0;
        let mut bearish_signals = 0;

        // Social sentiment
        if let Some(social) = self.social_sentiments.get(token) {
            if social.is_bullish() {
                bullish_signals += 1;
            } else if social.is_bearish() {
                bearish_signals += 1;
            }
        }

        // Funding rate
        if let Some(funding) = self.funding_rates.get(token) {
            if funding.is_extremely_positive() {
                bullish_signals += 1;
            } else if funding.is_extremely_negative() {
                bearish_signals += 1;
            }
        }

        // Long/Short ratio
        if let Some(ratio) = self.long_short_ratios.get(token) {
            if ratio.longs_dominant() {
                bullish_signals += 1;
            } else if ratio.shorts_dominant() {
                bearish_signals += 1;
            }
        }

        OverallSentiment {
            token: token.to_string(),
            bullish_signals,
            bearish_signals,
            category: if bullish_signals > bearish_signals * 2 {
                SentimentCategory::ExtremelyBullish
            } else if bullish_signals > bearish_signals {
                SentimentCategory::Bullish
            } else if bearish_signals > bullish_signals * 2 {
                SentimentCategory::ExtremelyBearish
            } else if bearish_signals > bullish_signals {
                SentimentCategory::Bearish
            } else {
                SentimentCategory::Neutral
            },
        }
    }
}

impl Default for MarketSentimentAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Overall sentiment for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallSentiment {
    /// Token symbol this sentiment belongs to
    pub token: String,
    /// Number of bullish signals from different indicators
    pub bullish_signals: u32,
    /// Number of bearish signals from different indicators
    pub bearish_signals: u32,
    /// Aggregated sentiment category
    pub category: SentimentCategory,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_social_sentiment() {
        let mut sentiment = SocialSentiment::new("BTC".to_string());

        sentiment.update(70, 20, 10);

        assert!(sentiment.is_bullish());
        assert_eq!(sentiment.category(), SentimentCategory::Bullish);
    }

    #[test]
    fn test_fear_and_greed() {
        let mut index = FearAndGreedIndex::new();

        index.components.volatility = 20; // High volatility = fear
        index.components.volume = 30;
        index.components.social = 25;
        index.components.dominance = 20;
        index.components.trends = 25;

        index.calculate();

        assert!(index.value < 50);
        assert!(matches!(
            index.category,
            FearGreedCategory::Fear | FearGreedCategory::ExtremeFear
        ));
    }

    #[test]
    fn test_funding_rate() {
        let mut funding = FundingRateAnalysis::new("BTC".to_string());

        // Add enough data points to determine trend (need at least 10)
        for i in 1..=15 {
            funding.add_rate(dec!(0.0005) + Decimal::from(i) * dec!(0.0001));
        }

        assert!(funding.is_extremely_positive());
        assert_eq!(funding.trend, FundingTrend::Increasing);
    }

    #[test]
    fn test_long_short_ratio() {
        let mut ratio = LongShortRatio::new("BTC".to_string());

        ratio.update(700, 300); // 70% long, 30% short

        assert!(ratio.longs_dominant());
        assert!(matches!(
            ratio.sentiment,
            RatioSentiment::Bullish | RatioSentiment::ExtremelyBullish
        ));
    }

    #[test]
    fn test_market_sentiment_aggregator() {
        let mut aggregator = MarketSentimentAggregator::new();

        let mut social = SocialSentiment::new("BTC".to_string());
        social.update(80, 20, 0);
        aggregator
            .social_sentiments
            .insert("BTC".to_string(), social);

        let mut ratio = LongShortRatio::new("BTC".to_string());
        ratio.update(700, 300);
        aggregator
            .long_short_ratios
            .insert("BTC".to_string(), ratio);

        let sentiment = aggregator.get_overall_sentiment("BTC");
        assert!(sentiment.bullish_signals > 0);
    }
}
