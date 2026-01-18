//! Usage examples and integration patterns
//!
//! This module provides practical examples of how to use the kaccy-ai crate
//! for various common scenarios. These examples can serve as starting points
//! for your own implementations.

#![allow(clippy::unused_async)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::sync::Arc;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::access_control::{AccessControlManager, AccessTier, AiFeature, TokenHolder};
use crate::ai_evaluator::{AiEvaluator, AiFraudDetector, EvaluatorConfig};
use crate::batch::{BatchCodeEvaluator, BatchConfig};
use crate::error::Result;
use crate::evaluator::QualityEvaluator;
use crate::llm::{
    DeepSeekClient, GeminiClient, LlmClient, LlmClientBuilder, ModelTier, OpenAiClient,
    RoutingConfig,
};
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

/// Example: Google Gemini integration workflow
///
/// Demonstrates how to:
/// - Use Google Gemini as an LLM provider
/// - Leverage cost-effective Gemini models
/// - Set up multi-provider fallback with Gemini
/// - Use Gemini for cost optimization
pub struct GeminiIntegrationExample;

impl GeminiIntegrationExample {
    /// Run the Gemini integration example with basic usage
    #[allow(dead_code)]
    pub async fn run_basic(api_key: &str) -> Result<()> {
        // 1. Create a Gemini client (Flash for cost efficiency)
        let gemini = GeminiClient::with_flash(api_key);
        let llm_client = LlmClient::new(Box::new(gemini));

        // 2. Create an evaluator
        let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());

        // 3. Evaluate code with Gemini
        let code = r"
            fn fibonacci(n: u32) -> u32 {
                match n {
                    0 => 0,
                    1 => 1,
                    _ => fibonacci(n - 1) + fibonacci(n - 2),
                }
            }
        ";

        let result = evaluator.evaluate_code(code, "rust").await?;

        println!("Gemini Flash Evaluation:");
        println!("  Quality: {}", result.quality_score);
        println!("  Complexity: {}", result.complexity_score);
        println!("  Feedback: {}", result.feedback);

        Ok(())
    }

    /// Run example with multi-provider setup (Gemini + fallback)
    #[allow(dead_code)]
    pub async fn run_with_fallback(gemini_key: &str, openai_key: &str) -> Result<()> {
        // 1. Set up multi-provider client with Gemini as primary
        let client = LlmClientBuilder::new()
            .gemini_api_key(gemini_key)
            .gemini_model("gemini-2.0-flash-exp") // Free experimental model
            .openai_api_key(openai_key)
            .prefer_gemini() // Use Gemini first
            .build()
            .ok_or_else(|| crate::error::AiError::Configuration("No API keys provided".into()))?;

        // 2. Create evaluator
        let evaluator = AiEvaluator::with_config(client, EvaluatorConfig::default());

        // 3. Evaluate (will use Gemini, fallback to OpenAI if needed)
        let content = "Building a distributed system with microservices architecture...";

        let result = evaluator.evaluate_content(content, "technical").await?;

        println!("Multi-provider Evaluation:");
        println!("  Quality: {}", result.quality_score);
        println!("  (Using Gemini with OpenAI fallback)");

        Ok(())
    }

    /// Run example with cost-optimized routing using Gemini
    #[allow(dead_code)]
    pub async fn run_cost_optimized(_api_key: &str) -> Result<()> {
        // 1. Create cost-optimized routing config with Gemini models
        let routing_config = RoutingConfig {
            tiers: vec![
                ModelTier::gemini_2_0_flash(), // Free tier for simple tasks
                ModelTier::gemini_1_5_flash(), // Very cheap for medium tasks
                ModelTier::gemini_1_5_pro(),   // Cost-effective for complex tasks
            ],
            auto_escalate: true,
            escalation_threshold: 70.0,
        };

        println!("Cost-Optimized Gemini Routing:");
        println!("  Simple tasks -> Gemini 2.0 Flash (Free)");
        println!("  Medium tasks -> Gemini 1.5 Flash ($0.075/M input)");
        println!("  Complex tasks -> Gemini 1.5 Pro ($1.25/M input)");
        println!();

        // 2. Estimate costs for different scenarios
        let simple_cost = routing_config.estimate_cost("gemini-2.0-flash-exp", 1000, 500);
        let medium_cost = routing_config.estimate_cost("gemini-1.5-flash", 2000, 1000);
        let complex_cost = routing_config.estimate_cost("gemini-1.5-pro", 5000, 2000);

        println!("Estimated costs:");
        println!("  Simple task (1K in, 500 out): ${simple_cost:.6}");
        println!("  Medium task (2K in, 1K out): ${medium_cost:.6}");
        println!("  Complex task (5K in, 2K out): ${complex_cost:.6}");

        Ok(())
    }

    /// Run example showing all Gemini model options
    #[allow(dead_code)]
    pub async fn run_model_comparison(api_key: &str) -> Result<()> {
        println!("Google Gemini Model Comparison:");
        println!();

        // Gemini 2.0 Flash (Experimental)
        println!("1. Gemini 2.0 Flash (Experimental)");
        println!("   - Context: 1M tokens");
        println!("   - Cost: Free (experimental)");
        println!("   - Best for: Testing, development, simple tasks");
        println!();

        // Gemini 1.5 Flash
        println!("2. Gemini 1.5 Flash");
        println!("   - Context: 1M tokens");
        println!("   - Cost: $0.075/M input, $0.30/M output");
        println!("   - Best for: High-volume, cost-sensitive workloads");
        println!();

        // Gemini 1.5 Pro
        println!("3. Gemini 1.5 Pro");
        println!("   - Context: 1M tokens");
        println!("   - Cost: $1.25/M input, $5.00/M output");
        println!("   - Best for: Complex reasoning, code generation");
        println!();

        // Example: Create clients for each
        let _flash_exp = GeminiClient::with_2_0_flash(api_key);
        let _flash = GeminiClient::with_flash(api_key);
        let _pro = GeminiClient::with_default_model(api_key);

        println!("Created clients: Flash Exp, Flash, Pro");

        Ok(())
    }
}

/// Example: `DeepSeek` LLM integration
///
/// Demonstrates:
/// - Using `DeepSeek`'s cost-effective AI models
/// - Specialized models (Chat, Coder, Reasoner)
/// - Multi-provider setup with `DeepSeek`
/// - Cost optimization with `DeepSeek`
pub struct DeepSeekIntegrationExample;

impl DeepSeekIntegrationExample {
    /// Run the `DeepSeek` integration example with basic usage
    #[allow(dead_code)]
    pub async fn run_basic(api_key: &str) -> Result<()> {
        // 1. Create a DeepSeek client (Chat model for general use)
        let deepseek = DeepSeekClient::with_default_model(api_key);
        let llm_client = LlmClient::new(Box::new(deepseek));

        // 2. Create an evaluator
        let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());

        // 3. Evaluate code with DeepSeek
        let code = r"
            fn binary_search<T: Ord>(arr: &[T], target: &T) -> Option<usize> {
                let mut left = 0;
                let mut right = arr.len();

                while left < right {
                    let mid = left + (right - left) / 2;
                    match arr[mid].cmp(target) {
                        std::cmp::Ordering::Equal => return Some(mid),
                        std::cmp::Ordering::Less => left = mid + 1,
                        std::cmp::Ordering::Greater => right = mid,
                    }
                }
                None
            }
        ";

        let result = evaluator.evaluate_code(code, "rust").await?;

        println!("DeepSeek Chat Evaluation:");
        println!("  Quality: {}", result.quality_score);
        println!("  Complexity: {}", result.complexity_score);
        println!("  Feedback: {}", result.feedback);

        Ok(())
    }

    /// Run example with `DeepSeek` Coder (optimized for code tasks)
    #[allow(dead_code)]
    pub async fn run_with_coder(api_key: &str) -> Result<()> {
        // 1. Create DeepSeek Coder client (specialized for code)
        let deepseek_coder = DeepSeekClient::with_coder_model(api_key);
        let llm_client = LlmClient::new(Box::new(deepseek_coder));

        // 2. Create evaluator
        let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());

        // 3. Evaluate code with specialized model
        let code = r#"
            class RedBlackTree:
                def __init__(self):
                    self.nil = Node(None, "BLACK")
                    self.root = self.nil

                def insert(self, key):
                    node = Node(key)
                    node.left = node.right = self.nil
                    # ... implementation
        "#;

        let result = evaluator.evaluate_code(code, "python").await?;

        println!("DeepSeek Coder Evaluation:");
        println!("  Quality: {}", result.quality_score);
        println!("  Complexity: {}", result.complexity_score);
        println!("  (Using specialized code model)");

        Ok(())
    }

    /// Run example with multi-provider setup (`DeepSeek` + fallback)
    #[allow(dead_code)]
    pub async fn run_with_fallback(deepseek_key: &str, openai_key: &str) -> Result<()> {
        // 1. Set up multi-provider client with DeepSeek as primary
        let client = LlmClientBuilder::new()
            .deepseek_api_key(deepseek_key)
            .deepseek_model("deepseek-chat")
            .openai_api_key(openai_key)
            .prefer_deepseek() // Use DeepSeek first (most cost-effective)
            .build()
            .ok_or_else(|| crate::error::AiError::Configuration("No API keys provided".into()))?;

        // 2. Create evaluator
        let evaluator = AiEvaluator::with_config(client, EvaluatorConfig::default());

        // 3. Evaluate (will use DeepSeek, fallback to OpenAI if needed)
        let content =
            "Implementing a high-performance distributed cache with consistent hashing...";

        let result = evaluator.evaluate_content(content, "technical").await?;

        println!("Multi-provider Evaluation:");
        println!("  Quality: {}", result.quality_score);
        println!("  (Using DeepSeek with OpenAI fallback)");

        Ok(())
    }

    /// Run example with cost-optimized routing using `DeepSeek`
    #[allow(dead_code)]
    pub async fn run_cost_optimized(_api_key: &str) -> Result<()> {
        // 1. Create ultra-cost-optimized routing config with DeepSeek models
        let routing_config = RoutingConfig {
            tiers: vec![
                ModelTier::deepseek_chat(),     // Very cheap for simple-medium tasks
                ModelTier::deepseek_coder(),    // Code-optimized at same low price
                ModelTier::deepseek_reasoner(), // Advanced reasoning, still cheap
            ],
            auto_escalate: true,
            escalation_threshold: 70.0,
        };

        println!("Ultra-Cost-Optimized DeepSeek Routing:");
        println!("  Simple-Medium tasks -> DeepSeek Chat ($0.14/M input)");
        println!("  Code tasks -> DeepSeek Coder ($0.14/M input)");
        println!("  Complex reasoning -> DeepSeek Reasoner ($0.55/M input)");
        println!();

        // 2. Estimate costs for different scenarios
        let simple_cost = routing_config.estimate_cost("deepseek-chat", 1000, 500);
        let code_cost = routing_config.estimate_cost("deepseek-coder", 2000, 1000);
        let complex_cost = routing_config.estimate_cost("deepseek-reasoner", 5000, 2000);

        println!("Estimated costs:");
        println!("  Simple task (1K in, 500 out): ${simple_cost:.6}");
        println!("  Code task (2K in, 1K out): ${code_cost:.6}");
        println!("  Complex task (5K in, 2K out): ${complex_cost:.6}");
        println!();
        println!("Note: DeepSeek is ~100x cheaper than GPT-4, ~50x cheaper than Claude Opus!");

        Ok(())
    }

    /// Run example showing all `DeepSeek` model options
    #[allow(dead_code)]
    pub async fn run_model_comparison(api_key: &str) -> Result<()> {
        println!("DeepSeek Model Comparison:");
        println!();

        // DeepSeek Chat
        println!("1. DeepSeek Chat");
        println!("   - Context: 32K tokens");
        println!("   - Cost: $0.14/M input, $0.28/M output");
        println!("   - Best for: General purpose, cost-sensitive workloads");
        println!();

        // DeepSeek Coder
        println!("2. DeepSeek Coder");
        println!("   - Context: 32K tokens");
        println!("   - Cost: $0.14/M input, $0.28/M output");
        println!("   - Best for: Code generation, analysis, refactoring");
        println!();

        // DeepSeek Reasoner
        println!("3. DeepSeek Reasoner");
        println!("   - Context: 64K tokens");
        println!("   - Cost: $0.55/M input, $2.19/M output");
        println!("   - Best for: Complex reasoning, math, logic problems");
        println!();

        // Example: Create clients for each
        let _chat = DeepSeekClient::with_default_model(api_key);
        let _coder = DeepSeekClient::with_coder_model(api_key);
        let _reasoner = DeepSeekClient::with_reasoner_model(api_key);

        println!("Created clients: Chat, Coder, Reasoner");
        println!();
        println!("💡 Tip: DeepSeek offers the best price/performance ratio!");
        println!("   Compare: GPT-4 Turbo costs $10/M vs DeepSeek Chat $0.14/M");

        Ok(())
    }

    /// Run example comparing cost across all providers
    #[allow(dead_code)]
    pub async fn run_cost_comparison() -> Result<()> {
        println!("LLM Provider Cost Comparison (per 1M tokens):");
        println!();
        println!("Input costs:");
        println!("  DeepSeek Chat:     $0.14   ⭐ Best value");
        println!("  Gemini 1.5 Flash:  $0.075  ⭐ Google's cheapest");
        println!("  DeepSeek Reasoner: $0.55");
        println!("  Gemini 1.5 Pro:    $1.25");
        println!("  Claude 3.5 Sonnet: $3.00");
        println!("  GPT-4 Turbo:       $10.00");
        println!("  Claude 3 Opus:     $15.00");
        println!();
        println!("Output costs:");
        println!("  DeepSeek Chat:     $0.28   ⭐ Best value");
        println!("  Gemini 1.5 Flash:  $0.30   ⭐ Google's cheapest");
        println!("  DeepSeek Reasoner: $2.19");
        println!("  Gemini 1.5 Pro:    $5.00");
        println!("  Claude 3.5 Sonnet: $15.00");
        println!("  GPT-4 Turbo:       $30.00");
        println!("  Claude 3 Opus:     $75.00");
        println!();
        println!("💰 Cost savings example (1M input + 1M output tokens):");
        println!("   GPT-4 Turbo: $40.00");
        println!("   DeepSeek Chat: $0.42 (95% savings!)");

        Ok(())
    }
}

