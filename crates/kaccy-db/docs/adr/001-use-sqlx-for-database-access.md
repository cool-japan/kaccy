# ADR-001: Use SQLx for Database Access

**Status**: Accepted
**Date**: 2024-01-15
**Deciders**: Core Team

## Context

The `kaccy-db` crate needs a robust, type-safe, and performant way to interact with PostgreSQL databases. Several Rust database libraries are available, each with different trade-offs:

1. **SQLx** - Async, compile-time checked SQL
2. **Diesel** - Sync-first, strong type safety with DSL
3. **Tokio-Postgres** - Low-level async driver
4. **SeaORM** - Async ORM with entity-based approach

Key requirements:
- Async/await support (critical for high concurrency)
- Type safety at compile time
- Raw SQL capability (for complex queries)
- Connection pooling
- PostgreSQL-specific features (JSONB, arrays, etc.)
- Active maintenance and community

## Decision

We will use **SQLx** as the primary database access library for `kaccy-db`.

## Rationale

### Why SQLx?

1. **Compile-time SQL verification**
   - The `query!` macro validates SQL against actual database schema
   - Catches SQL errors before runtime
   - Automatic Rust type inference from queries

2. **Async-first design**
   - Built on tokio, perfect for high-concurrency workloads
   - Non-blocking I/O for all operations
   - Excellent connection pool implementation

3. **Flexibility**
   - Supports both raw SQL and query builder patterns
   - Not opinionated about ORM vs raw SQL
   - Easy to write complex queries when needed

4. **PostgreSQL support**
   - Excellent support for PostgreSQL-specific types
   - JSONB, arrays, custom types all work seamlessly
   - Supports all PostgreSQL features we need

5. **Active maintenance**
   - Regular updates and bug fixes
   - Large community
   - Well-documented

### Why not others?

**Diesel**:
- Primarily sync (async support is secondary)
- DSL can be verbose for complex queries
- Learning curve for the query builder DSL

**Tokio-Postgres**:
- Too low-level (would need to build abstractions ourselves)
- No compile-time query checking
- More boilerplate code

**SeaORM**:
- Heavy ORM approach (we want flexibility)
- Additional layer of abstraction
- Performance overhead
- Less mature than SQLx

## Consequences

### Positive

1. **Type Safety**
   - Compile-time SQL verification catches bugs early
   - Refactoring is safer (schema changes break at compile time)

2. **Performance**
   - Async I/O enables high concurrency
   - Efficient connection pooling out of the box
   - No ORM overhead

3. **Developer Experience**
   - Write SQL directly (familiar to database developers)
   - Great error messages from compiler
   - Less magic, more explicit

4. **Testing**
   - Easy to test with real database
   - Offline mode for CI/CD without database

### Negative

1. **Compile-time overhead**
   - Need database running during development (or offline mode)
   - Longer compile times due to macro expansion

2. **No ORM abstractions**
   - Need to write more SQL by hand
   - No automatic migrations from schema changes

3. **Learning curve**
   - Developers need to learn SQLx macros
   - Need to understand async/await patterns

### Mitigation Strategies

1. **Use offline mode for CI**
   ```bash
   cargo sqlx prepare
   ```
   Generates `sqlx-data.json` for compilation without database

2. **Create repository abstractions**
   - Build our own repository layer on top of SQLx
   - Provides common CRUD patterns
   - Reduces boilerplate

3. **Documentation and examples**
   - Provide clear examples for common patterns
   - Document best practices
   - Include migration guides

## Alternatives Considered

### 1. Diesel

**Pros**:
- Strong type safety
- Compile-time query checking
- Large community

**Cons**:
- Sync-first (async support is experimental)
- Query builder DSL has learning curve
- Less flexible for raw SQL

**Rejected because**: We need async-first for high concurrency

### 2. SeaORM

**Pros**:
- High-level abstractions
- Active Record / Entity pattern
- Async support

**Cons**:
- ORM overhead
- Less control over SQL
- Additional abstraction layer

**Rejected because**: We want flexibility to optimize queries, not be constrained by ORM

### 3. Tokio-Postgres + Manual Abstractions

**Pros**:
- Full control
- No magic
- Lightweight

**Cons**:
- Need to build everything ourselves
- No compile-time SQL checking
- Significant development effort

**Rejected because**: Would take too long to build equivalent features

## Implementation Notes

### Setup

```toml
[dependencies]
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio", "tls-native-tls"] }
```

### Usage Pattern

```rust
// Compile-time checked
let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id)
    .fetch_one(&pool)
    .await?;

// Runtime queries (when schema is dynamic)
let result = sqlx::query("SELECT * FROM users WHERE email = $1")
    .bind(email)
    .fetch_one(&pool)
    .await?;
```

### Migration Management

Use sqlx-cli for migrations:
```bash
sqlx migrate add create_users_table
sqlx migrate run
```

## References

- [SQLx Documentation](https://docs.rs/sqlx)
- [SQLx GitHub](https://github.com/launchbadge/sqlx)
- [Async Rust Book](https://rust-lang.github.io/async-book/)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)

## Superseded By

None

## Related Decisions

- ADR-002: Multi-Tier Caching Strategy (builds on SQLx foundation)
- ADR-003: Replica Load Balancing (uses SQLx pools)
