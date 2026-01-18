# kaccy-ai TODO

## Phase 0 (MVP)

> Note: AI features are NOT included in MVP. Manual admin verification is used instead. This crate provides the foundation for Phase 1 AI integration.

### Basic Structure

- [x] Crate setup
- [x] Evaluator module stub
- [x] Verifier module stub
- [x] Error types

### Manual Verification Support

- [x] Admin verification workflow support
  - [x] Generate verification prompt for admin (generate_verification_prompt())
  - [x] Structure evidence for easy review (ParsedEvidence, StructuredEvidence)
  - [x] Suggest verification decision (SuggestedDecision, Recommendation enum)
- [x] Evidence parsing (evidence.rs module)
  - [x] Extract URLs from evidence text (extract_urls())
  - [x] Basic URL validation (validate_url())
  - [x] Evidence type detection (EvidenceType enum: GitHub, YouTube, Twitter, etc.)
  - [x] Structured data parsing (GitHub commit/PR/release, YouTube video ID, Twitter tweet ID)
  - [x] Screenshot/image support (ocr.rs)
    - [x] OCR provider trait abstraction
    - [x] LLM-based vision OCR (LlmOcrProvider)
    - [x] Image format detection
    - [x] Screenshot analysis

## Phase 1 (Post-MVP)

### LLM Integration

- [x] OpenAI client setup (OpenAiClient)
  - [x] GPT-4 Turbo API integration
  - [x] Request/response handling
  - [x] Streaming support (streaming.rs, StreamingLlmProvider)
- [x] Anthropic client setup (AnthropicClient)
  - [x] Claude API integration (Claude 3 Opus, Sonnet)
  - [x] Streaming support (streaming.rs, StreamingLlmProvider)
- [x] Google Gemini client setup (GeminiClient)
  - [x] Gemini API integration (Gemini 1.5 Pro, Flash, 2.0 Flash)
  - [x] Streaming support (streaming.rs, StreamingLlmProvider)
- [x] Provider abstraction (LlmProvider trait)
  - [x] Common interface for multiple LLMs (OpenAI, Anthropic, Gemini)
  - [x] Easy provider switching (LlmClient, LlmClientBuilder)
  - [x] Fallback provider support
  - [x] Model cost information (ModelInfo)

### Code Evaluator (AiEvaluator)

- [x] Language detection
  - [x] Supports any language via prompt (user specifies)
- [x] Code quality scoring (evaluate_code method)
  - [x] Correctness analysis (quality_score)
  - [x] Complexity scoring (complexity_score)
  - [x] Originality scoring (originality_score)
  - [x] Feedback generation
- [x] GitHub integration (github.rs)
  - [x] Fetch code from repositories (GitHubClient::get_file_contents)
  - [x] Diff analysis for commits (GitHubVerifier::verify_commit)
  - [x] PR review automation (GitHubClient::get_pull_request)
  - [x] Release verification (GitHubVerifier::verify_release)
  - [x] Issue verification (GitHubVerifier::verify_issue)
  - [x] Repository info (GitHubClient::get_repository)

### Content Evaluator (AiEvaluator)

- [x] Text quality scoring (evaluate_content method)
  - [x] Clarity metrics (quality_score)
  - [x] Complexity check (complexity_score)
  - [x] Originality scoring (originality_score)
- [x] Document analysis (document.rs)
  - [x] PDF parsing (lopdf integration)
    - [x] PdfParser for text extraction
    - [x] PDF metadata extraction
    - [x] Link extraction from annotations
  - [x] Markdown/HTML processing
    - [x] DocumentParser with format detection
    - [x] Heading, link, image extraction
    - [x] Code block parsing with language detection
    - [x] QualityAnalyzer for document quality scoring
    - [x] TocGenerator for table of contents
    - [x] MetadataExtractor for document metadata

### Commitment Verifier (AiCommitmentVerifier)

- [x] Evidence parsing (via verify_evidence method)
  - [x] URL validation
  - [x] Screenshot OCR (ocr.rs)
  - [x] Video transcript extraction (transcript.rs)
    - [x] YouTube transcript provider
    - [x] Multi-platform support
    - [x] LLM transcript analysis
- [x] GitHub verification (via GitHubVerifier)
  - [x] Release verification (verify_release)
  - [x] Commit verification (verify_commit)
  - [x] Issue/PR closure verification (verify_issue, verify_pr)
- [x] Social media verification (social.rs)
  - [x] Twitter/X post verification (SocialMediaParser, TwitterPost)
  - [x] YouTube video verification (YouTubeVideo, video ID extraction)
  - [x] LinkedIn post verification (LinkedInPost)
  - [x] Platform detection (SocialPlatform enum)
  - [x] URL parsing and validation
  - [x] Verification confidence scoring
