//! Basic usage examples: code evaluation, batch processing, oracle consensus,
//! complete service setup, fraud detection, and integration patterns.

use std::sync::Arc;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::access_control::{AccessControlManager, AccessTier, AiFeature, TokenHolder};
use crate::ai_evaluator::{AiEvaluator, AiFraudDetector, EvaluatorConfig};
use crate::batch::{BatchCodeEvaluator, BatchConfig};
use crate::error::Result;
use crate::evaluator::QualityEvaluator;
use crate::llm::{LlmClient, OpenAiClient};
use crate::oracle::AiOracle;
use crate::presets::{AccessTierPresets, ProductionPreset};
use crate::service::{AiServiceBuilder, AiServiceHub};

/// Example: Basic code evaluation workflow
///
/// Demonstrates how to:
/// - Set up an LLM client
/// - Create an evaluator
/// - Evaluate code quality
pub struct BasicCodeEvaluationExample;

impl BasicCodeEvaluationExample {
    /// Run the basic code evaluation example
    ///
    /// # Example
    /// ```no_run
    /// use kaccy_ai::examples::BasicCodeEvaluationExample;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// BasicCodeEvaluationExample::run("your-api-key").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(api_key: &str) -> Result<()> {
        // 1. Create an OpenAI client
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = LlmClient::new(Box::new(openai));

        // 2. Create an evaluator with default config
        let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());

        // 3. Evaluate some code
        let code = r"
            fn factorial(n: u64) -> u64 {
                if n == 0 { 1 } else { n * factorial(n - 1) }
            }
        ";

        let result = evaluator.evaluate_code(code, "rust").await?;

        println!("Quality Score: {}", result.quality_score);
        println!("Complexity Score: {}", result.complexity_score);
        println!("Originality Score: {}", result.originality_score);
        println!("Feedback: {}", result.feedback);

        Ok(())
    }
}

/// Example: Batch processing workflow
///
/// Demonstrates how to:
/// - Set up batch processing
/// - Process multiple code samples efficiently
/// - Handle errors in batch operations
pub struct BatchProcessingExample;

impl BatchProcessingExample {
    /// Run the batch processing example
    #[allow(dead_code)]
    pub async fn run(api_key: &str) -> Result<()> {
        // 1. Create the evaluator
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = LlmClient::new(Box::new(openai));
        let evaluator = Arc::new(AiEvaluator::with_config(
            llm_client,
            EvaluatorConfig::default(),
        ));

        // 2. Create batch processor with production settings
        let batch_config = ProductionPreset::batch_config();
        let batch_evaluator = BatchCodeEvaluator::new(evaluator, batch_config);

        // 3. Prepare multiple code samples
        let codes = vec![
            (
                "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
                "rust".to_string(),
            ),
            (
                "function add(a, b) { return a + b; }".to_string(),
                "javascript".to_string(),
            ),
            (
                "def add(a, b): return a + b".to_string(),
                "python".to_string(),
            ),
        ];

        // 4. Process in batch
        let result = batch_evaluator.evaluate_batch(codes).await?;

        println!("Total processed: {}", result.total);
        println!("Successes: {}", result.success_count());
        println!("Failures: {}", result.failure_count());
        println!("Success rate: {:.2}%", result.success_rate() * 100.0);

        Ok(())
    }
}

/// Example: AI Oracle with multi-model consensus
///
/// Demonstrates how to:
/// - Set up an AI Oracle
/// - Use multi-model consensus
/// - Learn from human feedback
pub struct OracleConsensusExample;

impl OracleConsensusExample {
    /// Run the oracle consensus example
    #[allow(dead_code)]
    pub async fn run(_api_key: &str) -> Result<()> {
        // 1. Create oracle with production config
        let oracle_config = ProductionPreset::oracle_config();
        let mut oracle = AiOracle::new(oracle_config);

        // 2. Add models via config (models are managed through the config)
        // In production, you would configure multiple LLM providers through the oracle config

        // 3. Make a decision with consensus
        let request = crate::ai_evaluator::VerificationRequest {
            commitment_title: "Deploy smart contract".to_string(),
            commitment_description: Some("Deploy audited smart contract to mainnet".to_string()),
            deadline: "2024-12-31".to_string(),
            evidence_url: "https://etherscan.io/address/0x123...".to_string(),
            evidence_description: Some("Contract deployed with verified source code".to_string()),
        };
        let decision = oracle.verify_with_consensus(&request).await?;

        println!("Consensus Decision: {}", decision.approved);
        println!("Confidence: {}", decision.confidence);
        println!("Reasoning: {}", decision.reasoning);

        // 4. Learn from human feedback (if decision was wrong)
        let human_decision = true; // Assume human verified this was correct
        oracle.record_feedback(
            "commitment-123".to_string(),
            decision.approved,
            decision.confidence,
            human_decision,
        );

        Ok(())
    }
}

