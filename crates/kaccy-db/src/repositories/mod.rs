//! Database repositories for Kaccy Protocol

/// API key repository: storage and lookup of API keys.
pub mod api_key;
/// API key usage repository: per-key request tracking and statistics.
pub mod api_key_usage;
/// Audit log repository: immutable record of administrative actions.
pub mod audit;
/// Audit event repository: structured event capture for compliance.
pub mod audit_event;
/// Background job repository: persistent async job queue management.
pub mod background_job;
/// Balance repository: token holdings per user.
pub mod balance;
/// Commitment repository: user commitment records and statistics.
pub mod commitment;
/// Notification preferences repository: per-user channel preferences.
pub mod notification_preferences;
/// Order repository: limit and market order management.
pub mod order;
/// Reputation event repository: immutable reputation-change audit trail.
pub mod reputation_event;
/// Session repository: user authentication session lifecycle.
pub mod session;
/// System configuration repository: runtime key-value configuration.
pub mod system_config;
/// Token repository: token issuance, supply, and status management.
pub mod token;
/// Trade repository: executed trade records and analytics.
pub mod trade;
/// User repository: account management and profile data.
pub mod user;

pub use api_key::{ApiKey, ApiKeyRepository, ApiKeySummary, CreateApiKey};
pub use api_key_usage::{
    ApiKeyUsage, ApiKeyUsageRepository, EndpointStats, HourlyUsage, RecordUsage, UsageStats,
};
pub use audit::{
    AdminAction, AuditLog, AuditRepository, AuditSummary, TableChangeReport, UserAuditActivity,
};
pub use audit_event::{
    AuditEvent, AuditEventRepository, CorrelationTrace, CreateAuditEvent, DailyEventSummary,
    EventTypeStats, UserEventActivity,
};
pub use background_job::{
    BackgroundJob, BackgroundJobRepository, CreateBackgroundJob, JobResult, JobStatus, JobTypeStats,
};
pub use balance::{
    BalanceRepository, BalanceRow, BalanceStatistics, BalanceWithTokenRow, HolderInfo,
    PortfolioSummary,
};
pub use commitment::{
    CommitmentRepository, CommitmentRow, CommitmentStats, CommitmentTrend, CommitmentWithDetails,
    TokenCommitmentStats, UserPerformance,
};
pub use notification_preferences::{
    CreateNotificationPreferences, EmailNotificationType, NotificationPreferences,
    NotificationPreferencesRepository, PushNotificationType, SmsNotificationType,
    UpdateNotificationPreferences,
};
pub use order::{BidAsk, OrderBookDepth, OrderRepository, PriceLevel};
pub use reputation_event::{
    DailyReputationStats, EventBalance, EventTypeSummary, ReputationEventRepository,
    ReputationEventRow, ReputationHistorySummary, UserReputationGain,
};
pub use session::{
    ActiveSessionSummary, CreateSession, SessionRepository, SessionStats, UserSession,
};
pub use system_config::{
    ConfigCategorySummary, ConfigHistory, CreateSystemConfig, SystemConfig, SystemConfigRepository,
    UpdateSystemConfig, ValueType,
};
pub use token::{TokenRepository, TokenRow, TokenStatistics, TokenWithIssuerRow};
pub use trade::{
    CreateTradeParams, DailyTradeStats, PricePoint, TimeRange, TokenTradingVolume, TradeRepository,
    TradeRow, TradeSummary, TradeWithContextRow, TraderActivity, UserTradeStats,
};
pub use user::{UserRepository, UserRow, UserStatistics};
