//! Batch processing utilities for efficient AI operations
//!
//! This module provides utilities for processing multiple items efficiently,
//! with support for parallel execution, progress tracking, and error handling.

use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use tokio::time::sleep;

use crate::ai_evaluator::{AiCommitmentVerifier, AiEvaluator, AiFraudDetector};
use crate::ai_evaluator::{
    FraudCheckRequest, FraudCheckResult, VerificationRequest, VerificationResult,
};
use crate::error::{AiError, Result};
use crate::evaluator::{EvaluationResult, QualityEvaluator};

/// Configuration for batch processing
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of concurrent operations
    pub max_concurrent: usize,
    /// Delay between batch items to avoid rate limits
    pub delay_between_items: Duration,
    /// Whether to continue on errors or fail fast
    pub continue_on_error: bool,
    /// Maximum retries per item
    pub max_retries: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            delay_between_items: Duration::from_millis(100),
            continue_on_error: true,
            max_retries: 2,
        }
    }
}

impl BatchConfig {
    /// Create a new batch config with custom concurrency
    #[must_use]
    pub fn with_concurrency(concurrency: usize) -> Self {
        Self {
            max_concurrent: concurrency,
            ..Default::default()
        }
    }

    /// Set the delay between items
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay_between_items = delay;
        self
    }

    /// Set whether to continue on errors
    #[must_use]
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Set maximum retries
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Result of a batch operation
#[derive(Debug)]
pub struct BatchResult<T> {
    /// Successfully processed items
    pub successes: Vec<T>,
    /// Failed items with their errors
    pub failures: Vec<(usize, AiError)>,
    /// Total items processed
    pub total: usize,
}

impl<T> BatchResult<T> {
    /// Create a new empty batch result
    fn new() -> Self {
        Self {
            successes: Vec::new(),
            failures: Vec::new(),
            total: 0,
        }
    }

    /// Check if all items succeeded
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        self.failures.is_empty()
    }

    /// Get success rate (0.0 to 1.0)
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.successes.len() as f64 / self.total as f64
    }

    /// Get the number of failures
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }

    /// Get the number of successes
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.successes.len()
    }
}

/// Batch code evaluator for processing multiple code samples
pub struct BatchCodeEvaluator {
    evaluator: Arc<AiEvaluator>,
    config: BatchConfig,
}

impl BatchCodeEvaluator {
    /// Create a new batch code evaluator
    #[must_use]
    pub fn new(evaluator: Arc<AiEvaluator>, config: BatchConfig) -> Self {
        Self { evaluator, config }
    }

    /// Evaluate multiple code samples
    ///
    /// Returns a batch result with successes and failures
    ///
    /// # Errors
    ///
    /// Returns an error if the batch evaluation fails critically.
    /// Individual evaluation errors are captured in the result's failures list.
    pub async fn evaluate_batch(
        &self,
        codes: Vec<(String, String)>, // (code, language) pairs
    ) -> Result<BatchResult<EvaluationResult>> {
        let total = codes.len();
        let mut result = BatchResult::new();
        result.total = total;

        let results = stream::iter(codes.into_iter().enumerate())
            .map(|(idx, (code, lang))| {
                let evaluator = Arc::clone(&self.evaluator);
                let delay = self.config.delay_between_items;
                async move {
                    if delay > Duration::ZERO {
                        sleep(delay).await;
                    }
                    match evaluator.evaluate_code(&code, &lang).await {
                        Ok(res) => Ok((idx, res)),
                        Err(e) => Err((idx, e)),
                    }
                }
            })
            .buffer_unordered(self.config.max_concurrent)
            .collect::<Vec<_>>()
            .await;

        for item_result in results {
            match item_result {
                Ok((_, eval_result)) => {
                    result.successes.push(eval_result);
                }
                Err((idx, error)) => {
                    if !self.config.continue_on_error {
                        return Err(error);
                    }
                    result.failures.push((idx, error));
                }
            }
        }

        Ok(result)
    }

    /// Evaluate multiple code samples with custom languages
    ///
    /// # Errors
    ///
    /// Returns an error if the batch evaluation fails critically.
    /// Individual evaluation errors are captured in the result's failures list.
    pub async fn evaluate_with_languages(
        &self,
        items: Vec<(&str, &str)>, // (code, language) pairs
    ) -> Result<BatchResult<EvaluationResult>> {
        let codes: Vec<(String, String)> = items
            .into_iter()
            .map(|(code, lang)| (code.to_string(), lang.to_string()))
            .collect();
        self.evaluate_batch(codes).await
    }
}

/// Batch commitment verifier for processing multiple verification requests
pub struct BatchCommitmentVerifier {
    verifier: Arc<AiCommitmentVerifier>,
    config: BatchConfig,
}

impl BatchCommitmentVerifier {
    /// Create a new batch commitment verifier
    #[must_use]
    pub fn new(verifier: Arc<AiCommitmentVerifier>, config: BatchConfig) -> Self {
        Self { verifier, config }
    }

