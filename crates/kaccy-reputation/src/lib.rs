//! # Kaccy Reputation System
//!
//! A comprehensive, production-ready reputation system for the Kaccy Protocol that provides
//! scoring, commitment tracking, tier management, and advanced analytics.
//!
//! ## Overview
//!
//! The reputation system enables users to build trust through verifiable commitments and
//! track their standing within the community. It includes sophisticated features like:
//!
//! - **Dynamic Scoring**: Multi-component reputation scoring (0-1000 scale)
//! - **Tier System**: 6 tiers from Unverified to Diamond with progressive benefits
//! - **Commitment Tracking**: Create, verify, and track deliverable commitments
//! - **Circuit Breaker**: Automatic protection against low-reputation actors
//! - **Caching Layer**: High-performance in-memory caching
//! - **Analytics**: System-wide statistics and user insights
//! - **Export/Import**: Data portability and backup support
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use kaccy_reputation::*;
//! use rust_decimal_macros::dec;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Calculate a reputation score from components
//! let components = ScoreComponents {
//!     commitment_fulfillment: dec!(85.0),  // 35% weight
//!     response_time: dec!(75.0),           // 15% weight
//!     quality_rating: dec!(90.0),          // 30% weight
//!     community_trust: dec!(80.0),         // 15% weight
//!     longevity: dec!(70.0),               // 5% weight
//! };
//!
//! let overall = components.calculate_overall();      // Weighted average (0-100)
//! let score = components.calculate_scaled_overall(); // Scaled to 0-1000
//!
//! // Determine tier from score
//! let tier = ReputationTier::from_score(score);
//! let benefits = tier.benefits();
//!
//! println!("Score: {}, Tier: {}, Can issue tokens: {}",
//!          score, tier, benefits.can_issue_tokens);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into several key subsystems:
//!
//! ### Core Reputation System
//! - [`score`] - Reputation scoring and calculation
//! - [`tier`] - Tier system and benefits
//! - [`circuit_breaker`] - Automatic restrictions for low reputation
//! - [`confidence`] - Data confidence tracking and weighting
//!
//! ### Commitment System
//! - [`commitment`] - Basic commitment tracking
//! - [`milestone`] - Milestone-based commitments
//! - [`recurring`] - Recurring commitment schedules
//! - [`templates`] - Pre-defined commitment templates
//!
//! ### Advanced Features
//! - [`cache`] - In-memory caching for performance
//! - [`events`] - Event sourcing and notifications
//! - [`holder_ratings`] - Community trust ratings
//! - [`time_decay`] - Inactivity-based score decay
//!
//! ### AI & Verification
//! - [`ai_scoring`] - AI-assisted quality evaluation
//! - [`dispute`] - Dispute resolution and appeals
//! - [`sbt`] - Soulbound token integration
//! - [`portability`] - Cross-platform reputation import/export
//!
//! ### Analytics & Monitoring
//! - [`analytics`] - System-wide statistics
//! - [`audit`] - Comprehensive audit trail
//! - [`batch`] - High-performance bulk operations
//! - [`leaderboard`] - Rankings and achievements
//! - [`maintenance`] - Health checks and data integrity
//!
//! ### Integration
//! - [`integration`] - High-level orchestration layer ([`ReputationOrchestrator`])
//! - [`validation`] - Input validation utilities
//! - [`export_import`] - Data export and import
//!
//! ## Performance Considerations
//!
//! - Use [`ReputationCache`] for frequently accessed scores (reduces database load)
//! - Run [`batch`] operations during off-peak hours
//! - Set appropriate cache TTL based on your update frequency
//! - Use database indexes (see migrations) for optimal query performance
//!
//! ## Error Handling
//!
//! All operations return `Result<T, ReputationError>`. See [`error::ReputationError`]
//! for comprehensive error types and handling patterns.
//!
//! ## Examples
//!
//! See the `examples/` directory for comprehensive usage examples:
//! - `basic_reputation_management` - Core concepts and workflows
//! - `commitment_lifecycle` - Managing commitments
//! - `caching_integration` - Performance optimization
//! - `advanced_analytics` - Statistics and insights
//! - `export_import` - Data portability
//!
//! ## Feature Flags
//!
//! This crate has no optional features. All functionality is included by default.
//!
//! [`ReputationOrchestrator`]: integration::ReputationOrchestrator
//! [`ReputationCache`]: cache::ReputationCache

// Phase 0-1 modules
pub mod cache;
pub mod circuit_breaker;
pub mod commitment;
pub mod confidence;
pub mod error;
pub mod events;
pub mod export_import;
pub mod history;
pub mod holder_ratings;
pub mod milestone;
pub mod recurring;
pub mod score;
pub mod score_history;
pub mod templates;
pub mod tier;
pub mod time_decay;
pub mod validation;

// Phase 2 modules
pub mod ai_scoring;
pub mod dispute;
pub mod portability;
pub mod sbt;

// Phase 6 modules
pub mod analytics;
pub mod audit;
pub mod batch;
pub mod leaderboard;
pub mod maintenance;