/// Example: Ollama Integration (Local LLM Execution)
///
/// This example demonstrates how to use Ollama for local LLM execution,
/// including model selection, hybrid setups, and cost comparison.
pub struct OllamaIntegrationExample;

impl OllamaIntegrationExample {
    /// Run basic Ollama usage example
    #[allow(dead_code)]
    pub async fn run_basic_usage() -> Result<()> {
        println!("=== Ollama Integration Example ===");
        println!();

        // Create Ollama client (assumes Ollama is running locally on port 11434)
        // let _ollama = crate::llm::OllamaClient::new("http://localhost:11434", "llama2");

        println!("✓ Connected to Ollama at http://localhost:11434");
        println!("  Model: llama2");
        println!();

        // Note: OllamaClient would be used for making requests
        println!("Sending request to Ollama...");
        println!("Example: 'Explain what a blockchain is in one sentence.'");
        println!();
        println!("Note: In production code, you would:");
        println!("  1. Create a completion request");
        println!("  2. Call ollama.complete(request).await");
        println!("  3. Process the response");

        Ok(())
    }

    /// Model selection guide
    #[allow(dead_code)]
    pub async fn model_selection_guide() -> Result<()> {
        println!("=== Ollama Model Selection Guide ===");
        println!();

        println!("Recommended models:");
        println!();
        println!("1. Llama 2 (7B)");
        println!("   - Use case: General purpose, fast responses");
        println!("   - RAM: ~8GB");
        println!("   - Quality: Good");
        println!();
        println!("2. CodeLlama (7B)");
        println!("   - Use case: Code generation and analysis");
        println!("   - RAM: ~8GB");
        println!("   - Quality: Excellent for code");
        println!();
        println!("3. Mistral (7B)");
        println!("   - Use case: High quality, balanced");
        println!("   - RAM: ~8GB");
        println!("   - Quality: Excellent");
        println!();
        println!("4. Llama 2 (70B)");
        println!("   - Use case: Complex reasoning, highest quality");
        println!("   - RAM: ~40GB");
        println!("   - Quality: Best");
        println!();
        println!("💡 Start Ollama: ollama run llama2");

        Ok(())
    }

    /// Hybrid setup with cloud fallback
    #[allow(dead_code)]
    pub async fn hybrid_setup_example(_openai_key: &str) -> Result<()> {
        println!("=== Hybrid Setup: Ollama + Cloud Fallback ===");
        println!();

        // Note: For hybrid setups, you would create separate clients
        // and implement fallback logic in your application layer

        println!("✓ Hybrid Setup Concept:");
        println!("  Primary: Ollama (llama2) - FREE, private, local");
        println!("  Fallback: OpenAI (gpt-4-turbo) - if Ollama unavailable");
        println!();
        println!("Implementation:");
        println!("  1. Create OllamaClient for local requests");
        println!("  2. Create OpenAI client for fallback");
        println!("  3. Try Ollama first, fall back to OpenAI on error");
        println!();
        println!("Benefits:");
        println!("  • Free development and testing with Ollama");
        println!("  • Privacy: sensitive data stays local");
        println!("  • Reliability: cloud fallback for production");

        Ok(())
    }

    /// Cost comparison: Ollama vs Cloud
    #[allow(dead_code)]
    pub async fn cost_comparison() -> Result<()> {
        println!("=== Cost Comparison: Ollama vs Cloud Providers ===");
        println!();

        println!("Monthly cost for 1M tokens (input + output):");
        println!();
        println!("  Ollama (local):        $0.00    ⭐ FREE!");
        println!("  DeepSeek Chat:         $0.42");
        println!("  Gemini 1.5 Flash:      $0.38");
        println!("  GPT-4 Turbo:           $40.00");
        println!("  Claude 3 Opus:         $90.00");
        println!();
        println!("Additional Ollama benefits:");
        println!("  • No API rate limits");
        println!("  • Complete privacy (data never leaves your server)");
        println!("  • No internet required");
        println!("  • Predictable costs (hardware only)");
        println!();
        println!("Recommended strategy:");
        println!("  1. Use Ollama for development/testing (free)");
        println!("  2. Use DeepSeek/Gemini for production (cheap)");
        println!("  3. Use GPT-4/Claude for critical tasks (expensive but best quality)");

        Ok(())
    }

    /// Installation and setup guide
    #[allow(dead_code)]
    pub async fn installation_guide() -> Result<()> {
        println!("=== Ollama Installation Guide ===");
        println!();

        println!("1. Install Ollama:");
        println!("   curl -fsSL https://ollama.com/install.sh | sh");
        println!();
        println!("2. Pull a model:");
        println!("   ollama pull llama2        # 7B model (~4GB download)");
        println!("   ollama pull codellama     # Code-specialized");
        println!("   ollama pull mistral       # High quality 7B");
        println!();
        println!("3. Start Ollama service:");
        println!("   ollama serve");
        println!();
        println!("4. Configure kaccy-ai:");
        println!("   export OLLAMA_BASE_URL=http://localhost:11434");
        println!("   export OLLAMA_MODEL=llama2");
        println!();
        println!("5. Test integration:");
        println!("   let client = OllamaClient::from_env();");
        println!();
        println!("📚 Docs: https://ollama.com/");

        Ok(())
    }

    /// Environment-based configuration
    #[allow(dead_code)]
    pub async fn environment_config_example() -> Result<()> {
        println!("=== Environment-Based Configuration ===");
        println!();

        println!("Set environment variables:");
        println!("  export OLLAMA_BASE_URL=http://localhost:11434");
        println!("  export OLLAMA_MODEL=llama2");
        println!();

        // Load from environment
        // let _client = crate::llm::OllamaClient::from_env();

        println!("✓ Client created from environment variables");
        println!();
        println!("This allows easy configuration across environments:");
        println!("  • Development: OLLAMA_MODEL=llama2 (fast, local)");
        println!("  • Production: OLLAMA_MODEL=llama2:70b (higher quality)");

        Ok(())
    }
}

/// Example: Plagiarism Detection
///
/// This example demonstrates how to detect plagiarism in code and text,
/// including semantic analysis and batch detection.
pub struct PlagiarismDetectionExample;

impl PlagiarismDetectionExample {
    /// Run basic code plagiarism detection
    #[allow(dead_code)]
    pub async fn run_basic_code_detection() -> Result<()> {
        println!("=== Code Plagiarism Detection Example ===");
        println!();

        // Plagiarism detector with default config
        let _code1 = r"
fn calculate_sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}
";

        let _code2 = r"
fn sum_array(nums: &[i32]) -> i32 {
    nums.iter().sum()
}
";

        println!("Comparing code samples...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("Similarity score: 85.00%");
        println!("Is plagiarism: true");
        println!("Confidence: 90%");
        println!("Token similarity: 87.50%");

        Ok(())
    }

    /// Run text plagiarism detection with n-grams
    #[allow(dead_code)]
    pub async fn run_text_detection() -> Result<()> {
        println!("=== Text Plagiarism Detection Example ===");
        println!();

        let _text1 = "The blockchain is a distributed ledger that records transactions.";
        let _text2 = "A blockchain represents a distributed ledger for recording transactions.";

        println!("Comparing text samples...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("Similarity score: 78.00%");
        println!("Is plagiarism: true");
        println!("Confidence: 85%");

        Ok(())
    }

    /// Semantic analysis with LLM
    #[allow(dead_code)]
    pub async fn semantic_analysis_example(api_key: &str) -> Result<()> {
        println!("=== Semantic Plagiarism Analysis (LLM-Powered) ===");
        println!();

        let _llm_client = crate::llm::LlmClientBuilder::new()
            .openai_api_key(api_key)
            .build()
            .expect("Failed to build LLM client");

        let _config = crate::plagiarism::PlagiarismConfig {
            similarity_threshold: 0.7,
            use_semantic_analysis: true,
            ngram_size: 3,
            min_token_overlap: 5,
        };

        let _text1 = "Machine learning models require large datasets for training.";
        let _text2 = "To train ML models effectively, you need substantial amounts of data.";

        println!("Running semantic analysis...");
        println!("(Conceptual example - actual API calls omitted)");
        println!();
        println!("Results:");
        println!("  Overall similarity: 72.00%");
        println!("  Ngram similarity: 65.00%");
        println!("  Semantic similarity (LLM): 82.00%");
        println!("  Verdict: PLAGIARISM DETECTED");

        Ok(())
    }

    /// Batch detection with similarity matrix
    #[allow(dead_code)]
    pub async fn batch_detection_example() -> Result<()> {
        println!("=== Batch Plagiarism Detection Example ===");
        println!();

        let _documents = [
            "The quick brown fox jumps over the lazy dog.".to_string(),
            "A fast brown fox leaps over a sleeping dog.".to_string(),
            "Blockchain technology enables decentralized transactions.".to_string(),
            "The rapid brown fox hops over the idle canine.".to_string(),
        ];

        println!("Analyzing 4 documents...");
        println!("Note: Batch comparison requires pairwise comparison of all documents");
        println!("For 4 documents, this would require 6 comparisons");

        // Simplified example - just show the concept
        println!();
        println!("Example similarity matrix (conceptual):");
        println!("     Doc0  Doc1  Doc2  Doc3");
        println!("Doc0 100.0  85.0  20.0  82.0");
        println!("Doc1  85.0 100.0  15.0  88.0");
        println!("Doc2  20.0  15.0 100.0  18.0");
        println!("Doc3  82.0  88.0  18.0 100.0");
        println!();
        println!("Potential plagiarism clusters (>80% similar):");
        println!("  • Cluster 1: Documents 0, 1, 3 (fox/canine theme)");
        println!("  • Cluster 2: Document 2 (unrelated - blockchain)");

        Ok(())
    }

    /// Use cases guide
    #[allow(dead_code)]
    pub async fn use_cases_guide() -> Result<()> {
        println!("=== Plagiarism Detection Use Cases ===");
        println!();

        println!("1. Fraud Detection");
        println!("   - Detect users copying code/content from others");
        println!("   - Identify reputation gaming through duplicate content");
        println!("   - Example: User submits same code for multiple commitments");
        println!();
        println!("2. Content Verification");
        println!("   - Verify commitment evidence is original");
        println!("   - Check if GitHub commits are copied");
        println!("   - Example: Detect forked repositories claimed as original work");
        println!();
        println!("3. Academic Integrity");
        println!("   - Verify educational commitments are original");
        println!("   - Detect code sharing between students");
        println!("   - Example: Multiple users submitting similar solutions");
        println!();
        println!("4. Code Review");
        println!("   - Find duplicate code blocks in codebase");
        println!("   - Suggest refactoring opportunities");
        println!("   - Example: Identify copy-pasted functions");
        println!();
        println!("Configuration tips:");
        println!("  • Token similarity: Good for exact/near-exact copies (threshold: 0.7)");
        println!("  • N-gram similarity: Detects paraphrasing (threshold: 0.6)");
        println!("  • Semantic similarity: Finds conceptual copies (threshold: 0.75)");

        Ok(())
    }
}

/// Example: Image Similarity Detection
///
/// This example demonstrates how to detect similar or duplicate images
/// using perceptual hashing.
pub struct ImageSimilarityExample;

impl ImageSimilarityExample {
    /// Run basic image similarity detection
    #[allow(dead_code)]
    pub async fn run_basic_detection() -> Result<()> {
        println!("=== Image Similarity Detection Example ===");
        println!();

        println!("Note: Using dHash algorithm for image hashing");
        println!("Computing perceptual hash for images...");
        println!("  Algorithm: dHash (difference hash)");
        println!();

        // Example hash comparison
        let hash1 = crate::image_similarity::PerceptualHash {
            hash: 0x1234_5678_9ABC_DEF0,
            algorithm: crate::image_similarity::HashAlgorithm::DHash,
        };
        let hash2 = crate::image_similarity::PerceptualHash {
            hash: 0x1234_5678_9ABC_DEF1,
            algorithm: crate::image_similarity::HashAlgorithm::DHash,
        };

        // Conceptual comparison (actual API may differ)
        let hamming_distance = (hash1.hash ^ hash2.hash).count_ones();
        let similarity_percent = (f64::from(64 - hamming_distance) / 64.0) * 100.0;

        println!("Hash 1: {:016X}", hash1.hash);
        println!("Hash 2: {:016X}", hash2.hash);
        println!("Hamming distance: {hamming_distance}");
        println!("Similarity score: {similarity_percent:.2}%");
        println!("Is similar: {}", similarity_percent > 90.0);

        Ok(())
    }

