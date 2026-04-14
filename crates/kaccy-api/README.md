# kaccy-api

[![Status](https://img.shields.io/badge/status-alpha-orange.svg)]()
[![Crates.io](https://img.shields.io/crates/v/kaccy-api.svg)](https://crates.io/crates/kaccy-api)
[![Documentation](https://docs.rs/kaccy-api/badge.svg)](https://docs.rs/kaccy-api)
[![License](https://img.shields.io/crates/l/kaccy-api.svg)](LICENSE)

Production REST + GraphQL + WebSocket API layer for the Kaccy Protocol, built on Axum with a full middleware stack covering authentication, observability, real-time streaming, background jobs, and multi-channel notifications.

## Features

### Core API
- 27 route modules spanning Phases 0–3 (auth through admin)
- RESTful endpoints with versioning middleware
- GraphQL endpoint via `async-graphql` 7.2
- Batch request processing

### Auth & Security
- JWT bearer authentication with refresh tokens
- Argon2 password hashing
- TOTP two-factor authentication
- OAuth2 provider integration
- API key management with per-key analytics
- Session management
- KYC (Know Your Customer) workflow
- IP filtering, request signing, and request validation middleware
- Security headers middleware (HSTS, CSP, etc.)
- Circuit breaker middleware

### Real-time
- WebSocket (`ws`) endpoint for live data feeds
- Server-Sent Events (`streaming`) for unidirectional push
- Event bus with publisher/subscriber pattern and typed handlers

### Observability
- OpenTelemetry tracing via `telemetry` middleware
- Prometheus metrics via `metrics` middleware
- Structured request/response logging
- Request-ID propagation
- Profiling middleware
- Audit log trail for sensitive operations

### Tooling
- SDK generator for TypeScript, Python, Rust, and Go clients
- Background job queue with scheduler and worker
- Behavior, metrics, and performance analytics subsystem
- Multi-channel notifications: email, SMS, push, in-app
- OpenAPI/Swagger docs endpoint

## Architecture

| Subsystem | Location | Description |
|-----------|----------|-------------|
| Routes | `src/routes/` | 27 handler modules (REST + GraphQL + WS) |
| Middleware | `src/middleware/` | 17 tower middleware layers |
| Jobs | `src/jobs/` | Background queue, scheduler, worker |
| Analytics | `src/analytics/` | Behavior, metrics, performance, tracker |
| Events | `src/events/` | Event bus: publisher, subscriber, handlers |
| Notifications | `src/notifications/` | Email, SMS, push, in-app channels |
| SDK Generator | `src/sdk_generator/` | TS, Python, Rust, Go client generation |

## Routes

### Auth (`/api/auth`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/register` | Register new account |
| POST | `/api/auth/login` | Login, receive JWT + refresh token |
| POST | `/api/auth/logout` | Invalidate session |
| POST | `/api/auth/refresh` | Exchange refresh token for new JWT |
| POST | `/api/auth/totp/enable` | Enrol TOTP device |
| POST | `/api/auth/totp/verify` | Verify TOTP code |
| GET/POST | `/api/auth/oauth2/...` | OAuth2 provider flow |

### Users (`/api/users`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/users/me` | Get own profile |
| PUT | `/api/users/me` | Update own profile |
| DELETE | `/api/users/me` | Delete account |
| GET | `/api/users/:id` | Get user by ID |
| POST | `/api/users/kyc` | Submit KYC documents |
| GET | `/api/users/kyc/status` | Get KYC verification status |

### Tokens (`/api/tokens`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/tokens` | List tokens (filterable, paginated) |
| GET | `/api/tokens/:id` | Get token details |
| POST | `/api/tokens` | Create token (protected) |

### Trading (`/api`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/orders` | List orders |
| POST | `/api/orders` | Place order |
| GET | `/api/orders/:id` | Get order |
| DELETE | `/api/orders/:id` | Cancel order |
| GET | `/api/trades` | List trades |
| GET | `/api/trades/:id` | Get trade |
| GET | `/api/pool_stats` | Liquidity pool statistics |
| GET | `/api/commitments` | List commitments |
| POST | `/api/commitments` | Create commitment |

### Real-time

| Method | Path | Description |
|--------|------|-------------|
| GET | `/ws` | WebSocket connection (live feed) |
| GET | `/api/streaming` | Server-Sent Events stream |

### Admin (`/api/admin`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST/... | `/api/admin/...` | Platform administration |
| GET/PUT | `/api/admin/rate_limit` | Rate-limit policy management |
| GET/PUT | `/api/admin/system_config` | System configuration |
| GET | `/api/audit` | Audit log query |
| POST | `/api/batch` | Bulk operation batch endpoint |

### API Keys (`/api/api_keys`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/api_keys` | List API keys |
| POST | `/api/api_keys` | Create API key |
| DELETE | `/api/api_keys/:id` | Revoke API key |
| GET | `/api/api_keys/:id/analytics` | Per-key usage analytics |

### Sessions (`/api/sessions`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sessions` | List active sessions |
| DELETE | `/api/sessions/:id` | Terminate session |

### Notifications (`/api`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/notifications` | List notifications |
| PUT | `/api/notifications/:id/read` | Mark as read |
| GET | `/api/webhooks` | List webhooks |
| POST | `/api/webhooks` | Register webhook |
| DELETE | `/api/webhooks/:id` | Remove webhook |

### Developer (`/api`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/docs` | OpenAPI / Swagger UI |
| GET/POST | `/graphql` | GraphQL endpoint (async-graphql 7.2) |
| GET | `/metrics` | Prometheus metrics scrape |
| GET | `/health` | Liveness / readiness probe |

## Middleware Stack

The following Tower middleware layers are composed in order:

| Layer | Description |
|-------|-------------|
| `request_id` | Assigns a unique ID to every request |
| `logging` | Structured request/response logging |
| `security_headers` | Adds HSTS, CSP, X-Frame-Options, etc. |
| `ip_filter` | Allow/block lists based on client IP |
| `rate_limit` | Token-bucket rate limiting per key/IP |
| `circuit_breaker` | Opens on upstream error threshold breach |
| `auth` (JWT) | Validates Bearer token, injects claims |
| `api_key` | Validates API-key header alternative |
| `request_signing` | Verifies HMAC request signatures |
| `request_validation` | Schema-validates incoming request bodies |
| `versioning` | API version negotiation and routing |
| `cache` | Response caching layer |
| `audit` | Records sensitive operations to audit log |
| `metrics` | Increments Prometheus counters/histograms |
| `telemetry` | OpenTelemetry span propagation |
| `profiling` | Optional CPU/memory profiling hooks |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
kaccy-api = "0.2.0"
```

## Quick Start

```rust,ignore
use kaccy_api::state::AppState;
use kaccy_db::create_pool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build database pool
    let pool = create_pool(&std::env::var("DATABASE_URL")?).await?;

    // Construct shared application state
    let state = AppState::builder()
        .pool(pool)
        .jwt_secret(std::env::var("JWT_SECRET")?)
        .redis_url(std::env::var("REDIS_URL")?)
        .build()?;

    // Create the full router (all 27 route modules + middleware stack)
    let app = kaccy_api::router::create(state).await?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

## Configuration

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string |
| `REDIS_URL` | Yes | Redis connection string (sessions, cache, rate-limit) |
| `JWT_SECRET` | Yes | HMAC secret for JWT signing (64+ chars) |
| `HOST` | No | Bind address (default: `0.0.0.0`) |
| `PORT` | No | Listen port (default: `8080`) |
| `RUST_LOG` | No | Log filter, e.g. `kaccy_api=debug,tower_http=info` |
| `SMTP_HOST` | No | SMTP host for email notifications |
| `SMTP_PORT` | No | SMTP port (default: `587`) |
| `SMTP_USERNAME` | No | SMTP authentication user |
| `SMTP_PASSWORD` | No | SMTP authentication password |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | OpenTelemetry collector endpoint |

## Testing

The crate has **201 passing tests**. 18 integration tests require a live PostgreSQL instance and a running server; they are automatically skipped in CI via `#[ignore]`.

```bash
# Run all unit tests (no external services needed)
cargo nextest run -p kaccy-api

# Run integration tests (requires DATABASE_URL and a running server)
cargo nextest run -p kaccy-api -- --include-ignored
```

---

Part of the [COOLJAPAN](https://github.com/cool-japan) ecosystem.
