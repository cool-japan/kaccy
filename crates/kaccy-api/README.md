# kaccy-api

REST API layer for Kaccy Protocol.

## Overview

This crate provides the HTTP API endpoints using the Axum web framework. It exposes all platform functionality through a RESTful interface with JWT-based authentication.

## Modules

### `routes`

API route handlers:

- **`auth`** - Authentication endpoints (register, login)
- **`users`** - User profile management
- **`tokens`** - Token CRUD and trading

### `middleware`

HTTP middleware:

- **`auth`** - JWT authentication and authorization
- (Planned) Rate limiting, CORS, logging

### `state`

Application state management:

- **`AppState`** - Shared state with database pool, JWT secret, Bitcoin client

### `error`

API error handling:

- **`ApiError`** - Unified error responses with HTTP status codes

## API Endpoints

### Authentication

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/register` | Register new user |
| POST | `/api/auth/login` | Login and get JWT token |

### Users (Protected)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/users/me` | Get current user profile |
| PUT | `/api/users/me` | Update profile |
| GET | `/api/users/:id` | Get user by ID |

### Tokens

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/tokens` | List tokens (with filters) |
| GET | `/api/tokens/:id` | Get token details |
| POST | `/api/tokens` | Create new token (protected) |
| POST | `/api/tokens/:id/buy` | Create buy order (protected) |
| POST | `/api/tokens/:id/sell` | Create sell order (protected) |

### Health

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |

## Usage

```rust
use kaccy_api::state::AppState;
use kaccy_db::create_pool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = create_pool(&std::env::var("DATABASE_URL")?).await?;

    let state = AppState::new(
        pool,
        std::env::var("JWT_SECRET")?,
    );

    // Build and run the server
    let app = kaccy_api::routes::create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

## Configuration

Environment variables:

- `DATABASE_URL` - PostgreSQL connection string
- `JWT_SECRET` - Secret key for JWT signing (64+ characters)
- `HOST` - Server host (default: `0.0.0.0`)
- `PORT` - Server port (default: `8080`)
- `RUST_LOG` - Log level (e.g., `debug`, `info`)

## Architecture

```
kaccy-api/
├── src/
│   ├── lib.rs          # Crate root
│   ├── main.rs         # Server entrypoint
│   ├── state.rs        # Application state
│   ├── error.rs        # Error handling
│   ├── routes/
│   │   ├── mod.rs      # Router setup
│   │   ├── auth.rs     # Auth endpoints
│   │   ├── users.rs    # User endpoints
│   │   └── tokens.rs   # Token endpoints
│   └── middleware/
│       ├── mod.rs
│       └── auth.rs     # JWT middleware
└── Cargo.toml
```

## Dependencies

- `axum` - Web framework
- `tower` - Middleware and service utilities
- `tower-http` - HTTP-specific middleware (CORS, tracing)
- `jsonwebtoken` - JWT encoding/decoding
- `argon2` - Password hashing

## Testing

```bash
# Run unit tests
cargo test -p kaccy-api

# Integration test with curl
curl http://localhost:8080/health
```