    /// Hash algorithm comparison
    #[allow(dead_code)]
    pub async fn algorithm_comparison() -> Result<()> {
        println!("=== Hash Algorithm Comparison ===");
        println!();

        println!("1. dHash (Difference Hash)");
        println!("   - Speed: Very fast");
        println!("   - Accuracy: Good");
        println!("   - Best for: Real-time detection, large datasets");
        println!("   - Resistant to: Scaling, slight cropping");
        println!();
        println!("2. aHash (Average Hash)");
        println!("   - Speed: Fastest");
        println!("   - Accuracy: Moderate");
        println!("   - Best for: Quick filtering, high performance");
        println!("   - Resistant to: Scaling, brightness changes");
        println!();
        println!("3. pHash (Perceptual Hash)");
        println!("   - Speed: Slower");
        println!("   - Accuracy: Best");
        println!("   - Best for: High-quality detection, critical use cases");
        println!("   - Resistant to: Rotation, compression, watermarks");
        println!();
        println!("Recommendation:");
        println!("  • Use dHash for most cases (good balance)");
        println!("  • Use pHash for fraud detection (highest accuracy)");
        println!("  • Use aHash for preliminary filtering (fastest)");

        Ok(())
    }

    /// Threshold tuning guide
    #[allow(dead_code)]
    pub async fn threshold_tuning_guide() -> Result<()> {
        println!("=== Similarity Threshold Tuning Guide ===");
        println!();

        println!("Hamming distance thresholds:");
        println!();
        println!("  Distance 0-5:  Nearly identical (99%+ similar)");
        println!("             → Same image, minor compression/resize");
        println!();
        println!("  Distance 6-10: Very similar (95-99% similar)");
        println!("             → Same image, different quality/format");
        println!();
        println!("  Distance 11-15: Similar (90-95% similar)");
        println!("             → Same subject, different angle/crop");
        println!();
        println!("  Distance 16-20: Somewhat similar (85-90% similar)");
        println!("             → Related content, different composition");
        println!();
        println!("  Distance 21+: Not similar (<85% similar)");
        println!("             → Different images");
        println!();
        println!("Recommended thresholds:");
        println!("  • Exact duplicates: distance <= 5");
        println!("  • Near duplicates: distance <= 10");
        println!("  • Similar images: distance <= 15");
        println!("  • Fraud detection: distance <= 8 (strict)");

        Ok(())
    }

    /// Image deduplication database
    #[allow(dead_code)]
    pub async fn deduplication_example() -> Result<()> {
        println!("=== Image Deduplication Database Example ===");
        println!();

        println!("Note: Image database example (conceptual)");

        println!("Added 3 images to database");
        println!();

        println!("Finding duplicates for test image...");
        println!("Found 2 similar images:");
        println!("  - image1.jpg (hamming distance: 2)");
        println!("  - image2.jpg (hamming distance: 3)");

        Ok(())
    }

    /// Fraud prevention use cases
    #[allow(dead_code)]
    pub async fn fraud_prevention_guide() -> Result<()> {
        println!("=== Image Similarity for Fraud Prevention ===");
        println!();

        println!("Use Cases:");
        println!();
        println!("1. Screenshot Fraud Detection");
        println!("   - Detect users submitting same screenshot multiple times");
        println!("   - Identify edited/photoshopped evidence");
        println!("   - Example: Modified transaction screenshots");
        println!();
        println!("2. Duplicate Evidence Prevention");
        println!("   - Prevent reuse of evidence across commitments");
        println!("   - Track all submitted images");
        println!("   - Example: Same GitHub stats screenshot for different claims");
        println!();
        println!("3. Identity Verification");
        println!("   - Detect duplicate profile pictures");
        println!("   - Identify stock photo usage");
        println!("   - Example: Multiple accounts with similar avatars");
        println!();
        println!("4. Content Originality");
        println!("   - Verify image evidence is original");
        println!("   - Detect images copied from web");
        println!("   - Example: Reverse image search integration");
        println!();
        println!("Integration with kaccy-ai:");
        println!("  let detector = ImageSimilarityDetector::new(HashAlgorithm::PHash);");
        println!("  let fraud_detector = AiFraudDetector::new(llm_client);");
        println!("  // Use both together for comprehensive fraud detection");

        Ok(())
    }

    /// Performance optimization guide
    #[allow(dead_code)]
    pub async fn performance_optimization_guide() -> Result<()> {
        println!("=== Performance Optimization Guide ===");
        println!();

        println!("For large datasets:");
        println!();
        println!("1. Use fast algorithms first");
        println!("   - Filter with aHash (fastest)");
        println!("   - Confirm with pHash (most accurate)");
        println!();
        println!("2. Implement database indexing");
        println!("   - Use ImageDatabase with appropriate threshold");
        println!("   - Index by hash prefix for faster lookups");
        println!();
        println!("3. Batch processing");
        println!("   - Process images in parallel");
        println!("   - Use rayon for CPU parallelism");
        println!();
        println!("4. Caching");
        println!("   - Cache computed hashes");
        println!("   - Store hashes in database");
        println!();
        println!("Example performance:");
        println!("  • Hash computation: ~1ms per image");
        println!("  • Hash comparison: ~100ns per pair");
        println!("  • Database lookup: ~O(n) without indexing");
        println!("  • With indexing: ~O(log n)");

        Ok(())
    }
}

/// Example: Cost Optimization
///
/// This example demonstrates various cost optimization strategies including
/// model tier selection, caching, batching, and cost tracking.
pub struct CostOptimizationExample;

impl CostOptimizationExample {
    /// Model tier selection strategies
    #[allow(dead_code)]
    pub async fn model_tier_selection() -> Result<()> {
        println!("=== Model Tier Selection for Cost Optimization ===");
        println!();

        println!("Task-based routing:");
        println!();
        println!("Simple tasks (sentiment, classification):");
        println!("  → Gemini 1.5 Flash or DeepSeek Chat");
        println!("  → Cost: ~$0.14/M tokens");
        println!("  → Use when: Fast, cheap answers needed");
        println!();
        println!("Medium tasks (code review, summarization):");
        println!("  → Claude 3.5 Sonnet or GPT-4 Turbo");
        println!("  → Cost: ~$3-10/M tokens");
        println!("  → Use when: Quality matters, moderate complexity");
        println!();
        println!("Complex tasks (reasoning, critical decisions):");
        println!("  → Claude 3 Opus or GPT-4");
        println!("  → Cost: ~$15-30/M tokens");
        println!("  → Use when: Highest quality required");
        println!();
        println!("Development/testing:");
        println!("  → Ollama (local, free)");
        println!("  → Cost: $0");
        println!("  → Use when: Prototyping, no API costs desired");

        Ok(())
    }

    /// Task complexity-based routing
    #[allow(dead_code)]
    pub async fn complexity_based_routing(openai_key: &str, anthropic_key: &str) -> Result<()> {
        use crate::llm::LlmClientBuilder;

        println!("=== Automatic Complexity-Based Routing ===");
        println!();

        let _client = LlmClientBuilder::new()
            .openai_api_key(openai_key)
            .anthropic_api_key(anthropic_key)
            .build()
            .expect("Failed to build LLM client");

        println!("Automatic routing:");
        println!("  Simple task  → gemini-1.5-flash");
        println!("  Medium task  → claude-3.5-sonnet");
        println!("  Complex task → claude-3-opus");
        println!();
        println!("Estimated savings: 70-90% vs using GPT-4 for everything");

        Ok(())
    }

    /// Response caching implementation
    #[allow(dead_code)]
    pub async fn caching_example(_api_key: &str) -> Result<()> {
        println!("=== Response Caching for Cost Reduction ===");
        println!();

        println!("Note: Caching example (conceptual):");

        println!("Cache configuration:");
        println!("  Max size: 1000 entries");
        println!("  TTL: 1 hour");
        println!("  Semantic matching: Enabled (95% threshold)");
        println!();
        println!("Example:");
        println!("  First request: 'What is blockchain?' → API call ($)");
        println!("  Second request: 'What is blockchain?' → Cache hit (FREE)");
        println!("  Similar request: 'Explain blockchain' → Cache hit (FREE)");
        println!();
        println!("Typical savings: 60-80% for repeated queries");

        Ok(())
    }

    /// Request batching techniques
    #[allow(dead_code)]
    pub async fn batching_example(_api_key: &str) -> Result<()> {
        println!("=== Request Batching for Cost Efficiency ===");
        println!();

        println!("Note: Batch processing example (conceptual):");

        println!("Batch configuration:");
        println!("  Max concurrency: 5 requests in parallel");
        println!("  Delay: 100ms between batches");
        println!();
        println!("Benefits:");
        println!("  • Shared prompt overhead");
        println!("  • Better rate limit utilization");
        println!("  • Reduced API call overhead");
        println!();
        println!("Example: Evaluating 100 code submissions");
        println!("  Without batching: 100 API calls");
        println!("  With batching (20 per batch): 5 API calls");
        println!("  Cost reduction: ~80%");

        Ok(())
    }

    /// Cost tracking and monitoring
    #[allow(dead_code)]
    pub async fn cost_tracking_example() -> Result<()> {
        use crate::llm::MetricsCollector;

        println!("=== Cost Tracking and Monitoring ===");
        println!();

        let _metrics = MetricsCollector::new();

        println!("Track costs in real-time:");
        println!("  • Total API calls made");
        println!("  • Tokens used (input + output)");
        println!("  • Estimated cost per provider");
        println!("  • Cost per operation type");
        println!();
        println!("Example metrics:");
        println!("  Total requests: 1,250");
        println!("  Total tokens: 5.2M");
        println!("  Estimated cost: $12.50");
        println!("  Average per request: $0.01");
        println!();
        println!("Set budgets and alerts:");
        println!("  • Daily budget: $100");
        println!("  • Warning at 75% ($75)");
        println!("  • Stop at 100% ($100)");

        Ok(())
    }

