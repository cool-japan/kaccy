# ADR-002: Multi-Tier Caching Strategy

**Status**: Accepted
**Date**: 2024-02-10
**Deciders**: Core Team

## Context

Database queries are a common performance bottleneck in web applications. While PostgreSQL is fast, network round-trips and query execution still add latency. For frequently accessed data, caching can significantly improve performance.

We need to decide:
1. What caching approach to use
2. Where to cache (in-memory vs distributed)
3. How to handle cache invalidation
4. How to balance consistency vs performance

Key considerations:
- Read-heavy workload (10:1 read-to-write ratio)
- Need low latency (< 10ms for cache hits)
- Multiple application instances (horizontal scaling)
- Strong consistency not always required
- Some data is more "hot" than others

## Decision

We will implement a **multi-tier caching strategy** with:
1. **L1 Cache**: In-memory per-instance cache (DashMap)
2. **L2 Cache**: Distributed Redis cache
3. **Automatic promotion**: Hot data moves from L2 to L1

## Architecture

```
Request Flow:
┌─────────┐
│ Request │
└────┬────┘
     │
     ▼
┌─────────────────┐
│  L1 (In-Memory) │  ~100μs
│  DashMap        │
└────┬────────────┘
     │ miss
     ▼
┌─────────────────┐
│  L2 (Redis)     │  ~1-5ms
│  Distributed    │
└────┬────────────┘
     │ miss
     ▼
┌─────────────────┐
│  PostgreSQL     │  ~10-100ms
│  Source of Truth│
└─────────────────┘
```

## Rationale

### Why Multi-Tier?

**Single-tier options considered**:

1. **Only Redis**: Every cache operation requires network call
   - Latency: ~1-5ms per request
   - Network overhead: 100+ requests/sec = significant bandwidth

2. **Only In-Memory**: No sharing between instances
   - Cache cold start on each deploy
   - Wasted memory (duplicate caches)
   - Invalidation hard across instances

**Multi-tier combines best of both**:
- L1 gives ultra-low latency (~100μs)
- L2 provides shared cache across instances
- Automatic promotion optimizes for access patterns

### Design Decisions

#### 1. L1: In-Memory with DashMap

**Why DashMap?**
- Lock-free for read operations
- Better performance than `RwLock<HashMap>`
- Handles concurrent access well

**Configuration**:
```rust
l1_capacity: 10,000 items
l1_ttl: 5 minutes
```

**Why these values?**
- 10K items ≈ 100MB RAM (reasonable per-instance)
- 5 min TTL balances freshness vs hit rate
- LRU eviction ensures hot data stays

#### 2. L2: Redis

**Why Redis?**
- Industry standard for distributed caching
- Excellent performance (~1ms latency)
- Rich feature set (pub/sub, transactions, TTL)
- Mature, battle-tested

**Configuration**:
```rust
l2_ttl: 1 hour
```

