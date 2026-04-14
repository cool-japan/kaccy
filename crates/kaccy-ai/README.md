# kaccy-ai

AI services for Kaccy Protocol - Comprehensive LLM-powered evaluation, verification, fraud detection, and automation.

**Version:** 0.2.0 | **Tests:** 308 passing

## Overview

The `kaccy-ai` crate provides advanced AI capabilities for the Kaccy Protocol, including:

- **Code & Content Evaluation** - Quality scoring using multiple LLM providers
- **Commitment Verification** - Automated evidence validation (GitHub, social media, documents)
- **Fraud Detection** - Sybil attack detection, wash trading, image manipulation, reputation gaming
- **Plagiarism Detection** - Code and text similarity detection with semantic analysis
- **Image Similarity** - Perceptual hashing for duplicate detection and fraud prevention
- **AI Oracle** - Multi-model consensus for automated decision-making with continuous learning
- **Token-Gated AI Access** - Tiered AI features based on token holdings
- **Cost Optimization** - Smart model routing, caching, batching, and request deduplication
- **Local LLM Support** - Privacy-focused deployment with Ollama for zero-cost execution

## Features

### Multi-Provider LLM Integration

Supports multiple LLM providers with automatic fallback:

- **OpenAI** (GPT-4 Turbo, GPT-4o, GPT-3.5 Turbo)
- **Anthropic** (Claude 3 Opus, Claude 3 Sonnet, Claude 3 Haiku)
- **Google Gemini** (Gemini 1.5 Pro, Gemini 1.5 Flash, Gemini 2.0 Flash)
- **DeepSeek** (Chat, Coder, Reasoner) - Cost-effective models optimized for code
- **Ollama** (Llama, Mistral, CodeLlama, etc.) - Privacy-focused local LLM execution

### Core Capabilities

#### 1. Code Evaluation
```rust
use kaccy_ai::{AiEvaluator, EvaluatorConfig};
use kaccy_ai::llm::{LlmClient, OpenAiClient};

let openai = OpenAiClient::with_default_model("your-api-key");
let llm_client = LlmClient::new(Box::new(openai));
let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());

let result = evaluator.evaluate_code(
    "fn factorial(n: u64) -> u64 { if n == 0 { 1 } else { n * factorial(n - 1) } }",
    "rust"
).await?;

println!("Quality: {}, Complexity: {}, Originality: {}",
    result.quality_score, result.complexity_score, result.originality_score);
```

#### 2. Commitment Verification
```rust
use kaccy_ai::{AiCommitmentVerifier, VerificationRequest};

let verifier = AiCommitmentVerifier::new(llm_client);

let request = VerificationRequest {
    commitment: "Release v2.0 of my Rust library".to_string(),
    evidence: "https://github.com/user/repo/releases/tag/v2.0".to_string(),
    context: Some("Library should include async support".to_string()),
    author: Some("github_username".to_string()),
};

let result = verifier.verify_evidence(&request).await?;
println!("Verified: {}, Confidence: {}%", result.verified, result.confidence);
```

#### 3. Fraud Detection
```rust
use kaccy_ai::{FraudAnalysisService, FraudAnalysisInput};

let fraud_service = FraudAnalysisService::new(llm_client);

let input = FraudAnalysisInput {
    user_id: "user123".to_string(),
    content: "Submitted evidence".to_string(),
    metadata: Default::default(),
};

let report = fraud_service.analyze(&input).await?;
println!("Risk Level: {:?}, Findings: {}", report.risk_level, report.findings.len());
```

#### 4. AI Oracle (Multi-Model Consensus)
```rust
use kaccy_ai::{AiOracle, OracleConfig, ConsensusStrategy};

let mut oracle = AiOracle::new(OracleConfig {
    consensus_strategy: ConsensusStrategy::Majority,
    min_confidence: 0.7,
    enable_learning: true,
});

oracle.add_model("gpt-4-turbo", llm_client1, 1.2); // Higher weight
oracle.add_model("claude-3-opus", llm_client2, 1.0);
oracle.add_model("gemini-1.5-pro", llm_client3, 0.8);

let decision = oracle.verify_with_consensus(&verification_request).await?;
println!("Decision: {}, Consensus Confidence: {}", decision.verified, decision.confidence);
```