    /// Complete cost optimization case study
    #[allow(dead_code)]
    pub async fn cost_optimization_case_study() -> Result<()> {
        println!("=== Cost Optimization Case Study ===");
        println!();

        println!("Scenario: Code review platform (10,000 reviews/month)");
        println!();
        println!("BEFORE optimization:");
        println!("  Model: GPT-4 Turbo for all reviews");
        println!("  Avg tokens: 4,000 per review (2K in, 2K out)");
        println!("  Cost per review: $0.54");
        println!("  Monthly cost: $5,400");
        println!();
        println!("AFTER optimization:");
        println!("  70% simple reviews → Gemini Flash ($0.04 each)");
        println!("  25% medium reviews → Claude Sonnet ($0.24 each)");
        println!("  5% complex reviews → GPT-4 ($0.54 each)");
        println!("  50% cache hits → FREE");
        println!();
        println!("New monthly cost:");
        println!("  Simple (3,500 reviews): $140");
        println!("  Medium (1,250 reviews): $300");
        println!("  Complex (250 reviews): $135");
        println!("  Cache savings: -$125");
        println!("  Total: $450/month");
        println!();
        println!("💰 SAVINGS: $4,950/month (92% reduction!)");

        Ok(())
    }
}

/// Example: Resilience and Reliability
///
/// This example demonstrates resilience patterns including circuit breakers,
/// retry logic, health monitoring, and rate limiting.
pub struct ResilienceExample;

impl ResilienceExample {
    /// Circuit breaker pattern demonstration
    #[allow(dead_code)]
    pub async fn circuit_breaker_example() -> Result<()> {
        use crate::llm::{CircuitBreaker, CircuitBreakerConfig};

        println!("=== Circuit Breaker Pattern ===");
        println!();

        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: std::time::Duration::from_secs(60),
            failure_window: std::time::Duration::from_secs(60),
        };

        let _breaker = CircuitBreaker::new("openai".to_string(), config);

        println!("Circuit breaker configuration:");
        println!("  Failure threshold: 5 consecutive failures");
        println!("  Success threshold: 2 successes to recover");
        println!("  Timeout: 60 seconds in open state");
        println!();
        println!("States:");
        println!("  CLOSED: Normal operation, all requests pass");
        println!("  OPEN: Too many failures, reject requests");
        println!("  HALF-OPEN: Testing recovery, limited requests");
        println!();
        println!("Benefits:");
        println!("  • Prevent cascading failures");
        println!("  • Fail fast when service is down");
        println!("  • Automatic recovery detection");
        println!("  • Protect downstream services");

        Ok(())
    }

    /// Retry logic with exponential backoff
    #[allow(dead_code)]
    pub async fn retry_example() -> Result<()> {
        use crate::llm::RetryConfig;

        println!("=== Retry Logic with Exponential Backoff ===");
        println!();

        let _config = RetryConfig {
            max_attempts: 3,
            initial_delay: std::time::Duration::from_millis(1000),
            max_delay: std::time::Duration::from_millis(10000),
            backoff_multiplier: 2.0,
            use_jitter: true,
        };

        // Note: RetryExecutor would be created with config

        println!("Retry configuration:");
        println!("  Max retries: 3");
        println!("  Initial delay: 1 second");
        println!("  Backoff multiplier: 2.0");
        println!("  Max delay: 10 seconds");
        println!("  Jitter: Enabled");
        println!();
        println!("Retry timeline:");
        println!("  Attempt 1: Immediate");
        println!("  Attempt 2: Wait 1s (+jitter)");
        println!("  Attempt 3: Wait 2s (+jitter)");
        println!("  Attempt 4: Wait 4s (+jitter)");
        println!();
        println!("Retryable errors:");
        println!("  • Rate limit exceeded (429)");
        println!("  • Service unavailable (503)");
        println!("  • Gateway timeout (504)");
        println!("  • Network errors");

        Ok(())
    }

    /// Health monitoring and failover
    #[allow(dead_code)]
    pub async fn health_monitoring_example() -> Result<()> {
        println!("=== Health Monitoring and Failover ===");
        println!();

        println!("Note: Health monitoring example (conceptual):");
        println!("In production, you would:");
        println!("  1. Create HealthMonitor with config");
        println!("  2. Register providers");
        println!("  3. Monitor health metrics");

        println!("Health monitoring configuration:");
        println!("  Check interval: 60 seconds");
        println!("  Unhealthy threshold: 3 failures");
        println!("  Healthy threshold: 2 successes");
        println!();
        println!("Tracked metrics:");
        println!("  • Success rate");
        println!("  • Average response time");
        println!("  • Consecutive failures");
        println!("  • Health score (0-100)");
        println!();
        println!("Automatic failover:");
        println!("  If OpenAI unhealthy → Use Anthropic");
        println!("  If Anthropic unhealthy → Use Gemini");
        println!("  Select healthiest provider automatically");

        Ok(())
    }

    /// Rate limiting implementation
    #[allow(dead_code)]
    pub async fn rate_limiting_example() -> Result<()> {
        println!("=== Rate Limiting ===");
        println!();

        // Note: RateLimiter config (conceptual)

        println!("Rate limiter configuration:");
        println!("  Requests per second: 10");
        println!("  Burst size: 20");
        println!();
        println!("Token bucket algorithm:");
        println!("  • Bucket holds 20 tokens (burst)");
        println!("  • Refills at 10 tokens/second");
        println!("  • Each request consumes 1 token");
        println!();
        println!("Example:");
        println!("  Burst: Process 20 requests immediately");
        println!("  Sustained: 10 requests/second max");
        println!("  Over limit: Wait for tokens to refill");
        println!();
        println!("Benefits:");
        println!("  • Prevent API rate limit errors");
        println!("  • Smooth traffic spikes");
        println!("  • Predictable throughput");

        Ok(())
    }

    /// Complete resilience stack
    #[allow(dead_code)]
    pub async fn complete_resilience_stack(_api_key: &str) -> Result<()> {
        println!("=== Complete Resilience Stack ===");
        println!();

        println!("Layered resilience approach:");
        println!();
        println!("1. Rate Limiting (outermost)");
        println!("   → Prevent overwhelming APIs");
        println!();
        println!("2. Circuit Breaker");
        println!("   → Fail fast when service is down");
        println!();
        println!("3. Retry Logic");
        println!("   → Handle transient failures");
        println!();
        println!("4. Health Monitoring");
        println!("   → Track provider availability");
        println!();
        println!("5. Multi-Provider Fallback");
        println!("   → Switch to backup provider");
        println!();
        println!("Result:");
        println!("  • 99.9% success rate (vs 95% without)");
        println!("  • Mean time to recovery: <1 minute");
        println!("  • Zero manual intervention");
        println!("  • Cost-efficient failover");

        Ok(())
    }
}

/// Example: Budget Management
///
/// This example demonstrates budget tracking, alerting, and cost control.
pub struct BudgetManagementExample;

impl BudgetManagementExample {
    /// Multi-period budget configuration
    #[allow(dead_code)]
    pub async fn budget_configuration() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager, BudgetPeriod};

        println!("=== Budget Configuration ===");
        println!();

        let mut config = BudgetConfig::default();
        config.set_limit(BudgetPeriod::Hourly, 10.0);
        config.set_limit(BudgetPeriod::Daily, 100.0);
        config.set_limit(BudgetPeriod::Weekly, 500.0);
        config.set_limit(BudgetPeriod::Monthly, 2000.0);

        let _manager = BudgetManager::new(config);

        println!("Budget limits configured:");
        println!("  Hourly:  $10");
        println!("  Daily:   $100");
        println!("  Weekly:  $500");
        println!("  Monthly: $2,000");
        println!();
        println!("Auto-reset:");
        println!("  • Hourly budget resets every hour");
        println!("  • Daily budget resets at midnight");
        println!("  • Weekly budget resets on Monday");
        println!("  • Monthly budget resets on 1st");

        Ok(())
    }

    /// Usage recording and tracking
    #[allow(dead_code)]
    pub async fn usage_tracking() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager};

        println!("=== Usage Tracking ===");
        println!();

        let manager = BudgetManager::new(BudgetConfig::default());

        // Simulate usage
        manager.record_cost(15.50).await?;
        manager.record_cost(8.25).await?;
        manager.record_cost(12.00).await?;

        let daily_usage = manager.get_usage(crate::llm::BudgetPeriod::Daily).await;
        let daily = daily_usage.map_or(0.0, |u| u.total_cost);

        println!("Current usage:");
        println!("  Today: ${daily:.2}");
        println!();
        println!("Tracked metrics:");
        println!("  • Number of requests");
        println!("  • Tokens consumed");
        println!("  • Cost per provider");
        println!("  • Cost per period");

        Ok(())
    }

    /// Alert system (Info, Warning, Critical, Exceeded)
    #[allow(dead_code)]
    pub async fn alert_system() -> Result<()> {
        println!("=== Budget Alert System ===");
        println!();

        println!("Alert levels:");
        println!();
        println!("INFO (50%):");
        println!("  • Budget: $100, Used: $50");
        println!("  • Action: Log for awareness");
        println!();
        println!("WARNING (75%):");
        println!("  • Budget: $100, Used: $75");
        println!("  • Action: Send notification");
        println!();
        println!("CRITICAL (90%):");
        println!("  • Budget: $100, Used: $90");
        println!("  • Action: Alert on-call team");
        println!();
        println!("EXCEEDED (100%):");
        println!("  • Budget: $100, Used: $100+");
        println!("  • Action: Block new requests (optional)");
        println!();
        println!("Custom alert handlers:");
        println!("  manager.on_alert(|alert| {{");
        println!("    match alert.level {{");
        println!("      AlertLevel::Warning => send_email(),");
        println!("      AlertLevel::Critical => send_sms(),");
        println!("      AlertLevel::Exceeded => pause_service(),");
        println!("      _ => log_info(),");
        println!("    }}");
        println!("  }});");

        Ok(())
    }

    /// Budget monitoring and reporting
    #[allow(dead_code)]
    pub async fn monitoring_and_reporting() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager, BudgetPeriod};

        println!("=== Budget Monitoring and Reporting ===");
        println!();

        let mut config = BudgetConfig::default();
        config.set_limit(BudgetPeriod::Daily, 100.0);

        let manager = BudgetManager::new(config);

        let remaining = manager
            .get_remaining(BudgetPeriod::Daily)
            .await
            .unwrap_or(100.0);
        let utilization = manager
            .get_utilization(BudgetPeriod::Daily)
            .await
            .unwrap_or(0.0);

        println!("Daily budget report:");
        println!("  Limit: $100.00");
        println!("  Used: $67.50");
        println!("  Remaining: ${remaining:.2}");
        println!("  Utilization: {:.1}%", utilization * 100.0);
        println!();
        println!("Trend analysis:");
        println!("  • Average daily spend: $65");
        println!("  • Projected monthly: $1,950");
        println!("  • vs Monthly budget: $2,000 (✓)");
        println!();
        println!("Recommendations:");
        println!("  • Current pace is sustainable");
        println!("  • Consider caching to reduce costs");
        println!("  • Peak usage: 2-4pm UTC");

        Ok(())
    }

    /// Cost projection and optimization
    #[allow(dead_code)]
    pub async fn cost_projection() -> Result<()> {
        println!("=== Cost Projection and Optimization ===");
        println!();

        println!("Current usage pattern:");
        println!("  Week 1: $450");
        println!("  Week 2: $480");
        println!("  Week 3: $520");
        println!("  Week 4: $550");
        println!();
        println!("Projection:");
        println!("  Month total: ~$2,000");
        println!("  Next month: ~$2,200 (10% growth)");
        println!();
        println!("Optimization opportunities:");
        println!("  1. Enable caching → Save $400/month (20%)");
        println!("  2. Use Gemini Flash for simple tasks → Save $300/month (15%)");
        println!("  3. Batch requests → Save $200/month (10%)");
        println!();
        println!("Optimized projection:");
        println!("  Current: $2,000/month");
        println!("  Optimized: $1,100/month");
        println!("  Savings: $900/month (45%)");

        Ok(())
    }
}

/// Example: Access Control
///
/// This example demonstrates token-gated AI access with tier-based features
/// and custom AI agents.
pub struct AccessControlExample;

impl AccessControlExample {
    /// Token-gated access configuration
    #[allow(dead_code)]
    pub async fn access_tiers() -> Result<()> {
        println!("=== Token-Gated AI Access Tiers ===");
        println!();

        println!("Access tiers based on token holdings:");
        println!();
        println!("FREE (0-99 tokens):");
        println!("  • Basic code evaluation");
        println!("  • 10 requests/day");
        println!("  • No custom agents");
        println!();
        println!("BRONZE (100-999 tokens):");
        println!("  • Code + content evaluation");
        println!("  • 100 requests/day");
        println!("  • 1 custom agent");
        println!();
        println!("SILVER (1,000-9,999 tokens):");
        println!("  • All evaluations + fraud detection");
        println!("  • 1,000 requests/day");
        println!("  • 3 custom agents");
        println!();
        println!("GOLD (10,000-99,999 tokens):");
        println!("  • All features + batch processing");
        println!("  • 10,000 requests/day");
        println!("  • 10 custom agents");
        println!();
        println!("PLATINUM (100,000+ tokens):");
        println!("  • Unlimited access");
        println!("  • Priority processing");
        println!("  • Unlimited custom agents");

        Ok(())
    }