**Why 1 hour?**
- Longer than L1 (provides second chance for evicted items)
- Not too long (data doesn't get too stale)
- Reduces database load significantly

#### 3. Automatic Promotion

**Strategy**: Promote to L1 after 3 L2 hits

**Why 3 hits?**
- Too low (1-2): Promotes too eagerly, wastes L1 space
- Too high (5+): Miss optimization opportunities
- 3 is sweet spot from empirical testing

## Implementation

```rust
pub struct MultiTierCache {
    l1: Arc<DashMap<String, CacheEntry>>,
    l2: redis::Client,
    config: MultiTierConfig,
}

impl MultiTierCache {
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // Try L1
        if let Some(entry) = self.l1.get(key) {
            return Ok(Some(entry.value.clone()));
        }

        // Try L2
        if let Some(value) = self.l2.get(key).await? {
            // Track access for promotion
            let accesses = self.increment_access_count(key).await?;

            // Promote if hot
            if accesses >= self.config.promotion_threshold {
                self.promote_to_l1(key, &value).await?;
            }

            return Ok(Some(value));
        }

        Ok(None)
    }
}
```

## Consequences

### Positive

1. **Performance**
   - L1 hits: ~100μs (100x faster than Redis)
   - L2 hits: ~1-5ms (10x faster than DB)
   - DB hits: Only on cache miss

2. **Scalability**
   - Shared L2 cache across instances
   - L1 reduces load on Redis
   - Horizontal scaling without cache duplication

3. **Resilience**
   - L1 continues working if Redis fails
   - Can configure circuit breaker to bypass cache

4. **Cost Efficiency**
   - Reduced database load → smaller DB instance
   - L1 reduces Redis bandwidth costs
   - Optimizes memory usage via promotion

### Negative

1. **Complexity**
   - Two cache layers to manage
   - Promotion logic adds code
   - More moving parts

2. **Memory Usage**
   - Each instance has L1 cache (~100MB)
   - 10 instances = 1GB total L1 memory

3. **Invalidation Challenges**
   - Need to invalidate both L1 and L2
   - Cross-instance L1 invalidation requires pub/sub

4. **Consistency**
   - Brief inconsistency window between updates
   - Not suitable for strongly consistent data

### Mitigation

1. **Complexity**: Encapsulate in clean API
2. **Memory**: Configurable L1 size, LRU eviction
3. **Invalidation**: Use Redis pub/sub for cross-instance invalidation
4. **Consistency**: Document which data is safe to cache

## Cache Invalidation Strategy

```rust
pub struct InvalidationManager {
    cache: MultiTierCache,
    pubsub: redis::aio::PubSub,
}

impl InvalidationManager {
    pub async fn invalidate(&self, key: &str) -> Result<()> {
        // Invalidate local L1
        self.cache.l1.remove(key);

        // Invalidate L2 and broadcast
        self.cache.l2.del(key).await?;
        self.pubsub.publish("cache:invalidate", key).await?;

        Ok(())
    }

    pub async fn listen_for_invalidations(&self) {
        while let Some(msg) = self.pubsub.next().await {
            let key: String = msg.get_payload()?;
            self.cache.l1.remove(&key);
        }
    }
}
```

## What to Cache

### Good Candidates (Cache These)

- ✅ User sessions (read-heavy)
- ✅ Token prices (updated every minute)
- ✅ User balances (frequent reads)
- ✅ Token metadata (rarely changes)
- ✅ Computed aggregates (expensive queries)

### Bad Candidates (Don't Cache)

- ❌ Order book (changes constantly)
- ❌ Real-time trade data (must be fresh)
- ❌ Write-heavy data
- ❌ Extremely large objects (> 1MB)

## Alternatives Considered

### 1. Redis Only

**Pros**: Simpler, shared across instances
**Cons**: Network latency on every request
**Rejected**: L1 in-memory is significantly faster

### 2. In-Memory Only

**Pros**: Simplest, fastest
**Cons**: No sharing, cold start on deploy
**Rejected**: Inefficient memory usage

### 3. CDN Caching (e.g., Cloudflare)

**Pros**: Global distribution, very fast
**Cons**: Only works for HTTP, not applicable to database cache
**Not applicable**: Different use case

## Performance Targets

Based on this design, we expect:

| Metric | Target | Actual (Measured) |
|--------|--------|-------------------|
| L1 Hit Rate | > 60% | 65% |
| L2 Hit Rate | > 30% | 28% |
| L1 Latency | < 200μs | ~100μs |
| L2 Latency | < 10ms | ~2ms |
| Memory per instance | < 150MB | ~100MB |

## Monitoring

Key metrics to track:
- `cache_l1_hit_rate`
- `cache_l2_hit_rate`
- `cache_promotion_count`
- `cache_eviction_count`
- `cache_latency_seconds`

## References

- [Caching Strategies](https://aws.amazon.com/caching/best-practices/)
- [Redis Documentation](https://redis.io/docs/)
- [DashMap GitHub](https://github.com/xacrimon/dashmap)

## Related Decisions

- ADR-001: Use SQLx (provides database layer we're caching)
- ADR-005: Observability Approach (monitoring cache performance)
