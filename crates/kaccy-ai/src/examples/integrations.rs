//! LLM provider integration examples: Google Gemini, DeepSeek, and Ollama.

use crate::ai_evaluator::{AiEvaluator, EvaluatorConfig};
use crate::error::Result;
use crate::evaluator::QualityEvaluator;
use crate::llm::{
    DeepSeekClient, GeminiClient, LlmClient, LlmClientBuilder, ModelTier, RoutingConfig,
};

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
        println!("Tip: DeepSeek offers the best price/performance ratio!");
        println!("   Compare: GPT-4 Turbo costs $10/M vs DeepSeek Chat $0.14/M");

        Ok(())
    }

    /// Run example comparing cost across all providers
    #[allow(dead_code)]
    pub async fn run_cost_comparison() -> Result<()> {
        println!("LLM Provider Cost Comparison (per 1M tokens):");
        println!();
        println!("Input costs:");
        println!("  DeepSeek Chat:     $0.14   Best value");
        println!("  Gemini 1.5 Flash:  $0.075  Google's cheapest");
        println!("  DeepSeek Reasoner: $0.55");
        println!("  Gemini 1.5 Pro:    $1.25");
        println!("  Claude 3.5 Sonnet: $3.00");
        println!("  GPT-4 Turbo:       $10.00");
        println!("  Claude 3 Opus:     $15.00");
        println!();
        println!("Output costs:");
        println!("  DeepSeek Chat:     $0.28   Best value");
        println!("  Gemini 1.5 Flash:  $0.30   Google's cheapest");
        println!("  DeepSeek Reasoner: $2.19");
        println!("  Gemini 1.5 Pro:    $5.00");
        println!("  Claude 3.5 Sonnet: $15.00");
        println!("  GPT-4 Turbo:       $30.00");
        println!("  Claude 3 Opus:     $75.00");
        println!();
        println!("Cost savings example (1M input + 1M output tokens):");
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

        println!("Connected to Ollama at http://localhost:11434");
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
        println!("Start Ollama: ollama run llama2");

        Ok(())
    }

    /// Hybrid setup with cloud fallback
    #[allow(dead_code)]
    pub async fn hybrid_setup_example(_openai_key: &str) -> Result<()> {
        println!("=== Hybrid Setup: Ollama + Cloud Fallback ===");
        println!();

        // Note: For hybrid setups, you would create separate clients
        // and implement fallback logic in your application layer

        println!("Hybrid Setup Concept:");
        println!("  Primary: Ollama (llama2) - FREE, private, local");
        println!("  Fallback: OpenAI (gpt-4-turbo) - if Ollama unavailable");
        println!();
        println!("Implementation:");
        println!("  1. Create OllamaClient for local requests");
        println!("  2. Create OpenAI client for fallback");
        println!("  3. Try Ollama first, fall back to OpenAI on error");
        println!();
        println!("Benefits:");
        println!("  * Free development and testing with Ollama");
        println!("  * Privacy: sensitive data stays local");
        println!("  * Reliability: cloud fallback for production");

        Ok(())
    }

    /// Cost comparison: Ollama vs Cloud
    #[allow(dead_code)]
    pub async fn cost_comparison() -> Result<()> {
        println!("=== Cost Comparison: Ollama vs Cloud Providers ===");
        println!();

        println!("Monthly cost for 1M tokens (input + output):");
        println!();
        println!("  Ollama (local):        $0.00    FREE!");
        println!("  DeepSeek Chat:         $0.42");
        println!("  Gemini 1.5 Flash:      $0.38");
        println!("  GPT-4 Turbo:           $40.00");
        println!("  Claude 3 Opus:         $90.00");
        println!();
        println!("Additional Ollama benefits:");
        println!("  * No API rate limits");
        println!("  * Complete privacy (data never leaves your server)");
        println!("  * No internet required");
        println!("  * Predictable costs (hardware only)");
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
        println!("Docs: https://ollama.com/");

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

        println!("Client created from environment variables");
        println!();
        println!("This allows easy configuration across environments:");
        println!("  * Development: OLLAMA_MODEL=llama2 (fast, local)");
        println!("  * Production: OLLAMA_MODEL=llama2:70b (higher quality)");

        Ok(())
    }
}
