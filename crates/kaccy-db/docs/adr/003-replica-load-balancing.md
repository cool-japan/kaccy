# ADR-003: Replica Load Balancing Strategy

**Status**: Accepted
**Date**: 2024-02-15
**Deciders**: Core Team

## Context

As the platform grows, read traffic increases significantly. A single primary database becomes a bottleneck:
- High CPU usage on primary during peak hours
- Slow query response times
- Limited horizontal scaling capability

Read replicas help distribute read load, but we need to decide:
1. How to route queries (automatic vs manual)
2. Which load balancing algorithm to use
3. How to handle replica failures
4. How to manage replication lag

Requirements:
- Transparent routing (minimal application code changes)
- Even load distribution across replicas
- Automatic failover on replica failure
- Awareness of replication lag
- Support for read-after-write consistency when needed

## Decision

We will implement a **Smart Client** pattern with configurable load balancing strategies:

1. **Automatic Query Routing**
   - Reads → Replicas
   - Writes → Primary
   - Transactions → Primary

2. **Multiple Load Balancing Strategies**
   - Round Robin (default)
   - Least Connections
   - Random
   - First Available

3. **Health Monitoring**
   - Periodic health checks
   - Automatic removal of unhealthy replicas
   - Replication lag measurement

## Architecture

```
                     ┌─────────────┐
                     │ Application │
                     └──────┬──────┘
                            │
                            ▼
                   ┌────────────────┐
                   │ SmartDbClient  │
                   │ (Auto Router)  │
                   └────────┬───────┘
                            │
               ┌────────────┼────────────┐
               │            │            │
          (writes)     (reads)      (health)
               │            │            │
               ▼            ▼            ▼
         ┌─────────┐  ┌──────────┐  ┌──────────┐
         │ Primary │  │ Replica1 │  │ Replica2 │
         └─────────┘  └──────────┘  └──────────┘
```

## Load Balancing Strategies

### 1. Round Robin

Distributes requests evenly across all healthy replicas.

**Algorithm**:
```rust
let index = self.counter.fetch_add(1, Ordering::Relaxed);
let replica_index = index % replicas.len();
```

**Pros**:
- Simple and predictable
- Even distribution
- No state required

**Cons**:
- Doesn't account for replica load
- Doesn't consider connection pool state

**Use when**: All replicas have similar capacity

### 2. Least Connections

Routes to replica with fewest active connections.

**Algorithm**:
```rust
replicas.iter()
    .min_by_key(|r| r.pool.active_connections())
    .unwrap()
```

**Pros**:
- Balances actual load
- Adapts to slow queries
- Works well with varying query complexity

**Cons**:
- Requires pool metrics
- Slightly more overhead

**Use when**: Queries have varying execution times

### 3. Random

Randomly selects a replica.

**Algorithm**:
```rust
let index = rng.random_range(0..replicas.len());
replicas[index]
```

**Pros**:
- Simple, fast
- Good distribution over time
- No synchronization needed

**Cons**:
- Less predictable
- Not perfectly even

**Use when**: Simplicity is priority

### 4. First Available

Always tries first replica, falls back to next.

**Algorithm**:
```rust
for replica in replicas {
    if replica.is_healthy() {
        return replica;
    }
}
```

**Pros**:
- Predictable
- Keeps standby replicas idle

**Cons**:
- Uneven load
- First replica overloaded

**Use when**: You want hot standby replicas

## Implementation

```rust
pub struct ReplicaPoolManager {
    primary: PgPool,
    replicas: Vec<ReplicaPool>,
    strategy: LoadBalancingStrategy,
}

pub struct SmartDbClient {
    manager: Arc<ReplicaPoolManager>,
}

impl SmartDbClient {
    pub async fn query<T>(&self, sql: &str) -> Result<T> {
        // Detect query type
        let query_type = parse_query_type(sql);

        let pool = match query_type {
            QueryType::Read => self.manager.get_replica_pool().await?,
            QueryType::Write => self.manager.get_primary_pool(),
            QueryType::Transaction => self.manager.get_primary_pool(),
        };

        sqlx::query_as(sql).fetch_one(pool).await
    }
}
```

## Query Type Detection

```rust
enum QueryType {
    Read,   // SELECT
    Write,  // INSERT, UPDATE, DELETE
    Transaction,
}

fn parse_query_type(sql: &str) -> QueryType {
    let sql_upper = sql.trim_start().to_uppercase();

    if sql_upper.starts_with("SELECT") {
        QueryType::Read
    } else if sql_upper.starts_with("BEGIN") || sql_upper.starts_with("START") {
        QueryType::Transaction
    } else {
        QueryType::Write
    }
}
```

## Health Monitoring

```rust
pub struct ReplicaHealthStatus {
    pub is_healthy: bool,
    pub lag_ms: Option<i64>,
    pub last_check: Instant,
}

impl ReplicaPoolManager {
    pub async fn check_replica_health(
        &self,
        replica_id: &str,
        max_lag_ms: i64,
    ) -> Result<ReplicaHealthStatus> {
        let replica = self.get_replica(replica_id)?;

        // Check connection
        let is_connected = replica.pool.acquire().await.is_ok();

        // Measure replication lag
        let lag_ms = if is_connected {
            self.measure_replication_lag(replica).await.ok()
        } else {
            None
        };

        // Determine health
        let is_healthy = is_connected &&
            lag_ms.map_or(false, |lag| lag <= max_lag_ms);

        Ok(ReplicaHealthStatus {
            is_healthy,
            lag_ms,
            last_check: Instant::now(),
        })
    }
}
```

