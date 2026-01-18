# Configuration Templates

This directory contains configuration templates for various `kaccy-db` deployment scenarios.

## Available Templates

### 1. `pool_config.toml`
Basic database connection pool configuration.

**Use for**: All deployments (required)

**Key settings**:
- Connection pool size
- Timeouts and health checks
- Retry policies
- Logging and metrics

### 2. `cache_config.toml`
Multi-tier caching configuration with Redis.

**Use for**: Production deployments with high read traffic

**Key settings**:
- L1 (in-memory) and L2 (Redis) cache
- Cache promotion and eviction
- Invalidation strategies
- Circuit breaker

### 3. `replica_config.toml`
Read replica configuration for scaling read workloads.

**Use for**: Read-heavy applications, multi-region deployments

**Key settings**:
- Primary and replica databases
- Load balancing strategies
- Health checks and failover
- Geographic routing

### 4. `sharding_config.toml`
Database sharding for horizontal scalability.

**Use for**: Very large datasets (> 1TB), high write throughput

**Key settings**:
- Sharding strategy (hash, range)
- Shard definitions
- Cross-shard queries
- Rebalancing

## Usage

### 1. Copy Template

```bash
cp docs/templates/pool_config.toml config/database.toml
```

### 2. Customize for Your Environment

Edit the configuration file with your specific values:

```toml
[database]
url = "postgres://your_user:your_password@your_host:5432/your_database"

[pool]
max_connections = 50
min_connections = 10
```

### 3. Load Configuration in Your Application

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
    min_connections: u32,
    // ... other fields
}

// Load from TOML file
let config: DatabaseConfig = toml::from_str(&std::fs::read_to_string("config/database.toml")?)?;

// Or load from environment variables
let url = std::env::var("DATABASE_URL")?;
```

## Environment-Specific Configurations

### Development

```toml
[database]
url = "postgres://postgres:password@localhost:5432/kaccy_dev"

[pool]
max_connections = 10
min_connections = 2

[logging]
level = "debug"
slow_query_threshold_ms = 50

[cache]
enabled = false  # Use database directly for easier debugging
```

### Staging

```toml
[database]
url = "postgres://user:password@staging-db.example.com:5432/kaccy_staging"

[pool]
max_connections = 20
min_connections = 5

[logging]
level = "info"
slow_query_threshold_ms = 100

[cache]
enabled = true
strategy = "redis_only"
```

### Production

```toml
[database]
url = "postgres://user:password@prod-db.example.com:5432/kaccy_production"

[pool]
max_connections = 50
min_connections = 10
warmup_strategy = "min_connections"

[logging]
level = "warn"
slow_query_threshold_ms = 100
log_parameters = false  # Security: don't log query params in prod

[cache]
enabled = true
strategy = "multi_tier"

[metrics]
enabled = true
```

## Combining Configurations

For complex deployments, you can combine multiple configurations:

```bash
# Directory structure
config/
├── database.toml      # Basic pool config
├── cache.toml         # Caching config
├── replicas.toml      # Read replica config
└── sharding.toml      # Sharding config (if needed)
```

Load in code:

```rust
let pool_config = load_config("config/database.toml")?;
let cache_config = load_config("config/cache.toml")?;
let replica_config = load_config("config/replicas.toml")?;
```

## Configuration Validation

Before deploying, validate your configuration:

```bash
# Check TOML syntax
toml-cli check config/database.toml

# Test database connection
cargo run --example validate_config
```

## Best Practices

1. **Never commit secrets**: Use environment variables for passwords
   ```toml
   url = "${DATABASE_URL}"  # Substitute from env var
   ```

2. **Start conservative**: Begin with lower pool sizes, increase based on metrics

3. **Monitor and adjust**: Use metrics to tune configuration over time

4. **Document changes**: Keep a changelog of configuration updates

5. **Version control**: Track configuration files (without secrets)

6. **Test thoroughly**: Validate configuration in staging before production

## See Also

- [Performance Tuning Guide](../PERFORMANCE_TUNING.md)
- [Troubleshooting Guide](../TROUBLESHOOTING.md)
- [Migration Guide](../MIGRATION_GUIDE.md)
