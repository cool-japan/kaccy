//! Trading and order execution module
//!
//! This module provides order matching, trade execution, and market making functionality
//! for the Kaccy Protocol.
//!
//! # Components
//!
//! ## Order Book
//!
//! The [`order_book`] module implements a limit order book with:
//! - Price-time priority matching
//! - Partial fills
//! - Market depth snapshots
//! - Order book management across multiple tokens
//!
//! ## Trade Executor
//!
//! The [`executor`] module handles:
//! - Bonding curve order execution
//! - Atomic balance updates
//! - Transaction rollback on failure
//! - Fee calculation and distribution
//!
//! ## Market Maker
//!
//! The [`market_maker`] module provides:
//! - Automated market making with configurable spreads
//! - Inventory management and skew-based pricing
//! - Dynamic spread adjustment based on volatility
//! - Multi-token market maker management
//!
//! ## AMM (Automated Market Maker)
//!
//! The [`amm`] module implements:
//! - x*y=k constant product pools
//! - LP token generation and redemption
//! - Flash swap support
//! - Dynamic fee adjustment
//! - Multi-hop routing
//!
//! ## Circuit Breakers
//!
//! The [`price_circuit_breaker`] module protects against:
//! - Price manipulation (max price change limits)
//! - Volume spikes (per-user volume limits)
//! - Rapid trading (cooldown periods)
//! - Wash trading detection
//!
//! ## Security
//!
//! The [`security`] module provides:
//! - Emergency pause mechanism
//! - Multi-signature operations
//! - Withdrawal rate limiting
//! - Anomaly detection
//!
//! ## Arbitrage Prevention
//!
//! The [`arbitrage`] module includes:
//! - Arbitrage opportunity detection
//! - Rate limiting per user
//! - User flagging for excessive arbitrage
//! - Price discrepancy monitoring
//!
//! ## Batch Auctions (MEV Protection)
//!
//! The [`batch_auction`] module provides:
//! - Batch order collection during fixed time periods
//! - Uniform clearing price computation
//! - Front-running prevention
//! - Fair price discovery for all participants
//!
//! ## Commit-Reveal Orders (MEV Protection)
//!
//! The [`commit_reveal`] module implements:
//! - Two-phase order submission (commit → reveal)
//! - Cryptographic commitment using SHA-256
//! - Order privacy during commit phase
//! - Verification of revealed orders
//!
//! ## Private Transaction Pool (MEV Protection)
//!
//! The [`private_pool`] module provides:
//! - Encrypted order submission
//! - Time-delayed execution
//! - Front-running prevention
//! - Fair ordering based on submission time
//!
//! ## Automated Trading Strategies
//!
//! The [`automated_strategies`] module implements:
//! - Grid trading with configurable price levels
//! - Dollar-cost averaging (DCA) for periodic purchases
//! - Portfolio rebalancing to maintain target allocations
//!
//! ## Options & Derivatives
//!
//! The [`options`] module provides:
//! - European-style call and put options
//! - Black-Scholes pricing model
//! - Greeks calculation (delta, gamma, theta, vega, rho)
//! - Implied volatility calculation
//! - Cash settlement at expiration
//!
//! ## Order Flow Analysis
//!
//! The [`order_flow`] module provides:
//! - Trade flow direction detection (buy/sell pressure)
//! - Order imbalance calculation
//! - Aggressive vs passive order classification
//! - Flow toxicity metrics (VPIN)
//!
//! ## Market Depth Analytics
//!
//! The [`market_depth`] module provides:
//! - Order book skewness calculation
//! - Bid-ask spread dynamics tracking
//! - Depth imbalance indicators
//! - Liquidity heat maps
//!
//! ## Price Discovery
//!
//! The [`price_discovery`] module provides:
//! - Microprice calculation (weighted mid-price)
//! - Price improvement tracking
//! - Effective spread measurement
//! - Quote stability metrics
//!
//! # Example
//!
//! ```rust,no_run
//! use kaccy_core::trading::{OrderBook, OrderSide, LimitOrder};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! // Create an order book for a token
//! let token_id = Uuid::new_v4();
//! let mut order_book = OrderBook::new(token_id);
//!
//! // Add a limit buy order
//! let buy_order = LimitOrder {
//!     order_id: Uuid::new_v4(),
//!     user_id: Uuid::new_v4(),
//!     token_id,
//!     side: OrderSide::Buy,
//!     price: dec!(10.0),
//!     amount: dec!(100),
//!     filled_amount: dec!(0),
//!     timestamp: chrono::Utc::now().timestamp(),
//! };
//!
//! order_book.add_order(buy_order);
//! ```

pub mod advanced_execution;
pub mod advanced_matching;
pub mod amm;
pub mod arbitrage;
pub mod automated_strategies;
pub mod avellaneda_stoikov;
pub mod backtesting;
pub mod batch_auction;
pub mod commit_reveal;
pub mod conditional_orders;
pub mod executor;
pub mod flash_loan;
pub mod hft_metrics;
pub mod market_depth;
pub mod market_impact;
pub mod market_maker;
pub mod mev_detection;
pub mod mev_mitigation;
pub mod mm_analytics;
pub mod options;
pub mod order_book;
pub mod order_book_optimized;
pub mod order_flow;
pub mod order_flow_toxicity;
pub mod order_splitting;
pub mod price_circuit_breaker;
pub mod price_discovery;
pub mod private_pool;
pub mod rl_market_maker;
pub mod routing;
pub mod security;
pub mod sentiment;
pub mod smart_execution;
pub mod smart_routing;
pub mod social_trading;
pub mod tca;
pub mod time_based_orders;
pub mod trade_validation;
pub mod vpin;