#### 5. Batch Processing
```rust
use kaccy_ai::{BatchCodeEvaluator, BatchConfig};
use std::sync::Arc;

let batch_config = BatchConfig {
    max_concurrent: 5,
    delay_between_batches_ms: 1000,
    continue_on_error: true,
};

let batch_evaluator = BatchCodeEvaluator::new(Arc::new(evaluator), batch_config);

let codes = vec![
    ("fn add(a: i32, b: i32) -> i32 { a + b }".to_string(), "rust".to_string()),
    ("function multiply(a, b) { return a * b; }".to_string(), "javascript".to_string()),
];

let results = batch_evaluator.evaluate_batch(codes).await?;
println!("Processed: {}/{}", results.successful.len(), results.total);
```

#### 6. Plagiarism Detection
```rust
use kaccy_ai::{PlagiarismDetector, PlagiarismConfig};

let config = PlagiarismConfig {
    similarity_threshold: 0.7,
    use_semantic_analysis: true,
    ngram_size: 3,
    min_token_overlap: 10,
};

let detector = PlagiarismDetector::new(config);

// Detect code plagiarism
let result = detector.detect_code_plagiarism(&code1, &code2).await?;
println!("Similarity: {:.1}%", result.similarity_score * 100.0);
println!("Is Plagiarized: {}", result.is_plagiarized);

// Detect text plagiarism with semantic analysis
let result = detector.detect_text_plagiarism(&text1, &text2).await?;
println!("Token Similarity: {:.1}%", result.details.token_similarity * 100.0);
println!("Semantic Similarity: {:.1}%", result.details.semantic_similarity.unwrap_or(0.0) * 100.0);
```

#### 7. Image Similarity Detection
```rust
use kaccy_ai::{ImageSimilarityDetector, HashAlgorithm, ImageDatabase};

// Create detector with perceptual hashing
let detector = ImageSimilarityDetector::new(10, HashAlgorithm::DHash);

// Hash and compare images
let hash1 = detector.hash_image(&image1_bytes)?;
let hash2 = detector.hash_image(&image2_bytes)?;
let score = detector.compare_hashes(&hash1, &hash2);

if score.is_similar {
    println!("Duplicate detected! {:.1}% similar", score.similarity_percent);
}

// Build deduplication database
let mut db = ImageDatabase::new(detector);
db.add_image("img1.png", &image1_bytes)?;
db.add_image("img2.png", &image2_bytes)?;

let duplicates = db.find_duplicates();
println!("Found {} duplicate pairs", duplicates.len());
```

## Advanced Features

### Cost Optimization

#### Model Router
Automatically routes requests to cost-effective models based on task complexity:

```rust
use kaccy_ai::llm::{ModelRouter, TaskComplexity};

let router = ModelRouter::new(routing_config);

// Simple tasks → Gemini Flash (cheapest)
let result = router.route_and_execute(&simple_request, TaskComplexity::Simple).await?;

// Complex tasks → GPT-4 Turbo or Claude Opus (most capable)
let result = router.route_and_execute(&complex_request, TaskComplexity::High).await?;
```

#### LLM Cache
```rust
use kaccy_ai::llm::{CachedLlmClient, LlmCacheConfig};

let cache_config = LlmCacheConfig {
    ttl_seconds: 3600,
    max_entries: 1000,
    enable_semantic_similarity: true,
    similarity_threshold: 0.95,
};

let cached_client = CachedLlmClient::new(llm_client, cache_config);
```

#### Request Batching
```rust
use kaccy_ai::llm::BatchProcessor;

let batch_processor = BatchProcessor::new(max_batch_size, wait_time_ms);
```

### Resilience & Reliability

#### Circuit Breaker
```rust
use kaccy_ai::llm::{CircuitBreaker, CircuitBreakerConfig};

let circuit_breaker = CircuitBreaker::new(CircuitBreakerConfig {
    failure_threshold: 5,
    timeout_seconds: 60,
    half_open_max_calls: 3,
});

let result = circuit_breaker.call(|| async {
    llm_client.chat(request).await
}).await?;
```

