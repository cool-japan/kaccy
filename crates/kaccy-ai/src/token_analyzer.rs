//! Token value analysis through sentiment and market prediction
//!
//! This module provides experimental AI-powered analysis of:
//! - Social sentiment for tokens
//! - Market trend prediction (experimental)
//! - Community engagement metrics

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{AiError, Result};
use crate::llm::{ChatMessage, ChatRequest, ChatRole, LlmClient};

/// Sentiment score for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentScore {
    /// Overall sentiment (-100 to 100, negative to positive)
    pub score: f64,
    /// Sentiment category
    pub category: SentimentCategory,
    /// Confidence in sentiment analysis (0-100)
    pub confidence: f64,
    /// Sentiment breakdown by source
    pub by_source: HashMap<String, f64>,
    /// Key themes identified
    pub themes: Vec<String>,
    /// Sample mentions analyzed
    pub sample_size: usize,
}

/// Sentiment category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentCategory {
    /// Very negative sentiment
    VeryNegative,
    /// Negative sentiment
    Negative,
    /// Neutral sentiment
    Neutral,
    /// Positive sentiment
    Positive,
    /// Very positive sentiment
    VeryPositive,
}

impl SentimentCategory {
    /// Convert from sentiment score
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < -60.0 => SentimentCategory::VeryNegative,
            s if s < -20.0 => SentimentCategory::Negative,
            s if s < 20.0 => SentimentCategory::Neutral,
            s if s < 60.0 => SentimentCategory::Positive,
            _ => SentimentCategory::VeryPositive,
        }
    }
}

/// Social mention data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialMention {
    /// Platform (Twitter, Reddit, etc.)
    pub platform: String,
    /// Content of the mention
    pub content: String,
    /// Author info (if available)
    pub author: Option<String>,
    /// Engagement metrics (likes, retweets, etc.)
    pub engagement: Option<u32>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Market prediction (experimental)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrediction {
    /// Predicted trend direction
    pub trend: TrendDirection,
    /// Confidence in prediction (0-100)
    pub confidence: f64,
    /// Time horizon for prediction
    pub horizon_days: u32,
    /// Key factors influencing prediction
    pub factors: Vec<String>,
    /// Risk warnings
    pub warnings: Vec<String>,
    /// Disclaimer
    pub disclaimer: String,
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Strong downward trend
    StronglyBearish,
    /// Downward trend
    Bearish,
    /// Sideways/neutral
    Neutral,
    /// Upward trend
    Bullish,
    /// Strong upward trend
    StronglyBullish,
}

/// Community engagement metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityMetrics {
    /// Total mentions in period
    pub total_mentions: usize,
    /// Unique authors
    pub unique_authors: usize,
    /// Average engagement per mention
    pub avg_engagement: f64,
    /// Engagement trend
    pub engagement_trend: TrendDirection,
    /// Top contributors
    pub top_contributors: Vec<String>,
    /// Geographic distribution (if available)
    pub geographic_distribution: HashMap<String, usize>,
}

/// Token analyzer for sentiment and market analysis
pub struct TokenAnalyzer {
    llm: LlmClient,
}

impl TokenAnalyzer {
    /// Create a new token analyzer
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Analyze sentiment from social mentions
    pub async fn analyze_sentiment(
        &self,
        token_name: &str,
        mentions: &[SocialMention],
    ) -> Result<SentimentScore> {
        if mentions.is_empty() {
            return Ok(SentimentScore {
                score: 0.0,
                category: SentimentCategory::Neutral,
                confidence: 0.0,
                by_source: HashMap::new(),
                themes: vec![],
                sample_size: 0,
            });
        }

        let prompt = self.build_sentiment_prompt(token_name, mentions);

        let request = ChatRequest {
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are an expert at analyzing social media sentiment for cryptocurrency tokens. Provide objective, data-driven sentiment analysis.".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(0.3),
            max_tokens: Some(1000),
            stop: None,
            images: None,
        };

        let response = self.llm.chat(request).await?;
        self.parse_sentiment_response(&response.message.content, mentions)
    }

