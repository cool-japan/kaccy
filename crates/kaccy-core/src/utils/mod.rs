//! Utility functions and helpers
//!
//! This module provides various utilities for common tasks including pagination,
//! caching, validation, query optimization, and testing.
//!
//! # Modules
//!
//! ## Pagination
//!
//! The [`pagination`] module offers:
//! - Offset-based pagination
//! - Cursor-based pagination
//! - Paginated response wrappers
//!
//! ## Query Builders
//!
//! The [`query`] module provides:
//! - Filtering utilities with multiple operators
//! - Sorting utilities with multi-field support
//! - Date range and numeric range builders
//!
//! ## Query Optimization
//!
//! The [`query_optimization`] module includes:
//! - Index recommendations based on slow queries
//! - Query analysis and performance tracking
//! - N+1 query detection
//!
//! ## Caching
//!
//! The [`cache`] module implements:
//! - In-memory cache with TTL support
//! - Cache invalidation strategies
//! - Cache warming utilities
//! - Multi-get operations
//!
//! ## Validation
//!
//! The [`validation`] module offers:
//! - Email and username validation
//! - BTC address validation
//! - Token symbol validation
//! - Custom validation rules (length, range, URL)
//!
//! ## Serialization
//!
//! The [`serialization`] module provides:
//! - API response wrappers
//! - DTO converters
//! - Partial update helpers
//!
//! ## Batch Operations
//!
//! The [`batch`] module supports:
//! - Bulk insert operations
//! - Bulk update operations
//! - Bulk delete operations
//! - Success rate tracking
//!
//! ## Benchmarking
//!
//! The [`benchmark`] module includes:
//! - Performance testing utilities
//! - Load testing helpers
//! - Profiling tools
//!
//! ## Testing
//!
//! The [`testing`] module provides:
//! - Model factories for common entities
//! - Test fixtures and builders
//! - Mock data generators
//!
//! ## Risk Management
//!
//! The [`risk`] module includes:
//! - Position sizing calculators
//! - Value at Risk (VaR) calculations
//! - Portfolio risk metrics (Sharpe ratio, max drawdown)
//! - Concentration and diversification analysis
//!
//! ## Insurance
//!
//! The [`insurance`] module provides:
//! - Insurance pool management
//! - Coverage policy creation
//! - Claim submission and verification
//! - Premium calculation
//! - Payout processing
//!
//! ## Protocol Protection
//!
//! The [`protocol_protection`] module includes:
//! - Exploit detection
//! - Automatic pause triggers
//! - Security event tracking
//! - Compensation distribution
//! - Recovery mechanisms
//!
//! ## Stress Testing
//!
//! The [`stress_testing`] module provides:
//! - Portfolio stress testing and scenario analysis
//! - Historical stress scenarios (2008 crisis, COVID-19, crypto winter)
//! - Monte Carlo simulation for portfolio risk
//! - Extreme value analysis
//!
//! ## Counterparty Risk
//!
//! The [`counterparty_risk`] module includes:
//! - Credit exposure calculation
//! - Default probability estimation
//! - Exposure aggregation by credit rating
//! - Collateral management and haircuts
//! - Credit Value Adjustment (CVA) calculation
//!
//! ## Liquidity Risk
//!
//! The [`liquidity_risk`] module provides:
//! - Liquidity Coverage Ratio (LCR) calculation
//! - Funding liquidity metrics and gap analysis
//! - Market liquidity scores and categorization
//! - Liquidity stress scenarios and testing

pub mod access_control;
pub mod advanced_cache;
pub mod advanced_risk;
pub mod analytics;
pub mod atomic_swaps;
pub mod attribution_modeling;
pub mod audit_system;
pub mod batch;
pub mod benchmark;
pub mod bridge;
pub mod cache;
pub mod cohort_analysis;
pub mod compliance;
pub mod counterparty_risk;
pub mod cross_chain_liquidity;
pub mod cross_market;
pub mod db_backup;
pub mod db_optimization;
pub mod db_replicas;
pub mod db_sharding;
pub mod distributed_cache;
pub mod dynamic_params;
pub mod economic_security;
pub mod encryption;
pub mod fuzz_testing;
pub mod health;
pub mod institutional;
pub mod insurance;
pub mod integration_testing;
pub mod interoperability;
pub mod kyc_aml;
pub mod liquidity_risk;
pub mod load_testing;
pub mod market_sentiment;
pub mod math;
pub mod memory_optimization;
pub mod memory_profiler;
pub mod metrics;
pub mod mev_redistribution;
pub mod multi_tenancy;
pub mod network_effects;
pub mod nft;
pub mod notifications;
pub mod onchain_analytics;
pub mod oracle;
pub mod pagination;
pub mod pentest;
pub mod performance_benchmark;
pub mod plugin;
pub mod portfolio;
pub mod portfolio_optimization;
pub mod position_limits;
pub mod post_quantum_crypto;
pub mod pretrade_risk;
pub mod protocol_protection;
pub mod query;
pub mod query_optimization;
pub mod rate_limiter;
pub mod regulatory_reporting;
pub mod reporting;
pub mod risk;
pub mod security;
pub mod serialization;
pub mod streaming;
pub mod stress_testing;
pub mod submarine_sends;
pub mod systemic_risk;
pub mod tail_risk;
pub mod testing;
pub mod threshold_encryption;
pub mod token_velocity;
pub mod tracing;
pub mod validation;
pub mod webhooks;
pub mod zero_copy;