#### Retry with Exponential Backoff
```rust
use kaccy_ai::llm::{RetryExecutor, RetryConfig};

let retry_config = RetryConfig {
    max_attempts: 3,
    initial_delay_ms: 1000,
    max_delay_ms: 10000,
    exponential_base: 2.0,
    jitter: true,
};

let executor = RetryExecutor::new(retry_config);
let result = executor.execute(|| async { api_call().await }).await?;
```

#### Rate Limiting
```rust
use kaccy_ai::llm::{RateLimiter, RateLimiterConfig};

let rate_limiter = RateLimiter::new(RateLimiterConfig {
    requests_per_second: 10.0,
    burst_size: 20,
});

rate_limiter.acquire().await;
let result = llm_client.chat(request).await?;
```

### Token-Gated AI Access

Create tiered AI access based on token holdings:

```rust
use kaccy_ai::{AccessControlManager, AccessTier, TokenHolder, AiFeature};
use rust_decimal_macros::dec;

let mut access_manager = AccessControlManager::new();

let holder = TokenHolder {
    token_id: "token123".to_string(),
    user_id: "user456".to_string(),
    balance: dec!(50000), // Silver tier (10k-100k tokens)
};

// Check access to features
if access_manager.check_access(&holder, AiFeature::CodeEvaluation)? {
    // User has access to code evaluation
}

// Get feature quota
let quota = access_manager.get_feature_quota(&holder, AiFeature::BatchProcessing)?;
println!("Batch processing quota: {} requests/day", quota.max_requests_per_day);
```

**Access Tiers:**
- **Free** (0-999 tokens): Basic features, limited quotas
- **Bronze** (1k-10k): Increased quotas, fraud detection
- **Silver** (10k-100k): Batch processing, custom agents (up to 3)
- **Gold** (100k-1M): Priority support, advanced features, custom agents (up to 10)
- **Platinum** (1M+): Unlimited access, dedicated support, custom agents (unlimited)

### Knowledge Base

Store and retrieve domain-specific knowledge:

```rust
use kaccy_ai::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};

let mut kb = KnowledgeBase::new();

kb.add_entry(KnowledgeEntry {
    id: "rust-001".to_string(),
    domain: KnowledgeDomain::Programming,
    title: "Rust Ownership Rules".to_string(),
    content: "Each value has a single owner...".to_string(),
    tags: vec!["rust".to_string(), "ownership".to_string()],
    metadata: Default::default(),
});

let results = kb.search("ownership rules", Some(KnowledgeDomain::Programming), 5)?;
```

### Reputation Prediction

Predict future commitment success based on historical data:

```rust
use kaccy_ai::{ReputationPredictor, IssuerHistory};

let predictor = ReputationPredictor::new(llm_client);

let history = IssuerHistory {
    issuer_id: "issuer123".to_string(),
    commitments: vec![/* historical commitments */],
    total_commitments: 50,
    completed_commitments: 45,
    failed_commitments: 5,
    average_completion_time_days: 7.5,
};

let prediction = predictor.predict_commitment_success(&history, "New commitment").await?;
println!("Success probability: {}%, Risk: {:?}",
    prediction.success_probability * 100.0, prediction.risk_level);
```

### GitHub Integration

Comprehensive GitHub verification:

```rust
use kaccy_ai::{GitHubClient, GitHubVerifier};

let github_client = GitHubClient::new("github-token".to_string());
let github_verifier = GitHubVerifier::new(github_client);

// Verify commit
let commit_result = github_verifier.verify_commit(
    "https://github.com/owner/repo",
    "commit-sha",
    Some("expected_author")
).await?;

// Verify release
let release_result = github_verifier.verify_release(
    "https://github.com/owner/repo",
    "v1.0.0",
    "Release notes should mention feature X"
).await?;

// Verify PR/Issue closure
let pr_result = github_verifier.verify_pr(
    "https://github.com/owner/repo/pull/123",
    Some("author")
).await?;
```

### Document Analysis

