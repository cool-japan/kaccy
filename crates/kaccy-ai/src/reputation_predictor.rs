//! Reputation prediction for future commitment success
//!
//! This module provides AI-powered prediction of:
//! - Likelihood of commitment completion
//! - Risk scoring for new token issuers
//! - Historical pattern analysis for reputation forecasting

use serde::{Deserialize, Serialize};

use crate::error::{AiError, Result};
use crate::llm::{ChatMessage, ChatRequest, ChatRole, LlmClient};

/// Historical commitment record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalCommitment {
    /// Commitment ID
    pub id: String,
    /// Commitment description
    pub description: String,
    /// Target amount
    pub target_amount: u64,
    /// Actual amount delivered
    pub delivered_amount: Option<u64>,
    /// Was it completed?
    pub completed: bool,
    /// Days to complete (if completed)
    pub days_to_complete: Option<u32>,
    /// Quality score (0-100)
    pub quality_score: Option<f64>,
    /// Verification evidence quality (0-100)
    pub evidence_quality: Option<f64>,
}

/// Issuer history for prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerHistory {
    /// Issuer identifier
    pub issuer_id: String,
    /// Total commitments made
    pub total_commitments: usize,
    /// Completed commitments
    pub completed_commitments: usize,
    /// Average completion time (days)
    pub avg_completion_days: Option<f64>,
    /// Average quality score
    pub avg_quality_score: Option<f64>,
    /// Historical commitments
    pub commitments: Vec<HistoricalCommitment>,
    /// Days since first commitment
    pub days_active: u32,
    /// Reputation tier (from kaccy-reputation)
    pub reputation_tier: String,
    /// Current reputation score
    pub reputation_score: f64,
}

impl IssuerHistory {
    /// Calculate completion rate
    #[must_use]
    pub fn completion_rate(&self) -> f64 {
        if self.total_commitments == 0 {
            0.0
        } else {
            (self.completed_commitments as f64 / self.total_commitments as f64) * 100.0
        }
    }

    /// Get recent trend (last 5 commitments)
    #[must_use]
    pub fn recent_trend(&self) -> Trend {
        if self.commitments.len() < 2 {
            return Trend::Neutral;
        }

        let recent: Vec<_> = self.commitments.iter().rev().take(5).collect();

        let recent_completed = recent.iter().filter(|c| c.completed).count();
        let recent_rate = recent_completed as f64 / recent.len() as f64;

        let overall_rate = self.completion_rate() / 100.0;

        if recent_rate > overall_rate + 0.2 {
            Trend::Improving
        } else if recent_rate < overall_rate - 0.2 {
            Trend::Declining
        } else {
            Trend::Neutral
        }
    }
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    /// Performance improving
    Improving,
    /// Performance stable
    Neutral,
    /// Performance declining
    Declining,
}

/// Prediction result for commitment success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentPrediction {
    /// Probability of completion (0-100)
    pub completion_probability: f64,
    /// Expected quality if completed (0-100)
    pub expected_quality: f64,
    /// Expected days to completion
    pub expected_days: Option<u32>,
    /// Risk factors identified
    pub risk_factors: Vec<String>,
    /// Positive indicators
    pub positive_indicators: Vec<String>,
    /// Confidence in prediction (0-100)
    pub confidence: f64,
    /// Reasoning for prediction
    pub reasoning: String,
}

/// Risk assessment for new issuer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerRiskAssessment {
    /// Overall risk score (0-100, higher = more risky)
    pub risk_score: f64,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Recommended initial commitment limit
    pub recommended_limit: Option<u64>,
    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Mitigation recommendations
    pub mitigations: Vec<String>,
    /// Confidence in assessment (0-100)
    pub confidence: f64,
}

/// Risk level for issuer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Very low risk
    VeryLow,
    /// Low risk
    Low,
    /// Medium risk
    Medium,
    /// High risk
    High,
    /// Very high risk
    VeryHigh,
}

impl RiskLevel {
    /// Convert from risk score
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 20.0 => RiskLevel::VeryLow,
            s if s < 40.0 => RiskLevel::Low,
            s if s < 60.0 => RiskLevel::Medium,
            s if s < 80.0 => RiskLevel::High,
            _ => RiskLevel::VeryHigh,
        }
    }
}

/// Individual risk factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// Factor name
    pub name: String,
    /// Description
    pub description: String,
    /// Severity (0-100)
    pub severity: f64,
    /// Impact on overall risk
    pub impact: Impact,
}

/// Impact level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Impact {
    /// Low impact
    Low,
    /// Medium impact
    Medium,
    /// High impact
    High,
}

/// Reputation predictor
pub struct ReputationPredictor {
    llm: LlmClient,
}

