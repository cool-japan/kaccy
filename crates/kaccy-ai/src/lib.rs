//! AI services for Kaccy Protocol
//!
//! This crate provides AI-powered quality evaluation,
//! content verification, fraud detection, document analysis,
//! social media verification, OCR, and GitHub integration.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::float_cmp)]
#![allow(clippy::unused_self)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::for_kv_map)]
#![allow(clippy::ref_option)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::unnecessary_map_or)]

pub mod access_control;
pub mod ai_evaluator;
pub mod batch;
pub mod dashboard;
pub mod document;
pub mod error;
pub mod evaluator;
pub mod evidence;
pub mod examples;
pub mod fraud;
pub mod github;
pub mod image_similarity;
pub mod knowledge_base;
pub mod llm;
pub mod model_version;
pub mod ocr;
pub mod oracle;
pub mod plagiarism;
pub mod presets;
pub mod profiling;
pub mod reports;
pub mod reputation_predictor;
pub mod service;
pub mod social;
pub mod token_analyzer;
pub mod transcript;
pub mod utils;
pub mod verifier;

pub use access_control::{
    AccessControlManager, AccessTier, AiFeature, CustomAgentConfig, FeatureQuota, TierConfig,
    TokenHolder,
};
pub use ai_evaluator::{
    AiCommitmentVerifier, AiEvaluator, AiFraudDetector, EvaluatorConfig, FraudCheckRequest,
    FraudCheckResult, VerificationRequest, VerificationResult,
};
pub use batch::{
    BatchCodeEvaluator, BatchCommitmentVerifier, BatchConfig, BatchFraudDetector, BatchResult,
};
pub use dashboard::{
    ComponentHealth, DashboardMetrics, GrafanaDataPoint, HealthCheckStatus, MetricType,
    PrometheusMetric, TimeSeriesPoint, to_grafana_format,
};
pub use document::{
    CodeBlock, DocumentFormat, DocumentMetadata, DocumentParser, DocumentQuality, DocumentStats,
    DocumentStructure, Heading, Image, IssueSeverity, Link, MetadataExtractor, PdfMetadata,
    PdfParseError, PdfParser, QualityAnalyzer, QualityIssue, TocEntry, TocGenerator,
};
pub use error::AiError;
pub use evaluator::{DefaultEvaluator, EvaluationResult, QualityEvaluator};
pub use evidence::{EvidenceParser, EvidenceType, ParsedEvidence, Recommendation};
pub use examples::{
    AccessControlExample, BasicCodeEvaluationExample, BatchProcessingExample,
    BudgetManagementExample, CompleteServiceExample, CostOptimizationExample,
    DashboardIntegrationExample, DeepSeekIntegrationExample, FraudDetectionExample,
    GeminiIntegrationExample, ImageSimilarityExample, IntegrationExample, KnowledgeBaseExample,
    ModelVersionManagementExample, OllamaIntegrationExample, OracleConsensusExample,
    PerformanceProfilingExample, PlagiarismDetectionExample, ReportGenerationExample,
    ResilienceExample,
};
pub use fraud::{
    CommitmentRecord, ComprehensiveFraudReport, FraudAnalysisInput, FraudAnalysisResult,
    FraudAnalysisService, FraudDetector, FraudFinding, FraudType, ImageAnalysisResult,
    ImageManipulationDetector, RelatedAccount, RelationshipType, ReputationGamingDetector,
    RiskLevel, SybilDetector, TradeRecord, WashTradingDetector,
};
pub use github::{
    Commit, CommitVerification, GitHubClient, GitHubConfig, GitHubVerificationResult,
    GitHubVerifier, Issue, IssueVerification, PrVerification, PullRequest, Release,
    ReleaseVerification, Repository, parse_github_url,
};
pub use image_similarity::{
    HashAlgorithm, ImageDatabase, ImageSimilarityDetector, PerceptualHash, SimilarityScore,
};
pub use knowledge_base::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};
pub use llm::{
    AlertLevel, AnthropicClient, BudgetAlert, BudgetConfig, BudgetManager, BudgetPeriod, CacheInfo,
    CacheStats, CachedLlmClient, CachedResponse, ChatMessage, ChatRequest, ChatResponse, ChatRole,
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerMetrics, CircuitState, CompletionRequest,
    CompletionResponse, DeepSeekClient, GeminiClient, HealthCheckConfig, HealthMonitor,
    HealthStatus, HealthSummary, LlmCache, LlmCacheConfig, LlmClient, LlmClientBuilder,
    LlmOperation, LlmProvider, LogLevel, MetricsCollector, MetricsSnapshot, ModelInfo,
    OpenAiClient, OperationTimer, PerformanceSpan, PeriodUsage, ProviderHealth, ProviderMetrics,
    RateLimitGuard, RateLimiter, RateLimiterConfig, RequestDeduplicator, RetryConfig,
    RetryExecutor, RetryPolicy, StreamAccumulator, StreamChunk, StreamHandler, StreamResponse,
    StreamingChatRequest, StreamingChatResponse, StreamingLlmProvider, TieredRateLimiter,
    TokenUsage, collect_stream, retry_with_backoff,
};
pub use model_version::{ModelMetrics, ModelRegistry, ModelVersion, VersionComparison};
pub use ocr::{
    BlockType, BoundingBox, ImageAnalyzer, ImageFormat, LlmOcrConfig, LlmOcrProvider, OcrProvider,
    OcrResult, ScreenshotAnalysis, ScreenshotOcr, TextBlock, TextRegionEstimate,
};
pub use oracle::{
    AiOracle, AutoDecision, BatchLearningResult, ConsensusResult, ConsensusStrategy,
    FeedbackAnalysis, FeedbackEntry, LearningStats, ModelVote, ModelWeight, OracleConfig,
};
pub use plagiarism::{
    BatchPlagiarismDetector, PlagiarismConfig, PlagiarismDetector, PlagiarismReport,
    PlagiarismResult, SimilarityDetails, SimilarityMatrix,
};
pub use presets::{
    AccessTierPresets, CostOptimizedPreset, DevelopmentPreset, HighVolumePreset, ProductionPreset,
};
pub use profiling::{
    OperationMetrics, OperationStats, PerformanceProfiler, PerformanceReport, ScopedProfiler,
};
pub use reports::{
    BenchmarkSummary, CostAnalysisReport, FraudSummaryReport, OperationBenchmark,
    PerformanceBenchmarkReport, ReportFormat, ReportGenerator, ReportType,
};
pub use reputation_predictor::{
    CommitmentPrediction, HistoricalCommitment, Impact, IssuerHistory, IssuerRiskAssessment,
    NewIssuerInfo, ReputationPredictor, RiskFactor, Trend,
};
pub use service::{AiServiceBuilder, AiServiceConfig, AiServiceHub};
pub use social::{
    LinkedInPost, LinkedInPostType, SocialDetails, SocialMediaParser, SocialMediaVerifier,
    SocialPlatform, SocialVerificationResult, TwitterPost, VerificationStatus, VerificationSummary,
    YouTubeVideo,
};
pub use token_analyzer::{
    CommunityMetrics, HistoricalDataPoint, MarketPrediction, SentimentCategory, SentimentScore,
    SocialMention, TokenAnalyzer, TrendDirection,
};
pub use transcript::{
    LlmTranscriptAnalyzer, QualityIndicators, SearchResult, TopicMention, TranscriptAnalysis,
    TranscriptProvider, TranscriptResult, TranscriptSearch, TranscriptSegment, TranscriptService,
    VideoMetadata, VideoPlatform, YouTubeTranscriptProvider,
};
pub use utils::{
    AggregationStrategy, FraudCheckRequestBuilder, VerificationRequestBuilder, aggregate_scores,
    calculate_average, calculate_coefficient_of_variation, calculate_consensus, calculate_median,
    calculate_percentile, calculate_std_dev, calculate_success_rate, calculate_variance,
    calculate_weighted_average, clamp, combine_quality_originality, confidence_to_risk_level,
    format_cost, format_duration, format_file_size, format_percentage, format_tokens,
    is_excellent_score, is_passing_score, normalize_score, retry_with_exponential_backoff,
    score_difference_percent, score_to_grade, score_to_tier, scores_significantly_different,
    validate_confidence, validate_model_name, validate_quality_score, validate_temperature,
    validate_token_count, validate_url,
};
