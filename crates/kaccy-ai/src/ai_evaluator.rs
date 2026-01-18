//! AI-powered quality evaluation using LLM providers
//!
//! This module uses OpenAI/Anthropic to evaluate:
//! - Code quality and correctness
//! - Content quality and clarity
//! - Commitment evidence verification
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_ai::AiEvaluator;
//! use kaccy_ai::llm::{LlmClient, OpenAiClient};
//! use kaccy_ai::evaluator::QualityEvaluator;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an LLM client
//! let openai = OpenAiClient::with_default_model("your-api-key");
//! let llm_client = LlmClient::new(Box::new(openai));
//!
//! // Create an AI evaluator
//! let evaluator = AiEvaluator::new(llm_client);
//!
//! // Evaluate code quality
//! let code = "fn hello() { println!(\"Hello\"); }";
//! let result = evaluator.evaluate_code(code, "rust").await?;
//! println!("Quality: {}", result.quality_score);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{AiError, Result};
use crate::evaluator::{EvaluationResult, QualityEvaluator};
use crate::llm::{ChatRequest, LlmClient};

/// AI-powered quality evaluator
pub struct AiEvaluator {
    llm: LlmClient,
    config: EvaluatorConfig,
}

/// Configuration for the AI evaluator
#[derive(Debug, Clone)]
pub struct EvaluatorConfig {
    /// Maximum tokens for evaluation response
    pub max_response_tokens: u32,
    /// Temperature for evaluation (lower = more consistent)
    pub temperature: f32,
    /// Whether to include detailed feedback
    pub detailed_feedback: bool,
}

impl Default for EvaluatorConfig {
    fn default() -> Self {
        Self {
            max_response_tokens: 1024,
            temperature: 0.3, // Low temperature for consistent evaluations
            detailed_feedback: true,
        }
    }
}

impl AiEvaluator {
    /// Create a new AI evaluator with the given LLM client
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self {
            llm,
            config: EvaluatorConfig::default(),
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(llm: LlmClient, config: EvaluatorConfig) -> Self {
        Self { llm, config }
    }

    /// Build the code evaluation prompt
    fn build_code_prompt(code: &str, language: &str) -> String {
        format!(
            r#"You are an expert code reviewer evaluating a piece of {language} code.

Evaluate the following code and provide scores from 0-100 for each criterion:

1. **Quality Score**: Code correctness, best practices, proper error handling
2. **Complexity Score**: Appropriate complexity (not over-engineered, not too simple)
3. **Originality Score**: Creative solutions, good design patterns

Also provide brief feedback (2-3 sentences) on the code.

Respond in JSON format:
{{
    "quality_score": <number>,
    "complexity_score": <number>,
    "originality_score": <number>,
    "feedback": "<string>"
}}

Code to evaluate:
```{language}
{code}
```"#
        )
    }

    /// Build the content evaluation prompt
    fn build_content_prompt(content: &str, content_type: &str) -> String {
        format!(
            r#"You are an expert content evaluator assessing a piece of {content_type} content.

Evaluate the following content and provide scores from 0-100 for each criterion:

1. **Quality Score**: Clarity, accuracy, professionalism
2. **Complexity Score**: Appropriate depth for the audience
3. **Originality Score**: Unique insights, creative presentation

Also provide brief feedback (2-3 sentences) on the content.

Respond in JSON format:
{{
    "quality_score": <number>,
    "complexity_score": <number>,
    "originality_score": <number>,
    "feedback": "<string>"
}}

Content to evaluate:
---
{content}
---"#
        )
    }

    /// Parse the evaluation response
    ///
    /// # Errors
    ///
    /// Returns an error if the response cannot be parsed as valid JSON or does not match the expected format.
    fn parse_evaluation(response: &str) -> Result<EvaluationResult> {
        // Try to extract JSON from the response
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: EvalResponse = serde_json::from_str(json_str).map_err(|e| {
            AiError::EvaluationFailed(format!("Failed to parse evaluation response: {e}"))
        })?;

        let overall =
            (parsed.quality_score + parsed.complexity_score + parsed.originality_score) / 3.0;

        Ok(EvaluationResult {
            quality_score: parsed.quality_score,
            complexity_score: parsed.complexity_score,
            originality_score: parsed.originality_score,
            overall_score: overall,
            feedback: parsed.feedback,
        })
    }
}

#[derive(Debug, Deserialize)]
struct EvalResponse {
    quality_score: f64,
    complexity_score: f64,
    originality_score: f64,
    feedback: String,
}

#[async_trait]
impl QualityEvaluator for AiEvaluator {
    async fn evaluate_code(&self, code: &str, language: &str) -> Result<EvaluationResult> {
        // Check code length
        if code.len() > 50000 {
            return Err(AiError::Validation(
                "Code too long for evaluation (max 50KB)".to_string(),
            ));
        }

        let prompt = Self::build_code_prompt(code, language);

        let request = ChatRequest::with_system(
            "You are an expert code reviewer. Always respond with valid JSON.",
            prompt,
        )
        .max_tokens(self.config.max_response_tokens)
        .temperature(self.config.temperature);

        let response = self.llm.chat(request).await?;

        Self::parse_evaluation(&response.message.content)
    }