- [x] AI-assisted decision
  - [x] Confidence scoring (0-100)
  - [x] Reasoning explanation
  - [x] Escalation to manual when uncertain (needs_human_review flag)

### Fraud Detection (AiFraudDetector)

- [x] Pattern detection (check_fraud method)
  - [x] Content analysis for gaming indicators
  - [x] User history analysis
- [x] Risk scoring
  - [x] Risk level (low/medium/high/critical)
  - [x] Risk score (0-100)
  - [x] Suspicious indicators list
  - [x] Recommendation generation
- [x] Advanced detection (fraud.rs)
  - [x] Sybil attack detection (SybilDetector)
  - [x] Image manipulation detection (ImageManipulationDetector)
  - [x] Wash trading detection (WashTradingDetector)
  - [x] Reputation gaming detection (ReputationGamingDetector)
  - [x] Comprehensive fraud analysis service (FraudAnalysisService)

## Phase 2 (Post-MVP)

### Advanced AI Features

- [x] AI Oracle (oracle.rs)
  - [x] Fully automated verification
  - [x] Multi-model consensus (Unanimous, Majority, WeightedAverage)
  - [x] Continuous learning from admin decisions
  - [x] Learning statistics tracking
- [x] Reputation prediction (reputation_predictor.rs)
  - [x] Predict future commitment success
  - [x] Risk scoring for new issuers
  - [x] Historical pattern analysis
  - [x] Trend detection (Improving/Neutral/Declining)
- [x] Token value analysis (token_analyzer.rs)
  - [x] Sentiment analysis from social mentions
  - [x] Market prediction (experimental)
  - [x] Community engagement metrics
  - [x] Historical data analysis

### AI Agent Access Control

- [x] Token-gated AI access (access_control.rs)
  - [x] Token holders get AI agent access
  - [x] Tiered access based on holdings (5 tiers: Free, Bronze, Silver, Gold, Platinum)
  - [x] Access control middleware (AccessControlManager)
  - [x] Feature-level access control with quotas
  - [x] Usage tracking and quota management
- [x] Custom AI agents per issuer (CustomAgentConfig)
  - [x] Custom agent registration and management
  - [x] Tier-based agent limits
  - [x] Agent configuration (system prompt, model, temperature)
  - [ ] Trained on issuer's content (requires external ML infrastructure)
  - [ ] Personalized assistance (future enhancement)
  - [ ] Custom model endpoints (future enhancement)

### Model Fine-tuning

> Note: Partial implementation via oracle feedback. Full fine-tuning requires external infrastructure.

- [x] Feedback loop (implemented via oracle.rs)
  - [x] Learn from admin corrections
  - [x] Improve over time via model weight updates
  - [x] Oracle state persistence (save/load to file)
  - [x] Feedback export to CSV for analysis
  - [x] Batch learning capabilities
  - [x] Feedback pattern analysis (false positives/negatives)
  - [x] CSV import for bulk learning
- [x] Domain-specific knowledge (knowledge_base.rs)
  - [x] Knowledge base storage and retrieval
  - [x] Domain categorization (Programming, Blockchain, Security, etc.)
  - [x] Search and relevance scoring
  - [x] Knowledge persistence
- [x] Model versioning and deployment (model_version.rs)
  - [x] Model version tracking
  - [x] Performance metrics per version
  - [x] Version comparison and recommendation
  - [x] Active version management
  - [x] Cost-effectiveness analysis
  - [x] Model deprecation workflow
- [ ] Custom model training (requires external ML infrastructure)
  - [ ] Train on verification decisions
  - [ ] Trained on issuer's content

### Cost Optimization

- [x] Request batching (llm/cost_optimizer.rs)
  - [x] BatchProcessor for aggregating multiple evaluations
  - [x] Configurable batch size and wait time
- [x] Caching (llm/cache.rs)
  - [x] Cache common evaluations with TTL
  - [x] Semantic similarity for cache hits (configurable)
  - [x] Cache statistics and hit rate tracking
  - [x] LRU eviction strategy
- [x] Model routing (llm/cost_optimizer.rs)
  - [x] Use cheaper models for simple tasks
  - [x] Escalate to powerful models when needed
  - [x] Cost estimation and tracking
  - [x] ModelRouter with TaskComplexity levels

### Resilience & Reliability (Phase 2 Enhancements)

