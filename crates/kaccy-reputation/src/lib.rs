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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    // --- ScoreComponents: Default gives all zeros ---

    #[test]
    fn test_score_components_default_is_zero() {
        let c = ScoreComponents::default();
        assert_eq!(c.commitment_fulfillment, dec!(0));
        assert_eq!(c.response_time, dec!(0));
        assert_eq!(c.quality_rating, dec!(0));
        assert_eq!(c.community_trust, dec!(0));
        assert_eq!(c.longevity, dec!(0));
    }

    // --- ScoreComponents: weighted calculate_overall ---

    #[test]
    fn test_score_components_calculate_overall_uniform() {
        // When all components = 100, overall = 100 (weights sum to 1)
        let c = ScoreComponents {
            commitment_fulfillment: dec!(100),
            response_time: dec!(100),
            quality_rating: dec!(100),
            community_trust: dec!(100),
            longevity: dec!(100),
        };
        assert_eq!(c.calculate_overall(), dec!(100));
    }

    #[test]
    fn test_score_components_calculate_overall_weighted_sum() {
        // commitment=100(w=0.35), response=0, quality=0, trust=0, longevity=0 => 35
        let c = ScoreComponents {
            commitment_fulfillment: dec!(100),
            response_time: dec!(0),
            quality_rating: dec!(0),
            community_trust: dec!(0),
            longevity: dec!(0),
        };
        assert_eq!(c.calculate_overall(), dec!(35));
    }

    // --- calculate_scaled_overall = 10 * overall ---

    #[test]
    fn test_calculate_scaled_overall_is_ten_times_overall() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(70),
            quality_rating: dec!(90),
            community_trust: dec!(60),
            longevity: dec!(50),
        };
        let overall = c.calculate_overall();
        assert_eq!(c.calculate_scaled_overall(), overall * dec!(10));
    }

    // --- is_all_above_threshold ---

    #[test]
    fn test_is_all_above_threshold_true() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(75),
            quality_rating: dec!(90),
            community_trust: dec!(70),
            longevity: dec!(65),
        };
        assert!(c.is_all_above_threshold(dec!(60)));
    }

    #[test]
    fn test_is_all_above_threshold_false_when_one_below() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(40), // below threshold
            quality_rating: dec!(90),
            community_trust: dec!(70),
            longevity: dec!(65),
        };
        assert!(!c.is_all_above_threshold(dec!(60)));
    }

    // --- average_component_score ---

    #[test]
    fn test_average_component_score() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(100),
            response_time: dec!(80),
            quality_rating: dec!(60),
            community_trust: dec!(40),
            longevity: dec!(20),
        };
        // (100+80+60+40+20)/5 = 60
        assert_eq!(c.average_component_score(), dec!(60));
    }

    // --- weakest_component / strongest_component ---

    #[test]
    fn test_weakest_component_identifies_lowest() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(30), // weakest
            quality_rating: dec!(70),
            community_trust: dec!(60),
            longevity: dec!(50),
        };
        let (name, value) = c.weakest_component();
        assert_eq!(name, "response_time");
        assert_eq!(value, dec!(30));
    }

    #[test]
    fn test_strongest_component_identifies_highest() {
        let c = ScoreComponents {
            commitment_fulfillment: dec!(80),
            response_time: dec!(30),
            quality_rating: dec!(95), // strongest
            community_trust: dec!(60),
            longevity: dec!(50),
        };
        let (name, value) = c.strongest_component();
        assert_eq!(name, "quality_rating");
        assert_eq!(value, dec!(95));
    }

    // --- ReputationTier::from_score boundary values ---

    #[test]
    fn test_reputation_tier_from_score_boundaries() {
        assert_eq!(
            ReputationTier::from_score(dec!(0)),
            ReputationTier::Unverified
        );
        assert_eq!(
            ReputationTier::from_score(dec!(199)),
            ReputationTier::Unverified
        );
        assert_eq!(
            ReputationTier::from_score(dec!(200)),
            ReputationTier::Bronze
        );
        assert_eq!(
            ReputationTier::from_score(dec!(399)),
            ReputationTier::Bronze
        );
        assert_eq!(
            ReputationTier::from_score(dec!(400)),
            ReputationTier::Silver
        );
        assert_eq!(
            ReputationTier::from_score(dec!(599)),
            ReputationTier::Silver
        );
        assert_eq!(ReputationTier::from_score(dec!(600)), ReputationTier::Gold);
        assert_eq!(ReputationTier::from_score(dec!(799)), ReputationTier::Gold);
        assert_eq!(
            ReputationTier::from_score(dec!(800)),
            ReputationTier::Platinum
        );
        assert_eq!(
            ReputationTier::from_score(dec!(899)),
            ReputationTier::Platinum
        );
        assert_eq!(
            ReputationTier::from_score(dec!(900)),
            ReputationTier::Diamond
        );
        assert_eq!(
            ReputationTier::from_score(dec!(1000)),
            ReputationTier::Diamond
        );
    }

    // --- ReputationTier::Display ---

    #[test]
    fn test_reputation_tier_display() {
        assert_eq!(ReputationTier::Unverified.to_string(), "Unverified");
        assert_eq!(ReputationTier::Bronze.to_string(), "Bronze");
        assert_eq!(ReputationTier::Silver.to_string(), "Silver");
        assert_eq!(ReputationTier::Gold.to_string(), "Gold");
        assert_eq!(ReputationTier::Platinum.to_string(), "Platinum");
        assert_eq!(ReputationTier::Diamond.to_string(), "Diamond");
    }

    // --- ReputationTier::FromStr roundtrip ---

    #[test]
    fn test_reputation_tier_from_str_roundtrip() {
        for tier in ReputationTier::all_variants() {
            let s = tier.to_string().to_lowercase();
            let parsed: ReputationTier = s.parse().expect("parse tier from str");
            assert_eq!(parsed, tier);
        }
    }

    #[test]
    fn test_reputation_tier_from_str_invalid() {
        let result: Result<ReputationTier, _> = "invalid_tier".parse();
        assert!(result.is_err());
    }

    // --- ComponentWeights::validate ---

    #[test]
    fn test_component_weights_default_valid() {
        let weights = ComponentWeights::default();
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_component_weights_sum_off_is_invalid() {
        let weights = ComponentWeights {
            commitment_fulfillment: dec!(0.5),
            response_time: dec!(0.5),
            quality_rating: dec!(0.5),
            community_trust: dec!(0.5),
            longevity: dec!(0.5),
        };
        assert!(weights.validate().is_err());
    }

    // --- ReputationScore helpers ---

    fn make_score(score: rust_decimal::Decimal) -> ReputationScore {
        let tier = ReputationTier::from_score(score);
        ReputationScore {
            user_id: Uuid::new_v4(),
            overall_score: score,
            tier,
            components: ScoreComponents::default(),
        }
    }

    #[test]
    fn test_reputation_score_is_critical() {
        let s = make_score(dec!(150));
        assert!(s.is_critical());
        let s2 = make_score(dec!(200));
        assert!(!s2.is_critical());
    }

    #[test]
    fn test_reputation_score_is_excellent() {
        let s = make_score(dec!(950));
        assert!(s.is_excellent());
        let s2 = make_score(dec!(899));
        assert!(!s2.is_excellent());
    }

    #[test]
    fn test_reputation_score_compare_with_improvement() {
        // current=Gold(600), previous=Silver(450): different tiers, improved
        let current = make_score(dec!(600));
        let previous = make_score(dec!(450));
        let cmp = current.compare_with(&previous);
        assert_eq!(cmp.difference, dec!(150));
        assert!(cmp.is_improvement);
        assert!(cmp.tier_changed);
        assert!(cmp.tier_improved);
    }

    #[test]
    fn test_reputation_score_compare_with_decline() {
        let current = make_score(dec!(300));
        let previous = make_score(dec!(450));
        let cmp = current.compare_with(&previous);
        assert!(cmp.difference < dec!(0));
        assert!(!cmp.is_improvement);
    }

    #[test]
    fn test_reputation_score_distance_to_next_tier() {
        let s = make_score(dec!(350)); // Bronze, next is Silver at 400
        let dist = s
            .distance_to_next_tier()
            .expect("distance should exist for Bronze");
        assert_eq!(dist, dec!(50));
    }

    #[test]
    fn test_reputation_score_distance_to_next_tier_diamond_is_none() {
        let s = make_score(dec!(950)); // Diamond - no next tier
        assert!(s.distance_to_next_tier().is_none());
    }

    #[test]
    fn test_reputation_score_tier_progress_percentage() {
        // Score 300 in Bronze range [200,399]: (300-200)/(399-200)*100
        let s = make_score(dec!(300));
        let progress = s.tier_progress_percentage();
        assert!(progress > dec!(0));
        assert!(progress <= dec!(100));
    }
}