    async fn evaluate_content(
        &self,
        content: &str,
        content_type: &str,
    ) -> Result<EvaluationResult> {
        // Check content length
        if content.len() > 100_000 {
            return Err(AiError::Validation(
                "Content too long for evaluation (max 100KB)".to_string(),
            ));
        }

        let prompt = Self::build_content_prompt(content, content_type);

        let request = ChatRequest::with_system(
            "You are an expert content evaluator. Always respond with valid JSON.",
            prompt,
        )
        .max_tokens(self.config.max_response_tokens)
        .temperature(self.config.temperature);

        let response = self.llm.chat(request).await?;

        Self::parse_evaluation(&response.message.content)
    }
}

/// Commitment evidence verifier using AI
pub struct AiCommitmentVerifier {
    llm: LlmClient,
}

impl AiCommitmentVerifier {
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Verify commitment evidence
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The LLM request fails
    /// - The response cannot be parsed as valid JSON
    /// - The response does not match the expected format
    pub async fn verify_evidence(
        &self,
        request: &VerificationRequest,
    ) -> Result<VerificationResult> {
        let prompt = Self::build_verification_prompt(request);

        let chat_request = ChatRequest::with_system(
            "You are an expert at verifying commitment fulfillment. Be fair but thorough. Always respond with valid JSON.",
            prompt,
        )
        .max_tokens(1024)
        .temperature(0.2);

        let response = self.llm.chat(chat_request).await?;

        Self::parse_verification(&response.message.content)
    }

    fn build_verification_prompt(request: &VerificationRequest) -> String {
        format!(
            r#"Verify if the following commitment has been fulfilled based on the evidence provided.

**Commitment Title:** {}
**Commitment Description:** {}
**Deadline:** {}
**Evidence URL:** {}
**Evidence Description:** {}

Evaluate the evidence and determine:
1. Is the evidence valid and accessible?
2. Does the evidence match the commitment requirements?
3. Was it completed on time?
4. What is your confidence level (0-100)?

Respond in JSON format:
{{
    "fulfilled": <boolean>,
    "confidence": <number 0-100>,
    "reasoning": "<string explaining your decision>",
    "suggestions": "<string with any suggestions for improvement, or null>"
}}"#,
            request.commitment_title,
            request
                .commitment_description
                .as_deref()
                .unwrap_or("Not provided"),
            request.deadline,
            request.evidence_url,
            request
                .evidence_description
                .as_deref()
                .unwrap_or("Not provided")
        )
    }

    /// Parse the verification response
    ///
    /// # Errors
    ///
    /// Returns an error if the response cannot be parsed as valid JSON or does not match the expected format.
    fn parse_verification(response: &str) -> Result<VerificationResult> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: VerifyResponse = serde_json::from_str(json_str).map_err(|e| {
            AiError::VerificationFailed(format!("Failed to parse verification response: {e}"))
        })?;