- [x] Retry logic with exponential backoff (llm/retry.rs)
  - [x] Configurable retry attempts and delays
  - [x] Exponential backoff with jitter
  - [x] Retryable error detection
  - [x] RetryExecutor with policy-based retries
  - [x] Comprehensive test coverage (11 unit tests)
- [x] Circuit breaker pattern (llm/circuit_breaker.rs)
  - [x] Prevent cascading failures
  - [x] Configurable failure thresholds
  - [x] Half-open state for recovery testing
  - [x] Manual reset capability
  - [x] Circuit breaker metrics
  - [x] Sliding window for failure counting
  - [x] Comprehensive test coverage (6 unit tests)
- [x] Metrics and monitoring (llm/metrics.rs)
  - [x] Request/response latency tracking
  - [x] Token usage monitoring
  - [x] Error rate calculation
  - [x] Provider-specific statistics
  - [x] Operation timer for performance measurement
  - [x] Error type breakdown
  - [x] Comprehensive test coverage (6 unit tests)
- [x] Rate limiting (llm/rate_limiter.rs)
  - [x] Token bucket algorithm implementation
  - [x] Configurable requests per second
  - [x] Burst size support
  - [x] Tiered rate limiting for access control
  - [x] Async-friendly acquire/try_acquire API
  - [x] Automatic token refilling
  - [x] Comprehensive test coverage (8 unit tests)
- [x] Observability and logging (llm/observability.rs)
  - [x] Structured logging helpers
  - [x] LLM operation tracking
  - [x] Performance span measurement
  - [x] Domain-specific logging (evaluation, verification, fraud detection)
  - [x] Cache hit/miss logging
  - [x] Circuit breaker state change tracking
  - [x] Rate limiting alerts
  - [x] Retry attempt logging

### Convenience & Usability (Phase 2+)

- [x] LlmClient cloneability (llm/mod.rs, llm/openai.rs, llm/anthropic.rs)
  - [x] Clone trait implementation for LlmClient
  - [x] clone_box method in LlmProvider trait
  - [x] Clone derive for OpenAiClient and AnthropicClient
  - [x] Enables easy component creation and sharing
- [x] High-level service facade (service.rs)
  - [x] AiServiceHub for unified configuration management
  - [x] Component factory methods (evaluator, verifier, fraud_detector, etc.)
  - [x] AiServiceBuilder for fluent configuration
  - [x] Oracle and GitHub integration helpers
  - [x] Access control integration
  - [x] Knowledge base integration
  - [x] Configuration accessors
- [x] Batch processing utilities (batch.rs)
  - [x] BatchConfig for configuring batch operations
  - [x] BatchCodeEvaluator for parallel code evaluation
  - [x] BatchCommitmentVerifier for batch verification
  - [x] BatchFraudDetector for batch fraud detection
  - [x] BatchResult with success/failure tracking
  - [x] Configurable concurrency, delays, and error handling
- [x] Configuration presets (presets.rs)
  - [x] DevelopmentPreset for dev/test environments
  - [x] ProductionPreset for production deployments
  - [x] HighVolumePreset for high-throughput scenarios
  - [x] CostOptimizedPreset for minimal cost operations
  - [x] AccessTierPresets for all access control tiers
  - [x] Pre-configured settings for all major components
- [x] Comprehensive examples module (examples.rs)
  - [x] BasicCodeEvaluationExample
  - [x] BatchProcessingExample
  - [x] OracleConsensusExample
  - [x] CompleteServiceExample with full setup
  - [x] FraudDetectionExample
  - [x] IntegrationExample for real-world use cases
- [x] Performance profiling utilities (profiling.rs)
  - [x] PerformanceProfiler for operation tracking
  - [x] OperationMetrics for detailed metrics
  - [x] OperationStats for aggregated statistics
  - [x] PerformanceReport generation
  - [x] ScopedProfiler for automatic timing
  - [x] Cost and token usage tracking
- [x] Utility functions and helpers (utils.rs)
  - [x] VerificationRequestBuilder for easy request creation
  - [x] FraudCheckRequestBuilder for fraud checks
  - [x] Validation helpers (URL, confidence, quality scores)
  - [x] Formatting utilities (duration, cost)
  - [x] Statistical functions (average, median, std dev)
  - [x] Score utilities (normalization, clamping, risk levels)
  - [x] Retry helper with exponential backoff
  - [x] Comprehensive test coverage (15 unit tests)
- [x] Testing infrastructure
  - [x] Integration tests for multi-component workflows
  - [x] Performance benchmark specifications
  - [x] 163 total unit tests with 100% pass rate (155 + 8 Gemini tests)
  - [x] 16 doc tests for examples and documentation