## Handling Replication Lag

### Problem

Read replicas may lag behind primary (typically < 1s, but can be higher).

### Solution: Lag-Aware Routing

```rust
pub async fn get_replica_pool(&self) -> Result<&PgPool> {
    // Filter out replicas with excessive lag
    let healthy_replicas: Vec<_> = self.replicas
        .iter()
        .filter(|r| r.lag_ms < self.config.max_acceptable_lag_ms)
        .collect();

    if healthy_replicas.is_empty() {
        // Fallback to primary if all replicas are lagging
        return Ok(&self.primary);
    }

    // Apply load balancing strategy
    self.select_replica(&healthy_replicas)
}
```

### Read-After-Write Consistency

For situations requiring immediate consistency:

```rust
impl SmartDbClient {
    // Force read from primary
    pub async fn query_primary<T>(&self, sql: &str) -> Result<T> {
        let pool = self.manager.get_primary_pool();
        sqlx::query_as(sql).fetch_one(pool).await
    }

    // Regular read (may use replica)
    pub async fn query<T>(&self, sql: &str) -> Result<T> {
        let pool = self.manager.get_replica_pool().await?;
        sqlx::query_as(sql).fetch_one(pool).await
    }
}

// Usage
// Write
user_repo.update_email(&client.manager.primary, user_id, email).await?;

// Read immediately after (use primary)
let user = client.query_primary("SELECT * FROM users WHERE id = $1").await?;

// Read later (can use replica)
let user = client.query("SELECT * FROM users WHERE id = $1").await?;
```

## Consequences

### Positive

1. **Scalability**
   - Horizontal scaling for read traffic
   - Can add more replicas as traffic grows
   - Reduced primary database load

2. **Performance**
   - Distributed query load
   - Lower latency (can use nearest replica)
   - Better resource utilization

3. **Resilience**
   - Automatic failover to healthy replicas
   - Graceful degradation (fallback to primary)
   - Health monitoring prevents using bad replicas

4. **Flexibility**
   - Multiple strategies for different workloads
   - Can switch strategies without code changes
   - Easy to add custom strategies

### Negative

1. **Complexity**
   - More infrastructure to manage
   - Health monitoring overhead
   - Query routing logic

2. **Eventual Consistency**
   - Replicas may lag (brief inconsistency)
   - Developers must be aware of this
   - Need explicit primary reads when needed

3. **Cost**
   - Additional database instances
   - More monitoring and ops overhead

4. **Testing Complexity**
   - Need to test with multiple replicas
   - Simulate replication lag in tests
   - More failure scenarios to cover

### Mitigation

1. **Documentation**: Clear guidelines on when to use primary vs replica
2. **Defaults**: Safe defaults (round robin, reasonable lag tolerance)
3. **Monitoring**: Extensive metrics for replica health and lag
4. **Fallback**: Always fallback to primary if replicas unavailable

## Configuration

```rust
pub struct ReplicaConfig {
    // Max acceptable replication lag (ms)
    pub max_lag_ms: i64,

    // Health check interval
    pub health_check_interval: Duration,

    // Load balancing strategy
    pub strategy: LoadBalancingStrategy,

    // Fallback to primary on replica failure
    pub fallback_to_primary: bool,
}

impl Default for ReplicaConfig {
    fn default() -> Self {
        Self {
            max_lag_ms: 5000,  // 5 seconds
            health_check_interval: Duration::from_secs(10),
            strategy: LoadBalancingStrategy::LeastConnections,
            fallback_to_primary: true,
        }
    }
}
```

## Monitoring Metrics

Key metrics to track:

- `replica_query_count`: Queries routed to each replica
- `replica_health_status`: Boolean per replica
- `replica_lag_ms`: Replication lag per replica
- `replica_failover_count`: Number of failovers
- `primary_fallback_count`: Reads routed to primary due to replica issues

## Alternatives Considered

### 1. External Load Balancer (HAProxy, PgBouncer)

**Pros**:
- Offloads routing logic
- Centralized management
- Battle-tested

**Cons**:
- Additional infrastructure
- Less flexible (can't change strategy at runtime)
- No application-level context

**Rejected**: Want more control and flexibility

### 2. Manual Routing

**Pros**:
- Explicit control
- No magic

**Cons**:
- Error-prone
- Lots of boilerplate
- Hard to maintain

**Rejected**: Too much burden on developers

### 3. Always Read from Primary

**Pros**:
- Simplest
- Strong consistency

**Cons**:
- Doesn't scale
- Primary becomes bottleneck

**Rejected**: Doesn't meet scalability requirements

## Migration Path

For existing code using single pool:

```rust
// Before
let pool = create_pool(db_url).await?;
let user = sqlx::query_as("SELECT * FROM users WHERE id = $1")
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

// After
let mut manager = ReplicaPoolManager::new(primary_pool);
manager.add_replica("replica1", replica_pool).await;

let client = manager.create_smart_client();
let user = client.query("SELECT * FROM users WHERE id = $1").await?;
```

Minimal code changes required!

## References

- [PostgreSQL Replication](https://www.postgresql.org/docs/current/high-availability.html)
- [Read Replica Best Practices](https://aws.amazon.com/blogs/database/best-practices-for-amazon-rds-postgresql-replication/)
- [Load Balancing Algorithms](https://www.nginx.com/blog/choosing-nginx-plus-load-balancing-techniques/)

## Related Decisions

- ADR-001: Use SQLx (provides pool abstraction we build on)
- ADR-004: Sharding Strategy (complementary scaling approach)
- ADR-005: Observability (monitoring replica health)