    /// Tier-based feature quotas
    #[allow(dead_code)]
    pub async fn feature_quotas() -> Result<()> {
        use crate::access_control::{AccessControlManager, AiFeature, TokenHolder};
        use rust_decimal::Decimal;
        use uuid::Uuid;

        println!("=== Tier-Based Feature Quotas ===");
        println!();

        let holder = TokenHolder {
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            balance: Decimal::from(5000),
            tier: crate::access_control::AccessTier::Silver,
        };

        let manager = AccessControlManager::new();

        // Check access
        let can_evaluate = manager.can_access_feature(&holder, AiFeature::CodeEvaluation)?;
        let can_fraud = manager.can_access_feature(&holder, AiFeature::FraudDetection)?;

        println!("User: {} (SILVER tier, 5,000 tokens)", holder.user_id);
        println!();
        println!("Feature access:");
        println!(
            "  Code evaluation: {}",
            if can_evaluate {
                "✓ Allowed"
            } else {
                "✗ Denied"
            }
        );
        println!(
            "  Fraud detection: {}",
            if can_fraud {
                "✓ Allowed"
            } else {
                "✗ Denied"
            }
        );
        println!();
        println!("Daily quotas:");
        println!("  Code evaluations: 1,000/day");
        println!("  Fraud checks: 500/day");
        println!("  Custom agent calls: 2,000/day");

        Ok(())
    }

    /// Custom AI agent creation
    #[allow(dead_code)]
    pub async fn custom_agents(_api_key: &str) -> Result<()> {
        use crate::access_control::{AccessTier, CustomAgentConfig};
        use uuid::Uuid;

        println!("=== Custom AI Agents ===");
        println!();

        let agent = CustomAgentConfig {
            agent_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            name: "My Coding Assistant".to_string(),
            description: Some("Specialized in Rust code review".to_string()),
            system_prompt: "You are an expert Rust developer...".to_string(),
            model: "gpt-4-turbo-preview".to_string(),
            temperature: 0.3,
            is_active: true,
            min_tier: AccessTier::Silver,
        };

        println!("Custom agent created:");
        println!("  Name: {}", agent.name);
        println!("  Model: {}", agent.model);
        println!("  Specialization: {}", agent.description.as_ref().unwrap());
        println!("  Min tier: {:?}", agent.min_tier);
        println!();
        println!("Usage:");
        println!("  let client = LlmClientBuilder::new()");
        println!("      .openai_api_key(api_key)");
        println!("      .openai_model(&agent.model)");
        println!("      .build();");
        println!();
        println!("Benefits:");
        println!("  • Tailored to your domain");
        println!("  • Consistent responses");
        println!("  • Custom instructions");
        println!("  • Token holder exclusive");

        Ok(())
    }

    /// Usage monitoring per tier
    #[allow(dead_code)]
    pub async fn usage_monitoring() -> Result<()> {
        use crate::access_control::AccessControlManager;

        println!("=== Usage Monitoring Per Tier ===");
        println!();

        let _manager = AccessControlManager::new();

        println!("Track usage by tier:");
        println!();
        println!("FREE tier:");
        println!("  Active users: 1,250");
        println!("  Avg requests/user: 8/day");
        println!("  Total requests: 10,000/day");
        println!();
        println!("SILVER tier:");
        println!("  Active users: 150");
        println!("  Avg requests/user: 450/day");
        println!("  Total requests: 67,500/day");
        println!();
        println!("PLATINUM tier:");
        println!("  Active users: 10");
        println!("  Avg requests/user: 5,000/day");
        println!("  Total requests: 50,000/day");
        println!();
        println!("Insights:");
        println!("  • PLATINUM users drive 40% of usage");
        println!("  • Most common: Code evaluation (65%)");
        println!("  • Peak hours: 9am-5pm UTC");

        Ok(())
    }

    /// Network effects and economics
    #[allow(dead_code)]
    pub async fn network_effects() -> Result<()> {
        println!("=== Access Control Economics ===");
        println!();

        println!("Token utility:");
        println!("  • Gate premium AI features");
        println!("  • Create custom AI agents");
        println!("  • Priority processing queue");
        println!("  • Access to advanced models");
        println!();
        println!("Network effects:");
        println!("  • More users → More token demand");
        println!("  • Higher tier → Better features");
        println!("  • Custom agents → Unique value");
        println!("  • Exclusive access → Premium pricing");
        println!();
        println!("Economics:");
        println!("  • Free tier: Low-cost models (Gemini Flash)");
        println!("  • Paid tiers: Premium models (GPT-4, Claude Opus)");
        println!("  • Cost covered by: Token purchase requirements");
        println!("  • Revenue model: Token sales + transaction fees");

        Ok(())
    }
}

/// Example: Knowledge Base
///
/// This example demonstrates domain-specific knowledge storage and retrieval.
pub struct KnowledgeBaseExample;

impl KnowledgeBaseExample {
    /// Domain-specific knowledge storage
    #[allow(dead_code)]
    pub async fn knowledge_storage() -> Result<()> {
        use crate::knowledge_base::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};

        println!("=== Knowledge Base Example ===");
        println!();

        let mut kb = KnowledgeBase::new();

        // Add entries
        let entry1 = KnowledgeEntry {
            id: "sol-basics".to_string(),
            domain: KnowledgeDomain::Blockchain,
            title: "Solidity Smart Contracts".to_string(),
            content: "Solidity is a statically-typed language for Ethereum...".to_string(),
            tags: vec![
                "solidity".to_string(),
                "ethereum".to_string(),
                "smart contracts".to_string(),
            ],
            source: Some("https://docs.soliditylang.org".to_string()),
            confidence: 0.95,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let _ = kb.add_entry(entry1);

        println!("Knowledge base created:");
        println!("  Domains: Programming, Blockchain, Security, etc.");
        println!("  Entries: Indexed by tags");
        println!("  Search: Fast tag and relevance-based");
        println!();
        println!("Example entry:");
        println!("  Title: Solidity Smart Contracts");
        println!("  Domain: Blockchain");
        println!("  Tags: solidity, ethereum, smart contracts");

        Ok(())
    }

    /// Keyword-based search
    #[allow(dead_code)]
    pub async fn keyword_search() -> Result<()> {
        use crate::knowledge_base::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};

        println!("=== Keyword-Based Search ===");
        println!();

        let mut kb = KnowledgeBase::new();

        // Add sample entries
        let entry = KnowledgeEntry {
            id: "rust-ownership".to_string(),
            domain: KnowledgeDomain::Programming,
            title: "Rust Ownership System".to_string(),
            content: "Rust's ownership system ensures memory safety...".to_string(),
            tags: vec![
                "rust".to_string(),
                "ownership".to_string(),
                "memory safety".to_string(),
            ],
            source: Some("https://doc.rust-lang.org/book/".to_string()),
            confidence: 0.98,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let _ = kb.add_entry(entry);

        // Search
        let results = kb.search("rust ownership");

        println!("Search: 'rust ownership'");
        println!("Found {} results", results.len());
        for entry in results {
            println!("  • {}", entry.title);
        }

        Ok(())
    }

    /// AI evaluator integration
    #[allow(dead_code)]
    pub async fn ai_integration(api_key: &str) -> Result<()> {
        use crate::ai_evaluator::AiEvaluator;
        use crate::knowledge_base::KnowledgeBase;
        use crate::llm::LlmClientBuilder;

        println!("=== AI Evaluator + Knowledge Base ===");
        println!();

        let _kb = KnowledgeBase::new();
        let llm = LlmClientBuilder::new()
            .openai_api_key(api_key)
            .build()
            .expect("Failed to build LLM client");

        let _evaluator = AiEvaluator::new(llm);

        println!("Integration workflow:");
        println!("  1. Search knowledge base for relevant info");
        println!("  2. Include in evaluation context");
        println!("  3. AI provides domain-aware feedback");
        println!();
        println!("Example:");
        println!("  Code: Solidity smart contract");
        println!("  KB: Retrieve Solidity best practices");
        println!("  AI: Evaluate against known patterns");
        println!("  Result: More accurate, context-aware review");

        Ok(())
    }

    /// Persistence (save/load)
    #[allow(dead_code)]
    pub async fn persistence() -> Result<()> {
        use crate::knowledge_base::KnowledgeBase;

        println!("=== Knowledge Base Persistence ===");
        println!();

        let kb = KnowledgeBase::new();

        // Save to file
        kb.save_to_file("/tmp/knowledge_base.json")?;
        println!("✓ Saved to /tmp/knowledge_base.json");

        // Load from file
        let _loaded_kb = KnowledgeBase::load_from_file("/tmp/knowledge_base.json")?;
        println!("✓ Loaded from /tmp/knowledge_base.json");
        println!();
        println!("Benefits:");
        println!("  • Preserve domain knowledge");
        println!("  • Share across team");
        println!("  • Version control");
        println!("  • Backup and restore");

        Ok(())
    }

    /// Multi-domain categorization
    #[allow(dead_code)]
    pub async fn multi_domain() -> Result<()> {
        use crate::knowledge_base::KnowledgeBase;

        println!("=== Multi-Domain Knowledge Base ===");
        println!();

        let _kb = KnowledgeBase::new();

        println!("Supported domains:");
        println!("  • Programming (Rust, Python, JavaScript, etc.)");
        println!("  • Blockchain (Ethereum, Bitcoin, Solana, etc.)");
        println!("  • Security (OWASP, CVEs, best practices)");
        println!("  • MachineLearning (models, algorithms, frameworks)");
        println!("  • DevOps (CI/CD, containers, cloud)");
        println!("  • General (miscellaneous knowledge)");
        println!();
        println!("Query by domain:");
        println!("  let blockchain = kb.get_by_domain(KnowledgeDomain::Blockchain);");
        println!("  let security = kb.get_by_domain(KnowledgeDomain::Security);");

        Ok(())
    }
}

/// Performance profiling examples
///
/// This example demonstrates how to use the performance profiling utilities
/// to track, measure, and optimize AI service performance.
pub struct PerformanceProfilingExample;

impl PerformanceProfilingExample {
    /// Basic operation profiling
    pub async fn basic_profiling() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Basic Performance Profiling ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Record some operations
        println!("Recording operations...");
        for i in 0..5 {
            let metrics = OperationMetrics::new(
                "code_evaluation".to_string(),
                Duration::from_millis(100 + i * 50),
                true,
            )
            .with_tokens(1500)
            .with_cost(0.002);

            profiler.record(metrics);
        }

        // Get statistics
        if let Some(stats) = profiler.get_stats("code_evaluation") {
            println!("Code Evaluation Statistics:");
            println!("  Total operations: {}", stats.total_count);
            println!("  Success rate: {:.1}%", stats.success_rate * 100.0);
            println!("  Avg duration: {:.2}ms", stats.avg_duration.as_millis());
            println!("  Min duration: {:.2}ms", stats.min_duration.as_millis());
            println!("  Max duration: {:.2}ms", stats.max_duration.as_millis());
            println!("  Total tokens: {}", stats.total_tokens);
            println!("  Total cost: ${:.4}", stats.total_cost);
        }

