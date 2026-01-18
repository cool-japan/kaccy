//! Database repositories for Kaccy Protocol

pub mod api_key;
pub mod api_key_usage;
pub mod audit;
pub mod audit_event;
pub mod background_job;
pub mod balance;
pub mod commitment;
pub mod notification_preferences;
pub mod order;
pub mod reputation_event;
pub mod session;
pub mod system_config;
pub mod token;
pub mod trade;
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