    /// Verify multiple commitments
    ///
    /// # Errors
    ///
    /// Returns an error if the batch verification fails critically.
    /// Individual verification errors are captured in the result's failures list.
    pub async fn verify_batch(
        &self,
        requests: Vec<VerificationRequest>,
    ) -> Result<BatchResult<VerificationResult>> {
        let total = requests.len();
        let mut result = BatchResult::new();
        result.total = total;

        let results = stream::iter(requests.into_iter().enumerate())
            .map(|(idx, request)| {
                let verifier = Arc::clone(&self.verifier);
                let delay = self.config.delay_between_items;
                async move {
                    if delay > Duration::ZERO {
                        sleep(delay).await;
                    }
                    match verifier.verify_evidence(&request).await {
                        Ok(res) => Ok((idx, res)),
                        Err(e) => Err((idx, e)),
                    }
                }
            })
            .buffer_unordered(self.config.max_concurrent)
            .collect::<Vec<_>>()
            .await;

        for item_result in results {
            match item_result {
                Ok((_, verification_result)) => {
                    result.successes.push(verification_result);
                }
                Err((idx, error)) => {
                    if !self.config.continue_on_error {
                        return Err(error);
                    }
                    result.failures.push((idx, error));
                }
            }
        }

        Ok(result)
    }
}

/// Batch fraud detector for processing multiple fraud check requests
pub struct BatchFraudDetector {
    detector: Arc<AiFraudDetector>,
    config: BatchConfig,
}

impl BatchFraudDetector {
    /// Create a new batch fraud detector
    #[must_use]
    pub fn new(detector: Arc<AiFraudDetector>, config: BatchConfig) -> Self {
        Self { detector, config }
    }

    /// Check fraud for multiple requests
    pub async fn check_batch(
        &self,
        requests: Vec<FraudCheckRequest>,
    ) -> Result<BatchResult<FraudCheckResult>> {
        let total = requests.len();
        let mut result = BatchResult::new();
        result.total = total;

        let results = stream::iter(requests.into_iter().enumerate())
            .map(|(idx, request)| {
                let detector = Arc::clone(&self.detector);
                let delay = self.config.delay_between_items;
                async move {
                    if delay > Duration::ZERO {
                        sleep(delay).await;
                    }
                    match detector.check_fraud(&request).await {
                        Ok(res) => Ok((idx, res)),
                        Err(e) => Err((idx, e)),
                    }
                }
            })
            .buffer_unordered(self.config.max_concurrent)
            .collect::<Vec<_>>()
            .await;

        for item_result in results {
            match item_result {
                Ok((_, fraud_result)) => {
                    result.successes.push(fraud_result);
                }
                Err((idx, error)) => {
                    if !self.config.continue_on_error {
                        return Err(error);
                    }
                    result.failures.push((idx, error));
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_evaluator::EvaluatorConfig;
    use crate::llm::{LlmClient, OpenAiClient};

    #[test]
    fn test_batch_config_default() {
        let config = BatchConfig::default();
        assert_eq!(config.max_concurrent, 5);
        assert!(config.continue_on_error);
        assert_eq!(config.max_retries, 2);
    }

    #[test]
    fn test_batch_config_builder() {
        let config = BatchConfig::with_concurrency(10)
            .with_delay(Duration::from_millis(200))
            .with_continue_on_error(false)
            .with_max_retries(3);

        assert_eq!(config.max_concurrent, 10);
        assert_eq!(config.delay_between_items, Duration::from_millis(200));
        assert!(!config.continue_on_error);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_batch_result_success_rate() {
        let mut result: BatchResult<i32> = BatchResult::new();
        result.total = 10;
        result.successes = vec![1, 2, 3, 4, 5, 6, 7];
        result.failures = vec![
            (0, AiError::Configuration("error".to_string())),
            (1, AiError::Configuration("error".to_string())),
            (2, AiError::Configuration("error".to_string())),
        ];

        assert_eq!(result.success_rate(), 0.7);
        assert_eq!(result.success_count(), 7);
        assert_eq!(result.failure_count(), 3);
        assert!(!result.all_succeeded());
    }

    #[test]
    fn test_batch_result_all_succeeded() {
        let mut result: BatchResult<i32> = BatchResult::new();
        result.total = 5;
        result.successes = vec![1, 2, 3, 4, 5];

        assert_eq!(result.success_rate(), 1.0);
        assert!(result.all_succeeded());
    }

    #[test]
    fn test_batch_evaluator_creation() {
        let openai = OpenAiClient::with_default_model("test-key");
        let llm_client = LlmClient::new(Box::new(openai));
        let evaluator = Arc::new(AiEvaluator::with_config(
            llm_client,
            EvaluatorConfig::default(),
        ));
        let config = BatchConfig::default();

        let batch_evaluator = BatchCodeEvaluator::new(evaluator, config);
        assert_eq!(batch_evaluator.config.max_concurrent, 5);
    }
}
