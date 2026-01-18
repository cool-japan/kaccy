# Performance Tuning Guide

This guide helps you optimize `kaccy-db` performance for production workloads.

## Table of Contents

- [Quick Wins](#quick-wins)
- [Connection Pool Tuning](#connection-pool-tuning)
- [Query Optimization](#query-optimization)
- [Caching Strategy](#caching-strategy)
- [Indexing](#indexing)
- [Replication and Sharding](#replication-and-sharding)
- [Monitoring and Metrics](#monitoring-and-metrics)
- [Hardware and Infrastructure](#hardware-and-infrastructure)

## Quick Wins

### 1. Enable Connection Pooling

**Impact**: High | **Effort**: Low

```rust
use kaccy_db::pool::{create_pool_with_config, PoolConfig, WarmupStrategy};
use std::time::Duration;

let config = PoolConfig {
    max_connections: 20,
    min_connections: 5,
    acquire_timeout: Duration::from_secs(10),
    idle_timeout: Some(Duration::from_secs(300)),
    max_lifetime: Some(Duration::from_secs(1800)),
    warmup_strategy: WarmupStrategy::MinConnections,
};

let pool = create_pool_with_config(database_url, config).await?;
```

**Why it helps**: Reuses connections, reduces connection overhead

### 2. Enable Multi-Tier Caching

**Impact**: High | **Effort**: Medium

```rust
use kaccy_db::multi_tier_cache::{MultiTierCache, MultiTierConfig};

let config = MultiTierConfig {
    l1_capacity: 10000,        // In-memory cache size
    l1_ttl_seconds: 300,       // 5 minutes
    l2_ttl_seconds: 3600,      // 1 hour in Redis
    promotion_threshold: 3,    // Promote after 3 L2 hits
};

let cache = MultiTierCache::new(config, redis_client).await?;
```

**Why it helps**: L1 in-memory cache avoids network calls for hot data

### 3. Use Read Replicas

**Impact**: High | **Effort**: Medium

```rust
use kaccy_db::replica::{ReplicaPoolManager, LoadBalancingStrategy};

let mut manager = ReplicaPoolManager::new(primary_pool);
manager.add_replica("replica1", replica_pool_1).await;
manager.add_replica("replica2", replica_pool_2).await;
manager.set_strategy(LoadBalancingStrategy::LeastConnections);

// Automatically routes reads to replicas
let smart_client = manager.create_smart_client();
```

**Why it helps**: Distributes read load across multiple databases

### 4. Enable Prepared Statement Caching

**Impact**: Medium | **Effort**: Low

```rust
use kaccy_db::statement_cache::{StatementCache, StatementCacheConfig};
use std::time::Duration;

let config = StatementCacheConfig {
    max_size: 1000,
    ttl: Duration::from_secs(3600),
};

let cache = StatementCache::new(config);
```

**Why it helps**: Reuses query plans, reduces parsing overhead

### 5. Optimize Indexes

**Impact**: High | **Effort**: Low

```rust
use kaccy_db::index_analyzer::IndexAnalyzer;

let analyzer = IndexAnalyzer::new();
let suggestions = analyzer.analyze_slow_queries(&pool, 100.0).await?;

for suggestion in suggestions {
    if suggestion.priority == IndexPriority::Critical {
        println!("{}", suggestion.to_sql());
        // Run the suggested CREATE INDEX statement
    }
}
```

**Why it helps**: Proper indexes can reduce query time by orders of magnitude

## Connection Pool Tuning

### Sizing the Pool

**Formula**: `max_connections = (core_count * 2) + effective_spindle_count`

For cloud databases (e.g., RDS):
- **Small app** (< 10 req/s): 10-20 connections
- **Medium app** (10-100 req/s): 20-50 connections
- **Large app** (> 100 req/s): 50-100 connections

**Never exceed** your database's max connections limit!

```rust
let config = PoolConfig {
    // Production values
    max_connections: 50,
    min_connections: 10,

    // Prevent connection leaks
    acquire_timeout: Duration::from_secs(10),
    idle_timeout: Some(Duration::from_secs(600)),
    max_lifetime: Some(Duration::from_secs(3600)),

    // Pre-warm connections
    warmup_strategy: WarmupStrategy::MinConnections,
};
```

### Connection Warm-up

Warm up connections on startup to avoid cold start latency:

```rust
use kaccy_db::pool::{warmup_pool, WarmupStrategy};

// After creating pool
warmup_pool(&pool, WarmupStrategy::HalfCapacity).await?;
```

**Strategies**:
- `None`: No warm-up (fastest startup)
- `MinConnections`: Warm up min_connections (balanced)
- `HalfCapacity`: Warm up 50% of max (recommended)
- `FullCapacity`: Warm up all connections (slowest startup, best steady-state)

### Connection Validation

Validate connections periodically to detect stale connections:

```rust
use kaccy_db::pool::validate_pool_connections;

// In a background task
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Err(e) = validate_pool_connections(&pool).await {
            tracing::warn!("Pool validation failed: {}", e);
        }
    }
});
```

## Query Optimization

### 1. Use Query Streaming for Large Result Sets

**Before** (loads everything into memory):
```rust
let users: Vec<User> = sqlx::query_as("SELECT * FROM users")
    .fetch_all(&pool)
    .await?;
```

**After** (streams results):
```rust
use kaccy_db::query_streaming::{QueryStream, StreamConfig};

let config = StreamConfig {
    fetch_size: 1000,
    buffer_size: 100,
};

let stream = QueryStream::new(&pool, "SELECT * FROM users", config);
stream.for_each(|user: User| {
    // Process one user at a time
    println!("{:?}", user);
}).await?;
```

**Impact**: Reduces memory usage from O(n) to O(1)

### 2. Use Bulk Operations

**Before** (N queries):
```rust
for user in users {
    sqlx::query("INSERT INTO users (id, name) VALUES ($1, $2)")
        .bind(user.id)
        .bind(user.name)
        .execute(&pool)
        .await?;
}
```

**After** (1 query):
```rust
use kaccy_db::bulk_operations::BulkInserter;

let inserter = BulkInserter::new(&pool, "users", vec!["id", "name"]);
let rows: Vec<Vec<String>> = users.iter()
    .map(|u| vec![u.id.to_string(), u.name.clone()])
    .collect();

inserter.insert_batch(rows).await?;
```

**Impact**: Up to 100x faster for large batches

### 3. Use Cursor Pagination for Large Lists

**Before** (OFFSET pagination):
```rust
// Slow for large offsets
let users = sqlx::query_as("SELECT * FROM users LIMIT 100 OFFSET 10000")
    .fetch_all(&pool)
    .await?;
```

**After** (cursor pagination):
```rust
use kaccy_db::cursor_pagination::{CursorPagination, Direction};

let pagination = CursorPagination::new(&pool, "users", "id", 100);
let page = pagination.fetch(cursor, Direction::Forward).await?;

// Returns cursor for next page
println!("Next cursor: {}", page.page_info.end_cursor.unwrap());
```

**Impact**: Consistent O(1) performance regardless of offset

### 4. Optimize Joins

Analyze join performance:

```sql
-- Use EXPLAIN ANALYZE to understand query plan
EXPLAIN ANALYZE
SELECT u.*, t.*
FROM users u
JOIN tokens t ON t.issuer_id = u.id
WHERE u.reputation_score > 80;
```

Common optimizations:
- Add indexes on join columns
- Use INNER JOIN instead of LEFT JOIN when possible
- Limit result set early with WHERE clauses
- Consider denormalization for frequently joined data

### 5. Use Transactions Wisely

Group related operations in transactions:

```rust
use kaccy_db::transaction::TransactionExt;

let mut tx = pool.begin().await?;

// Multiple operations in one transaction
user_repo.create(&mut tx, user).await?;
balance_repo.update_balance(&mut tx, user_id, token_id, amount).await?;
audit_repo.log_action(&mut tx, "user_created", user_id).await?;

tx.commit().await?;
```

**But**: Don't hold transactions too long!
- Keep transaction duration < 100ms if possible
- Don't perform external API calls inside transactions
- Use savepoints for complex rollback logic

## Caching Strategy

### Cache Hierarchy

```
Request -> L1 (In-Memory) -> L2 (Redis) -> Database
            100μs              1-5ms         10-100ms
```

### What to Cache

**Good candidates**:
- User sessions (high read rate)
- Token prices (updated infrequently)
- Configuration data
- Computed aggregates

**Bad candidates**:
- Rapidly changing data
- Large objects (> 1MB)
- User-specific data with low reuse

### Cache Warming

Pre-load hot data on startup:

```rust
use kaccy_db::cache_warming::{CacheWarmer, WarmingStrategy};

let warmer = CacheWarmer::new(cache, data_source);
warmer.preload(vec!["popular_tokens", "top_users"]).await?;

// Or use predictive warming
warmer.start_predictive_warming(Duration::from_secs(300)).await;
```

### Cache Invalidation

Use tag-based invalidation for related data:

```rust
use kaccy_db::cache_invalidation::{InvalidationManager, TagRegistry};

let manager = InvalidationManager::new(cache);

// Tag data when caching
manager.set_with_tags("user:123", user_data, vec!["user", "user:123"]).await?;

// Invalidate all user data
manager.invalidate_by_tag("user").await?;
```

### Circuit Breaker for Cache

Protect your database when cache fails:

```rust
use kaccy_db::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

let config = CircuitBreakerConfig {
    failure_threshold: 5,
    success_threshold: 2,
    timeout: Duration::from_secs(60),
    half_open_max_calls: 3,
};

let circuit = CircuitBreaker::new(config);

// Automatically falls back to database on cache failure
let value = circuit.call(|| cache.get("key")).await?;
```

## Indexing

### Index Analysis

```rust
use kaccy_db::index_analyzer::{IndexAnalyzer, IndexPriority};

let analyzer = IndexAnalyzer::new();

// Find slow queries needing indexes
let suggestions = analyzer.analyze_slow_queries(&pool, 100.0).await?;

// Find unused indexes (waste of space and write performance)
let unused = analyzer.find_unused_indexes(&pool).await?;

// Find duplicate indexes
let duplicates = analyzer.find_duplicate_indexes(&pool).await?;
```

### Index Types

**B-tree** (default):
```sql
CREATE INDEX idx_users_email ON users(email);
```
Use for: equality and range queries

**Hash**:
```sql
CREATE INDEX idx_users_id_hash ON users USING HASH(id);
```
Use for: exact equality only (faster than B-tree)

**GIN** (Generalized Inverted Index):
```sql
CREATE INDEX idx_tokens_metadata ON tokens USING GIN(metadata);
```
Use for: JSONB, arrays, full-text search

**Partial**:
```sql
CREATE INDEX idx_active_tokens ON tokens(symbol) WHERE status = 'active';
```
Use for: queries with common WHERE clauses

### Composite Indexes

Order matters!

```sql
-- Good for: WHERE user_id = X AND created_at > Y
-- Good for: WHERE user_id = X
-- Bad for: WHERE created_at > Y (won't use index)
CREATE INDEX idx_orders_user_created ON orders(user_id, created_at);

-- Better for range queries on created_at:
CREATE INDEX idx_orders_created_user ON orders(created_at, user_id);
```

**Rule of thumb**: Most selective column first

### Index Maintenance

```sql
-- Rebuild bloated indexes
REINDEX INDEX idx_users_email;

-- Update statistics
ANALYZE users;

-- Find bloated indexes
SELECT schemaname, tablename, indexname,
       pg_size_pretty(pg_relation_size(indexrelid)) AS size
FROM pg_stat_user_indexes
ORDER BY pg_relation_size(indexrelid) DESC;
```

## Replication and Sharding

### Read Replicas

Configure read replicas for read-heavy workloads:

```rust
use kaccy_db::replica::{ReplicaPoolManager, LoadBalancingStrategy};

let mut manager = ReplicaPoolManager::new(primary_pool);

// Add multiple replicas
manager.add_replica("us-west-1a", replica1).await;
manager.add_replica("us-west-1b", replica2).await;
manager.add_replica("us-west-1c", replica3).await;

// Use least connections for even distribution
manager.set_strategy(LoadBalancingStrategy::LeastConnections);

// Monitor replication lag
let health = manager.check_replica_health("us-west-1a", 5000).await?;
println!("Lag: {} ms", health.lag_ms.unwrap_or(0));
```

### Multi-Region Setup

For global applications:

```rust
use kaccy_db::multi_region::{MultiRegionPoolManager, Region};

let mut manager = MultiRegionPoolManager::new();

manager.add_region(Region {
    name: "us-west".into(),
    latitude: 37.7749,
    longitude: -122.4194,
    pool: us_west_pool,
    is_primary: true,
}).await;

manager.add_region(Region {
    name: "eu-west".into(),
    latitude: 51.5074,
    longitude: -0.1278,
    pool: eu_west_pool,
    is_primary: false,
}).await;

// Routes to nearest region
let pool = manager.get_nearest_pool(user_lat, user_lon).await?;
```

### Sharding

For very large datasets:

```rust
use kaccy_db::sharding::{ShardPoolManager, ShardStrategy, ShardKey};

let mut manager = ShardPoolManager::new();

// Add shards
manager.add_shard(0, shard0_pool);
manager.add_shard(1, shard1_pool);
manager.add_shard(2, shard2_pool);
manager.add_shard(3, shard3_pool);

// Use hash-based sharding
manager.set_strategy(ShardStrategy::Hash { num_shards: 4 });

// Automatically routes to correct shard
let shard = manager.get_shard_for_key(&ShardKey::UserId(user_id))?;
```

**Sharding considerations**:
- Choose shard key carefully (can't easily change later)
- Avoid cross-shard queries when possible
- Plan for rebalancing as data grows

## Monitoring and Metrics

### Prometheus Integration

```rust
use kaccy_db::metrics::{record_query, record_cache_hit, record_pool_acquire};
use std::time::Instant;

// Record query metrics
let start = Instant::now();
let result = sqlx::query("SELECT ...").fetch_all(&pool).await;
record_query("users", "select", start.elapsed(), result.is_ok());

// Record cache metrics
let start = Instant::now();
if let Some(value) = cache.get("key").await? {
    record_cache_hit("user_cache", start.elapsed());
} else {
    record_cache_miss("user_cache", start.elapsed());
}
```

### Key Metrics to Monitor

**Pool Metrics**:
- `pool_connections_active`: Should be < 80% of max
- `pool_connections_idle`: Should be > 0
- `pool_acquire_wait_seconds`: Should be < 100ms

**Query Metrics**:
- `query_duration_seconds`: p50, p95, p99
- `slow_query_count`: Should trend downward
- `query_error_rate`: Should be < 1%

**Cache Metrics**:
- `cache_hit_rate`: Should be > 80% for hot data
- `cache_eviction_count`: High value = cache too small
- `cache_latency_seconds`: Should be < 5ms for Redis

### Slow Query Logging

```rust
use kaccy_db::query_logger::{QueryLogger, SlowQueryThreshold};
use std::time::Duration;

let logger = QueryLogger::new(SlowQueryThreshold::Custom(Duration::from_millis(100)));

// Logs queries slower than 100ms
let result = logger.log_query(
    &pool,
    "SELECT * FROM large_table",
    &[],
).await?;
```

### Performance Anomaly Detection

```rust
use kaccy_db::logging::{AnomalyDetector, PerformanceTracker};

let mut detector = AnomalyDetector::new(2.0); // 2 std devs
let mut tracker = PerformanceTracker::new();

// Track query performance
tracker.record_query("get_user", 150.0);

// Detect anomalies
if detector.is_anomaly("get_user", 500.0) {
    tracing::warn!("Anomalous query performance detected!");
}
```

## Hardware and Infrastructure

### Database Server

**CPU**:
- More cores for OLTP workloads
- Higher clock speed for OLAP workloads
- Recommended: 8+ cores for production

**Memory**:
- Rule of thumb: RAM = 25% of database size (minimum)
- Shared buffers = 25% of RAM
- Recommended: 16GB+ for production

**Storage**:
- Use SSD (NVMe preferred)
- Separate WAL and data directories
- RAID 10 for best balance of performance and redundancy

### PostgreSQL Configuration

Key settings to tune:

```ini
# Connection settings
max_connections = 200
shared_buffers = 4GB  # 25% of RAM

# WAL settings
wal_buffers = 16MB
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB

# Query planner
effective_cache_size = 12GB  # 75% of RAM
random_page_cost = 1.1  # Lower for SSD
effective_io_concurrency = 200  # Higher for SSD

# Autovacuum (critical!)
autovacuum = on
autovacuum_max_workers = 4
autovacuum_naptime = 10s
```

### Network

- Use connection pooler (PgBouncer) for 1000+ connections
- Enable connection compression for remote databases
- Use VPC peering for cloud databases
- Keep application and database in same region

## Benchmarking

### Using Criterion

```rust
// In benches/query_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kaccy_db::repositories::UserRepository;

fn bench_find_user(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(async {
        create_pool("postgres://localhost/bench").await.unwrap()
    });

    c.bench_function("find_user_by_id", |b| {
        b.iter(|| {
            rt.block_on(async {
                UserRepository::find_by_id(&pool, black_box(1)).await
            })
        })
    });
}

criterion_group!(benches, bench_find_user);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

### Load Testing

Use tools like:
- **k6**: Modern load testing
- **Apache JMeter**: Enterprise-grade
- **wrk**: Simple HTTP benchmarking

## Performance Checklist

Before going to production:

- [ ] Connection pool sized appropriately
- [ ] All frequently-used queries indexed
- [ ] Slow query logging enabled (> 100ms)
- [ ] Caching enabled for hot data
- [ ] Read replicas configured (if read-heavy)
- [ ] Monitoring and alerting set up
- [ ] Load testing completed
- [ ] Database configuration tuned for workload
- [ ] Backup and recovery tested
- [ ] Database on SSD storage
- [ ] Application and DB in same region
- [ ] Circuit breakers configured
- [ ] Statement caching enabled
- [ ] Bulk operations used where possible

## See Also

- [Troubleshooting Guide](TROUBLESHOOTING.md)
- [Migration Guide](MIGRATION_GUIDE.md)
- [PostgreSQL Performance Tuning](https://wiki.postgresql.org/wiki/Performance_Optimization)