    /// Build sentiment analysis prompt
    fn build_sentiment_prompt(&self, token_name: &str, mentions: &[SocialMention]) -> String {
        let sample = mentions
            .iter()
            .take(20)
            .map(|m| {
                format!(
                    "- [{}] {}: {} (engagement: {})",
                    m.platform,
                    m.author.as_deref().unwrap_or("Anonymous"),
                    m.content,
                    m.engagement.unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Analyze sentiment for token: {}

SOCIAL MENTIONS (sample of {}/{}):
{}

Provide sentiment analysis:
1. Overall sentiment score (-100 to 100, where -100 is very negative and 100 is very positive)
2. Sentiment by platform
3. Key themes and topics discussed
4. Confidence in your analysis (0-100)

Format as JSON:
{{
  "score": <number>,
  "by_source": {{"Twitter": <number>, "Reddit": <number>, ...}},
  "themes": [<strings>],
  "confidence": <number>,
  "reasoning": "<explanation>"
}}
"#,
            token_name,
            mentions.len().min(20),
            mentions.len(),
            sample
        )
    }

    /// Parse sentiment response
    fn parse_sentiment_response(
        &self,
        content: &str,
        mentions: &[SocialMention],
    ) -> Result<SentimentScore> {
        let json_str = content
            .find('{')
            .and_then(|start| {
                content[start..]
                    .rfind('}')
                    .map(|end| &content[start..=(start + end)])
            })
            .ok_or_else(|| AiError::ParseError("No JSON found in response".to_string()))?;

        #[derive(Deserialize)]
        struct ParsedSentiment {
            score: f64,
            by_source: HashMap<String, f64>,
            themes: Vec<String>,
            confidence: f64,
        }

        let parsed: ParsedSentiment = serde_json::from_str(json_str)
            .map_err(|e| AiError::ParseError(format!("Failed to parse sentiment: {e}")))?;

        let score = parsed.score.clamp(-100.0, 100.0);

        Ok(SentimentScore {
            score,
            category: SentimentCategory::from_score(score),
            confidence: parsed.confidence.clamp(0.0, 100.0),
            by_source: parsed.by_source,
            themes: parsed.themes,
            sample_size: mentions.len(),
        })
    }

    /// Predict market trend (experimental)
    pub async fn predict_market_trend(
        &self,
        token_name: &str,
        sentiment: &SentimentScore,
        community_metrics: &CommunityMetrics,
        historical_data: &[HistoricalDataPoint],
    ) -> Result<MarketPrediction> {
        let prompt =
            self.build_prediction_prompt(token_name, sentiment, community_metrics, historical_data);

        let request = ChatRequest {
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are a market analyst. Provide experimental trend predictions with clear disclaimers. This is NOT financial advice.".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(0.4),
            max_tokens: Some(800),
            stop: None,
            images: None,
        };

        let response = self.llm.chat(request).await?;
        self.parse_prediction_response(&response.message.content)
    }

    /// Build market prediction prompt
    fn build_prediction_prompt(
        &self,
        token_name: &str,
        sentiment: &SentimentScore,
        metrics: &CommunityMetrics,
        historical: &[HistoricalDataPoint],
    ) -> String {
        let history_summary = if historical.is_empty() {
            "No historical data available".to_string()
        } else {
            format!(
                "{} data points over {} days",
                historical.len(),
                historical.last().expect("historical is non-empty per is_empty check above").days_ago
            )
        };

        format!(
            r#"Provide an EXPERIMENTAL market trend prediction for token: {}

IMPORTANT: This is experimental analysis only, NOT financial advice.

SENTIMENT DATA:
- Overall Score: {:.1} ({:?})
- Confidence: {:.1}%
- Themes: {}

COMMUNITY METRICS:
- Total Mentions: {}
- Unique Authors: {}
- Engagement Trend: {:?}

HISTORICAL DATA:
{}

Provide prediction including:
1. Trend direction (StronglyBearish, Bearish, Neutral, Bullish, StronglyBullish)
2. Confidence level (0-100)
3. Key factors influencing prediction
4. Risk warnings

Format as JSON:
{{
  "trend": "<StronglyBearish|Bearish|Neutral|Bullish|StronglyBullish>",
  "confidence": <number>,
  "horizon_days": <number>,
  "factors": [<strings>],
  "warnings": [<strings>]
}}
"#,
            token_name,
            sentiment.score,
            sentiment.category,
            sentiment.confidence,
            sentiment.themes.join(", "),
            metrics.total_mentions,
            metrics.unique_authors,
            metrics.engagement_trend,
            history_summary
        )
    }

    /// Parse market prediction response
    fn parse_prediction_response(&self, content: &str) -> Result<MarketPrediction> {
        let json_str = content
            .find('{')
            .and_then(|start| {
                content[start..]
                    .rfind('}')
                    .map(|end| &content[start..=(start + end)])
            })
            .ok_or_else(|| AiError::ParseError("No JSON found in response".to_string()))?;

        #[derive(Deserialize)]
        struct ParsedPrediction {
            trend: String,
            confidence: f64,
            horizon_days: u32,
            factors: Vec<String>,
            warnings: Vec<String>,
        }

        let parsed: ParsedPrediction = serde_json::from_str(json_str)
            .map_err(|e| AiError::ParseError(format!("Failed to parse prediction: {e}")))?;

        let trend = match parsed.trend.as_str() {
            "StronglyBearish" => TrendDirection::StronglyBearish,
            "Bearish" => TrendDirection::Bearish,
            "Neutral" => TrendDirection::Neutral,
            "Bullish" => TrendDirection::Bullish,
            "StronglyBullish" => TrendDirection::StronglyBullish,
            _ => TrendDirection::Neutral,
        };

        Ok(MarketPrediction {
            trend,
            confidence: parsed.confidence.clamp(0.0, 100.0),
            horizon_days: parsed.horizon_days,
            factors: parsed.factors,
            warnings: parsed.warnings,
            disclaimer: "EXPERIMENTAL PREDICTION - NOT FINANCIAL ADVICE. This analysis is for informational purposes only and should not be used as the basis for any investment decisions.".to_string(),
        })
    }

    /// Calculate community engagement metrics
    #[must_use]
    pub fn calculate_community_metrics(&self, mentions: &[SocialMention]) -> CommunityMetrics {
        let unique_authors: std::collections::HashSet<_> =
            mentions.iter().filter_map(|m| m.author.as_ref()).collect();

        let total_engagement: u32 = mentions.iter().filter_map(|m| m.engagement).sum();
        let avg_engagement = if mentions.is_empty() {
            0.0
        } else {
            f64::from(total_engagement) / mentions.len() as f64
        };

        // Simple trend based on recent vs older engagement
        let engagement_trend = if mentions.len() >= 10 {
            let recent_avg: f64 = f64::from(
                mentions
                    .iter()
                    .rev()
                    .take(5)
                    .filter_map(|m| m.engagement)
                    .sum::<u32>(),
            ) / 5.0;

            let older_avg: f64 = f64::from(
                mentions
                    .iter()
                    .take(5)
                    .filter_map(|m| m.engagement)
                    .sum::<u32>(),
            ) / 5.0;

            if recent_avg > older_avg * 1.5 {
                TrendDirection::StronglyBullish
            } else if recent_avg > older_avg * 1.1 {
                TrendDirection::Bullish
            } else if recent_avg < older_avg * 0.5 {
                TrendDirection::StronglyBearish
            } else if recent_avg < older_avg * 0.9 {
                TrendDirection::Bearish
            } else {
                TrendDirection::Neutral
            }
        } else {
            TrendDirection::Neutral
        };

        CommunityMetrics {
            total_mentions: mentions.len(),
            unique_authors: unique_authors.len(),
            avg_engagement,
            engagement_trend,
            top_contributors: vec![],                // Could be enhanced
            geographic_distribution: HashMap::new(), // Would need geo data
        }
    }
}

/// Historical data point for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalDataPoint {
    /// Days ago from now
    pub days_ago: u32,
    /// Sentiment score at that time
    pub sentiment_score: f64,
    /// Mention volume
    pub mention_volume: usize,
    /// Average engagement
    pub avg_engagement: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentiment_category_from_score() {
        assert_eq!(
            SentimentCategory::from_score(-80.0),
            SentimentCategory::VeryNegative
        );
        assert_eq!(
            SentimentCategory::from_score(-30.0),
            SentimentCategory::Negative
        );
        assert_eq!(
            SentimentCategory::from_score(0.0),
            SentimentCategory::Neutral
        );
        assert_eq!(
            SentimentCategory::from_score(40.0),
            SentimentCategory::Positive
        );
        assert_eq!(
            SentimentCategory::from_score(80.0),
            SentimentCategory::VeryPositive
        );
    }

    #[test]
    fn test_community_metrics_calculation() {
        let mentions = vec![
            SocialMention {
                platform: "Twitter".to_string(),
                content: "Great project!".to_string(),
                author: Some("user1".to_string()),
                engagement: Some(10),
                timestamp: chrono::Utc::now(),
            },
            SocialMention {
                platform: "Twitter".to_string(),
                content: "Interesting idea".to_string(),
                author: Some("user2".to_string()),
                engagement: Some(5),
                timestamp: chrono::Utc::now(),
            },
            SocialMention {
                platform: "Reddit".to_string(),
                content: "Love it!".to_string(),
                author: Some("user1".to_string()),
                engagement: Some(15),
                timestamp: chrono::Utc::now(),
            },
        ];

        let analyzer = TokenAnalyzer {
            llm: crate::llm::LlmClient::new(Box::new(crate::llm::OpenAiClient::new(
                "dummy",
                "gpt-4-turbo",
            ))),
        };

        let metrics = analyzer.calculate_community_metrics(&mentions);

        assert_eq!(metrics.total_mentions, 3);
        assert_eq!(metrics.unique_authors, 2);
        assert_eq!(metrics.avg_engagement, 10.0);
    }
}
