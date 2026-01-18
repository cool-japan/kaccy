//! Bitcoin integration for Kaccy Protocol
//!
//! This crate provides Bitcoin Core RPC client, address generation,
//! HD wallet support (BIP84), transaction monitoring, Lightning Network,
//! and RBF handling.

pub mod activity_monitor;
pub mod adaptive_fees;
pub mod address_utils;
pub mod advanced_rbf;
pub mod atomic_swap;
pub mod audit;
pub mod batch_optimizer;
pub mod bip21;
pub mod bip324;
pub mod bip329;
pub mod bip47;
pub mod btc_utils;
pub mod cache;
pub mod chain_privacy;
pub mod client;
pub mod coin_control;
pub mod coin_selection_advanced;
pub mod coin_selection_privacy;
pub mod coinjoin;
pub mod compact_filters;
pub mod confirmation;
pub mod connection_pool;
pub mod descriptor;
pub mod dlc;
pub mod error;
pub mod fee_bumping;
pub mod frost;
pub mod gap_limit;
pub mod hd_wallet;
pub mod health;
pub mod hwi;
pub mod key_management;
pub mod lightning;
pub mod lightning_v2;
pub mod matcher;
pub mod mempool;
pub mod mempool_analytics;
pub mod mempool_space;
pub mod metrics;
pub mod miniscript_support;
pub mod monitor;
pub mod multisig;
pub mod musig2;
pub mod network_privacy;
pub mod notifications;
pub mod p2p;
pub mod package_relay;
pub mod payjoin;
pub mod privacy_metrics;
pub mod psbt;
pub mod psbt_analysis;
pub mod psbt_workflows;
pub mod rbf;
pub mod regtest_utils;
pub mod rsk;
pub mod script_analyzer;
pub mod script_optimizer;
pub mod seed_recovery;
pub mod signet;
pub mod silent_payments;
pub mod slip39;
pub mod stacks;
pub mod structured_logging;
pub mod submarine_swaps;
pub mod taproot;
pub mod tapscript_utils;
#[cfg(test)]
pub mod testing;
pub mod testnet4;
pub mod timelock;
pub mod transaction_history;
pub mod transaction_manager;
pub mod tx_limits;
pub mod tx_parser;
pub mod utxo;
pub mod utxo_optimizer;
pub mod utxo_privacy;
pub mod wallet;
pub mod wallet_recovery;
pub mod zmq;