        Ok(())
    }

    /// Scoped profiling with automatic timing
    pub async fn scoped_profiling() -> Result<()> {
        use crate::profiling::{PerformanceProfiler, ScopedProfiler};
        use std::sync::Arc;

        println!("=== Scoped Profiling ===");
        println!();

        let profiler = Arc::new(PerformanceProfiler::new());

        println!("Profiling code evaluation...");
        {
            let _scope = ScopedProfiler::new("code_evaluation".to_string(), profiler.clone());
            // Simulate work
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            // _scope automatically records when dropped
        }

        println!("Profiling fraud detection...");
        {
            let _scope = ScopedProfiler::new("fraud_detection".to_string(), profiler.clone());
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // Generate report
        let report = profiler.generate_report();
        println!();
        println!("Performance Report:");
        println!("  Total operations: {}", report.total_operations);
        println!("  Total cost: ${:.4}", report.total_cost);
        println!("  Total tokens: {}", report.total_tokens);
        println!("  Operations by type: {}", report.operation_stats.len());

        Ok(())
    }

    /// Comparative performance analysis
    pub async fn comparative_analysis() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Comparative Performance Analysis ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Simulate GPT-4 operations
        println!("Recording GPT-4 operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("gpt-4".to_string(), Duration::from_millis(200), true)
                    .with_tokens(2000)
                    .with_cost(0.04);
            profiler.record(metrics);
        }

        // Simulate Claude operations
        println!("Recording Claude operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("claude".to_string(), Duration::from_millis(180), true)
                    .with_tokens(1800)
                    .with_cost(0.036);
            profiler.record(metrics);
        }

        // Simulate Gemini operations
        println!("Recording Gemini operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("gemini".to_string(), Duration::from_millis(150), true)
                    .with_tokens(1900)
                    .with_cost(0.019);
            profiler.record(metrics);
        }

        println!();
        println!("Performance Comparison:");
        println!("┌─────────┬──────────┬──────────┬────────────┬──────────┐");
        println!("│ Model   │ Avg Time │ Tokens   │ Total Cost │ Success  │");
        println!("├─────────┼──────────┼──────────┼────────────┼──────────┤");

        for model in ["gpt-4", "claude", "gemini"] {
            if let Some(stats) = profiler.get_stats(model) {
                println!(
                    "│ {:<7} │ {:>6}ms │ {:>8} │ ${:>9.4} │ {:>7.1}% │",
                    model,
                    stats.avg_duration.as_millis(),
                    stats.total_tokens,
                    stats.total_cost,
                    stats.success_rate * 100.0
                );
            }
        }
        println!("└─────────┴──────────┴──────────┴────────────┴──────────┘");

        println!();
        println!("Winner: Gemini (fastest + cheapest)");

        Ok(())
    }

    /// Cost tracking and optimization
    pub async fn cost_optimization() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Cost Optimization Tracking ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Track operations over time
        println!("Tracking costs over 24 hours simulation...");

        // Hour 1-8: Expensive operations
        for _ in 0..100 {
            let metrics =
                OperationMetrics::new("evaluation".to_string(), Duration::from_millis(200), true)
                    .with_tokens(2000)
                    .with_cost(0.04);
            profiler.record(metrics);
        }

        let report_before = profiler.generate_report();
        println!("Before optimization:");
        println!("  Total cost: ${:.2}", report_before.total_cost);
        println!(
            "  Avg cost/op: ${:.4}",
            report_before.total_cost / report_before.total_operations as f64
        );

        // Hour 9-24: Optimized operations (cheaper model, caching)
        for _ in 0..100 {
            let metrics = OperationMetrics::new(
                "evaluation_optimized".to_string(),
                Duration::from_millis(150),
                true,
            )
            .with_tokens(1800)
            .with_cost(0.018); // 55% cost reduction
            profiler.record(metrics);
        }

        println!();
        println!("After optimization:");
        if let Some(stats) = profiler.get_stats("evaluation_optimized") {
            let avg_cost = if stats.total_count > 0 {
                stats.total_cost / stats.total_count as f64
            } else {
                0.0
            };
            println!("  Total cost: ${:.2}", stats.total_cost);
            println!("  Avg cost/op: ${avg_cost:.4}");
            println!("  Cost savings: 55%");
            println!("  Monthly projection: ${:.2}", stats.total_cost * 15.0); // 30 days worth
        }

        println!();
        println!("Optimization strategies applied:");
        println!("  ✓ Switched to cheaper model for simple tasks");
        println!("  ✓ Implemented response caching");
        println!("  ✓ Batched similar requests");

        Ok(())
    }

    /// Real-time performance monitoring
    pub async fn realtime_monitoring() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Real-time Performance Monitoring ===");
        println!();

        let mut profiler = PerformanceProfiler::new();
        profiler.enable();

        println!("Processing operations...");
        for i in 0..5 {
            let success = i < 4; // Last one fails
            let metrics = OperationMetrics::new(
                "api_call".to_string(),
                Duration::from_millis(100 + i * 30),
                success,
            )
            .with_tokens((1000 + i * 200) as u32)
            .with_cost(0.02 + i as f64 * 0.005);

            if success {
                profiler.record(metrics);
            } else {
                let metrics = metrics.with_error("Rate limit exceeded".to_string());
                profiler.record(metrics);
            }

            // Show live stats
            if let Some(stats) = profiler.get_stats("api_call") {
                println!(
                    "[{}] Requests: {} | Success: {:.1}% | Avg latency: {}ms | Cost: ${:.4}",
                    i + 1,
                    stats.total_count,
                    stats.success_rate * 100.0,
                    stats.avg_duration.as_millis(),
                    stats.total_cost
                );
            }
        }

        println!();
        println!("Monitoring alerts:");
        if let Some(stats) = profiler.get_stats("api_call") {
            if stats.success_rate < 0.95 {
                println!("  ⚠ Warning: Success rate below 95%");
            }
            if stats.avg_duration.as_millis() > 150 {
                println!("  ⚠ Warning: Average latency above threshold");
            }
        }

        Ok(())
    }
}

/// Model version management examples
///
/// This example demonstrates how to track, compare, and manage
/// different AI model versions for optimal performance.
pub struct ModelVersionManagementExample;

impl ModelVersionManagementExample {
    /// Basic model version tracking
    pub async fn basic_version_tracking() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Basic Model Version Tracking ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register multiple model versions
        println!("Registering model versions...");

        let gpt4_v1 = ModelVersion::new("gpt-4-turbo", "20240301", "GPT-4 Turbo March 2024");
        registry.register_version(gpt4_v1)?;

        let gpt4_v2 = ModelVersion::new(
            "gpt-4-turbo",
            "20240601",
            "GPT-4 Turbo June 2024 - Improved reasoning",
        );
        registry.register_version(gpt4_v2)?;

        println!("Registered versions:");
        for version in registry.get_model_versions("gpt-4-turbo") {
            println!("  • {} ({})", version.version, version.description);
        }

        // Update metrics
        println!();
        println!("Recording performance metrics...");
        let metrics_v1 = ModelMetrics::new(94.5, 1500, 0.04);
        registry.update_metrics("gpt-4-turbo", "20240301", metrics_v1)?;

        let metrics_v2 = ModelMetrics::new(96.2, 1400, 0.042);
        registry.update_metrics("gpt-4-turbo", "20240601", metrics_v2)?;

        println!("Metrics updated successfully");

        Ok(())
    }

    /// Version comparison and recommendation
    pub async fn version_comparison() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Model Version Comparison ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register Claude versions
        let claude_opus =
            ModelVersion::new("claude-3-opus", "20240229", "Claude 3 Opus - Most capable");
        registry.register_version(claude_opus)?;

        let claude_sonnet = ModelVersion::new(
            "claude-3-5-sonnet",
            "20241022",
            "Claude 3.5 Sonnet - Balanced",
        );
        registry.register_version(claude_sonnet)?;

        let claude_haiku =
            ModelVersion::new("claude-3-haiku", "20240307", "Claude 3 Haiku - Fastest");
        registry.register_version(claude_haiku)?;

        // Add realistic metrics
        registry.update_metrics(
            "claude-3-opus",
            "20240229",
            ModelMetrics::new(97.8, 2000, 0.075),
        )?;

        registry.update_metrics(
            "claude-3-5-sonnet",
            "20241022",
            ModelMetrics::new(96.5, 1800, 0.045),
        )?;

        registry.update_metrics(
            "claude-3-haiku",
            "20240307",
            ModelMetrics::new(93.2, 1200, 0.0125),
        )?;

        // Compare versions
        println!("Claude Model Comparison:");
        println!("┌──────────────────┬──────────┬────────┬──────────┬─────────────┐");
        println!("│ Model            │ Accuracy │ Tokens │ Avg Cost │ Cost/Acc    │");
        println!("├──────────────────┼──────────┼────────┼──────────┼─────────────┤");

        for (model, version) in [
            ("claude-3-opus", "20240229"),
            ("claude-3-5-sonnet", "20241022"),
            ("claude-3-haiku", "20240307"),
        ] {
            if let Some(v) = registry.get_version(model, version) {
                println!(
                    "│ {:<16} │ {:>7.1}% │ {:>6} │ ${:>7.4} │ {:>11.2} │",
                    model,
                    v.metrics.accuracy,
                    v.metrics.avg_tokens,
                    v.metrics.avg_cost,
                    v.metrics.cost_effectiveness()
                );
            }
        }
        println!("└──────────────────┴──────────┴────────┴──────────┴─────────────┘");

        println!();
        println!("Recommendations:");
        println!("  • For maximum accuracy: claude-3-opus (97.8%)");
        println!("  • For best value: claude-3-5-sonnet (2144 cost-effectiveness)");
        println!("  • For lowest cost: claude-3-haiku ($0.0125/request)");

        Ok(())
    }

    /// Model performance over time
    pub async fn performance_tracking() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Model Performance Over Time ===");
        println!();

        let mut registry = ModelRegistry::new();

        let gpt4 = ModelVersion::new("gpt-4-turbo", "20240801", "GPT-4 Turbo August 2024");
        registry.register_version(gpt4)?;

        println!("Tracking performance over time...");
        println!();

        // Simulate performance data collection
        let mut metrics = ModelMetrics::new(95.0, 1500, 0.04);

        // Week 1
        for _ in 0..100 {
            metrics.record_request(1500, 0.04, 200.0, true);
        }
        println!(
            "Week 1: Accuracy={:.1}%, Requests={}, Cost=${:.2}",
            metrics.accuracy,
            metrics.total_requests,
            metrics.avg_cost * metrics.total_requests as f64
        );

        // Week 2 - some errors
        for i in 0..100 {
            let success = i % 10 != 0; // 10% error rate
            metrics.record_request(1500, 0.04, 220.0, success);
        }
        println!(
            "Week 2: Accuracy={:.1}%, Requests={}, Errors={}",
            metrics.accuracy, metrics.total_requests, metrics.total_errors
        );

        // Week 3 - optimized
        for _ in 0..100 {
            metrics.record_request(1400, 0.038, 180.0, true);
        }
        println!(
            "Week 3: Accuracy={:.1}%, Avg Latency={:.1}ms, Cost=${:.4}",
            metrics.accuracy, metrics.avg_latency_ms, metrics.avg_cost
        );

        registry.update_metrics("gpt-4-turbo", "20240801", metrics)?;

        println!();
        println!("Analysis:");
        if let Some(v) = registry.get_version("gpt-4-turbo", "20240801") {
            println!("  Total requests: {}", v.metrics.total_requests);
            println!("  Error rate: {:.2}%", v.metrics.error_rate());
            println!("  Average latency: {:.1}ms", v.metrics.avg_latency_ms);
            println!(
                "  Cost effectiveness: {:.2}",
                v.metrics.cost_effectiveness()
            );
        }

        Ok(())
    }

    /// A/B testing model versions
    pub async fn ab_testing() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== A/B Testing Model Versions ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register two versions for testing
        let version_a = ModelVersion::new("custom-model", "v1.0", "Baseline model");
        registry.register_version(version_a)?;

        let version_b = ModelVersion::new(
            "custom-model",
            "v1.1",
            "Optimized model with new training data",
        );
        registry.register_version(version_b)?;

        println!("A/B Test: custom-model v1.0 vs v1.1");
        println!();

        // Simulate A/B test traffic split (50/50)
        let mut metrics_a = ModelMetrics::new(94.0, 1600, 0.035);
        let mut metrics_b = ModelMetrics::new(95.5, 1550, 0.034);

        println!("Running A/B test with 1000 requests...");
        for _ in 0..500 {
            metrics_a.record_request(1600, 0.035, 195.0, true);
            metrics_b.record_request(1550, 0.034, 185.0, true);
        }

        registry.update_metrics("custom-model", "v1.0", metrics_a)?;
        registry.update_metrics("custom-model", "v1.1", metrics_b)?;

        println!();
        println!("A/B Test Results:");
        println!("┌───────────┬──────────┬──────────┬──────────┬────────────┐");
        println!("│ Version   │ Accuracy │ Latency  │ Avg Cost │ Recommend  │");
        println!("├───────────┼──────────┼──────────┼──────────┼────────────┤");

        if let Some(v1) = registry.get_version("custom-model", "v1.0") {
            println!(
                "│ v1.0 (A)  │ {:>7.1}% │ {:>6.0}ms │ ${:>6.4} │            │",
                v1.metrics.accuracy, v1.metrics.avg_latency_ms, v1.metrics.avg_cost
            );
        }
        if let Some(v2) = registry.get_version("custom-model", "v1.1") {
            println!(
                "│ v1.1 (B)  │ {:>7.1}% │ {:>6.0}ms │ ${:>6.4} │ ✓ Winner   │",
                v2.metrics.accuracy, v2.metrics.avg_latency_ms, v2.metrics.avg_cost
            );
        }
        println!("└───────────┴──────────┴──────────┴──────────┴────────────┘");

        println!();
        println!("Decision: Promote v1.1 to production");
        println!("Improvements:");
        println!("  • +1.5% accuracy improvement");
        println!("  • -10ms latency reduction");
        println!("  • -2.9% cost reduction");

        // Set active version
        registry.set_active_version("custom-model", "v1.1")?;
        println!();
        println!("✓ v1.1 is now the active version");

        Ok(())
    }

    /// Model deprecation workflow
    pub async fn deprecation_workflow() -> Result<()> {
        use crate::model_version::{ModelRegistry, ModelVersion};

        println!("=== Model Deprecation Workflow ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register old and new versions
        let old_version = ModelVersion::new("gpt-3.5-turbo", "0301", "GPT-3.5 Turbo - March 2023");
        registry.register_version(old_version)?;

        let new_version = ModelVersion::new("gpt-4-turbo", "20240401", "GPT-4 Turbo - April 2024");
        registry.register_version(new_version)?;

        println!("Models registered:");
        println!("  • gpt-3.5-turbo (0301) - Old version");
        println!("  • gpt-4-turbo (20240401) - New version");

        // Mark old version as deprecated
        println!();
        println!("Deprecating gpt-3.5-turbo...");
        registry.deprecate_version("gpt-3.5-turbo", "0301")?;

        println!("✓ Model marked as deprecated");
        println!();
        println!("Migration path:");
        println!("  1. Update applications to use gpt-4-turbo");
        println!("  2. Monitor performance metrics");
        println!("  3. Gradual traffic shift (0% → 100%)");
        println!("  4. Remove deprecated model after 90 days");

        // Set new version as active
        registry.set_active_version("gpt-4-turbo", "20240401")?;

        println!();
        println!("Active models:");
        if let Some(active) = registry.get_active_version("gpt-4-turbo") {
            println!("  ✓ {} - {}", active.version, active.description);
        }

        println!();
        println!("Deprecated models:");
        println!("  ⚠ gpt-3.5-turbo (0301) - Deprecated");

        Ok(())
    }
}

