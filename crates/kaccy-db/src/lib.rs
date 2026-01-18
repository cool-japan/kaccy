//! Database layer for Kaccy Protocol
//!
//! This crate provides database connection pooling, query abstractions,
//! Redis caching, read replica support, index analysis, analytics,
//! backup/recovery utilities, database sharding, multi-region replication,
//! security features (encryption, sanitization, audit logging),
//! real-time event notifications (LISTEN/NOTIFY), and
//! comprehensive observability for PostgreSQL using SQLx.

pub mod analytics;
pub mod backup;
pub mod bulk_operations;
pub mod cache;
pub mod cache_invalidation;
pub mod cache_warming;
pub mod circuit_breaker;
pub mod connection_diagnostics;
pub mod connection_leak_detector;
pub mod cursor_pagination;
pub mod database_size_monitor;
pub mod error;
pub mod event_notifications;
pub mod generic_repository;
pub mod health_check;
pub mod helpers;
pub mod index_analyzer;
pub mod lock_monitoring;
pub mod logging;
pub mod maintenance;
pub mod metrics;
pub mod migration_runner;
pub mod migration_utils;
pub mod multi_region;
pub mod multi_tier_cache;
pub mod partitioning;
pub mod platform_stats;
pub mod pool;
pub mod pool_autoscaling;
pub mod pool_circuit_breaker;
pub mod pool_hooks;
pub mod pool_monitor;
pub mod query_builder;
pub mod query_logger;
pub mod query_performance_analyzer;
pub mod query_plan;
pub mod query_streaming;
pub mod replica;
pub mod replica_failover;
pub mod repositories;
pub mod retry_policy;
pub mod schema_validator;
// Security module disabled - needs sqlx offline mode and Rust 1.85 string literal fixes
pub mod security;
pub mod sharding;
pub mod statement_cache;
pub mod table_stats;
pub mod tracing;
pub mod transaction;
pub mod vacuum_scheduler;

