# kaccy-db

Comprehensive database layer for Kaccy Protocol with enterprise-grade features.

## Overview

This crate provides a complete PostgreSQL database solution with:

- Connection pooling with retry logic and health monitoring
- Repository pattern for all domain entities
- Redis caching layer with rate limiting and distributed locks
- Read replica support with automatic query routing and load balancing
- Database sharding with consistent hashing
- Multi-region replication with automatic failover
- TimescaleDB integration for time-series analytics
- Query performance monitoring and index analysis
- Transaction management with savepoints
- Comprehensive audit logging
- Backup/recovery automation with PITR support

## Core Modules

### Connection Management

- **`pool`** - Connection pool with retry logic and health checks
- **`replica`** - Read replica management with load balancing strategies
- **`multi_region`** - Geographic replication with proximity-based routing
- **`sharding`** - Database sharding with consistent hashing

### Repositories

All repositories provide type-safe, async database operations:

- **`UserRepository`** - User accounts, profiles, KYC, reputation
- **`TokenRepository`** - Personal tokens with bonding curves
- **`BalanceRepository`** - Token balances with locking support
- **`OrderRepository`** - Buy/sell orders with BTC integration
- **`TradeRepository`** - Trade execution and analytics
- **`ReputationEventRepository`** - Reputation scoring and history
- **`CommitmentRepository`** - User commitments with deadlines
- **`AuditRepository`** - Compliance reporting and audit logs

### Performance & Analytics

- **`cache`** - Redis caching with rate limiting and distributed locks
- **`query_logger`** - Query performance monitoring and slow query detection
- **`index_analyzer`** - Index optimization recommendations
- **`analytics`** - TimescaleDB integration with materialized views for dashboards

### Operations

- **`backup`** - Automated backups with pg_dump/restore and PITR
- **`transaction`** - Transaction management with isolation levels and savepoints

## Quick Start

```rust
use kaccy_db::{create_pool_with_retry, RetryConfig, UserRepository};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;

    // Create pool with automatic retry
    let retry_config = RetryConfig::default();
    let pool = create_pool_with_retry(&database_url, retry_config).await?;

    // Use repositories
    let user_repo = UserRepository::new(pool.clone());
    let user = user_repo.find_by_email("user@example.com").await?;

    Ok(())
}
```

## Advanced Features

### Redis Caching

```rust
use kaccy_db::{RedisCache, CacheConfig};

let cache_config = CacheConfig::default();
let cache = RedisCache::connect("redis://localhost", cache_config).await?;

// Cache user sessions
cache.set_session("user123", &session_data, 3600).await?;

// Rate limiting
let allowed = cache.check_rate_limit("api:user123", 100, 60).await?;
```

### Read Replicas

```rust
use kaccy_db::{ReplicaPoolManager, LoadBalanceStrategy, SmartDbClientBuilder};

let manager = ReplicaPoolManager::new(
    primary_pool,
    vec![replica1, replica2],
    LoadBalanceStrategy::LeastConnections,
).await?;

let client = SmartDbClientBuilder::new(manager).build();
// Automatically routes SELECTs to replicas, writes to primary
```

### Database Sharding

```rust
use kaccy_db::{ShardPoolManager, ShardingStrategy, ShardKey};

let shard_manager = ShardPoolManager::new(
    vec![shard1_pool, shard2_pool, shard3_pool],
    ShardingStrategy::Hash,
).await?;

// Route queries to appropriate shard
let user = shard_manager.execute_on_shard(
    &ShardKey::UserId(user_id),
    |pool| async move {
        UserRepository::new(pool).find_by_id(user_id).await
    }
).await?;
```

### Analytics & Time-Series

```rust
use kaccy_db::AnalyticsService;

let analytics = AnalyticsService::new(pool);

// Get dashboard metrics from materialized views
let metrics = analytics.get_dashboard_metrics().await?;

// Query time-series data (requires TimescaleDB)
let prices = analytics.get_price_history(token_id, start_time, end_time).await?;
```

## Configuration

Environment variables:

- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_URL` - Redis connection string (optional)

## Architecture

```
kaccy-db/
├── src/
│   ├── lib.rs                 # Public API
│   ├── pool.rs                # Connection pooling
│   ├── cache.rs               # Redis caching
│   ├── replica.rs             # Read replicas
│   ├── sharding.rs            # Database sharding
│   ├── multi_region.rs        # Geographic replication
│   ├── analytics.rs           # TimescaleDB & materialized views
│   ├── query_logger.rs        # Performance monitoring
│   ├── index_analyzer.rs      # Index optimization
│   ├── backup.rs              # Backup/recovery
│   ├── transaction.rs         # Transaction management
│   ├── error.rs               # Error types
│   └── repositories/
│       ├── user.rs
│       ├── token.rs
│       ├── balance.rs
│       ├── order.rs
│       ├── trade.rs
│       ├── reputation_event.rs
│       ├── commitment.rs
│       └── audit.rs
├── migrations/                # SQL migrations
└── Cargo.toml
```

## Dependencies

- `sqlx` - Async SQL with compile-time checking
- `redis` - Async Redis client
- `tokio` - Async runtime
- `serde` - Serialization
- `chrono` - Date/time handling
- `uuid` - UUID support
- `rust_decimal` - Decimal arithmetic

## Database Schema

See `migrations/` directory for the current schema. Key tables:

- `users` - User accounts with DID, KYC status, reputation
- `tokens` - Personal tokens with bonding curve parameters
- `balances` - User token balances
- `orders` - Buy/sell orders with BTC payment info
- `trades` - Executed trade records
- `reputation_events` - Reputation score changes
- `output_commitments` - User commitments with deadlines

## Testing

```bash
# Run with test database
DATABASE_URL=postgresql://test@localhost/kaccy_test cargo test -p kaccy-db

# Run migrations
sqlx migrate run
```
