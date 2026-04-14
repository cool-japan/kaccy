//! Quality evaluation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Scores and feedback from a quality evaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    /// Code/content quality score (0-100).
    pub quality_score: f64,
    /// Complexity score (0-100).
    pub complexity_score: f64,
    /// Originality score (0-100).
    pub originality_score: f64,
    /// Weighted average of all dimension scores (0-100).
    pub overall_score: f64,
    /// Human-readable feedback from the evaluator.
    pub feedback: String,
}

/// Trait for evaluating the quality of code or content.
#[async_trait]
pub trait QualityEvaluator: Send + Sync {
    /// Evaluate a code snippet written in `language`.
    async fn evaluate_code(&self, code: &str, language: &str) -> Result<EvaluationResult>;
    /// Evaluate a piece of content identified by `content_type`.
    async fn evaluate_content(&self, content: &str, content_type: &str)
    -> Result<EvaluationResult>;
}

/// Default evaluator that uses AI when available, otherwise falls back to manual review
pub struct DefaultEvaluator {
    ai_evaluator: Option<crate::ai_evaluator::AiEvaluator>,
}

impl Default for DefaultEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultEvaluator {
    /// Create a new `DefaultEvaluator` without AI capabilities
    #[must_use]
    pub fn new() -> Self {
        Self { ai_evaluator: None }
    }

    /// Create a new `DefaultEvaluator` with AI capabilities
    #[must_use]
    pub fn with_llm(llm: crate::llm::LlmClient) -> Self {
        Self {
            ai_evaluator: Some(crate::ai_evaluator::AiEvaluator::new(llm)),
        }
    }

    /// Create with custom AI evaluator configuration
    #[must_use]
    pub fn with_ai_evaluator(ai_evaluator: crate::ai_evaluator::AiEvaluator) -> Self {
        Self {
            ai_evaluator: Some(ai_evaluator),
        }
    }
}

#[async_trait]
impl QualityEvaluator for DefaultEvaluator {
    async fn evaluate_code(&self, code: &str, language: &str) -> Result<EvaluationResult> {
        // If AI evaluator is available, use it
        if let Some(ai) = &self.ai_evaluator {
            return ai.evaluate_code(code, language).await;
        }

        // Fallback to basic heuristic evaluation
        let lines = code.lines().count();
        let complexity = if lines < 10 {
            40.0
        } else if lines < 50 {
            50.0
        } else if lines < 100 {
            60.0
        } else {
            70.0
        };

        Ok(EvaluationResult {
            quality_score: 75.0,
            complexity_score: complexity,
            originality_score: 70.0,
            overall_score: (75.0 + complexity + 70.0) / 3.0,
            feedback: "Evaluation pending - manual review required (AI evaluator not configured)"
                .to_string(),
        })
    }

    async fn evaluate_content(
        &self,
        content: &str,
        content_type: &str,
    ) -> Result<EvaluationResult> {
        // If AI evaluator is available, use it
        if let Some(ai) = &self.ai_evaluator {
            return ai.evaluate_content(content, content_type).await;
        }

        // Fallback to basic heuristic evaluation
        let words = content.split_whitespace().count();
        let quality = if words < 50 {
            60.0
        } else if words < 200 {
            75.0
        } else if words < 500 {
            80.0
        } else {
            85.0
        };

        let complexity = if words < 100 {
            40.0
        } else if words < 300 {
            50.0
        } else {
            60.0
        };

        Ok(EvaluationResult {
            quality_score: quality,
            complexity_score: complexity,
            originality_score: 70.0,
            overall_score: (quality + complexity + 70.0) / 3.0,
            feedback: "Evaluation pending - manual review required (AI evaluator not configured)"
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_evaluate_code_without_ai() {
        let evaluator = DefaultEvaluator::new();
        let code = "fn main() { println!(\"Hello\"); }";
        let result = evaluator.evaluate_code(code, "rust").await.unwrap();

        assert!(result.quality_score > 0.0);
        assert!(result.overall_score > 0.0);
        assert!(result.feedback.contains("manual review required"));
    }

    #[tokio::test]
    async fn test_evaluate_content_without_ai() {
        let evaluator = DefaultEvaluator::new();
        let content = "This is a test document with some content that should be evaluated.";
        let result = evaluator.evaluate_content(content, "text").await.unwrap();

        assert!(result.quality_score > 0.0);
        assert!(result.overall_score > 0.0);
    }

    #[tokio::test]
    async fn test_heuristic_complexity_scaling() {
        let evaluator = DefaultEvaluator::new();

        // Short code
        let short_code = "fn test() {}";
        let short_result = evaluator.evaluate_code(short_code, "rust").await.unwrap();

        // Long code
        let long_code = (0..150)
            .map(|i| format!("let x{i} = {i};"))
            .collect::<Vec<_>>()
            .join("\n");
        let long_result = evaluator.evaluate_code(&long_code, "rust").await.unwrap();

        // Longer code should have higher complexity
        assert!(long_result.complexity_score > short_result.complexity_score);
    }

    #[tokio::test]
    async fn test_content_quality_scaling() {
        let evaluator = DefaultEvaluator::new();

        // Short content
        let short = "Brief.";
        let short_result = evaluator.evaluate_content(short, "text").await.unwrap();

        // Long content
        let long = (0..100).map(|_| "word").collect::<Vec<_>>().join(" ");
        let long_result = evaluator.evaluate_content(&long, "text").await.unwrap();

        // Longer content should have higher quality score
        assert!(long_result.quality_score > short_result.quality_score);
    }
}