pub use advanced_execution::{
    AdvancedExecutionManager, AdvancedExecutionOrder, AdvancedExecutionStatus,
    AdvancedExecutionType, DiscretionDirection, PegType,
};
pub use advanced_matching::{
    DarkPool, DarkPoolManager, HybridMatcher, MatchingAlgorithm, ProRataConfig, ProRataMatcher,
};
pub use amm::*;
pub use arbitrage::{
    ArbitrageDetector, ArbitrageOpportunity, ArbitragePreventionConfig, ArbitragePreventor,
    ArbitrageTrade, ArbitrageUserStats,
};
pub use automated_strategies::{
    DcaBot, DcaConfig, DcaPurchase, DcaStats, GridOrder, GridTradingBot, GridTradingConfig,
    GridTradingStats, PortfolioPosition, PortfolioRebalanceAction, PortfolioRebalancer,
    RebalanceConfig, RebalanceStats, TokenAllocation,
};
pub use avellaneda_stoikov::{
    ASQuote, AvellanedaStoikovConfig, AvellanedaStoikovMarketMaker, MarketState,
};
pub use batch_auction::{
    BatchAuction, BatchAuctionConfig, BatchAuctionManager, BatchMatch, BatchStats, BatchStatus,
};
pub use commit_reveal::{
    CommitRevealConfig, CommitRevealCycle, CommitRevealManager, CommitRevealPhase,
    CommitRevealStats, OrderCommitment, OrderParams, RevealedOrder,
};
pub use conditional_orders::{
    ConditionalOrder, ConditionalOrderManager, ConditionalOrderStatus, ConditionalOrderType,
    OrderAction,
};
pub use executor::*;
pub use flash_loan::{
    FlashLoanConfig, FlashLoanExecutor, FlashLoanManager, FlashLoanPool, FlashLoanPoolStats,
    FlashLoanRequest, FlashLoanResult, FlashLoanStatus,
};
pub use hft_metrics::{
    AdverseSelectionDetector, HftAnalyzer, HftMetrics, InventoryRiskCalculator, LatencyTracker,
    OrderEvent, OrderEventType,
};
pub use market_depth::{
    DepthImbalance, LiquidityHeatMap, MarketDepthAnalyzer, OrderBookSkewness, PriceLevel,
    SkewnessDirection, SpreadDynamics,
};
pub use market_impact::{
    AlmgrenChrissModel, ImpactDecayModel, ImpactEstimate, LinearImpactModel, SquareRootImpactModel,
    TransientImpactAnalyzer,
};
pub use market_maker::{
    InventoryState, MarketMakerConfig, MarketMakerManager, MarketMakerQuote, MarketMakerStats,
    MarketMakerSummary, QuoteLevel, RebalanceAction, TokenMarketMaker,
};
pub use mev_detection::{
    DetectorStats, MevAttackType, MevDetection, SandwichDetector, TransactionPattern, VictimTracker,
};
pub use mev_mitigation::{
    BundledTransaction, FairOrderingManager, MevProtectionCoordinator, MevRebate, MevRebateConfig,
    MevRebateManager, MitigationStrategy, PrivateExecutionEngine, RebateStatus, TransactionBundle,
};
pub use mm_analytics::{
    InventoryAnalyzer, MarketMakerPerformance, MarketMakingAnalytics, MmSide, MmTrade,
    QuoteAdjustments, SpreadComponents, SpreadDecomposition,
};
pub use options::{
    BlackScholesModel, Greeks, OptionContract, OptionStatus, OptionType, OptionsManager,
};
pub use order_book::{
    DepthLevel, LimitOrder, MatchResult, OrderBook, OrderBookDepth, OrderBookManager, OrderSide,
    Trade,
};
pub use order_book_optimized::{AtomicPriceStats, LockFreeOrderBook, OrderPool, PoolStats, simd};
pub use order_flow::{
    FlowDirection, FlowToxicity, FlowTrade, OrderAggressiveness, OrderFlowAnalyzer, OrderImbalance,
};
pub use order_flow_toxicity::{
    OrderFlowToxicityDetector, ToxicityMetrics, ToxicityTrade, TradeDirection,
};
pub use price_circuit_breaker::{
    CheckResult, CircuitBreakerReason, CircuitBreakerState, PriceCircuitBreaker,
    PriceCircuitBreakerConfig, TradeRecord,
};
pub use price_discovery::{
    EffectiveSpread, Microprice, PriceDiscoveryAnalyzer, PriceImprovement, QuoteStability,
    QuoteUpdate,
};
pub use private_pool::{
    DecryptedOrderData, EncryptionScheme, PrivateOrder, PrivateOrderStatus, PrivatePoolConfig,
    PrivatePoolStats, PrivateTransactionPool,
};
pub use rl_market_maker::{
    Experience, MarketAction, OnlineLearningMarketMaker, QNetwork, RLMarketMaker,
    RLMarketMakerConfig, RLMarketState, ReplayBuffer, TrainingStats,
};
pub use routing::*;
pub use security::*;
pub use sentiment::*;
pub use smart_execution::*;
pub use social_trading::*;
pub use tca::{
    ExecutionRecord, ExecutionSide, ExecutionStatistics, ImplementationShortfall, MarketImpact,
    TradeCostAnalyzer, VwapAnalyzer, VwapPerformance,
};
pub use time_based_orders::{
    TimeBasedOrder, TimeBasedOrderManager, TimeBasedOrderStatus, TimeBasedOrderType,
    TimeEventResult,
};
pub use trade_validation::{TradeValidationConfig, TradeValidator, ValidationResult};
pub use vpin::{ClassifiedTrade, EnhancedVPIN, ToxicityFeeAdjustment, VPINAnalyzer, VolumeClass};