Parse and analyze documents (PDF, Markdown, HTML):

```rust
use kaccy_ai::{DocumentParser, PdfParser, QualityAnalyzer};

// Parse PDF
let pdf_parser = PdfParser::new();
let (text, metadata) = pdf_parser.parse_file("document.pdf")?;

// Analyze document quality
let quality_analyzer = QualityAnalyzer::new();
let quality = quality_analyzer.analyze(&text)?;
println!("Readability: {}, Grammar: {}", quality.readability, quality.grammar);
```

### Social Media Verification

Verify social media posts (Twitter/X, YouTube, LinkedIn):

```rust
use kaccy_ai::{SocialMediaParser, SocialMediaVerifier};

let parser = SocialMediaParser::new();
let details = parser.parse_url("https://twitter.com/user/status/123456")?;

let verifier = SocialMediaVerifier::new(llm_client);
let result = verifier.verify_post(&details, "Expected content").await?;
```

## Configuration Presets

Use pre-configured settings for different environments:

```rust
use kaccy_ai::presets::{
    DevelopmentPreset,
    ProductionPreset,
    HighVolumePreset,
    CostOptimizedPreset
};

// Development: Fast feedback, lower costs
let dev_config = DevelopmentPreset::evaluator_config();
let dev_oracle = DevelopmentPreset::oracle_config();

// Production: High reliability, quality
let prod_config = ProductionPreset::evaluator_config();
let prod_circuit = ProductionPreset::circuit_breaker_config();

// High Volume: Maximum throughput
let hv_batch = HighVolumePreset::batch_config();
let hv_cache = HighVolumePreset::cache_config();

// Cost Optimized: Minimal cost
let cost_routing = CostOptimizedPreset::routing_config();
```

## Service Hub

Use the high-level service hub for simplified configuration:

```rust
use kaccy_ai::{AiServiceBuilder, AiServiceHub};

let service = AiServiceBuilder::new()
    .with_openai("openai-key")
    .with_anthropic("anthropic-key")
    .with_gemini("gemini-key")
    .with_github("github-token")
    .enable_oracle()
    .enable_access_control()
    .build()?;

// Use all services through a single hub
let evaluator = service.evaluator()?;
let verifier = service.verifier()?;
let fraud_detector = service.fraud_detector()?;
let oracle = service.oracle()?;
```

## Environment Variables

```bash
# LLM Provider API Keys
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GEMINI_API_KEY=...

# Model Selection
OPENAI_MODEL=gpt-4-turbo          # Default: gpt-4-turbo
ANTHROPIC_MODEL=claude-3-opus-20240229  # Default: claude-3-opus-20240229
GEMINI_MODEL=gemini-1.5-pro       # Default: gemini-1.5-pro

# GitHub Integration
GITHUB_TOKEN=ghp_...

# Optional Configuration
AI_MAX_TOKENS=2000                # Default: 2000
AI_TEMPERATURE=0.7                # Default: 0.7
```

## Performance & Observability

### Metrics Collection
```rust
use kaccy_ai::llm::MetricsCollector;

let mut metrics = MetricsCollector::new();
// Metrics are automatically collected during operations

let snapshot = metrics.snapshot();
println!("Total requests: {}", snapshot.total_requests);
println!("Average latency: {}ms", snapshot.average_latency_ms);
println!("Error rate: {}%", snapshot.error_rate * 100.0);
```

### Performance Profiling
```rust
use kaccy_ai::profiling::PerformanceProfiler;

let mut profiler = PerformanceProfiler::new();

{
    let _scope = profiler.start_operation("code_evaluation");
    // ... operation ...
}

let report = profiler.generate_report();
println!("{}", report.summary());
```

## Testing

The crate ships with **308 passing tests** covering all LLM providers, evaluators, fraud detection, image similarity, plagiarism detection, oracle consensus, batch processing, and access control.

```bash
# Run unit tests
cargo test -p kaccy-ai

# Run with nextest
cargo nextest run -p kaccy-ai --no-fail-fast

# Run doc tests
cargo test -p kaccy-ai --doc

# Check for warnings
cargo clippy -p kaccy-ai --all-targets --all-features

# Generate documentation
cargo doc -p kaccy-ai --no-deps --open
```

