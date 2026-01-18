# Troubleshooting Guide

This guide helps you diagnose and fix common issues with `kaccy-db`.

## Table of Contents

- [Connection Issues](#connection-issues)
- [Query Performance Problems](#query-performance-problems)
- [Cache Issues](#cache-issues)
- [Replication and Failover](#replication-and-failover)
- [Transaction Errors](#transaction-errors)
- [Migration Problems](#migration-problems)
- [Memory Issues](#memory-issues)
- [Debugging Tools](#debugging-tools)

## Connection Issues

### Problem: "Connection pool timeout"

**Symptoms**:
```
Error: DbError::ConnectionTimeout
Pool acquire timeout after 10s
```

**Causes**:
1. Pool exhausted (all connections busy)
2. Long-running queries blocking connections
3. Connection leaks
4. Pool sized too small

**Solutions**:

1. **Check pool metrics**:
```rust
use kaccy_db::pool::get_pool_metrics;

let metrics = get_pool_metrics(&pool);
println!("Active: {}, Idle: {}, Max: {}",
    metrics.active_connections,
    metrics.idle_connections,
    metrics.max_connections
);
```

2. **Increase pool size** (if active ≈ max):
```rust
let config = PoolConfig {
    max_connections: 50,  // Increase from 20
    acquire_timeout: Duration::from_secs(15),  // Give more time
    ..Default::default()
};
```

3. **Find long-running queries**:
```sql
-- PostgreSQL
SELECT pid, now() - query_start AS duration, query
FROM pg_stat_activity
WHERE state = 'active'
ORDER BY duration DESC;
```

4. **Enable connection validation**:
```rust
use kaccy_db::pool::validate_pool_connections;

// Run periodically
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        if let Err(e) = validate_pool_connections(&pool).await {
            tracing::error!("Pool validation failed: {}", e);
        }
    }
});
```

### Problem: "Connection refused" or "Connection reset"

**Symptoms**:
```
Error: IO error: Connection refused (os error 111)
Error: Connection reset by peer
```

**Causes**:
1. Database server not running
2. Incorrect connection string
3. Network firewall blocking
4. PostgreSQL not listening on network interface

**Solutions**:

1. **Verify database is running**:
```bash
# PostgreSQL
systemctl status postgresql
# or
pg_isready -h localhost -p 5432
```

2. **Check connection string**:
```rust
// Correct format
let url = "postgres://user:password@localhost:5432/database";

// Common mistakes:
// - Wrong port (default is 5432)
// - Missing database name
// - Incorrect credentials
// - Wrong host (use 'localhost' or IP, not 'postgres' unless in Docker)
```

3. **Verify PostgreSQL config**:
```ini
# In postgresql.conf
listen_addresses = '*'  # Or specific IP

# In pg_hba.conf
host    all             all             0.0.0.0/0            md5
```

4. **Test connection manually**:
```bash
psql -h localhost -U user -d database
```

### Problem: "Too many connections"

**Symptoms**:
```
Error: FATAL: remaining connection slots are reserved
Error: FATAL: too many connections for role "user"
```

**Causes**:
1. Multiple application instances using too many connections
2. Connection leaks
3. Database max_connections too low

**Solutions**:

1. **Check current connections**:
```sql
SELECT count(*) FROM pg_stat_activity;
SELECT max_connections FROM pg_settings WHERE name = 'max_connections';
```

2. **Use connection pooler** (PgBouncer):
```ini
# pgbouncer.ini
[databases]
mydb = host=localhost port=5432 dbname=mydb

[pgbouncer]
listen_addr = 127.0.0.1
listen_port = 6432
pool_mode = transaction
max_client_conn = 1000
default_pool_size = 20
```

3. **Reduce application pool sizes**:
```rust
// If running 10 app instances with max_connections=50 each,
// that's 500 connections! Reduce per-instance:
let config = PoolConfig {
    max_connections: 10,  // 10 instances × 10 = 100 total
    ..Default::default()
};
```

4. **Increase database limit** (requires restart):
```sql
ALTER SYSTEM SET max_connections = 200;
-- Then restart PostgreSQL
```

### Problem: "SSL connection required"

**Symptoms**:
```
Error: SSL connection required
```

**Solution**:
```rust
// Add sslmode to connection string
let url = "postgres://user:pass@localhost/db?sslmode=require";

// Or disable SSL (development only!)
let url = "postgres://user:pass@localhost/db?sslmode=disable";
```

## Query Performance Problems

### Problem: Queries suddenly slow

**Symptoms**:
- Queries that were fast (< 100ms) now take seconds
- Timeouts on simple queries

**Diagnostic Steps**:

1. **Check current activity**:
```sql
SELECT * FROM pg_stat_activity WHERE state = 'active';
```

2. **Look for blocking queries**:
```sql
SELECT blocked_locks.pid AS blocked_pid,
       blocking_locks.pid AS blocking_pid,
       blocked_activity.query AS blocked_statement,
       blocking_activity.query AS blocking_statement
FROM pg_locks blocked_locks
JOIN pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid
JOIN pg_locks blocking_locks ON blocking_locks.locktype = blocked_locks.locktype
JOIN pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_locks.pid
WHERE NOT blocked_locks.granted;
```

3. **Check for table bloat**:
```sql
SELECT schemaname, tablename,
       pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size,
       n_dead_tup, n_live_tup,
       ROUND(n_dead_tup * 100.0 / NULLIF(n_live_tup + n_dead_tup, 0), 2) AS dead_pct
FROM pg_stat_user_tables
ORDER BY n_dead_tup DESC;
```

**Solutions**:

1. **Run VACUUM**:
```sql
VACUUM ANALYZE users;  -- Specific table
VACUUM ANALYZE;        -- All tables
```

2. **Kill blocking queries** (if appropriate):
```sql
SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE pid = 12345;
```

3. **Update table statistics**:
```sql
ANALYZE users;
```

4. **Rebuild indexes**:
```sql
REINDEX TABLE users;
```

### Problem: Query missing index

**Symptoms**:
```
Slow query performance
EXPLAIN shows Seq Scan on large table
```

**Diagnostic**:

```sql
EXPLAIN ANALYZE
SELECT * FROM users WHERE email = 'user@example.com';

-- Look for:
-- "Seq Scan" (bad for large tables)
-- "Index Scan" (good)
-- "Bitmap Heap Scan" (okay)
```

**Solution**:

```rust
use kaccy_db::index_analyzer::IndexAnalyzer;

let analyzer = IndexAnalyzer::new();

// Analyze slow queries
let suggestions = analyzer.analyze_slow_queries(&pool, 100.0).await?;

for suggestion in suggestions {
    println!("{}", suggestion.to_sql());
    // CREATE INDEX idx_users_email ON users(email);
}
```

### Problem: Query planner choosing wrong index

**Symptoms**:
- EXPLAIN shows index scan but still slow
- Planner using wrong index

**Solution**:

1. **Update statistics**:
```sql
ANALYZE users;
```

2. **Force index usage** (temporary workaround):
```sql
-- Disable seq scan to force index
SET enable_seqscan = OFF;
SELECT * FROM users WHERE email = 'user@example.com';
SET enable_seqscan = ON;
```

3. **Adjust planner settings**:
```sql
-- If using SSD
SET random_page_cost = 1.1;  -- Default is 4.0
```

## Cache Issues

### Problem: Low cache hit rate

**Symptoms**:
```
Cache hit rate < 50%
High database load
```

**Diagnostic**:

```rust
use kaccy_db::metrics::get_cache_stats;

let stats = get_cache_stats("user_cache");
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
println!("Hits: {}, Misses: {}", stats.hits, stats.misses);
```

**Solutions**:

1. **Increase cache size**:
```rust
let config = MultiTierConfig {
    l1_capacity: 10000,  // Increase from 1000
    l1_ttl_seconds: 600,  // Increase TTL
    ..Default::default()
};
```

2. **Warm cache on startup**:
```rust
use kaccy_db::cache_warming::{CacheWarmer, WarmingStrategy};

let warmer = CacheWarmer::new(cache, data_source);
warmer.preload(vec!["hot_users", "popular_tokens"]).await?;
```

3. **Check cache eviction**:
```rust
if stats.evictions > stats.hits {
    // Cache is too small!
    eprintln!("Cache evictions ({}) > hits ({})", stats.evictions, stats.hits);
}
```

### Problem: Cache contains stale data

**Symptoms**:
- Displaying old data to users
- Inconsistent state between app instances

**Solutions**:

1. **Enable cache invalidation**:
```rust
use kaccy_db::cache_invalidation::InvalidationManager;

let manager = InvalidationManager::new(cache);

// After updating data
user_repo.update_profile(&pool, user_id, profile).await?;
manager.invalidate_by_tag(&format!("user:{}", user_id)).await?;
```

2. **Reduce TTL**:
```rust
// If data changes frequently
cache.set_with_ttl("key", value, Duration::from_secs(60)).await?;
```

3. **Use write-through caching**:
```rust
// Update both cache and database
cache.set("user:123", user_data).await?;
user_repo.update(&pool, user).await?;
```

### Problem: Redis connection failures

**Symptoms**:
```
Error: Redis connection refused
Cache operations failing
```

**Solutions**:

1. **Use circuit breaker**:
```rust
use kaccy_db::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};

let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());

// Automatically falls back to database
let result = breaker.call(|| cache.get("key")).await
    .unwrap_or_else(|_| {
        // Fallback to database
        user_repo.find_by_id(&pool, user_id).await
    });
```

2. **Configure Redis connection pooling**:
```rust
let client = redis::Client::open("redis://localhost")?
    .get_multiplexed_tokio_connection()
    .await?;
```

3. **Enable reconnection**:
```rust
// Redis client automatically reconnects
// Just handle errors gracefully
match cache.get("key").await {
    Ok(value) => value,
    Err(e) => {
        tracing::warn!("Cache error: {}, falling back to DB", e);
        fetch_from_database().await?
    }
}
```

## Replication and Failover

### Problem: High replication lag

**Symptoms**:
```
Reading stale data from replicas
Replication lag > 1 second
```

**Diagnostic**:

```rust
use kaccy_db::multi_region::MultiRegionPoolManager;

let lag = manager.measure_replication_lag("primary", "replica").await?;
println!("Replication lag: {} ms", lag);
```

```sql
-- On PostgreSQL replica
SELECT now() - pg_last_xact_replay_timestamp() AS replication_lag;
```

**Solutions**:

1. **Route reads to primary** (temporarily):
```rust
let pool = manager.get_primary_pool().await?;
```

2. **Check network latency**:
```bash
ping replica-host
traceroute replica-host
```

3. **Optimize WAL shipping**:
```ini
# On primary postgresql.conf
wal_sender_timeout = 60s
max_wal_senders = 10

# On replica
hot_standby_feedback = on
```

4. **Reduce write load on primary**:
- Batch writes
- Use bulk operations
- Optimize indexes (faster writes)

### Problem: Replica failover not working

**Symptoms**:
- Replica marked unhealthy but still receiving traffic
- Manual failover required

**Solution**:

```rust
use kaccy_db::replica_failover::{FailoverManager, FailoverStrategy};

let mut manager = FailoverManager::new(FailoverStrategy::Automatic {
    health_check_interval: Duration::from_secs(5),
    failure_threshold: 3,
});

// Manually trigger failover
manager.failover("replica-1").await?;

// Check cluster status
let status = manager.get_cluster_status().await?;
for node in status.nodes {
    println!("{}: {:?}", node.id, node.status);
}
```

## Transaction Errors

### Problem: "Deadlock detected"

**Symptoms**:
```
Error: deadlock detected
Detail: Process 123 waits for ShareLock on transaction 456
```

**Cause**:
Two transactions waiting for locks held by each other.

**Solution**:

1. **Always acquire locks in same order**:
```rust
// Bad - can deadlock
let mut tx1 = pool.begin().await?;
lock_user(&mut tx1, user_a).await?;
lock_token(&mut tx1, token_x).await?;

let mut tx2 = pool.begin().await?;
lock_token(&mut tx2, token_x).await?;  // Waits for tx1
lock_user(&mut tx2, user_a).await?;    // Waits for tx2 -> deadlock!

// Good - consistent ordering
fn acquire_locks(tx: &mut Transaction, user_id: i64, token_id: i64) {
    // Always lock in same order: user first, then token
    lock_user(tx, user_id).await?;
    lock_token(tx, token_id).await?;
}
```

2. **Use shorter transactions**:
```rust
// Keep transaction duration < 100ms
let start = Instant::now();
let mut tx = pool.begin().await?;

// ... do work ...

if start.elapsed() > Duration::from_millis(100) {
    tracing::warn!("Long transaction: {:?}", start.elapsed());
}

tx.commit().await?;
```

3. **Retry on deadlock**:
```rust
use kaccy_db::retry_policy::{RetryPolicy, RetryStrategy};

let policy = RetryPolicy::new(RetryStrategy::Exponential {
    initial_delay: Duration::from_millis(100),
    max_delay: Duration::from_secs(5),
    max_retries: 3,
});

policy.execute(|| async {
    let mut tx = pool.begin().await?;
    // ... transaction logic ...
    tx.commit().await
}).await?;
```

### Problem: "Serialization failure"

**Symptoms**:
```
Error: could not serialize access due to concurrent update
```

**Solution**:

1. **Use appropriate isolation level**:
```rust
// Default is Read Committed (usually fine)
let mut tx = pool.begin().await?;

// For stricter consistency
sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
    .execute(&mut *tx)
    .await?;
```

2. **Retry on serialization failure**:
```rust
// Similar to deadlock retry
policy.execute(|| async {
    // ... transaction ...
}).await?;
```

### Problem: "Transaction has already been committed"

**Symptoms**:
```
Error: transaction has already been committed or rolled back
```

**Cause**:
Trying to use transaction after commit/rollback.

**Solution**:

```rust
// Bad
let mut tx = pool.begin().await?;
tx.commit().await?;
user_repo.create(&mut tx, user).await?;  // Error!

// Good
let mut tx = pool.begin().await?;
user_repo.create(&mut tx, user).await?;
tx.commit().await?;  // Commit at end
```

## Migration Problems

### Problem: Migration fails partway through

**Symptoms**:
```
Error running migration 004_commitments.sql
Migration state is inconsistent
```

**Solution**:

1. **Check migration status**:
```bash
sqlx migrate info
```

2. **Rollback failed migration**:
```bash
# If migration is partially applied
psql -d mydb -c "DELETE FROM _sqlx_migrations WHERE version = 4;"

# Manually undo changes
psql -d mydb -f rollback_004.sql
```

3. **Fix migration and retry**:
```bash
# Edit migration file
vim migrations/004_commitments.sql

# Run again
sqlx migrate run
```

4. **Use transactions in migrations**:
```sql
-- migrations/004_commitments.sql
BEGIN;

CREATE TABLE commitments (...);
CREATE INDEX idx_commitments_user ON commitments(user_id);

COMMIT;
```

### Problem: "Column already exists"

**Symptoms**:
```
Error: column "email" of relation "users" already exists
```

**Solution**:

```sql
-- Use IF NOT EXISTS
ALTER TABLE users ADD COLUMN IF NOT EXISTS email VARCHAR(255);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
```

## Memory Issues

### Problem: Out of memory errors

**Symptoms**:
```
Error: Out of memory
Killed (signal 9)
```

**Causes**:
1. Loading too much data at once
2. Connection pool too large
3. Cache too large
4. Memory leaks

**Solutions**:

1. **Use query streaming**:
```rust
use kaccy_db::query_streaming::QueryStream;

// Instead of fetch_all()
let stream = QueryStream::new(&pool, "SELECT * FROM large_table", config);
stream.for_each(|row| {
    // Process incrementally
}).await?;
```

2. **Reduce pool size**:
```rust
let config = PoolConfig {
    max_connections: 20,  // Reduce from 100
    ..Default::default()
};
```

3. **Reduce cache size**:
```rust
let config = MultiTierConfig {
    l1_capacity: 1000,  // Reduce from 10000
    ..Default::default()
};
```

4. **Enable pagination**:
```rust
use kaccy_db::cursor_pagination::CursorPagination;

let pagination = CursorPagination::new(&pool, "users", "id", 100);
// Fetch 100 at a time instead of all
```

## Debugging Tools

### Enable debug logging

```rust
use tracing_subscriber;

// Initialize tracing
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

// Now all kaccy-db operations will log
```

### Query logging

```rust
use kaccy_db::query_logger::QueryLogger;

let logger = QueryLogger::new(SlowQueryThreshold::Always);

// Logs all queries
logger.log_query(&pool, "SELECT * FROM users WHERE id = $1", &[1]).await?;
```

### Pool metrics

```rust
use kaccy_db::pool::get_pool_metrics;

let metrics = get_pool_metrics(&pool);
println!("Pool state: {:?}", metrics);
```

### Database statistics

```rust
use kaccy_db::backup::get_database_stats;

let stats = get_database_stats(&pool).await?;
println!("Database size: {}", stats.total_size);
println!("Connections: {}", stats.connections);
```

### Profiling queries

```sql
-- PostgreSQL query profiling
EXPLAIN (ANALYZE, BUFFERS)
SELECT * FROM users WHERE email = 'user@example.com';

-- Shows:
-- - Execution plan
-- - Actual runtime
-- - Rows returned
-- - Buffer cache hits/misses
```

## Getting Help

If you're still stuck:

1. **Check logs**: Enable debug logging and review
2. **Review metrics**: Use Prometheus/Grafana dashboards
3. **Check PostgreSQL logs**: Often contains useful errors
4. **Use EXPLAIN ANALYZE**: Understand query execution
5. **Search GitHub issues**: Someone may have hit same problem
6. **Ask community**: Discord, forums, etc.

## Common Error Messages

| Error | Likely Cause | Solution |
|-------|--------------|----------|
| `Connection timeout` | Pool exhausted | Increase pool size or reduce query time |
| `Connection refused` | DB not running | Start database |
| `Too many connections` | Pool too large | Use connection pooler |
| `Deadlock detected` | Lock ordering issue | Acquire locks in consistent order |
| `Serialization failure` | Concurrent updates | Retry transaction |
| `Slow query` | Missing index | Run index analyzer |
| `Out of memory` | Loading too much data | Use streaming/pagination |
| `Replication lag` | Network or load issue | Check network, reduce writes |

## See Also

- [Performance Tuning Guide](PERFORMANCE_TUNING.md)
- [Migration Guide](MIGRATION_GUIDE.md)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