pub use access_control::{
    AbacPolicy, AccessControlManager, Attribute, Permission, PolicyCondition, Role, UserRoles,
};
pub use advanced_cache::{CacheStats, CacheWarmer, MultiTierCache, PredictivePreloader};
pub use advanced_risk::{
    ESMethod, ESResult, ExpectedShortfallCalculator, IRCResult, IncrementalRiskCalculator,
    StressVarCalculator, StressVarResult,
};
pub use analytics::*;
pub use atomic_swaps::{
    AtomicSwap, Chain, HTLC, HTLCError, HTLCState, OrderMatcher, OrderMatcherStats, SwapOrder,
    SwapOrderState, SwapState,
};
pub use attribution_modeling::{
    AttributionEngine, AttributionModel, Channel, ChannelAttribution, Conversion, FunnelAnalyzer,
    FunnelEvent, FunnelMetrics, FunnelStage, Touchpoint,
};
pub use batch::*;
pub use benchmark::*;
pub use bridge::*;
pub use cache::*;
pub use cohort_analysis::{
    ChurnFeatures, ChurnRiskLevel, CohortAnalyzer, CohortId, CohortPeriod, CohortRetention,
    CohortType, LtvMetrics, UserActivity,
};
pub use compliance::*;
pub use counterparty_risk::{
    CDSContract, CVACalculator, CollateralOptimizer, CollateralPosition, Counterparty,
    CounterpartyRiskManager, CreditRating, ExpectedLoss, Exposure, NettingAgreement, NettingType,
};
pub use cross_chain_liquidity::{Bridge, LiquidityAggregator, LiquidityPool, Route, RouteSegment};
pub use cross_market::{
    ArbitrageOpportunityEnhanced, ArbitrageType, CorrelationBreakdown, CrossVenueArbitrageDetector,
    MarketRegime, MarketRegimeDetector, PriceCorrelationAnalyzer, PricePoint, RegimeStatistics,
    TradeStep, VenuePriceFeed,
};
pub use db_backup::{
    BackupConfig, BackupManager, BackupMetadata, BackupStats, BackupStatus, BackupType,
    CleanupResult, PITRResult, RestoreResult,
};
pub use db_optimization::{
    BatchQueryExecutor, ConnectionPoolOptimizer, PoolMetrics, PoolSizeRecommendation,
    PreparedStatementCache, PreparedStatementStats, QueryPerformanceAnalyzer,
    QueryPerformanceStats, SlowQuery,
};
pub use db_replicas::{
    FailoverResult, HealthCheckReport, LoadBalanceStrategy, Replica, ReplicaConfig, ReplicaManager,
    ReplicaStats,
};
pub use db_sharding::{
    CrossShardQuery, HashStrategy, RebalanceMove, RebalancePlan, Shard, ShardConfig, ShardManager,
    ShardStats,
};
pub use distributed_cache::{
    CacheEntryState, CacheStatistics, CoherencyProtocol, DistributedCache, MockRedisCache,
    RedisCache, StampedeProtection, WriteBackBuffer, WriteMode,
};
pub use dynamic_params::{
    AlgorithmicFeeAdjuster, FeeAdjustment, IncentiveAllocation, IncentiveOptimizer,
    MarketConditions, MarketState, SupplyAdjustment, SupplyElasticityManager,
};
pub use economic_security::{
    AdjustmentType, AttackType, AttackVector, EconomicSecurityAnalyzer, GameTheoreticModel,
    IncentiveAdjustment, IncentiveAlignmentVerifier, Player, PlayerType, Priority, SecurityLevel,
    SecurityRecommendation, Strategy,
};
pub use encryption::{
    EncryptedField, EncryptionAlgorithm, EncryptionKey, HashUtil, KeyManager, KeyRotationPolicy,
};
pub use fuzz_testing::{
    CrashCategory, CrashDetector, CrashReport, FuzzGenerator, MutationFuzzer, PropertyTestResult,
    PropertyTester,
};
pub use health::{
    CacheHealthChecker, CircuitBreakerHealthChecker, DatabaseHealthChecker, HealthCheckManager,
    HealthCheckResult, HealthChecker, HealthReport, HealthStatus, ResourceHealthChecker,
};
pub use institutional::*;
pub use insurance::*;
pub use integration_testing::{
    IntegrationTestSuite, ServiceContract, StepStatus, SuiteResult, TestEnvironment, TestResult,
    TestScenario, TestService, TestStep,
};
pub use interoperability::{
    ChannelState, CrossChainMessage, IBCChannel, IBCConnectionManager, IBCPacket, LightClient,
    LightClientHeader, MessageStatus, StateProof, ValidatorSignature,
};
pub use liquidity_risk::*;
pub use load_testing::{
    LoadTestConfig, LoadTestResults, LoadTester, StressTestScenario, StressTester,
};
pub use market_sentiment::*;
pub use math::{
    EMACalculator, LinearRegression, MomentsCalculator, autocorrelation, correlation, covariance,
    geometric_mean, harmonic_mean, min_max_normalize, percentile, weighted_average, z_score,
};
pub use memory_optimization::{
    Arena, MemoryProfiler, MemoryStats, ObjectPool, PooledObject, SlabAllocator,
};
pub use memory_profiler::{MemoryDelta, MemoryGuard, TrackingAllocator};
pub use metrics::{
    BucketCount, Counter, Gauge, Histogram, HistogramStats, MetricType, MetricsRegistry, Summary,
    SummaryStats,
};
pub use mev_redistribution::{
    FairOrderingConfig, MEVAuctionBid, MEVOpportunity, MEVOpportunityType, MEVRecipient,
    MEVRedistribution, MEVRedistributionManager, MEVSearcher, OrderingMode, PGAMitigator,
};
pub use network_effects::{
    InfluencerMetrics, InfluencerTier, Interaction, InteractionType, NetworkAnalyzer, NetworkGraph,
    NetworkGrowthModel, UserNode, ViralCoefficient,
};
pub use nft::*;
pub use notifications::*;
pub use onchain_analytics::*;
pub use oracle::*;
pub use pagination::*;
pub use pentest::{
    PentestReport, PentestSuite, ScanSummary, Severity, VulnerabilityFinding, VulnerabilityScanner,
    VulnerabilityType,
};
pub use performance_benchmark::{
    BenchmarkIndex, InformationRatio, InformationRatioCalculator, PerformanceAttribution,
    PerformanceBenchmark, ReturnSeries, TrackingError, TrackingErrorCalculator,
};
pub use plugin::*;
pub use portfolio::*;
pub use portfolio_optimization::{
    AssetAllocation, EfficientFrontierPoint, OptimizationConstraint,
    Portfolio as OptimizationPortfolio, PortfolioMetrics, PortfolioOptimizer,
};
pub use position_limits::*;
pub use pretrade_risk::{
    PortfolioPosition, PreTradeRiskAssessor, RiskAssessmentConfig, RiskCheckResult, TradeRequest,
    TradeSide,
};
pub use protocol_protection::*;
pub use query::*;
pub use query_optimization::*;
pub use rate_limiter::{
    DistributedRateLimiter, LeakyBucket, RateLimitAlgorithm, RateLimitConfig, SlidingWindow,
    TokenBucket,
};
pub use risk::*;
pub use security::*;
pub use serialization::*;
pub use streaming::*;
pub use stress_testing::*;
pub use submarine_sends::{
    SubmarineCommitment, SubmarineSend, SubmarineSendBuilder, SubmarineSendManager,
    SubmarineSendState, SubmarineSendStats,
};
pub use systemic_risk::{
    ContagionResult, ExposureType, Interconnection, NetworkEntity, StressTestResult,
    SystemicRiskAnalyzer,
};
pub use tail_risk::{
    BlackSwanScenario, CrisisAlphaStrategy, TailHedgingStrategy, TailRiskHedger, TailRiskMetrics,
    TailRiskParity,
};
pub use testing::*;
pub use threshold_encryption::{
    DKGParticipant, DKGSession, DKGSessionStatus, EncryptedMempoolManager, EncryptedOrder,
    EncryptionMetadata, EncryptionScheme, KeyShare, MempoolStats, ParticipantStatus, TimeLockedKey,
};
pub use token_velocity::{
    BurnEvent, BurnRateManager, StakePosition, StakingIncentiveOptimizer, StakingTier,
    TokenVelocityManager, VelocityAdjustment, VelocityLevel, VelocityMetrics,
    VelocityReductionMechanism,
};
pub use tracing::*;
pub use validation::*;
pub use webhooks::*;
pub use zero_copy::*;