        Ok(VerificationResult {
            fulfilled: parsed.fulfilled,
            confidence: parsed.confidence,
            reasoning: parsed.reasoning,
            suggestions: parsed.suggestions,
            needs_human_review: parsed.confidence < 70.0,
        })
    }
}

/// Request for commitment verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub commitment_title: String,
    pub commitment_description: Option<String>,
    pub deadline: String,
    pub evidence_url: String,
    pub evidence_description: Option<String>,
}

/// Result of commitment verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the commitment was fulfilled
    pub fulfilled: bool,
    /// AI confidence in the decision (0-100)
    pub confidence: f64,
    /// Reasoning for the decision
    pub reasoning: String,
    /// Suggestions for improvement
    pub suggestions: Option<String>,
    /// Whether human review is recommended
    pub needs_human_review: bool,
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    fulfilled: bool,
    confidence: f64,
    reasoning: String,
    suggestions: Option<String>,
}

/// Fraud detection using AI
pub struct AiFraudDetector {
    llm: LlmClient,
}

impl AiFraudDetector {
    #[must_use]
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Check content for potential fraud indicators
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The LLM request fails
    /// - The response cannot be parsed as valid JSON
    /// - The response does not match the expected format
    pub async fn check_fraud(&self, request: &FraudCheckRequest) -> Result<FraudCheckResult> {
        let prompt = Self::build_fraud_prompt(request);

        let chat_request = ChatRequest::with_system(
            "You are a fraud detection specialist. Analyze content for signs of manipulation, gaming, or fraudulent activity. Be thorough but fair. Always respond with valid JSON.",
            prompt,
        )
        .max_tokens(1024)
        .temperature(0.2);

        let response = self.llm.chat(chat_request).await?;

        Self::parse_fraud_check(&response.message.content)
    }

    fn build_fraud_prompt(request: &FraudCheckRequest) -> String {
        format!(
            r#"Analyze the following for potential fraud or gaming indicators:

**Content Type:** {}
**Content:**
{}

**User History:**
- Commitments Made: {}
- Commitments Fulfilled: {}
- Average Quality Score: {}

Check for:
1. Plagiarized or generated content
2. Self-dealing or fake evidence
3. Pattern manipulation
4. Suspicious timing or velocity

Respond in JSON format:
{{
    "risk_level": "<low|medium|high|critical>",
    "risk_score": <number 0-100>,
    "indicators": ["<list of suspicious indicators found>"],
    "recommendation": "<string with recommended action>"
}}"#,
            request.content_type,
            request.content,
            request.commitments_made,
            request.commitments_fulfilled,
            request.avg_quality_score.unwrap_or(0.0)
        )
    }

    /// Parse the fraud check response
    ///
    /// # Errors
    ///
    /// Returns an error if the response cannot be parsed as valid JSON or does not match the expected format.
    fn parse_fraud_check(response: &str) -> Result<FraudCheckResult> {
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                response
            }
        } else {
            response
        };

        let parsed: FraudResponse = serde_json::from_str(json_str).map_err(|e| {
            AiError::EvaluationFailed(format!("Failed to parse fraud check response: {e}"))
        })?;

        Ok(FraudCheckResult {
            risk_level: parsed.risk_level,
            risk_score: parsed.risk_score,
            indicators: parsed.indicators,
            recommendation: parsed.recommendation,
        })
    }
}

/// Request for fraud check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudCheckRequest {
    pub content_type: String,
    pub content: String,
    pub commitments_made: i32,
    pub commitments_fulfilled: i32,
    pub avg_quality_score: Option<f64>,
}

/// Result of fraud check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudCheckResult {
    pub risk_level: String,
    pub risk_score: f64,
    pub indicators: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Deserialize)]
struct FraudResponse {
    risk_level: String,
    risk_score: f64,
    indicators: Vec<String>,
    recommendation: String,
}
