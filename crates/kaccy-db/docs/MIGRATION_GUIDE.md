# Migration Guide

This guide helps you migrate between major versions of `kaccy-db`.

## Table of Contents

- [General Migration Strategy](#general-migration-strategy)
- [Version-Specific Migrations](#version-specific-migrations)
- [Breaking Changes](#breaking-changes)
- [Database Schema Updates](#database-schema-updates)

## General Migration Strategy

### 1. Prepare for Migration

Before upgrading:

1. **Backup your database**
   ```bash
   # Using BackupManager
   use kaccy_db::backup::{BackupManager, BackupConfig, BackupStrategy};

   let config = BackupConfig {
       backup_dir: "/path/to/backups".into(),
       strategy: BackupStrategy::Full,
       retention_days: 30,
       compress: true,
   };

   let manager = BackupManager::new(config);
   manager.create_backup(&pool, "pre-upgrade-backup").await?;
   ```

2. **Test in a staging environment first**
   - Never upgrade production directly
   - Verify all features work as expected
   - Run full test suite

3. **Review changelog and breaking changes**
   - Read release notes carefully
   - Identify deprecated APIs you're using
   - Plan code changes needed

### 2. Update Dependencies

Update your `Cargo.toml`:

```toml
[dependencies]
kaccy-db = "0.2.0"  # Update to target version
```

### 3. Run Database Migrations

```bash
# Using sqlx-cli
sqlx migrate run

# Or programmatically
use sqlx::migrate::Migrator;

let migrator = Migrator::new(std::path::Path::new("./migrations")).await?;
migrator.run(&pool).await?;
```

### 4. Update Code

Follow version-specific migration guides below.

### 5. Test Thoroughly

- Run all unit tests: `cargo test`
- Run integration tests with real database
- Verify performance with load testing
- Check monitoring and metrics

## Version-Specific Migrations

### Migrating to 0.2.0 (Future Release)

**Target Date**: TBD

**Major Changes**:
- Enhanced security module (encryption at rest)
- Improved caching with multi-tier support
- Better observability with OpenTelemetry

**Breaking Changes**:
- None expected (backward compatible)

**Action Items**:
1. Enable new security features (optional)
2. Configure multi-tier caching if needed
3. Set up OpenTelemetry collector for distributed tracing

**Code Changes**:

```rust
// Before (0.1.0)
use kaccy_db::cache::RedisCache;

let cache = RedisCache::new("redis://localhost").await?;

// After (0.2.0) - Multi-tier caching
use kaccy_db::multi_tier_cache::{MultiTierCache, MultiTierConfig};

let config = MultiTierConfig {
    l1_capacity: 1000,
    l1_ttl_seconds: 300,
    l2_ttl_seconds: 3600,
    promotion_threshold: 3,
};

let cache = MultiTierCache::new(config, redis_client).await?;
```

### Migrating from 0.1.0 to 0.1.1

**Release Date**: Current stable

**Changes**:
- Bug fixes and performance improvements
- No breaking changes

**Action Items**:
- Update dependency version
- No code changes required

## Breaking Changes

### API Changes

This section documents all breaking API changes across versions.

#### Connection Pool Configuration

**Version**: 0.2.0 (planned)

**Before**:
```rust
let pool = create_pool("postgres://localhost/db").await?;
```

**After**:
```rust
use kaccy_db::pool::{PoolConfig, WarmupStrategy};

let config = PoolConfig {
    max_connections: 20,
    min_connections: 5,
    warmup_strategy: WarmupStrategy::MinConnections,
    ..Default::default()
};

let pool = create_pool_with_config("postgres://localhost/db", config).await?;
```

**Migration Path**:
- Old API still works but is deprecated
- Use builder pattern for new code
- Will be removed in 0.3.0

### Deprecated Features

#### Cache API

**Deprecated in**: 0.2.0
**Removed in**: 0.3.0

The old `set_with_expiry` method is deprecated in favor of `set_with_ttl`:

```rust
// Deprecated
cache.set_with_expiry("key", "value", 3600).await?;

// Use instead
cache.set_with_ttl("key", "value", Duration::from_secs(3600)).await?;
```

## Database Schema Updates

### Running Migrations

#### Automatic Migration

```rust
use kaccy_db::pool::create_pool;
use sqlx::migrate::Migrator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = create_pool("postgres://user:pass@localhost/db").await?;

    // Run migrations automatically on startup
    let migrator = Migrator::new(std::path::Path::new("./migrations")).await?;
    migrator.run(&pool).await?;

    Ok(())
}
```

#### Manual Migration

```bash
# Install sqlx-cli if not already installed
cargo install sqlx-cli --no-default-features --features postgres

# Run pending migrations
DATABASE_URL=postgres://user:pass@localhost/db sqlx migrate run

# Rollback last migration (if needed)
DATABASE_URL=postgres://user:pass@localhost/db sqlx migrate revert
```

### Migration Files

All migrations are located in `migrations/` directory:

```
migrations/
├── 001_initial_schema.sql
├── 002_add_indexes.sql
├── 003_reputation_system.sql
├── 004_commitments.sql
└── 005_audit_logs.sql
```

### Adding Custom Migrations

If you need custom schema changes:

```bash
# Create new migration
sqlx migrate add my_custom_change

# This creates: migrations/TIMESTAMP_my_custom_change.sql
```

### Schema Verification

After migration, verify database integrity:

```rust
use kaccy_db::backup::verify_database_integrity;

let issues = verify_database_integrity(&pool).await?;
if !issues.is_empty() {
    eprintln!("Database integrity issues found: {:?}", issues);
}
```

## Rollback Strategy

If you need to rollback after upgrade:

### 1. Stop Application

```bash
# Stop your application servers
systemctl stop your-app
```

### 2. Restore Database Backup

```rust
use kaccy_db::backup::{BackupManager, RestoreOptions};

let manager = BackupManager::new(config);
let options = RestoreOptions {
    drop_existing: true,
    verify_after_restore: true,
};

manager.restore_backup(&pool, "pre-upgrade-backup", options).await?;
```

### 3. Downgrade Dependencies

```toml
[dependencies]
kaccy-db = "0.1.0"  # Previous version
```

### 4. Restart Application

```bash
cargo build --release
systemctl start your-app
```

## Performance Considerations

### After Migration Checklist

1. **Analyze Query Performance**
   ```rust
   use kaccy_db::index_analyzer::IndexAnalyzer;

   let analyzer = IndexAnalyzer::new();
   let suggestions = analyzer.analyze_slow_queries(&pool, 100.0).await?;

   for suggestion in suggestions {
       println!("{}", suggestion.to_sql());
   }
   ```

2. **Update Statistics**
   ```sql
   ANALYZE;  -- Update table statistics
   VACUUM ANALYZE;  -- Reclaim space and update statistics
   ```

3. **Verify Index Usage**
   ```rust
   let unused = analyzer.find_unused_indexes(&pool).await?;
   println!("Unused indexes: {:?}", unused);
   ```

4. **Monitor Replication Lag** (if using replicas)
   ```rust
   use kaccy_db::multi_region::MultiRegionPoolManager;

   let lag = manager.measure_replication_lag("us-west", "us-east").await?;
   println!("Replication lag: {:?}", lag);
   ```

## Troubleshooting

### Common Migration Issues

#### Issue: Migration Fails with Constraint Violation

**Solution**:
```sql
-- Temporarily disable constraints
SET session_replication_role = replica;

-- Run migration
\i migrations/XXX_migration.sql

-- Re-enable constraints
SET session_replication_role = DEFAULT;
```

#### Issue: Pool Connection Timeout After Upgrade

**Solution**:
```rust
// Increase timeout in pool config
let config = PoolConfig {
    acquire_timeout: Duration::from_secs(30),
    ..Default::default()
};
```

#### Issue: Cache Keys Invalid After Upgrade

**Solution**:
```rust
// Clear all caches
cache.flush_all().await?;
```

## Getting Help

If you encounter issues during migration:

1. Check the [Troubleshooting Guide](TROUBLESHOOTING.md)
2. Review [GitHub Issues](https://github.com/cool-japan/kaccy/issues)
3. Join our community Discord
4. Contact support team

## See Also

- [Performance Tuning Guide](PERFORMANCE_TUNING.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
- [API Documentation](https://docs.rs/kaccy-db)
