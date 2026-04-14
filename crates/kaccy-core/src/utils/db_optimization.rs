//! Database query optimization utilities
//!
//! Provides prepared statement caching, batch query execution, and connection pooling helpers

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::error::CoreError;

/// Prepared statement cache
#[derive(Clone)]
pub struct PreparedStatementCache {
    /// Cached statements indexed by key
    statements: Arc<RwLock<HashMap<String, CachedStatement>>>,
    /// Maximum number of statements to keep in cache
    max_cache_size: usize,
}

/// A cached prepared statement with metadata
#[derive(Debug, Clone)]
struct CachedStatement {
    /// SQL query
    sql: String,
    /// Number of times used
    use_count: usize,
    /// Last used timestamp
    last_used: Instant,
    /// Creation time
    #[allow(dead_code)]
    created_at: Instant,
}

impl PreparedStatementCache {
    /// Create a new prepared statement cache
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            statements: Arc::new(RwLock::new(HashMap::new())),
            max_cache_size,
        }
    }

    /// Get or cache a prepared statement
    pub fn get_or_insert(&self, key: String, sql: String) -> Result<String, CoreError> {
        let mut statements = self.statements.write().unwrap();

        if let Some(stmt) = statements.get_mut(&key) {
            stmt.use_count += 1;
            stmt.last_used = Instant::now();
            return Ok(stmt.sql.clone());
        }

        // If cache is full, evict least recently used
        if statements.len() >= self.max_cache_size {
            if let Some((lru_key, _)) = statements
                .iter()
                .min_by_key(|(_, stmt)| stmt.last_used)
                .map(|(k, v)| (k.clone(), v.clone()))
            {
                statements.remove(&lru_key);
            }
        }

        let now = Instant::now();
        let stmt = CachedStatement {
            sql: sql.clone(),
            use_count: 1,
            last_used: now,
            created_at: now,
        };

        statements.insert(key, stmt);
        Ok(sql)
    }

    /// Get cache statistics
    pub fn stats(&self) -> PreparedStatementStats {
        let statements = self.statements.read().unwrap();
        let total_uses: usize = statements.values().map(|s| s.use_count).sum();

        PreparedStatementStats {
            cached_statements: statements.len(),
            max_cache_size: self.max_cache_size,
            total_uses,
            cache_hit_potential: if total_uses > statements.len() {
                ((total_uses - statements.len()) as f64 / total_uses as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.statements.write().unwrap().clear();
    }

    /// Get most frequently used statements
    pub fn get_top_statements(&self, limit: usize) -> Vec<(String, usize)> {
        let statements = self.statements.read().unwrap();
        let mut entries: Vec<_> = statements
            .iter()
            .map(|(k, v)| (k.clone(), v.use_count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);
        entries
    }
}

/// Prepared statement cache statistics
#[derive(Debug, Clone)]
pub struct PreparedStatementStats {
    /// Number of statements currently in cache
    pub cached_statements: usize,
    /// Maximum allowed cache size
    pub max_cache_size: usize,
    /// Total number of cache hits across all statements
    pub total_uses: usize,
    /// Estimated cache hit percentage (0–100)
    pub cache_hit_potential: f64,
}

/// Batch query executor for efficient bulk operations
pub struct BatchQueryExecutor {
    /// Maximum number of rows per batch
    max_batch_size: usize,
    /// Maximum time to wait before flushing a batch
    #[allow(dead_code)]
    batch_timeout: Duration,
}

impl BatchQueryExecutor {
    /// Create a new batch query executor
    pub fn new(max_batch_size: usize, batch_timeout: Duration) -> Self {
        Self {
            max_batch_size,
            batch_timeout,
        }
    }

    /// Calculate optimal batch size based on data size
    pub fn optimal_batch_size(&self, _total_items: usize, item_size_bytes: usize) -> usize {
        // Target ~1MB per batch or max_batch_size, whichever is smaller
        let target_batch_bytes = 1024 * 1024; // 1MB
        let items_per_mb = target_batch_bytes / item_size_bytes.max(1);

        items_per_mb.min(self.max_batch_size).max(1)
    }

    /// Split items into batches
    pub fn create_batches<T: Clone>(&self, items: Vec<T>) -> Vec<Vec<T>> {
        let batch_size = self.max_batch_size.min(items.len()).max(1);
        items
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Generate placeholders for batch insert
    pub fn generate_placeholders(&self, num_rows: usize, num_columns: usize) -> String {
        let row_placeholder = format!(
            "({})",
            (1..=num_columns)
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ")
        );

        (0..num_rows)
            .map(|_| row_placeholder.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Estimate query execution time based on batch size
    pub fn estimate_execution_time(&self, batch_size: usize, avg_row_time_ms: f64) -> Duration {
        let total_ms = (batch_size as f64) * avg_row_time_ms;
        Duration::from_millis(total_ms as u64)
    }
}

/// Connection pool optimizer
pub struct ConnectionPoolOptimizer {
    /// Minimum number of pooled connections
    min_connections: usize,
    /// Maximum number of pooled connections
    max_connections: usize,
    /// Timeout when acquiring a connection from the pool
    connection_timeout: Duration,
    /// Time after which idle connections are closed
    #[allow(dead_code)]
    idle_timeout: Duration,
}

impl ConnectionPoolOptimizer {
    /// Create a new connection pool optimizer
    pub fn new(
        min_connections: usize,
        max_connections: usize,
        connection_timeout: Duration,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            min_connections,
            max_connections,
            connection_timeout,
            idle_timeout,
        }
    }

    /// Calculate optimal pool size based on workload
    pub fn calculate_optimal_size(&self, concurrent_requests: usize) -> PoolSizeRecommendation {
        // Rule of thumb: connections = (core_count * 2) + effective_spindle_count
        // For web apps: connections = (concurrent_requests * 1.5) bounded by limits

        let recommended = ((concurrent_requests as f64) * 1.5) as usize;
        let optimal = recommended
            .max(self.min_connections)
            .min(self.max_connections);

        PoolSizeRecommendation {
            recommended_size: optimal,
            min_size: self.min_connections,
            max_size: self.max_connections,
            current_load: concurrent_requests,
            utilization: (concurrent_requests as f64 / optimal as f64) * 100.0,
        }
    }

    /// Validate pool configuration
    pub fn validate_config(&self) -> Result<(), CoreError> {
        if self.min_connections > self.max_connections {
            return Err(CoreError::Validation(
                "min_connections cannot be greater than max_connections".to_string(),
            ));
        }

        if self.min_connections == 0 {
            return Err(CoreError::Validation(
                "min_connections must be at least 1".to_string(),
            ));
        }

        if self.connection_timeout.as_secs() == 0 {
            return Err(CoreError::Validation(
                "connection_timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Get tuning recommendations
    pub fn get_tuning_recommendations(&self, metrics: &PoolMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for frequent timeouts
        if metrics.timeout_rate > 0.05 {
            recommendations.push(format!(
                "High timeout rate ({:.1}%). Consider increasing connection_timeout or max_connections",
                metrics.timeout_rate * 100.0
            ));
        }

        // Check for low utilization
        if metrics.avg_utilization < 0.2 {
            recommendations.push(format!(
                "Low utilization ({:.1}%). Consider reducing min_connections to save resources",
                metrics.avg_utilization * 100.0
            ));
        }

        // Check for high utilization
        if metrics.avg_utilization > 0.8 {
            recommendations.push(format!(
                "High utilization ({:.1}%). Consider increasing max_connections",
                metrics.avg_utilization * 100.0
            ));
        }

        // Check wait times
        if metrics.avg_wait_time_ms > 100.0 {
            recommendations.push(format!(
                "High average wait time ({:.1}ms). Increase pool size or optimize queries",
                metrics.avg_wait_time_ms
            ));
        }

        recommendations
    }
}

/// Pool size recommendation
#[derive(Debug, Clone)]
pub struct PoolSizeRecommendation {
    /// Recommended pool size for the current workload
    pub recommended_size: usize,
    /// Configured minimum pool size
    pub min_size: usize,
    /// Configured maximum pool size
    pub max_size: usize,
    /// Current number of concurrent requests
    pub current_load: usize,
    /// Estimated pool utilization as a percentage
    pub utilization: f64,
}

/// Connection pool metrics
#[derive(Debug, Clone)]
pub struct PoolMetrics {
    /// Total connections in the pool
    pub total_connections: usize,
    /// Connections currently executing queries
    pub active_connections: usize,
    /// Connections waiting for work
    pub idle_connections: usize,
    /// Average fraction of pool in use (0.0–1.0)
    pub avg_utilization: f64,
    /// Fraction of requests that timed out (0.0–1.0)
    pub timeout_rate: f64,
    /// Average milliseconds spent waiting for a connection
    pub avg_wait_time_ms: f64,
}

/// Query performance analyzer
pub struct QueryPerformanceAnalyzer {
    /// Queries slower than this threshold (ms) are flagged as slow
    slow_query_threshold_ms: u64,
    /// Ring-buffer of recent query execution records
    query_history: Arc<RwLock<Vec<QueryExecution>>>,
    /// Maximum number of records to retain in history
    max_history_size: usize,
}

/// A single query execution record
#[derive(Debug, Clone)]
struct QueryExecution {
    query_hash: String,
    execution_time_ms: u64,
    #[allow(dead_code)]
    timestamp: Instant,
    #[allow(dead_code)]
    rows_affected: usize,
}

impl QueryPerformanceAnalyzer {
    /// Create a new query performance analyzer
    pub fn new(slow_query_threshold_ms: u64, max_history_size: usize) -> Self {
        Self {
            slow_query_threshold_ms,
            query_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        }
    }

    /// Record a query execution
    pub fn record_query(&self, query: &str, execution_time_ms: u64, rows_affected: usize) {
        let query_hash = format!("{:x}", md5::compute(query));

        let execution = QueryExecution {
            query_hash,
            execution_time_ms,
            timestamp: Instant::now(),
            rows_affected,
        };

        let mut history = self.query_history.write().unwrap();
        history.push(execution);

        // Trim history if needed
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
    }

    /// Get slow queries
    pub fn get_slow_queries(&self) -> Vec<SlowQuery> {
        let history = self.query_history.read().unwrap();

        let mut slow_queries: HashMap<String, Vec<u64>> = HashMap::new();

        for execution in history.iter() {
            if execution.execution_time_ms >= self.slow_query_threshold_ms {
                slow_queries
                    .entry(execution.query_hash.clone())
                    .or_default()
                    .push(execution.execution_time_ms);
            }
        }

        slow_queries
            .into_iter()
            .map(|(hash, times)| {
                let avg = times.iter().sum::<u64>() / times.len() as u64;
                let max = *times.iter().max().unwrap_or(&0);
                SlowQuery {
                    query_hash: hash,
                    occurrences: times.len(),
                    avg_execution_time_ms: avg,
                    max_execution_time_ms: max,
                }
            })
            .collect()
    }

    /// Get query performance statistics
    pub fn get_stats(&self) -> QueryPerformanceStats {
        let history = self.query_history.read().unwrap();

        if history.is_empty() {
            return QueryPerformanceStats {
                total_queries: 0,
                avg_execution_time_ms: 0.0,
                slow_query_count: 0,
                slow_query_percentage: 0.0,
            };
        }

        let total = history.len();
        let sum: u64 = history.iter().map(|e| e.execution_time_ms).sum();
        let slow_count = history
            .iter()
            .filter(|e| e.execution_time_ms >= self.slow_query_threshold_ms)
            .count();

        QueryPerformanceStats {
            total_queries: total,
            avg_execution_time_ms: sum as f64 / total as f64,
            slow_query_count: slow_count,
            slow_query_percentage: (slow_count as f64 / total as f64) * 100.0,
        }
    }
}

/// Slow query information
#[derive(Debug, Clone)]
pub struct SlowQuery {
    /// MD5 hash identifying the query template
    pub query_hash: String,
    /// Number of times this slow query was seen
    pub occurrences: usize,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: u64,
    /// Worst-case execution time in milliseconds
    pub max_execution_time_ms: u64,
}

/// Query performance statistics
#[derive(Debug, Clone)]
pub struct QueryPerformanceStats {
    /// Total number of queries recorded
    pub total_queries: usize,
    /// Mean execution time across all queries (ms)
    pub avg_execution_time_ms: f64,
    /// Number of queries that exceeded the slow threshold
    pub slow_query_count: usize,
    /// Percentage of queries classified as slow
    pub slow_query_percentage: f64,
}

/// Materialized view manager for analytics
pub struct MaterializedViewManager {
    /// Registered materialized views indexed by name
    views: Arc<RwLock<HashMap<String, MaterializedView>>>,
}

/// A materialized view definition
#[derive(Debug, Clone)]
pub struct MaterializedView {
    /// View name
    pub name: String,
    /// Source query
    pub source_query: String,
    /// Last refresh time
    pub last_refresh: Instant,
    /// Refresh interval
    pub refresh_interval: Duration,
    /// Whether to refresh on demand or scheduled
    pub refresh_strategy: RefreshStrategy,
    /// Estimated row count
    pub estimated_rows: usize,
}

/// Refresh strategy for materialized views
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStrategy {
    /// Refresh on every query
    OnDemand,
    /// Refresh on a schedule
    Scheduled,
    /// Refresh when source data changes
    OnDataChange,
}

impl MaterializedViewManager {
    /// Create a new materialized view manager
    pub fn new() -> Self {
        Self {
            views: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a materialized view
    pub fn register_view(
        &self,
        name: String,
        source_query: String,
        refresh_interval: Duration,
        refresh_strategy: RefreshStrategy,
    ) {
        let view = MaterializedView {
            name: name.clone(),
            source_query,
            last_refresh: Instant::now(),
            refresh_interval,
            refresh_strategy,
            estimated_rows: 0,
        };

        self.views.write().unwrap().insert(name, view);
    }

    /// Check if a view needs refresh
    pub fn needs_refresh(&self, view_name: &str) -> bool {
        let views = self.views.read().unwrap();
        if let Some(view) = views.get(view_name) {
            match view.refresh_strategy {
                RefreshStrategy::OnDemand => true,
                RefreshStrategy::Scheduled => view.last_refresh.elapsed() >= view.refresh_interval,
                RefreshStrategy::OnDataChange => {
                    // Would check data version/timestamp in real implementation
                    view.last_refresh.elapsed() >= view.refresh_interval
                }
            }
        } else {
            false
        }
    }

    /// Mark view as refreshed
    pub fn mark_refreshed(&self, view_name: &str, row_count: usize) {
        let mut views = self.views.write().unwrap();
        if let Some(view) = views.get_mut(view_name) {
            view.last_refresh = Instant::now();
            view.estimated_rows = row_count;
        }
    }

    /// Get views that need refresh
    pub fn get_stale_views(&self) -> Vec<String> {
        let views = self.views.read().unwrap();
        views
            .iter()
            .filter(|(_name, view)| {
                view.refresh_strategy == RefreshStrategy::Scheduled
                    && view.last_refresh.elapsed() >= view.refresh_interval
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Generate SQL for creating materialized view (PostgreSQL syntax)
    pub fn generate_create_sql(&self, view_name: &str) -> Option<String> {
        let views = self.views.read().unwrap();
        views.get(view_name).map(|view| {
            format!(
                "CREATE MATERIALIZED VIEW {} AS {}",
                view_name, view.source_query
            )
        })
    }

    /// Generate SQL for refreshing materialized view
    pub fn generate_refresh_sql(&self, view_name: &str, concurrent: bool) -> String {
        if concurrent {
            format!("REFRESH MATERIALIZED VIEW CONCURRENTLY {}", view_name)
        } else {
            format!("REFRESH MATERIALIZED VIEW {}", view_name)
        }
    }

    /// Get view statistics
    pub fn get_view_stats(&self) -> MaterializedViewStats {
        let views = self.views.read().unwrap();
        let total = views.len();
        let stale = views
            .values()
            .filter(|v| {
                v.refresh_strategy == RefreshStrategy::Scheduled
                    && v.last_refresh.elapsed() >= v.refresh_interval
            })
            .count();

        MaterializedViewStats {
            total_views: total,
            stale_views: stale,
            total_estimated_rows: views.values().map(|v| v.estimated_rows).sum(),
        }
    }
}

impl Default for MaterializedViewManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Materialized view statistics
#[derive(Debug, Clone)]
pub struct MaterializedViewStats {
    /// Total number of registered views
    pub total_views: usize,
    /// Number of views that are past their refresh interval
    pub stale_views: usize,
    /// Sum of estimated row counts across all views
    pub total_estimated_rows: usize,
}

/// Partial index optimizer for hot queries
pub struct PartialIndexOptimizer {
    /// Accumulated index recommendations
    recommendations: Arc<RwLock<Vec<IndexRecommendation>>>,
}

/// Index recommendation
#[derive(Debug, Clone)]
pub struct IndexRecommendation {
    /// Table name
    pub table: String,
    /// Column(s) to index
    pub columns: Vec<String>,
    /// WHERE clause for partial index
    pub where_clause: Option<String>,
    /// Index type (btree, hash, gin, etc.)
    pub index_type: IndexType,
    /// Expected improvement percentage
    pub expected_improvement: f64,
    /// Reason for recommendation
    pub reason: String,
}

/// Index type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// B-tree index (default, supports <, <=, =, >=, >)
    BTree,
    /// Hash index (only supports =)
    Hash,
    /// GIN index (for full-text search, arrays, JSON)
    Gin,
    /// GiST index (for geometric data, full-text search)
    Gist,
    /// BRIN index (for large tables with natural ordering)
    Brin,
}

impl PartialIndexOptimizer {
    /// Create a new partial index optimizer
    pub fn new() -> Self {
        Self {
            recommendations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Analyze query and recommend partial index
    pub fn recommend_partial_index(
        &self,
        table: String,
        columns: Vec<String>,
        where_clause: Option<String>,
        query_frequency: usize,
        avg_execution_time_ms: u64,
    ) {
        let expected_improvement = if where_clause.is_some() {
            // Partial indexes can provide 2-5x improvement
            40.0 + (query_frequency as f64).log10() * 10.0
        } else {
            20.0 + (query_frequency as f64).log10() * 5.0
        };

        let reason = if where_clause.is_some() {
            format!(
                "Frequent query ({} times) with WHERE clause, avg {}ms",
                query_frequency, avg_execution_time_ms
            )
        } else {
            format!(
                "Frequent query ({} times), avg {}ms",
                query_frequency, avg_execution_time_ms
            )
        };

        let rec = IndexRecommendation {
            table,
            columns,
            where_clause,
            index_type: IndexType::BTree,
            expected_improvement: expected_improvement.min(95.0),
            reason,
        };

        self.recommendations.write().unwrap().push(rec);
    }

    /// Get all recommendations
    pub fn get_recommendations(&self) -> Vec<IndexRecommendation> {
        self.recommendations.read().unwrap().clone()
    }

    /// Generate CREATE INDEX SQL
    pub fn generate_index_sql(&self, rec: &IndexRecommendation, index_name: &str) -> String {
        let index_type_str = match rec.index_type {
            IndexType::BTree => "",
            IndexType::Hash => " USING HASH",
            IndexType::Gin => " USING GIN",
            IndexType::Gist => " USING GIST",
            IndexType::Brin => " USING BRIN",
        };

        let columns_str = rec.columns.join(", ");
        let where_clause = rec
            .where_clause
            .as_ref()
            .map(|w| format!(" WHERE {}", w))
            .unwrap_or_default();

        format!(
            "CREATE INDEX{} {} ON {} ({}){};",
            index_type_str, index_name, rec.table, columns_str, where_clause
        )
    }

    /// Clear recommendations
    pub fn clear(&self) {
        self.recommendations.write().unwrap().clear();
    }
}

impl Default for PartialIndexOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Index-only scan analyzer
pub struct IndexOnlyScanAnalyzer {
    /// Per-table scan statistics
    stats: Arc<RwLock<HashMap<String, ScanStats>>>,
}

/// Scan statistics for a table
#[derive(Debug, Clone)]
pub struct ScanStats {
    /// Total number of scans recorded
    pub total_scans: usize,
    /// Number of scans that were index-only (no heap access)
    pub index_only_scans: usize,
    /// Total heap page fetches for visibility checks
    pub heap_fetches: usize,
    /// Average number of rows returned per scan
    pub avg_rows_per_scan: f64,
}

impl IndexOnlyScanAnalyzer {
    /// Create a new index-only scan analyzer
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a scan operation
    pub fn record_scan(&self, table: String, index_only: bool, heap_fetches: usize, rows: usize) {
        let mut stats = self.stats.write().unwrap();
        let entry = stats.entry(table).or_insert(ScanStats {
            total_scans: 0,
            index_only_scans: 0,
            heap_fetches: 0,
            avg_rows_per_scan: 0.0,
        });

        entry.total_scans += 1;
        if index_only {
            entry.index_only_scans += 1;
        }
        entry.heap_fetches += heap_fetches;

        // Update average rows per scan
        let total_rows = entry.avg_rows_per_scan * (entry.total_scans - 1) as f64 + rows as f64;
        entry.avg_rows_per_scan = total_rows / entry.total_scans as f64;
    }

    /// Get scan efficiency for a table
    pub fn get_efficiency(&self, table: &str) -> Option<ScanEfficiency> {
        let stats = self.stats.read().unwrap();
        stats.get(table).map(|s| {
            let index_only_ratio = if s.total_scans > 0 {
                s.index_only_scans as f64 / s.total_scans as f64
            } else {
                0.0
            };

            let heap_fetch_ratio = if s.index_only_scans > 0 {
                s.heap_fetches as f64 / s.index_only_scans as f64
            } else {
                0.0
            };

            ScanEfficiency {
                index_only_ratio,
                heap_fetch_ratio,
                recommendation: if index_only_ratio < 0.5 {
                    "Consider adding covering indexes for frequently accessed columns".to_string()
                } else if heap_fetch_ratio > 0.1 {
                    "Run VACUUM to improve visibility map and reduce heap fetches".to_string()
                } else {
                    "Scan efficiency is good".to_string()
                },
            }
        })
    }

    /// Get all table stats
    pub fn get_all_stats(&self) -> Vec<(String, ScanStats)> {
        let stats = self.stats.read().unwrap();
        stats.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

impl Default for IndexOnlyScanAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan efficiency metrics
#[derive(Debug, Clone)]
pub struct ScanEfficiency {
    /// Fraction of scans that were index-only (0.0–1.0)
    pub index_only_ratio: f64,
    /// Heap fetches per index-only scan (lower is better)
    pub heap_fetch_ratio: f64,
    /// Human-readable optimization recommendation
    pub recommendation: String,
}

/// Query result cache with TTL
pub struct QueryResultCache {
    /// Cached results indexed by query key
    cache: Arc<RwLock<HashMap<String, CachedResult>>>,
    /// Default time-to-live for cache entries
    default_ttl: Duration,
    /// Maximum number of entries the cache may hold
    max_size: usize,
}

/// A cached query result
#[derive(Debug, Clone)]
struct CachedResult {
    /// Query hash
    _query_hash: String,
    /// Cached at timestamp
    cached_at: Instant,
    /// Time-to-live
    ttl: Duration,
    /// Result size in bytes (approximation)
    size_bytes: usize,
    /// Number of times accessed
    access_count: usize,
}

impl QueryResultCache {
    /// Create a new query result cache
    pub fn new(default_ttl: Duration, max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
            max_size,
        }
    }

    /// Generate cache key for a query
    pub fn generate_key(&self, query: &str, params: &[&str]) -> String {
        let combined = format!("{}:{}", query, params.join(":"));
        format!("{:x}", md5::compute(combined))
    }

    /// Check if result is cached and valid
    pub fn is_cached(&self, key: &str) -> bool {
        let cache = self.cache.read().unwrap();
        if let Some(result) = cache.get(key) {
            result.cached_at.elapsed() < result.ttl
        } else {
            false
        }
    }

    /// Store query result in cache
    pub fn store(
        &self,
        key: String,
        size_bytes: usize,
        ttl: Option<Duration>,
    ) -> Result<(), CoreError> {
        let mut cache = self.cache.write().unwrap();

        // Evict if cache is full
        if cache.len() >= self.max_size {
            // Evict least recently used
            if let Some((lru_key, _)) = cache
                .iter()
                .min_by_key(|(_, v)| v.cached_at)
                .map(|(k, v)| (k.clone(), v.clone()))
            {
                cache.remove(&lru_key);
            }
        }

        let result = CachedResult {
            _query_hash: key.clone(),
            cached_at: Instant::now(),
            ttl: ttl.unwrap_or(self.default_ttl),
            size_bytes,
            access_count: 0,
        };

        cache.insert(key, result);
        Ok(())
    }

    /// Increment access count for a cached result
    pub fn record_access(&self, key: &str) {
        let mut cache = self.cache.write().unwrap();
        if let Some(result) = cache.get_mut(key) {
            result.access_count += 1;
        }
    }

    /// Invalidate a specific cache entry
    pub fn invalidate(&self, key: &str) {
        self.cache.write().unwrap().remove(key);
    }

    /// Invalidate all cache entries matching a pattern
    pub fn invalidate_pattern(&self, pattern: &str) {
        let mut cache = self.cache.write().unwrap();
        cache.retain(|k, _| !k.contains(pattern));
    }

    /// Clear expired entries
    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.retain(|_, v| v.cached_at.elapsed() < v.ttl);
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> QueryCacheStats {
        let cache = self.cache.read().unwrap();
        let total_entries = cache.len();
        let total_size: usize = cache.values().map(|v| v.size_bytes).sum();
        let total_accesses: usize = cache.values().map(|v| v.access_count).sum();
        let expired = cache
            .values()
            .filter(|v| v.cached_at.elapsed() >= v.ttl)
            .count();

        QueryCacheStats {
            total_entries,
            expired_entries: expired,
            total_size_bytes: total_size,
            total_accesses,
            hit_rate: if total_entries > 0 {
                (total_accesses as f64 / (total_accesses + total_entries) as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Clear all cache
    pub fn clear(&self) {
        self.cache.write().unwrap().clear();
    }
}

/// Query cache statistics
#[derive(Debug, Clone)]
pub struct QueryCacheStats {
    /// Total number of entries currently in cache
    pub total_entries: usize,
    /// Number of entries that have expired but not yet evicted
    pub expired_entries: usize,
    /// Approximate total size of all cached results in bytes
    pub total_size_bytes: usize,
    /// Total number of times cached results were accessed
    pub total_accesses: usize,
    /// Cache hit rate as a percentage
    pub hit_rate: f64,
}

/// Transaction batcher for efficient bulk operations
pub struct TransactionBatcher {
    /// Transactions queued for the next batch
    pending: Arc<RwLock<Vec<PendingTransaction>>>,
    /// Maximum transactions per batch
    max_batch_size: usize,
    /// Maximum time between batch flushes
    batch_timeout: Duration,
    /// Timestamp of the most recent batch flush
    last_flush: Arc<RwLock<Instant>>,
}

/// A pending transaction
#[derive(Debug, Clone)]
pub struct PendingTransaction {
    /// Transaction ID
    pub id: String,
    /// SQL statements
    pub statements: Vec<String>,
    /// Queued at
    pub queued_at: Instant,
    /// Priority (higher = more urgent)
    pub priority: u8,
}

impl TransactionBatcher {
    /// Create a new transaction batcher
    pub fn new(max_batch_size: usize, batch_timeout: Duration) -> Self {
        Self {
            pending: Arc::new(RwLock::new(Vec::new())),
            max_batch_size,
            batch_timeout,
            last_flush: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Add a transaction to the batch
    pub fn add_transaction(
        &self,
        id: String,
        statements: Vec<String>,
        priority: u8,
    ) -> Result<(), CoreError> {
        let mut pending = self.pending.write().unwrap();

        let tx = PendingTransaction {
            id,
            statements,
            queued_at: Instant::now(),
            priority,
        };

        pending.push(tx);

        // Sort by priority (highest first)
        pending.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(())
    }

    /// Check if batch should be flushed
    pub fn should_flush(&self) -> bool {
        let pending = self.pending.read().unwrap();
        let last_flush = self.last_flush.read().unwrap();

        pending.len() >= self.max_batch_size || last_flush.elapsed() >= self.batch_timeout
    }

    /// Get batch for execution
    pub fn get_batch(&self) -> Vec<PendingTransaction> {
        let mut pending = self.pending.write().unwrap();
        let batch_size = self.max_batch_size.min(pending.len());

        let batch = pending.drain(0..batch_size).collect();
        *self.last_flush.write().unwrap() = Instant::now();

        batch
    }

    /// Get batch statistics
    pub fn get_stats(&self) -> TransactionBatchStats {
        let pending = self.pending.read().unwrap();
        let oldest_age = pending
            .first()
            .map(|tx| tx.queued_at.elapsed().as_millis() as u64)
            .unwrap_or(0);

        TransactionBatchStats {
            pending_count: pending.len(),
            oldest_transaction_age_ms: oldest_age,
            time_since_last_flush_ms: self.last_flush.read().unwrap().elapsed().as_millis() as u64,
        }
    }

    /// Generate batched SQL
    pub fn generate_batch_sql(&self, batch: &[PendingTransaction]) -> String {
        let mut sql = String::from("BEGIN;\n");

        for tx in batch {
            for stmt in &tx.statements {
                sql.push_str(stmt);
                sql.push_str(";\n");
            }
        }

        sql.push_str("COMMIT;");
        sql
    }
}

/// Transaction batch statistics
#[derive(Debug, Clone)]
pub struct TransactionBatchStats {
    /// Number of transactions waiting to be flushed
    pub pending_count: usize,
    /// Age in milliseconds of the oldest pending transaction
    pub oldest_transaction_age_ms: u64,
    /// Milliseconds elapsed since the last batch was flushed
    pub time_since_last_flush_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepared_statement_cache() {
        let cache = PreparedStatementCache::new(10);

        let sql1 = "SELECT * FROM users WHERE id = ?".to_string();
        let result = cache
            .get_or_insert("query1".to_string(), sql1.clone())
            .unwrap();
        assert_eq!(result, sql1);

        // Should retrieve from cache
        let result2 = cache
            .get_or_insert("query1".to_string(), sql1.clone())
            .unwrap();
        assert_eq!(result2, sql1);

        let stats = cache.stats();
        assert_eq!(stats.cached_statements, 1);
        assert_eq!(stats.total_uses, 2);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = PreparedStatementCache::new(2);

        cache
            .get_or_insert("q1".to_string(), "SQL1".to_string())
            .unwrap();
        cache
            .get_or_insert("q2".to_string(), "SQL2".to_string())
            .unwrap();
        cache
            .get_or_insert("q3".to_string(), "SQL3".to_string())
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.cached_statements, 2); // One should be evicted
    }

    #[test]
    fn test_batch_query_executor() {
        let executor = BatchQueryExecutor::new(100, Duration::from_secs(5));

        let items: Vec<i32> = (0..250).collect();
        let batches = executor.create_batches(items);

        assert_eq!(batches.len(), 3); // 100 + 100 + 50
        assert_eq!(batches[0].len(), 100);
        assert_eq!(batches[2].len(), 50);
    }

    #[test]
    fn test_optimal_batch_size() {
        let executor = BatchQueryExecutor::new(1000, Duration::from_secs(5));

        let batch_size = executor.optimal_batch_size(10000, 1024);
        assert!(batch_size > 0);
        assert!(batch_size <= 1000);
    }

    #[test]
    fn test_placeholder_generation() {
        let executor = BatchQueryExecutor::new(100, Duration::from_secs(5));

        let placeholders = executor.generate_placeholders(2, 3);
        assert_eq!(placeholders, "(?, ?, ?), (?, ?, ?)");
    }

    #[test]
    fn test_connection_pool_optimizer() {
        let optimizer =
            ConnectionPoolOptimizer::new(5, 20, Duration::from_secs(30), Duration::from_secs(600));

        assert!(optimizer.validate_config().is_ok());

        let recommendation = optimizer.calculate_optimal_size(10);
        assert!(recommendation.recommended_size >= 5);
        assert!(recommendation.recommended_size <= 20);
    }

    #[test]
    fn test_invalid_pool_config() {
        let optimizer = ConnectionPoolOptimizer::new(
            20, // min > max
            10,
            Duration::from_secs(30),
            Duration::from_secs(600),
        );

        assert!(optimizer.validate_config().is_err());
    }

    #[test]
    fn test_pool_tuning_recommendations() {
        let optimizer =
            ConnectionPoolOptimizer::new(5, 20, Duration::from_secs(30), Duration::from_secs(600));

        let metrics = PoolMetrics {
            total_connections: 20,
            active_connections: 18,
            idle_connections: 2,
            avg_utilization: 0.9,
            timeout_rate: 0.01,
            avg_wait_time_ms: 50.0,
        };

        let recommendations = optimizer.get_tuning_recommendations(&metrics);
        assert!(!recommendations.is_empty());
    }

    #[test]
    fn test_query_performance_analyzer() {
        let analyzer = QueryPerformanceAnalyzer::new(100, 1000);

        analyzer.record_query("SELECT * FROM users", 50, 100);
        analyzer.record_query("SELECT * FROM orders", 150, 50);
        analyzer.record_query("SELECT * FROM products", 200, 1000);

        let stats = analyzer.get_stats();
        assert_eq!(stats.total_queries, 3);
        assert!(stats.avg_execution_time_ms > 0.0);

        let slow_queries = analyzer.get_slow_queries();
        assert_eq!(slow_queries.len(), 2); // 2 queries >= 100ms
    }

    #[test]
    fn test_top_statements() {
        let cache = PreparedStatementCache::new(10);

        cache
            .get_or_insert("q1".to_string(), "SQL1".to_string())
            .unwrap();
        cache
            .get_or_insert("q1".to_string(), "SQL1".to_string())
            .unwrap();
        cache
            .get_or_insert("q2".to_string(), "SQL2".to_string())
            .unwrap();

        let top = cache.get_top_statements(5);
        assert_eq!(top[0].0, "q1");
        assert_eq!(top[0].1, 2);
    }

    #[test]
    fn test_materialized_view_manager() {
        let manager = MaterializedViewManager::new();

        manager.register_view(
            "daily_stats".to_string(),
            "SELECT date, COUNT(*) FROM trades GROUP BY date".to_string(),
            Duration::from_millis(1),
            RefreshStrategy::Scheduled,
        );

        // Initially just registered, so doesn't need refresh yet
        std::thread::sleep(Duration::from_millis(5));
        assert!(manager.needs_refresh("daily_stats"));
        manager.mark_refreshed("daily_stats", 100);

        let stats = manager.get_view_stats();
        assert_eq!(stats.total_views, 1);
        assert_eq!(stats.total_estimated_rows, 100);

        let sql = manager.generate_create_sql("daily_stats");
        assert!(sql.is_some());
        assert!(sql.unwrap().contains("CREATE MATERIALIZED VIEW"));
    }

    #[test]
    fn test_partial_index_optimizer() {
        let optimizer = PartialIndexOptimizer::new();

        optimizer.recommend_partial_index(
            "users".to_string(),
            vec!["email".to_string()],
            Some("deleted_at IS NULL".to_string()),
            1000,
            50,
        );

        let recommendations = optimizer.get_recommendations();
        assert_eq!(recommendations.len(), 1);
        assert_eq!(recommendations[0].table, "users");
        assert!(recommendations[0].expected_improvement > 0.0);

        let sql = optimizer.generate_index_sql(&recommendations[0], "idx_users_email_active");
        assert!(sql.contains("CREATE INDEX"));
        assert!(sql.contains("WHERE deleted_at IS NULL"));
    }

    #[test]
    fn test_index_only_scan_analyzer() {
        let analyzer = IndexOnlyScanAnalyzer::new();

        analyzer.record_scan("users".to_string(), true, 5, 100);
        analyzer.record_scan("users".to_string(), true, 2, 50);
        analyzer.record_scan("users".to_string(), false, 0, 200);

        let efficiency = analyzer.get_efficiency("users");
        assert!(efficiency.is_some());

        let eff = efficiency.unwrap();
        assert!(eff.index_only_ratio > 0.0);
        assert!(!eff.recommendation.is_empty());
    }

    #[test]
    fn test_query_result_cache() {
        let cache = QueryResultCache::new(Duration::from_secs(60), 100);

        let key = cache.generate_key("SELECT * FROM users WHERE id = ?", &["123"]);
        assert!(!cache.is_cached(&key));

        cache.store(key.clone(), 1024, None).unwrap();
        assert!(cache.is_cached(&key));

        cache.record_access(&key);
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_accesses, 1);

        cache.invalidate(&key);
        assert!(!cache.is_cached(&key));
    }

    #[test]
    fn test_transaction_batcher() {
        let batcher = TransactionBatcher::new(10, Duration::from_secs(5));

        batcher
            .add_transaction(
                "tx1".to_string(),
                vec!["UPDATE users SET balance = 100".to_string()],
                5,
            )
            .unwrap();

        batcher
            .add_transaction(
                "tx2".to_string(),
                vec!["INSERT INTO trades VALUES (1, 2, 3)".to_string()],
                3,
            )
            .unwrap();

        let stats = batcher.get_stats();
        assert_eq!(stats.pending_count, 2);

        let batch = batcher.get_batch();
        assert!(!batch.is_empty());

        let sql = batcher.generate_batch_sql(&batch);
        assert!(sql.contains("BEGIN"));
        assert!(sql.contains("COMMIT"));
    }

    #[test]
    fn test_materialized_view_stale_detection() {
        let manager = MaterializedViewManager::new();

        manager.register_view(
            "test_view".to_string(),
            "SELECT *".to_string(),
            Duration::from_millis(1),
            RefreshStrategy::Scheduled,
        );

        std::thread::sleep(Duration::from_millis(5));

        let stale = manager.get_stale_views();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0], "test_view");
    }

    #[test]
    fn test_query_cache_expiration() {
        let cache = QueryResultCache::new(Duration::from_millis(10), 100);

        let key = cache.generate_key("SELECT * FROM test", &[]);
        cache.store(key.clone(), 100, None).unwrap();
        assert!(cache.is_cached(&key));

        std::thread::sleep(Duration::from_millis(20));
        assert!(!cache.is_cached(&key));

        cache.cleanup_expired();
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 0);
    }
}