// Integration
pub mod integration;

// Phase 19 modules
pub mod query_builder;

// Phase 20 modules
pub mod statistics;

// Phase 0-1 exports
pub use cache::{CacheCleanupStats, CacheStats, CachedScore, CachedTier, ReputationCache};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerAction, CircuitBreakerStatus};
pub use commitment::{
    CommitmentService, CommitmentStatus, CreateCommitmentRequest, CreateCommitmentRequestBuilder,
    OutputCommitment, SubmitEvidenceRequest, VerifyCommitmentRequest,
};
pub use confidence::{
    ComponentConfidence, ConfidenceBreakdown, ConfidenceLevel, ConfidenceTracker, WeightedScore,
};
pub use error::ReputationError;
pub use events::{
    CreateEventRequest, EventService, EventSummary, EventType, NotificationHandler,
    NotificationSeverity, RecalculatedScore, ReputationEvent, ScoreNotification,
};
pub use export_import::{
    BulkReputationExport, CommitmentExport, EventExport, ExportImportService, ExportStats,
    RatingExport, ScoreHistoryExport, TierHistoryExport, UserReputationExport,
};
pub use history::{TierHistoryRecord, TierHistoryRepository, TierHistorySummary, TierTracker};
pub use holder_ratings::{
    HolderRating, HolderRatingService, RatingDistribution, RatingRequest, RatingStats,
};
pub use milestone::{
    CreateMilestoneCommitmentRequest, Milestone, MilestoneCommitment, MilestoneService,
    MilestoneStatus,
};
pub use recurring::{
    CreateRecurringRequest, GeneratedCommitment, RecurrenceFrequency, RecurringCommitment,
    RecurringCommitmentService, RecurringSummary, UpcomingDeadline,
};
pub use score::*;
pub use score_history::{ScoreHistoryRepository, ScoreSnapshot, ScoreTrend, TrendDirection};
pub use templates::{
    CategoryCount, CommitmentTemplate, CreateFromTemplateRequest, CreateTemplateRequest,
    TemplateCategory, TemplateService, TemplateSummary, TemplateUsage,
};
pub use tier::*;
pub use time_decay::{DecayConfig, TimeDecayService};
pub use validation::*;

// Phase 2 exports
pub use ai_scoring::{
    AIScoringService, AspectScores, DetectFraudRequest, EvaluateQualityRequest, FraudDetection,
    FraudPattern, PredictSuccessRequest, PredictionFactor, QualityEvaluation, QualityFlag,
    SuccessPrediction,
};
pub use dispute::{
    Appeal, AppealStatus, CreateAppealRequest, CreateDisputeRequest, Dispute, DisputeOutcome,
    DisputeResults, DisputeService, DisputeStatus, DisputeVote, VoteChoice, VoteDisputeRequest,
};
pub use portability::{
    CredentialProof, CredentialSubject, ExportReputationRequest, ExternalReputation,
    GitHubReputation, ImportReputationRequest, ImportResult, LinkedInReputation, Platform,
    PortabilityService, StackOverflowReputation, VerifiableCredential,
};
pub use sbt::{
    MintSBTRequest, OnChainSBT, SBTAttribute, SBTMetadata, SBTService, SoulboundToken,
    UpdateSBTRequest,
};

// Phase 6 exports
pub use analytics::{
    AnalyticsService, GrowthMetrics, ScoreDistribution, SystemStats, TierCount,
    UserCommitmentAnalytics, UserRanking,
};
pub use audit::{
    AuditEvent, AuditEventType, AuditRecord, AuditSearch, AuditService, AuditSummary, EntityType,
    EventTypeCount,
};
pub use batch::{
    BatchDecayResult, BatchExpireResult, BatchScoreResult, BatchService, BatchVerifyResult,
    FailedOperation, FailedVerification, LowScoreUser, PendingCommitmentInfo, ScoreUpdate,
};
pub use leaderboard::{
    Achievement, AchievementType, AwardedAchievement, Leaderboard, LeaderboardEntry,
    LeaderboardService, LeaderboardType, UserRankInfo,
};
pub use maintenance::{HealthCheckReport, MaintenanceReport, MaintenanceService};

// Integration exports
pub use integration::{
    AnalyticsDashboard, BulkVerificationReport, CompleteReputationProfile,
    ComprehensiveUserProfile, CrisisResponse, CrisisSeverity, CrisisType, DailyMaintenanceReport,
    InterventionAction, InterventionReport, InterventionType, PerformanceMetrics,
    ReputationOrchestrator, SystemHealthReport, UserActivityReport, UserOnboardingReport,
};

// Phase 19 exports
pub use query_builder::{
    CommitmentQueryBuilder, EventQueryBuilder, LeaderboardQueryBuilder, QueryResult,
};

// Phase 20 exports
pub use statistics::{
    ComponentCorrelation, MovingAverage, Percentiles, Quartiles, ScoreStatistics, ScoreVolatility,
    Statistics, TrendAnalysis, TrendMomentum,
};