## Examples

See the `examples` module for comprehensive usage examples:

- `BasicCodeEvaluationExample` - Simple code evaluation
- `BatchProcessingExample` - Batch processing with error handling
- `OracleConsensusExample` - Multi-model consensus
- `FraudDetectionExample` - Comprehensive fraud detection
- `GeminiIntegrationExample` - Gemini provider usage
- `CompleteServiceExample` - Full service hub setup
- `IntegrationExample` - Real-world integration patterns

## Architecture

```
kaccy-ai/
├── src/
│   ├── lib.rs                  # Public API exports
│   ├── access_control.rs       # Token-gated AI access
│   ├── ai_evaluator.rs         # Core AI evaluator
│   ├── batch.rs                # Batch processing utilities
│   ├── document.rs             # Document parsing and analysis
│   ├── error.rs                # Error types
│   ├── evaluator.rs            # Quality evaluation trait
│   ├── evidence.rs             # Evidence parsing
│   ├── examples.rs             # Usage examples
│   ├── fraud.rs                # Fraud detection
│   ├── github.rs               # GitHub integration
│   ├── knowledge_base.rs       # Knowledge storage
│   ├── model_version.rs        # Model versioning
│   ├── ocr.rs                  # OCR and image analysis
│   ├── oracle.rs               # AI oracle with consensus
│   ├── presets.rs              # Configuration presets
│   ├── profiling.rs            # Performance profiling
│   ├── reputation_predictor.rs # Reputation prediction
│   ├── service.rs              # Service hub
│   ├── social.rs               # Social media verification
│   ├── token_analyzer.rs       # Token analysis
│   ├── transcript.rs           # Video transcript analysis
│   ├── utils.rs                # Utility functions
│   ├── verifier.rs             # Commitment verifier
│   └── llm/
│       ├── mod.rs              # LLM client abstraction
│       ├── openai.rs           # OpenAI provider
│       ├── anthropic.rs        # Anthropic provider
│       ├── gemini.rs           # Google Gemini provider
│       ├── types.rs            # Common types
│       ├── cache.rs            # Response caching
│       ├── circuit_breaker.rs  # Circuit breaker pattern
│       ├── cost_optimizer.rs   # Cost optimization
│       ├── metrics.rs          # Metrics collection
│       ├── observability.rs    # Logging and tracing
│       ├── rate_limiter.rs     # Rate limiting
│       ├── retry.rs            # Retry logic
│       └── streaming.rs        # Streaming support
└── Cargo.toml
```

## Cost Management

Estimated costs per operation (using GPT-4 Turbo as baseline):

| Operation | Tokens | Cost |
|-----------|--------|------|
| Code Evaluation | ~1,500 | $0.015 |
| Content Evaluation | ~1,200 | $0.012 |
| Commitment Verification | ~2,000 | $0.020 |
| Fraud Detection | ~2,500 | $0.025 |
| Oracle (3 models) | ~6,000 | $0.060 |

**Cost Optimization Strategies:**
1. Use Gemini Flash for simple tasks ($0.000075/1k tokens - 200x cheaper)
2. Enable caching for repeated requests (50-90% cost reduction)
3. Use batch processing to reduce overhead
4. Route by complexity (simple → cheap, complex → powerful)

## Safety & Security

- **Rate Limiting**: Prevents API cost overruns
- **Circuit Breakers**: Protects against cascading failures
- **Human-in-the-Loop**: Escalates uncertain cases
- **Audit Logging**: Comprehensive decision tracking
- **Input Validation**: Prevents injection attacks
- **Quota Management**: Per-tier usage limits

## Dependencies

Core dependencies:
- `tokio` - Async runtime
- `reqwest` - HTTP client
- `serde` / `serde_json` - Serialization
- `async-trait` - Async traits
- `thiserror` - Error handling
- `tracing` - Logging and observability

Specialized:
- `lopdf` - PDF parsing
- `image` - Image processing
- `regex` - Pattern matching
- `rust_decimal` - Precise decimal math

## License

Apache-2.0