pub use analytics::{
    AnalyticsService, DailyStats, DashboardMetrics, OhlcData, PlatformVolumeHistory, PriceHistory,
    RefreshConfig, TokenMetricsSummary, UserActivitySummary, VolumeHistory,
};
pub use backup::{
    get_database_stats, verify_database_integrity, BackupConfig, BackupManager, BackupMetadata,
    DatabaseStats, PitrConfig,
};
pub use bulk_operations::{BulkDeleter, BulkInserter, BulkUpdater};
pub use cache::{keys as cache_keys, CacheConfig, CacheStats, CachedRepository, RedisCache};
pub use cache_invalidation::{
    CascadeRule, InvalidationConfig, InvalidationEvent, InvalidationManager, InvalidationType,
    TagRegistry,
};
pub use cache_warming::{CacheDataSource, CacheWarmer, CacheWarmingConfig, WarmingStrategy};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerStats, CircuitState,
};
pub use connection_diagnostics::{
    check_connection_health, find_long_running_queries, get_active_connections,
    get_pool_diagnostics, get_pool_stats, ActiveConnection, ConnectionHealth, ConnectionIssue,
    IssueType, LongRunningQuery, PoolDiagnostics, PoolStats, Severity,
};
pub use connection_leak_detector::{
    LeakDetector, LeakDetectorConfig, LeakReport, LeakStats, TrackedConnection,
};
pub use cursor_pagination::{
    Connection, Cursor, CursorBuilder, CursorPagination, Direction, Edge, PageInfo,
};
pub use database_size_monitor::{
    CapacityForecast, DatabaseSizeMonitor, DatabaseSnapshot, GrowthStats, SizeAlert, SizeAlertType,
    SizeMonitorConfig, SizeReport, TableSize,
};
pub use error::{DbError, Result};
pub use event_notifications::{
    DatabaseEvent, EventHandlerFn, EventListener, ListenerConfig, NotificationStats,
    NotificationTriggers, PostgresNotificationTriggers,
};
pub use generic_repository::{CachedGenericRepository, Page, Pagination, Repository};
pub use health_check::{
    perform_health_check, quick_health_check, HealthCategory, HealthCheckConfig, HealthCheckResult,
    HealthStatus, SystemHealthReport,
};
pub use helpers::{
    calculate_offset, calculate_percentage, column_exists, execute_raw, format_bytes,
    get_database_time, parse_decimal, parse_uuid, sanitize_column_name, sanitize_table_name,
    table_exists, PaginationMeta,
};
pub use index_analyzer::{
    AnalysisSummary, ExistingIndex, IndexAnalysis, IndexAnalyzer, IndexSuggestion,
    IndexSuggestionType, Priority, UnusedIndex,
};
pub use lock_monitoring::{
    detect_deadlocks, get_blocking_queries, get_current_locks, get_lock_wait_stats,
    kill_blocking_query, BlockingQuery, DeadlockInfo, LockInfo, LockType, LockWaitStats,
};
pub use logging::{
    AnomalyDetector, DbContext, LogLevel, PerformanceTracker, QueryLogEntry, QueryStats,
};
pub use maintenance::{
    analyze_table, auto_vacuum_tables, get_bloated_tables, get_tables_needing_vacuum,
    reindex_table, vacuum_analyze_table, vacuum_table, MaintenanceOperation, MaintenanceResult,
    TableBloatInfo, TableVacuumInfo,
};
pub use metrics::{
    cache_hit_rate, gather_metrics, record_cache_error, record_cache_hit, record_cache_miss,
    record_pool_stats, register_metrics, MetricsTimer,
};
pub use migration_runner::{
    Migration, MigrationConfig, MigrationHistory, MigrationResult, MigrationRunner,
};
pub use migration_utils::{
    get_migration_status, get_schema_version, is_migration_applied, list_applied_migrations,
    verify_schema_integrity, MigrationInfo, MigrationStatus, SchemaIntegrity,
};
pub use multi_region::{MultiRegionPoolManager, Region, RegionalPoolConfig, ReplicationLag};
pub use multi_tier_cache::{MultiTierCache, MultiTierCacheConfig, MultiTierStats};
pub use partitioning::{
    PartitionDefinition, PartitionInterval, PartitionManager, PartitionStats, PartitioningStrategy,
};
pub use platform_stats::{
    get_growth_metrics, get_health_indicators, get_platform_snapshot, get_platform_stats,
    GrowthMetrics, HealthIndicators, PlatformStats,
};
pub use pool::{
    create_pool, create_pool_with_retry, refresh_pool_connections, validate_pool_connections,
    warmup_pool, RetryConfig, WarmupStrategy,
};
pub use pool_autoscaling::{
    AutoscalingConfig, AutoscalingStats, PoolAutoscaler, ScalingEvent, ScalingEventType,
    ScalingStrategy, UtilizationMetrics,
};
pub use pool_circuit_breaker::{
    HealthBasedPoolSelector, PoolCircuitBreaker, PoolCircuitBreakerConfig, PoolCircuitState,
    PoolCircuitStats,
};
pub use pool_hooks::{
    AlertEventHandler, LoggingEventHandler, MetricsEventHandler, PoolEvent, PoolEventHandler,
    PoolEventManager, PoolEventType,
};
pub use pool_monitor::{
    AlertLevel, CapacityStats, MonitorConfig, MonitoringReport, PoolAlert, PoolHealth,
    PoolMetricsSnapshot, PoolMonitor,
};
pub use query_builder::{
    Filter, FilterValue, LogicalOp, Operator, OrderBy, OrderDirection, QueryBuilder,
};
pub use query_logger::{
    time_query, AggregateStats, QueryLogConfig, QueryLogger, QueryTimer, SlowQueryRecord,
};
pub use query_performance_analyzer::{
    AnalyzerConfig, OptimizationSuggestion, PerformanceIssue, PerformanceReport, QueryAnalysis,
    QueryPerformanceAnalyzer, ReportSummary, SuggestionType,
};
pub use query_plan::{
    explain_analyze_query, explain_query, get_expensive_queries, ExpensiveQuery, QueryPlan,
    QueryPlanNode,
};
pub use query_streaming::{BufferedStreamProcessor, QueryStream, StreamConfig};
pub use replica::{
    LoadBalanceStrategy, ReplicaConfig, ReplicaHealthStatus, ReplicaPoolManager, ReplicaStatus,
    SmartDbClient, SmartDbClientBuilder,
};
pub use replica_failover::{
    ClusterStatus, FailoverConfig, FailoverManager, FailoverStrategy, NodeStatus, ReplicaNode,
    ReplicaRole,
};
pub use repositories::*;
pub use retry_policy::{
    detect_idempotency, ErrorClass, Idempotency, RetryPolicy, RetryPolicyConfig, RetryStrategy,
};
pub use schema_validator::{
    detect_orphaned_records, get_all_orphaned_records, get_check_constraints, get_foreign_keys,
    get_nullable_foreign_key_columns, get_tables_without_primary_keys, validate_schema,
    CheckConstraintInfo, ForeignKeyInfo, OrphanedRecordInfo, SchemaValidationReport,
    ValidationIssue, ValidationSeverity,
};
pub use security::{
    AuditAction, AuditContext, AuditLogEntry, AuditLogger, ComplianceManager, ComplianceReport,
    EncryptedData, EncryptionError, EncryptionProvider, KeyRotationManager, PasswordHashingService,
};
pub use sharding::{
    ConsistentHashRing, ShardCoordinator, ShardInfo, ShardKey, ShardPoolManager, ShardingStrategy,
};
pub use sqlx::PgPool;
pub use statement_cache::{
    CacheStats as StatementCacheStats, StatementCache, StatementCacheConfig,
};
pub use table_stats::{
    get_all_table_stats, get_database_size, get_high_seq_scan_tables, get_largest_tables,
    get_table_cache_hit_ratio, get_table_index_stats, get_table_stats, get_unused_indexes,
    CacheHitInfo, DatabaseSizeInfo, IndexStats, SeqScanInfo, TableStats,
};
pub use tracing::{
    get_tracer, init_tracer, trace_db_operation, DbTraceConfig, DbTracer, QueryMetadata,
    SpanContext, TraceableOperation,
};
pub use transaction::{
    retry_transaction, retry_transaction_with_isolation, with_savepoint, IsolationLevel,
    SavepointGuard, TransactionBuilder, TransactionExt, TransactionManager, TransactionRetryConfig,
};
pub use vacuum_scheduler::{
    TableVacuumInfo as VacuumTableInfo, VacuumResult, VacuumScheduler, VacuumSchedulerConfig,
    VacuumStats, VacuumStrategy,
};