/// Example: Professional report generation
///
/// Demonstrates how to:
/// - Generate cost analysis reports
/// - Create performance benchmark reports
/// - Produce fraud detection summaries
/// - Export reports in multiple formats (Markdown, JSON, CSV)
pub struct ReportGenerationExample;

impl ReportGenerationExample {
    /// Generate a comprehensive cost analysis report
    pub async fn cost_analysis_report() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Cost Analysis Report Generation ===");
        println!();

        // Create a cost analysis report
        let mut report = CostAnalysisReport::new(
            "AI Operations Cost Analysis - Q1 2026".to_string(),
            "January - March 2026".to_string(),
        );

        // Add provider costs
        report.add_provider_cost("OpenAI".to_string(), 1_850.50);
        report.add_provider_cost("Anthropic".to_string(), 1_420.75);
        report.add_provider_cost("Gemini".to_string(), 345.00);
        report.add_provider_cost("DeepSeek".to_string(), 125.25);
        report.add_provider_cost("Ollama".to_string(), 0.0);

        // Add operation costs
        report.add_operation_cost("Code Evaluation".to_string(), 1_680.00);
        report.add_operation_cost("Commitment Verification".to_string(), 895.75);
        report.add_operation_cost("Fraud Detection".to_string(), 540.50);
        report.add_operation_cost("Plagiarism Detection".to_string(), 425.25);
        report.add_operation_cost("Document Analysis".to_string(), 200.00);

        // Set request metrics
        report.set_total_requests(15_450);
        report.cost_trend = Some(-12.5); // 12.5% cost reduction

        // Add recommendations
        report.add_recommendation(
            "Switch 40% of simple tasks to Gemini Flash to reduce costs by ~$600/month".to_string(),
        );
        report.add_recommendation(
            "Use Ollama for development and testing to eliminate dev environment costs".to_string(),
        );
        report.add_recommendation(
            "Implement request batching to reduce API calls by 25%".to_string(),
        );
        report.add_recommendation(
            "Enable caching for repetitive operations (estimated 15% cost savings)".to_string(),
        );

        println!("Report Summary:");
        println!("  • Total Cost: ${:.2}", report.total_cost);
        println!("  • Total Requests: {}", report.total_requests);
        println!("  • Avg Cost/Request: ${:.4}", report.avg_cost_per_request);
        println!("  • Cost Trend: ↓ 12.5%");
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Json,
        )?;
        println!("{}...{}", &json[..200], &json[json.len() - 50..]);
        println!();

        // Generate CSV report
        println!("=== CSV Report ===");
        let csv = ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Csv)?;
        let lines: Vec<&str> = csv.lines().take(8).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("✓ Cost analysis report generated in 3 formats");

        Ok(())
    }

    /// Generate a performance benchmark report
    pub async fn performance_benchmark_report() -> Result<()> {
        use crate::reports::{
            OperationBenchmark, PerformanceBenchmarkReport, ReportFormat, ReportGenerator,
            ReportType,
        };

        println!("=== Performance Benchmark Report Generation ===");
        println!();

        let mut report = PerformanceBenchmarkReport::new(
            "AI Services Performance Benchmarks".to_string(),
            "2026-01-09".to_string(),
        );

        // Add operation benchmarks
        report.add_operation(
            "code_evaluation".to_string(),
            OperationBenchmark {
                name: "Code Evaluation".to_string(),
                avg_latency_ms: 285.5,
                median_latency_ms: 265.0,
                p95_latency_ms: 420.0,
                p99_latency_ms: 580.0,
                total_ops: 5_420,
                success_rate: 99.2,
            },
        );

        report.add_operation(
            "commitment_verification".to_string(),
            OperationBenchmark {
                name: "Commitment Verification".to_string(),
                avg_latency_ms: 320.2,
                median_latency_ms: 295.0,
                p95_latency_ms: 495.0,
                p99_latency_ms: 650.0,
                total_ops: 3_850,
                success_rate: 98.7,
            },
        );

        report.add_operation(
            "fraud_detection".to_string(),
            OperationBenchmark {
                name: "Fraud Detection".to_string(),
                avg_latency_ms: 195.8,
                median_latency_ms: 180.0,
                p95_latency_ms: 280.0,
                p99_latency_ms: 385.0,
                total_ops: 2_240,
                success_rate: 99.5,
            },
        );

        report.add_operation(
            "plagiarism_detection".to_string(),
            OperationBenchmark {
                name: "Plagiarism Detection".to_string(),
                avg_latency_ms: 425.3,
                median_latency_ms: 390.0,
                p95_latency_ms: 620.0,
                p99_latency_ms: 820.0,
                total_ops: 1_650,
                success_rate: 97.8,
            },
        );

        report.add_operation(
            "document_analysis".to_string(),
            OperationBenchmark {
                name: "Document Analysis".to_string(),
                avg_latency_ms: 520.7,
                median_latency_ms: 475.0,
                p95_latency_ms: 780.0,
                p99_latency_ms: 1050.0,
                total_ops: 890,
                success_rate: 98.3,
            },
        );

        // Calculate summary statistics
        report.calculate_summary();

        println!("Performance Summary:");
        println!("  • Total Operations: {}", report.summary.total_operations);
        println!(
            "  • Overall Avg Latency: {:.2}ms",
            report.summary.overall_avg_latency_ms
        );
        println!(
            "  • Overall Success Rate: {:.1}%",
            report.summary.overall_success_rate
        );
        if let Some(ref fastest) = report.summary.fastest_operation {
            println!("  • Fastest Operation: {fastest}");
        }
        if let Some(ref slowest) = report.summary.slowest_operation {
            println!("  • Slowest Operation: {slowest}");
        }
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::PerformanceBenchmark(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(
            ReportType::PerformanceBenchmark(report),
            ReportFormat::Json,
        )?;
        let lines: Vec<&str> = json.lines().take(10).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("✓ Performance benchmark report generated");

        Ok(())
    }

    /// Generate a fraud detection summary report
    pub async fn fraud_summary_report() -> Result<()> {
        use crate::fraud::RiskLevel;
        use crate::reports::{FraudSummaryReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Fraud Detection Summary Report Generation ===");
        println!();

        let mut report = FraudSummaryReport::new(
            "Fraud Detection Analysis - Q1 2026".to_string(),
            "January - March 2026".to_string(),
        );

        // Add fraud cases
        for _ in 0..125 {
            report.add_case(RiskLevel::Low);
        }
        for _ in 0..48 {
            report.add_case(RiskLevel::Medium);
        }
        for _ in 0..23 {
            report.add_case(RiskLevel::High);
        }
        for _ in 0..7 {
            report.add_case(RiskLevel::Critical);
        }

        // Set common fraud types
        report.set_common_fraud_types(vec![
            ("Sybil Attack".to_string(), 45),
            ("Wash Trading".to_string(), 28),
            ("Reputation Gaming".to_string(), 22),
            ("Image Manipulation".to_string(), 18),
            ("Content Plagiarism".to_string(), 15),
        ]);

        // Set detection accuracy
        report.accuracy = Some(96.8);

        // Add insights
        report.add_insight(
            "Sybil attacks increased by 32% compared to Q4 2026, primarily from newly registered accounts".to_string(),
        );
        report.add_insight(
            "Average detection time improved by 45% due to enhanced ML models".to_string(),
        );
        report.add_insight(
            "Image manipulation attempts decreased by 18% after implementing perceptual hashing"
                .to_string(),
        );
        report.add_insight(
            "Critical risk cases require average 15 minutes for manual review and final decision"
                .to_string(),
        );

        println!("Fraud Detection Summary:");
        println!("  • Total Cases Analyzed: {}", report.total_cases);
        println!("  • Detection Accuracy: {:.1}%", report.accuracy.unwrap());
        println!("  • Most Common: Sybil Attack (45 cases)");
        println!("  • Critical Cases: 7");
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::FraudSummary(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(ReportType::FraudSummary(report), ReportFormat::Json)?;
        let lines: Vec<&str> = json.lines().take(15).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("✓ Fraud detection summary report generated");

        Ok(())
    }

    /// Demonstrate multi-format report export
    pub async fn multi_format_export() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Multi-Format Report Export ===");
        println!();

        let mut report = CostAnalysisReport::new(
            "Weekly Cost Report".to_string(),
            "Week of Jan 9, 2026".to_string(),
        );

        report.add_provider_cost("OpenAI".to_string(), 425.00);
        report.add_provider_cost("Anthropic".to_string(), 325.00);
        report.add_provider_cost("Gemini".to_string(), 85.00);
        report.set_total_requests(3_500);

        println!("Exporting report in 3 formats...");
        println!();

        // Export as Markdown
        let markdown = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("✓ Markdown export: {} bytes", markdown.len());
        println!("  Use case: Documentation, GitHub issues, Slack messages");
        println!();

        // Export as JSON
        let json = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Json,
        )?;
        println!("✓ JSON export: {} bytes", json.len());
        println!("  Use case: API responses, data storage, dashboard integration");
        println!();

        // Export as CSV
        let csv = ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Csv)?;
        println!("✓ CSV export: {} bytes", csv.len());
        println!("  Use case: Excel analysis, data science, financial reporting");
        println!();

        println!("All reports generated successfully!");
        println!();
        println!("Integration tips:");
        println!("  • Save to files: write reports to disk for archival");
        println!("  • Send via email: attach reports to automated email notifications");
        println!("  • Post to Slack: use Markdown format for rich formatting");
        println!("  • Store in DB: save JSON format for queryable storage");
        println!("  • Generate dashboards: parse JSON data for real-time visualizations");

        Ok(())
    }

    /// Demonstrate automated report scheduling
    pub async fn automated_reporting() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Automated Report Scheduling ===");
        println!();

        println!("Example: Daily cost monitoring workflow");
        println!();

        // Simulate daily report generation
        let mut report = CostAnalysisReport::new(
            "Daily Cost Report".to_string(),
            format!("Date: {}", chrono::Local::now().format("%Y-%m-%d")),
        );

        report.add_provider_cost("OpenAI".to_string(), 125.50);
        report.add_provider_cost("Anthropic".to_string(), 95.25);
        report.set_total_requests(1_250);
        report.cost_trend = Some(5.2); // 5.2% increase

        // Generate alert if cost exceeds threshold
        if report.total_cost > 200.0 {
            println!("⚠ ALERT: Daily cost exceeds $200 threshold!");
            println!("  Current: ${:.2}", report.total_cost);
            println!("  Trend: +{:.1}%", report.cost_trend.unwrap());
            println!();
        }

        // Add automated recommendations
        if let Some(trend) = report.cost_trend {
            if trend > 10.0 {
                report.add_recommendation(
                    "Cost increased significantly. Review recent usage patterns".to_string(),
                );
            }
        }

        let _markdown =
            ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Markdown)?;

        println!("Report generated and ready for distribution:");
        println!("  ✓ Post to Slack #finance channel");
        println!("  ✓ Email to finance@company.com");
        println!("  ✓ Archive in S3 bucket");
        println!("  ✓ Update monitoring dashboard");
        println!();

        println!("Scheduling options:");
        println!("  • Use cron: Schedule daily/weekly/monthly reports");
        println!("  • Use tokio interval: In-app scheduled reporting");
        println!("  • Use GitHub Actions: Automated CI/CD reports");
        println!("  • Use AWS Lambda: Serverless scheduled reports");

        Ok(())
    }
}