impl ReputationPredictor {
    /// Create a new reputation predictor
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Predict commitment success probability
    pub async fn predict_commitment_success(
        &self,
        commitment: &str,
        target_amount: u64,
        issuer_history: &IssuerHistory,
    ) -> Result<CommitmentPrediction> {
        let prompt = self.build_prediction_prompt(commitment, target_amount, issuer_history);

        let request = ChatRequest {
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are an expert at predicting project success and commitment completion based on historical data. Analyze the issuer's history and provide a detailed prediction.".to_string(),
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
        self.parse_prediction_response(&response.message.content, issuer_history)
    }

    /// Build prediction prompt
    fn build_prediction_prompt(
        &self,
        commitment: &str,
        target_amount: u64,
        history: &IssuerHistory,
    ) -> String {
        format!(
            r#"Analyze this commitment and predict its success probability:

COMMITMENT: {}
TARGET AMOUNT: {} sats

ISSUER HISTORY:
- Total Commitments: {}
- Completed: {} ({:.1}% completion rate)
- Average Completion Time: {} days
- Average Quality Score: {:.1}/100
- Recent Trend: {:?}
- Reputation Tier: {}
- Reputation Score: {:.1}
- Days Active: {}

RECENT COMMITMENTS:
{}

Provide a structured prediction including:
1. Completion probability (0-100%)
2. Expected quality if completed (0-100)
3. Expected days to completion
4. Risk factors (bullet points)
5. Positive indicators (bullet points)
6. Detailed reasoning

Format your response as JSON with these fields:
{{
  "completion_probability": <number>,
  "expected_quality": <number>,
  "expected_days": <number or null>,
  "risk_factors": [<strings>],
  "positive_indicators": [<strings>],
  "reasoning": "<detailed explanation>"
}}
"#,
            commitment,
            target_amount,
            history.total_commitments,
            history.completed_commitments,
            history.completion_rate(),
            history
                .avg_completion_days
                .map_or_else(|| "N/A".to_string(), |d| format!("{d:.1}")),
            history.avg_quality_score.unwrap_or(0.0),
            history.recent_trend(),
            history.reputation_tier,
            history.reputation_score,
            history.days_active,
            self.format_recent_commitments(&history.commitments),
        )
    }

    /// Format recent commitments for prompt
    fn format_recent_commitments(&self, commitments: &[HistoricalCommitment]) -> String {
        commitments
            .iter()
            .rev()
            .take(5)
            .map(|c| {
                format!(
                    "- {} (Target: {} sats, Completed: {}, Quality: {:.0}/100)",
                    c.description,
                    c.target_amount,
                    if c.completed { "Yes" } else { "No" },
                    c.quality_score.unwrap_or(0.0)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Parse prediction response
    fn parse_prediction_response(
        &self,
        content: &str,
        _history: &IssuerHistory,
    ) -> Result<CommitmentPrediction> {
        // Try to parse JSON from response
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
            completion_probability: f64,
            expected_quality: f64,
            expected_days: Option<u32>,
            risk_factors: Vec<String>,
            positive_indicators: Vec<String>,
            reasoning: String,
        }

        let parsed: ParsedPrediction = serde_json::from_str(json_str)
            .map_err(|e| AiError::ParseError(format!("Failed to parse prediction: {e}")))?;

        Ok(CommitmentPrediction {
            completion_probability: parsed.completion_probability.clamp(0.0, 100.0),
            expected_quality: parsed.expected_quality.clamp(0.0, 100.0),
            expected_days: parsed.expected_days,
            risk_factors: parsed.risk_factors,
            positive_indicators: parsed.positive_indicators,
            confidence: 75.0, // Base confidence, can be improved with more data
            reasoning: parsed.reasoning,
        })
    }

    /// Assess risk for new issuer
    pub async fn assess_new_issuer_risk(
        &self,
        issuer_info: &NewIssuerInfo,
    ) -> Result<IssuerRiskAssessment> {
        let prompt = self.build_risk_assessment_prompt(issuer_info);

        let request = ChatRequest {
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: "You are an expert at assessing risk for new token issuers. Analyze the provided information and identify potential risks.".to_string(),
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
        self.parse_risk_assessment_response(&response.message.content, issuer_info)
    }

    /// Build risk assessment prompt
    fn build_risk_assessment_prompt(&self, info: &NewIssuerInfo) -> String {
        format!(
            r#"Assess the risk for this new token issuer:

ISSUER INFORMATION:
- Has GitHub: {}
- GitHub Repos: {}
- GitHub Followers: {}
- Has Twitter: {}
- Twitter Followers: {}
- Has LinkedIn: {}
- Years of Experience: {}
- Previous Projects: {}
- Token Description: {}
- Requested Initial Limit: {} sats

Provide a comprehensive risk assessment including:
1. Overall risk score (0-100, higher = more risky)
2. Specific risk factors with severity
3. Recommended initial commitment limit
4. Mitigation strategies

Format as JSON:
{{
  "risk_score": <number>,
  "risk_factors": [
    {{"name": "<string>", "description": "<string>", "severity": <number>, "impact": "Low|Medium|High"}}
  ],
  "recommended_limit": <number or null>,
  "mitigations": [<strings>],
  "reasoning": "<detailed explanation>"
}}
"#,
            info.has_github,
            info.github_repos.unwrap_or(0),
            info.github_followers.unwrap_or(0),
            info.has_twitter,
            info.twitter_followers.unwrap_or(0),
            info.has_linkedin,
            info.years_experience.unwrap_or(0),
            info.previous_projects.len(),
            info.token_description,
            info.requested_limit,
        )
    }

    /// Parse risk assessment response
    fn parse_risk_assessment_response(
        &self,
        content: &str,
        _info: &NewIssuerInfo,
    ) -> Result<IssuerRiskAssessment> {
        let json_str = content
            .find('{')
            .and_then(|start| {
                content[start..]
                    .rfind('}')
                    .map(|end| &content[start..=(start + end)])
            })
            .ok_or_else(|| AiError::ParseError("No JSON found in response".to_string()))?;

        #[derive(Deserialize)]
        struct ParsedRisk {
            risk_score: f64,
            risk_factors: Vec<ParsedRiskFactor>,
            recommended_limit: Option<u64>,
            mitigations: Vec<String>,
        }

        #[derive(Deserialize)]
        struct ParsedRiskFactor {
            name: String,
            description: String,
            severity: f64,
            impact: String,
        }

        let parsed: ParsedRisk = serde_json::from_str(json_str)
            .map_err(|e| AiError::ParseError(format!("Failed to parse risk assessment: {e}")))?;

        let risk_factors = parsed
            .risk_factors
            .into_iter()
            .map(|f| RiskFactor {
                name: f.name,
                description: f.description,
                severity: f.severity.clamp(0.0, 100.0),
                impact: match f.impact.to_lowercase().as_str() {
                    "high" => Impact::High,
                    "medium" => Impact::Medium,
                    _ => Impact::Low,
                },
            })
            .collect();

        let risk_score = parsed.risk_score.clamp(0.0, 100.0);

        Ok(IssuerRiskAssessment {
            risk_score,
            risk_level: RiskLevel::from_score(risk_score),
            recommended_limit: parsed.recommended_limit,
            risk_factors,
            mitigations: parsed.mitigations,
            confidence: 70.0,
        })
    }
}

/// Information about a new issuer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewIssuerInfo {
    /// Token description
    pub token_description: String,
    /// Requested initial limit
    pub requested_limit: u64,
    /// Has GitHub profile
    pub has_github: bool,
    /// Number of GitHub repos
    pub github_repos: Option<u32>,
    /// GitHub followers
    pub github_followers: Option<u32>,
    /// Has Twitter profile
    pub has_twitter: bool,
    /// Twitter followers
    pub twitter_followers: Option<u32>,
    /// Has `LinkedIn` profile
    pub has_linkedin: bool,
    /// Years of professional experience
    pub years_experience: Option<u32>,
    /// Previous projects
    pub previous_projects: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issuer_history_completion_rate() {
        let history = IssuerHistory {
            issuer_id: "test".to_string(),
            total_commitments: 10,
            completed_commitments: 8,
            avg_completion_days: Some(15.0),
            avg_quality_score: Some(85.0),
            commitments: vec![],
            days_active: 100,
            reputation_tier: "Silver".to_string(),
            reputation_score: 75.0,
        };

        assert_eq!(history.completion_rate(), 80.0);
    }

    #[test]
    fn test_risk_level_from_score() {
        assert_eq!(RiskLevel::from_score(10.0), RiskLevel::VeryLow);
        assert_eq!(RiskLevel::from_score(30.0), RiskLevel::Low);
        assert_eq!(RiskLevel::from_score(50.0), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_score(70.0), RiskLevel::High);
        assert_eq!(RiskLevel::from_score(90.0), RiskLevel::VeryHigh);
    }

    #[test]
    fn test_trend_detection() {
        let history = IssuerHistory {
            issuer_id: "test".to_string(),
            total_commitments: 3,
            completed_commitments: 3,
            avg_completion_days: Some(15.0),
            avg_quality_score: Some(85.0),
            commitments: vec![
                HistoricalCommitment {
                    id: "1".to_string(),
                    description: "Test 1".to_string(),
                    target_amount: 1000,
                    delivered_amount: Some(1000),
                    completed: true,
                    days_to_complete: Some(10),
                    quality_score: Some(80.0),
                    evidence_quality: Some(90.0),
                },
                HistoricalCommitment {
                    id: "2".to_string(),
                    description: "Test 2".to_string(),
                    target_amount: 1000,
                    delivered_amount: Some(1000),
                    completed: true,
                    days_to_complete: Some(12),
                    quality_score: Some(85.0),
                    evidence_quality: Some(92.0),
                },
                HistoricalCommitment {
                    id: "3".to_string(),
                    description: "Test 3".to_string(),
                    target_amount: 1000,
                    delivered_amount: Some(1000),
                    completed: true,
                    days_to_complete: Some(8),
                    quality_score: Some(90.0),
                    evidence_quality: Some(95.0),
                },
            ],
            days_active: 100,
            reputation_tier: "Silver".to_string(),
            reputation_score: 75.0,
        };

        // All completed means overall rate is 100%, recent is also 100%, should be neutral
        let trend = history.recent_trend();
        assert_eq!(trend, Trend::Neutral);
    }
}