pub use activity_monitor::{
    ActivityMonitor, ActivityMonitorConfig, ActivityType, SuspiciousActivity, TransactionRecord,
};
pub use adaptive_fees::{
    AdaptiveFeeManager, BudgetStrategy, MultiTxFeeStrategy, PlannedTransaction, TimeBasedStrategy,
    TransactionUrgency, TxFeeAllocation,
};
pub use address_utils::{
    AddressBatchAnalyzer, AddressBatchStatistics, AddressComparator, AddressInfo, AddressType,
};
pub use advanced_rbf::{
    AdvancedRbfManager, BatchRbfOperation, BatchRbfResult, ConsolidatedPayment,
    ConsolidationSavings, RbfRecommendation, RecommendationReason, RecommendationUrgency,
};
pub use atomic_swap::{
    AtomicSwap, AtomicSwapConfig, AtomicSwapManager, SwapRole, SwapStatus, SwapStep,
};
pub use audit::{
    AuditEvent, AuditEventBuilder, AuditEventType, AuditLogger, AuditQueryCriteria, AuditSeverity,
    AuditStorage, FileAuditStorage, InMemoryAuditStorage,
};
pub use batch_optimizer::{
    BatchEfficiency, BatchOptimizer, BatchStrategy, BatchWithdrawal, OptimizedBatch,
};
pub use bip21::{BitcoinUri, BitcoinUriBuilder, QrCodeHelper};
pub use bip47::{
    NotificationTransaction, PaymentChannel, PaymentCode, PaymentCodeManager, PaymentCodeVersion,
};
pub use bip324::{ConnectionState, SessionKeys, V2Transport, V2TransportConfig, V2TransportStats};
pub use bip329::{LabelManager, LabelRecord, LabelType};
pub use btc_utils::{
    DUST_THRESHOLD, P2PKH_INPUT_VSIZE, P2PKH_OUTPUT_VSIZE, P2SH_OUTPUT_VSIZE,
    P2SH_P2WPKH_INPUT_VSIZE, P2TR_INPUT_VSIZE, P2TR_OUTPUT_VSIZE, P2WPKH_INPUT_VSIZE,
    P2WPKH_OUTPUT_VSIZE, TX_OVERHEAD_VSIZE, amount_to_satoshis, btc_to_satoshis,
    calculate_batch_fee, calculate_batch_savings, calculate_change, calculate_effective_value,
    calculate_fee_from_rate, calculate_fee_percentage, calculate_fee_rate,
    estimate_p2tr_keypath_witness_size, estimate_p2wpkh_witness_size,
    estimate_simple_transaction_vsize, estimate_transaction_vsize, is_dust, is_op_return,
    is_valid_amount, mbtc_to_satoshis, round_for_privacy, satoshis_to_btc, satoshis_to_mbtc,
    script_size, vsize_to_weight, weight_to_vsize,
};
pub use cache::{
    BlockHeader, BlockHeaderCache, CacheConfig, CacheManager, CacheStats, OverallCacheStats,
    TransactionCache, UtxoCache,
};
pub use chain_privacy::{
    AmountObfuscation, BroadcastPriority, ChangeOutputGenerator, ChangeStrategy,
    FingerprintingAnalyzer, FingerprintingIssue, PendingBroadcast, StructureRandomization,
    TimingCoordinator, TimingObfuscation, TransactionPrivacyEnhancer,
};
pub use client::{BitcoinClient, BitcoinNetwork, ReconnectConfig};
pub use coin_control::{
    CoinControl, ManualSelection, PrivacyPreferences, PrivacySelection, UtxoLabel,
};
pub use coin_selection_advanced::{
    KnapsackSelector, SelectionConfig, SelectionResult, SimulatedAnnealingSelector,
};
pub use coin_selection_privacy::{
    PrivacyCoinSelector, PrivacyConcerns, PrivacyScore, PrivateSelection,
};
pub use coinjoin::{
    CoinJoinClient, CoinJoinCoordinator, CoinJoinSession, Participant, ParticipantInput,
    SessionConfig, SessionState,
};
pub use compact_filters::{
    CompactFilter, CompactFilterConfig, CompactFilterManager, FilterHeader, FilterStatistics,
    FilterType, FilterVerifier,
};
pub use confirmation::{
    ConfirmationEvent, ConfirmationLevel, ConfirmationStats, ConfirmationTracker, NotificationType,
    TrackedOrder,
};
pub use connection_pool::{ConnectionPool, PoolConfig, PoolStats, PooledConnectionGuard};
pub use descriptor::{
    DescriptorConfig, DescriptorRange, DescriptorType, DescriptorWallet, OutputDescriptor,
};
pub use dlc::{
    CetBuilder, ContractOutcome, DlcContract, DlcManager, DlcStatus, Oracle, OracleAnnouncement,
    OracleAttestation,
};
pub use error::BitcoinError;
pub use fee_bumping::{
    AutoFeeBumper, BumpPriority, BumpStrategy, CpfpBuilder, FeeBudgetManager, FeeBumpResult,
    FeeBumpingPolicy, TrackedTransaction,
};
pub use frost::{
    FrostConfig, FrostCoordinator, FrostSigner, KeyGenOutput, NonceCommitment, SecretShare,
    SignatureShare, lagrange_coefficient,
};
pub use gap_limit::{
    AddressDiscovery, AddressUsage, DiscoveryResult, GapLimitConfig, GapLimitStats, GapLimitTracker,
};
pub use hd_wallet::{
    AddressBook, AddressInfo as HdAddressInfo, CachedAddress, HdWallet, HdWalletConfig, derivation,
};
pub use health::{
    ComponentHealth, DependencyHealth, HealthCheckConfig, HealthCheckManager, HealthReport,
    HealthStatus, ResourceUtilization,
};
pub use hwi::{
    AirGappedSigner, HardwareDevice, HardwareWalletManager, HardwareWalletType, HwiConfig,
};
pub use key_management::{
    Guardian, KeyRotationConfig, KeyRotationManager, RecoveryKey, RecoveryProgress, RecoveryShare,
    RotatedKey, SocialRecoveryConfig, SocialRecoveryManager, TimeDelayedRecoveryConfig,
    TimeDelayedRecoveryManager,
};
pub use lightning::{
    Channel, ChannelBalance, ChannelPoint, Invoice, InvoiceRequest, InvoiceStatus,
    InvoiceSubscription, InvoiceUpdate, LightningHealth, LightningPaymentManager,
    LightningProvider, LndClient, NodeInfo, OpenChannelRequest, OrderPaymentStatus, Payment,
    PaymentStatus as LightningPaymentStatus, RouteHop,
};
pub use matcher::{MatchGroups, MatchResult, MatchStatus, OrderMatch, TransactionMatcher};
pub use mempool::{
    AddressWatcher, MempoolEvent, MempoolMonitor, MempoolMonitorConfig, MempoolStats,
    MempoolTransaction, RemovalReason, ReorgInfo, ReorgTracker, UnconfirmedPayment,
};
pub use mempool_space::{
    FeeMarketAnalysis, FeeMarketCondition, FeeRecommendation, HistoricalFeeData,
    MempoolSpaceClient, MempoolSpaceConfig, MempoolSpaceStats,
};
pub use metrics::{
    BitcoinMetrics, InMemoryMetricsBackend, MetricType, MetricsBackend, MetricsStats, MetricsTimer,
    NoOpMetricsBackend,
};
pub use miniscript_support::{
    HashType, MiniscriptCompiler, MiniscriptDescriptor, MiniscriptPolicy, MiniscriptScriptType,
    PolicyAnalysis, PolicyTemplateBuilder, TimelockType,
};
pub use monitor::{
    MonitorConfig, PaymentDetails, PaymentEvent, PaymentMonitor, PaymentMonitorTask, PaymentStatus,
    PendingOrder, create_shutdown_signal,
};
pub use multisig::{
    ColdWalletInfo, CustodyManager, MultisigAddress, MultisigConfig, MultisigInput, MultisigOutput,
    MultisigSignature, MultisigTransaction, MultisigTxBuilder, MultisigTxStatus, MultisigWallet,
    SignatureCoordinator,
};
pub use musig2::{
    AdaptorSignature, AdaptorSignatureManager, AggregatedNonce, MuSig2Coordinator,
    MuSig2KeyAggregator, MuSig2Nonce, MuSig2Signer, PartialSignature,
};
pub use network_privacy::{
    BroadcastResult, DandelionBroadcaster, DandelionStats, NetworkPrivacyConfig,
    NetworkPrivacyManager, PeerConnection, PrivatePeerManager, TorConnectionStats, TorProxyManager,
};
pub use notifications::{
    AdminNotification, AdminNotificationService, NotificationCategory, NotificationMetadata,
    NotificationPriority, NotificationStats,
};
pub use p2p::{P2pClient, P2pConfig, P2pEvent, P2pStats, PeerInfo, PeerManager};
pub use package_relay::{CpfpHelper, PackageRelayManager, PackageStats, TransactionPackage};
pub use payjoin::{
    PayJoinCoordinator, PayJoinParams, PayJoinProposal, PayJoinReceiver, PayJoinResponse,
    PayJoinRole, PayJoinSender, PayJoinUriBuilder, PayJoinVersion, ReceiverContribution,
    ReceiverUtxo,
};
pub use privacy_metrics::{
    AddressClusterAnalyzer, ClusterAnalysis, HealthStatus as WalletHealthStatus, PrivacyAnalyzer,
    PrivacyHeuristic, PrivacyIssue as TransactionPrivacyIssue,
    PrivacyScore as TransactionPrivacyScore, TransactionInput as PrivacyTransactionInput,
    TransactionOutput as PrivacyTransactionOutput, TransactionPrivacyAnalysis,
    WalletPrivacyAnalyzer, WalletPrivacyHealth,
};
pub use psbt::{
    FeeEstimation, PsbtBuilder, PsbtManager, PsbtResult, SignedTransaction, TxOutput, UtxoInput,
    WithdrawalRequest, WithdrawalResult, WithdrawalStatus,
};
pub use psbt_analysis::{
    FeeVerification, InputAnalysis, OutputAnalysis, PsbtAnalysis, PsbtAnalyzer, SecurityIssue,
    ValidationWarning, verify_psbt_fees,
};
pub use psbt_workflows::{
    PsbtTemplate, PsbtVersion, PsbtWorkflow, PsbtWorkflowManager, SignatureRecord, SignerRole,
    SigningStatus, WorkflowState,
};
pub use rbf::{
    RbfConfig, RbfEvent, RbfReplacement, RbfStatus, RbfTracker, RbfTransactionBuilder,
    TrackedRbfTransaction,
};
pub use regtest_utils::{RegtestConfig, RegtestHelper, ReorgInfo as RegtestReorgInfo};
pub use rsk::{
    DeploymentStatus as RskDeploymentStatus, PegConfig, PegInTransaction, PegManager, PegOperation,
    PegOutTransaction, PegStatus, RskAddress, RskClient, RskContractDeployment,
    RskDeploymentResult, RskNetwork, RskTransactionReceipt,
};
pub use script_analyzer::{
    ScriptAnalysis, ScriptAnalyzer, ScriptDisassembler, ScriptTemplateBuilder,
    ScriptType as AnalyzerScriptType,
};
pub use script_optimizer::{
    OptimizationConfig, OptimizationEstimate, OptimizationResult, OptimizationType, ScriptOptimizer,
};
pub use seed_recovery::{
    Mnemonic, MnemonicGenerator, MnemonicLanguage, MnemonicStats, MnemonicValidator, SeedXor,
    WordCount,
};
pub use signet::{ChallengeParser, SignetClient, SignetConfig, SignetFaucet, SignetFaucets};
pub use silent_payments::{
    DetectedOutput, SilentPaymentAddress, SilentPaymentScanner, SilentPaymentSender,
    SilentPaymentWallet,
};
pub use stacks::{
    BridgeConfig as StacksBridgeConfig, BridgeOperation as StacksBridgeOperation,
    BridgeStatus as StacksBridgeStatus, BridgeTransaction as StacksBridgeTransaction,
    ClarityContract, ContractDeploymentManager, ContractId, DeploymentRequest, DeploymentResult,
    DeploymentStatus as StacksDeploymentStatus, StacksAddress, StacksClient, StacksNetwork,
    StacksTransaction, TokenBridge as StacksTokenBridge,
};
pub use structured_logging::{
    CorrelationId, PerformanceSpan, RequestResponseLogger, RpcLogContext,
};
pub use submarine_swaps::{
    SubmarineSwap, SubmarineSwapConfig, SubmarineSwapService, SubmarineSwapStatistics,
    SubmarineSwapStatus, SwapDirection,
};
pub use taproot::{
    TaprootAddress, TaprootConfig, TaprootKeyPair, TaprootManager, TaprootScriptLeaf,
    TaprootScriptTree, TaprootSpendPath, TaprootTxBuilder,
};
pub use tapscript_utils::{
    MerkleProof, ScriptPathOptimizer, TapscriptLeaf, TapscriptNode, TapscriptTreeBuilder,
};
pub use testnet4::{FaucetInfo, Testnet4Client, Testnet4Config, Testnet4Params};
pub use timelock::{
    HtlcConfig, HtlcContract, HtlcManager, HtlcScriptBuilder, HtlcStatus, TimeLockType,
    TimelockTxBuilder,
};
pub use transaction_history::{
    AddressReuseReport, HistoricalTransaction, PaginatedResult, PaginationOptions, SortField,
    SortOrder, TransactionFilter, TransactionHistory, TransactionSummary, TransactionType,
};
pub use transaction_manager::{
    TransactionManager, TransactionManagerConfig, TransactionStats, TransactionValidation,
};
pub use tx_limits::{
    LimitConfig, LimitEnforcer, LimitPeriod, LimitViolation, TransactionLimit, UsageRecord,
};
pub use tx_parser::{
    ConfirmationStatus, ParsedInput, ParsedOutput, ParsedTransaction, ScriptType, SenderConfidence,
    SenderInfo, TransactionAnalysis, TransactionParser,
};
pub use utxo::{
    ConsolidationConfig, ConsolidationPlan, SelectionStrategy, Utxo, UtxoManager, UtxoSelection,
};
pub use utxo_privacy::{
    ConsolidationPrivacyAnalyzer, ConsolidationPrivacyReport, PrivacyIssue, PrivacyRecommendation,
    ToxicChangeDetector, UtxoCluster, UtxoPrivacyAnalysis,
};
pub use wallet::WalletManager;
pub use wallet_recovery::{
    DerivationStandard, DiscoveredAddress, RecoveryConfig, RecoveryResult, RecoveryStats,
    WalletRecovery,
};