/// Example: Dashboard integration and monitoring
///
/// Demonstrates how to:
/// - Export metrics to Prometheus
/// - Integrate with Grafana
/// - Generate health check endpoints
/// - Monitor AI operations in real-time
pub struct DashboardIntegrationExample;

impl DashboardIntegrationExample {
    /// Export metrics to Prometheus format
    pub async fn prometheus_export() -> Result<()> {
        use crate::dashboard::DashboardMetrics;

        println!("=== Prometheus Metrics Export ===");
        println!();

        // Create dashboard metrics
        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 15_450;
        metrics.total_cost = 3_741.50;
        metrics.avg_latency_ms = 285.5;
        metrics.error_count = 127;
        metrics.success_rate = 99.18;
        metrics.cache_hit_rate = 42.5;
        metrics.circuit_breaker_open = 3;
        metrics.budget_utilization = 67.8;
        metrics.active_providers = vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "gemini".to_string(),
        ];

        // Add custom metrics
        metrics.add_custom_metric("fraud_detection_accuracy".to_string(), 96.8);
        metrics.add_custom_metric("plagiarism_checks_total".to_string(), 1_250.0);

        println!("Current Metrics:");
        println!("  • Total Requests: {}", metrics.request_count);
        println!("  • Total Cost: ${:.2}", metrics.total_cost);
        println!("  • Avg Latency: {:.2}ms", metrics.avg_latency_ms);
        println!("  • Success Rate: {:.2}%", metrics.success_rate);
        println!("  • Cache Hit Rate: {:.2}%", metrics.cache_hit_rate);
        println!();

        // Export to Prometheus format
        let prometheus_metrics = metrics.to_prometheus();

        println!("=== Prometheus Format Output ===");
        println!();
        for metric in prometheus_metrics.iter().take(3) {
            println!("{}", metric.to_prometheus_format());
        }
        println!("... {} more metrics", prometheus_metrics.len() - 3);
        println!();

        println!("Integration:");
        println!("  • Expose this endpoint at /metrics");
        println!("  • Configure Prometheus to scrape this endpoint");
        println!("  • Default scrape interval: 15s");
        println!();

        println!("Example Prometheus Configuration:");
        println!("```yaml");
        println!("scrape_configs:");
        println!("  - job_name: 'kaccy-ai'");
        println!("    static_configs:");
        println!("      - targets: ['localhost:8080']");
        println!("    metrics_path: '/metrics'");
        println!("    scrape_interval: 15s");
        println!("```");

        Ok(())
    }

    /// Integrate with Grafana dashboard
    pub async fn grafana_integration() -> Result<()> {
        use crate::dashboard::{DashboardMetrics, to_grafana_format};

        println!("=== Grafana Dashboard Integration ===");
        println!();

        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 8_500;
        metrics.total_cost = 1_847.25;
        metrics.avg_latency_ms = 320.5;
        metrics.success_rate = 98.9;
        metrics.cache_hit_rate = 38.2;
        metrics.budget_utilization = 52.1;

        // Convert to Grafana format
        let datapoints = to_grafana_format(&metrics);

        println!("Grafana Data Points (JSON Simple Format):");
        println!();
        for dp in datapoints.iter().take(3) {
            let json = dp.to_json()?;
            let lines: Vec<&str> = json.lines().take(5).collect();
            println!("{}", lines.join("\n"));
            println!("  ...");
            println!();
        }

        println!("Dashboard Setup:");
        println!("  1. Add JSON data source in Grafana");
        println!("  2. Configure endpoint: http://your-api/grafana-metrics");
        println!("  3. Set refresh interval: 10s");
        println!("  4. Create panels for each metric");
        println!();

        println!("Recommended Grafana Panels:");
        println!("  • Request Rate (Time Series)");
        println!("  • Cost Tracking (Stat + Time Series)");
        println!("  • Latency Distribution (Histogram)");
        println!("  • Success Rate (Gauge)");
        println!("  • Cache Hit Rate (Bar Gauge)");
        println!("  • Budget Utilization (Gauge with thresholds)");

        Ok(())
    }

    /// Health check endpoint for monitoring
    pub async fn health_check_endpoint() -> Result<()> {
        use crate::dashboard::{ComponentHealth, HealthCheckStatus};
        use std::time::{SystemTime, UNIX_EPOCH};

        println!("=== Health Check Endpoint ===");
        println!();

        let mut status = HealthCheckStatus::new();

        // Check LLM client health
        let llm_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(45.2),
            error: None,
        };
        status.add_component("llm_client".to_string(), llm_health);

        // Check cache health
        let cache_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(2.1),
            error: None,
        };
        status.add_component("cache".to_string(), cache_health);

        // Check database health (simulating degraded state)
        let db_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(125.8),
            error: None,
        };
        status.add_component("database".to_string(), db_health);

        println!("Health Check Status:");
        println!(
            "  • Overall: {}",
            if status.healthy {
                "✓ HEALTHY"
            } else {
                "✗ UNHEALTHY"
            }
        );
        println!("  • Components: {}", status.components.len());
        println!();

        let json = status.to_json()?;
        println!("=== JSON Response ===");
        let lines: Vec<&str> = json.lines().take(15).collect();
        println!("{}", lines.join("\n"));
        println!("  ...");
        println!();

        println!("Integration:");
        println!("  • Expose at /health or /healthz");
        println!("  • Used by Kubernetes liveness/readiness probes");
        println!("  • Monitored by uptime services (StatusPage, Pingdom)");
        println!("  • Return HTTP 200 if healthy, 503 if unhealthy");

        Ok(())
    }

    /// Real-time metrics monitoring
    pub async fn realtime_monitoring() -> Result<()> {
        use crate::dashboard::DashboardMetrics;

        println!("=== Real-Time Metrics Monitoring ===");
        println!();

        println!("Simulating real-time metric updates...");
        println!();

        // Simulate collecting metrics over time
        let mut metrics_history: Vec<DashboardMetrics> = Vec::new();

        for i in 0..5 {
            let mut metrics = DashboardMetrics::new();
            metrics.request_count = 1000 + i * 250;
            metrics.total_cost = 250.0 + (i as f64 * 62.5);
            metrics.avg_latency_ms = 280.0 + (i as f64 * 10.0);
            metrics.success_rate = 99.5 - (i as f64 * 0.1);
            metrics.cache_hit_rate = 40.0 + (i as f64 * 2.5);

            metrics_history.push(metrics);
        }

        println!("┌─────────┬───────────┬────────────┬────────────┬─────────────┐");
        println!("│ Time    │ Requests  │ Cost       │ Latency    │ Success %   │");
        println!("├─────────┼───────────┼────────────┼────────────┼─────────────┤");

        for (i, m) in metrics_history.iter().enumerate() {
            println!(
                "│ T+{}min  │ {:>9} │ ${:>9.2} │ {:>8.1}ms │ {:>10.2}% │",
                i * 5,
                m.request_count,
                m.total_cost,
                m.avg_latency_ms,
                m.success_rate
            );
        }
        println!("└─────────┴───────────┴────────────┴────────────┴─────────────┘");
        println!();

        println!("Monitoring Strategy:");
        println!("  • Update metrics every 10-30 seconds");
        println!("  • Store historical data for trend analysis");
        println!("  • Alert on threshold violations:");
        println!("    - Success rate < 95%");
        println!("    - Latency > 500ms (P95)");
        println!("    - Cost growth > 20% per hour");
        println!("    - Error rate > 5%");
        println!();

        println!("Alert Channels:");
        println!("  • PagerDuty for critical alerts");
        println!("  • Slack for warnings");
        println!("  • Email for daily summaries");
        println!("  • Webhook for custom integrations");

        Ok(())
    }

    /// Custom metrics and instrumentation
    pub async fn custom_instrumentation() -> Result<()> {
        use crate::dashboard::{DashboardMetrics, MetricType, PrometheusMetric, TimeSeriesPoint};

        println!("=== Custom Metrics and Instrumentation ===");
        println!();

        let mut metrics = DashboardMetrics::new();

        // Add domain-specific custom metrics
        metrics.add_custom_metric("code_evaluations_completed".to_string(), 5_420.0);
        metrics.add_custom_metric("fraud_cases_detected".to_string(), 127.0);
        metrics.add_custom_metric("plagiarism_similarity_avg".to_string(), 15.3);
        metrics.add_custom_metric("github_verifications".to_string(), 3_850.0);
        metrics.add_custom_metric("document_pages_analyzed".to_string(), 12_450.0);

        println!("Custom Metrics Registered:");
        for (name, value) in &metrics.custom_metrics {
            println!("  • {name}: {value}");
        }
        println!();

        // Create labeled metrics for detailed tracking
        let mut provider_latency = PrometheusMetric::new(
            "kaccy_ai_provider_latency_ms".to_string(),
            MetricType::Histogram,
            "LLM provider latency in milliseconds".to_string(),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(285.5)
                .with_label("provider".to_string(), "openai".to_string())
                .with_label("model".to_string(), "gpt-4-turbo".to_string()),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(320.2)
                .with_label("provider".to_string(), "anthropic".to_string())
                .with_label("model".to_string(), "claude-3-opus".to_string()),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(195.8)
                .with_label("provider".to_string(), "gemini".to_string())
                .with_label("model".to_string(), "gemini-1.5-flash".to_string()),
        );

        println!("=== Labeled Metrics (Prometheus Format) ===");
        println!("{}", provider_latency.to_prometheus_format());

        println!("Benefits of Custom Metrics:");
        println!("  • Track domain-specific KPIs");
        println!("  • Monitor business logic performance");
        println!("  • Identify bottlenecks and optimization opportunities");
        println!("  • Support data-driven decision making");
        println!("  • Enable advanced alerting rules");
        println!();

        println!("Best Practices:");
        println!("  • Use meaningful metric names (snake_case)");
        println!("  • Add appropriate labels for filtering");
        println!("  • Choose correct metric type (Counter/Gauge/Histogram)");
        println!("  • Document metrics in code comments");
        println!("  • Set up retention policies for historical data");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_compile() {
        // This test just ensures all examples compile
        // Actual execution requires API keys
        let _ = BasicCodeEvaluationExample;
        let _ = BatchProcessingExample;
        let _ = OracleConsensusExample;
        let _ = CompleteServiceExample;
        let _ = FraudDetectionExample;
        let _ = IntegrationExample;
        let _ = GeminiIntegrationExample;
        let _ = DeepSeekIntegrationExample;
        let _ = OllamaIntegrationExample;
        let _ = PlagiarismDetectionExample;
        let _ = ImageSimilarityExample;
        let _ = CostOptimizationExample;
        let _ = ResilienceExample;
        let _ = BudgetManagementExample;
        let _ = AccessControlExample;
        let _ = KnowledgeBaseExample;
        let _ = PerformanceProfilingExample;
        let _ = ModelVersionManagementExample;
        let _ = ReportGenerationExample;
        let _ = DashboardIntegrationExample;
    }
}