/// Example: Complete service setup with access control
///
/// Demonstrates how to:
/// - Set up the full AI service hub
/// - Configure access control
/// - Use tiered access
pub struct CompleteServiceExample;

impl CompleteServiceExample {
    /// Run the complete service example
    #[allow(dead_code)]
    pub async fn run(api_key: &str) -> Result<()> {
        // 1. Create LLM client
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));

        // 2. Build service with all features
        let mut service = AiServiceBuilder::new(llm_client)
            .evaluator_config(ProductionPreset::evaluator_config())
            .with_oracle(ProductionPreset::oracle_config())
            .with_access_control()
            .build();

        // 3. Set up access control with tiers
        let mut access_control = AccessControlManager::new();
        access_control.update_tier_config(AccessTierPresets::silver_tier());

        // 4. Create a token holder
        let holder = TokenHolder {
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            balance: Decimal::new(1000, 0),
            tier: AccessTier::Silver,
        };

        // 5. Check and use features
        if let Some(Ok(true)) = service.can_access_feature(&holder, AiFeature::CodeEvaluation) {
            let evaluator = service.evaluator();
            let code = "fn hello() { println!(\"Hello\"); }";
            let result = evaluator.evaluate_code(code, "rust").await?;

            // Record usage
            service.record_usage(&holder, AiFeature::CodeEvaluation);

            println!("Evaluation successful: {}", result.quality_score);
        }

        Ok(())
    }
}

/// Example: Fraud detection workflow
///
/// Demonstrates how to:
/// - Set up fraud detection
/// - Analyze user behavior
/// - Interpret risk scores
pub struct FraudDetectionExample;

impl FraudDetectionExample {
    /// Run the fraud detection example
    #[allow(dead_code)]
    pub async fn run(api_key: &str) -> Result<()> {
        // 1. Create fraud detector
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = LlmClient::new(Box::new(openai));
        let fraud_detector = AiFraudDetector::new(llm_client);

        // 2. Prepare fraud check request
        let request = crate::ai_evaluator::FraudCheckRequest {
            content_type: "Task completion claim".to_string(),
            content: "I have completed all 50 tasks! Here's proof: [screenshot]".to_string(),
            commitments_made: 50,
            commitments_fulfilled: 2, // Suspicious: claimed 50 but only fulfilled 2 before
            avg_quality_score: Some(85.0),
        };

        // 3. Check for fraud
        let result = fraud_detector.check_fraud(&request).await?;

        println!("Risk Level: {:?}", result.risk_level);
        println!("Risk Score: {}", result.risk_score);
        println!("Suspicious Indicators:");
        for indicator in &result.indicators {
            println!("  - {indicator}");
        }
        println!("Recommendation: {}", result.recommendation);

        Ok(())
    }
}

/// Example: Integration with external systems
///
/// Demonstrates how to:
/// - Integrate AI services with your application
/// - Handle errors gracefully
/// - Implement caching and optimization
pub struct IntegrationExample;

impl IntegrationExample {
    /// Example integration with a web service
    #[allow(dead_code)]
    pub async fn web_service_integration(api_key: &str) -> Result<()> {
        // 1. Create service hub (singleton in your app)
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = Arc::new(LlmClient::new(Box::new(openai)));
        let service = AiServiceHub::new(llm_client);

        // 2. Use in request handlers
        let code_to_evaluate = "fn main() {}";
        let evaluator = service.evaluator();

        match evaluator.evaluate_code(code_to_evaluate, "rust").await {
            Ok(result) => {
                println!("✓ Evaluation successful");
                println!("Quality: {}/100", result.quality_score);
            }
            Err(e) => {
                eprintln!("✗ Evaluation failed: {e}");
                // Handle error (return HTTP 500, log, etc.)
            }
        }

        Ok(())
    }

    /// Example batch processing for background jobs
    #[allow(dead_code)]
    pub async fn background_job_processing(api_key: &str) -> Result<()> {
        // 1. Set up batch processor
        let openai = OpenAiClient::with_default_model(api_key);
        let llm_client = LlmClient::new(Box::new(openai));
        let evaluator = Arc::new(AiEvaluator::with_config(
            llm_client,
            ProductionPreset::evaluator_config(),
        ));

        // 2. Use high-volume settings for background jobs
        let batch_config = BatchConfig::with_concurrency(20).with_continue_on_error(true);

        let batch_evaluator = BatchCodeEvaluator::new(evaluator, batch_config);

        // 3. Process queued items
        let queued_codes = vec![
            // ... fetch from database/queue
        ];

        let results = batch_evaluator.evaluate_batch(queued_codes).await?;

        println!("Processed {} items", results.total);
        println!("Success rate: {:.1}%", results.success_rate() * 100.0);

        Ok(())
    }
}
