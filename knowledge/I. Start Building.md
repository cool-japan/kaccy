# I: Start Building

# Kaccy Protocol - Start Building: Week 1 Kickoff

---

## 🎯 Week 1 Sprint Goal

**Goal:** Users can register and login (authentication system completed)

**Deliverables:**

- ✅ Backend API (Rust/Axum): `/api/auth/register`, `/api/auth/login`, `/api/users/me`
- ✅ Frontend (Next.js): Registration page, Login page, Dashboard (protected route)
- ✅ Database: PostgreSQL with users table
- ✅ Friday Demo: New user registration → Login → Authenticated dashboard display

---

## 📅 Day 1: Backend Foundation (6-8 hours)

### **Morning Session (3-4 hours): Project Setup**

### Step 1.1: VPS & Infrastructure Setup

```bash
Copy# ============================================
# LOCAL MACHINE (Your laptop)
# ============================================

# 1. SSH key generation (if not already have)
ssh-keygen -t ed25519 -C "kaccy-vps-key" -f ~/.ssh/kaccy_vps
# Press Enter for no passphrase (or set one)

# 2. Display public key (copy this)
cat ~/.ssh/kaccy_vps.pub
# Copy the output

# ============================================
# VPS PROVIDER (Hetzner example)
# ============================================

# Go to: https://console.hetzner.cloud/
# 1. Create Project: "kaccy-production"
# 2. Add SSH Key: Paste public key from above
# 3. Create Server:
#    - Location: Germany (or closest to you)
#    - Image: Ubuntu 22.04
#    - Type: CPX31 (4 vCPU, 8GB RAM) - €13/month
#    - Volume: None (use local storage)
#    - Network: Default
#    - Firewall: Create new "kaccy-firewall"
#      * Inbound: 22 (SSH), 80 (HTTP), 443 (HTTPS)
#      * Outbound: All
# 4. Click "Create & Buy"
# 5. Note the IP address: e.g., 95.217.XXX.XXX

# ============================================
# CONNECT TO VPS
# ============================================

# Replace with your VPS IP
export VPS_IP=95.217.XXX.XXX

# SSH into VPS
ssh -i ~/.ssh/kaccy_vps root@$VPS_IP

# You should now be in the VPS terminal
# Prompt will look like: root@ubuntu-kaccy:~#

```

### Step 1.2: VPS Initial Configuration

```bash
Copy# ============================================
# ON VPS (all commands below are on VPS)
# ============================================

# Update system
apt update && apt upgrade -y

# Install essential tools
apt install -y \
    build-essential \
    curl \
    git \
    vim \
    htop \
    ufw \
    fail2ban

# Set timezone
timedatectl set-timezone Europe/Tallinn  # Or your timezone

# Configure firewall
ufw default deny incoming
ufw default allow outgoing
ufw allow 22/tcp    # SSH
ufw allow 80/tcp    # HTTP
ufw allow 443/tcp   # HTTPS
ufw --force enable

# Start fail2ban (brute-force protection)
systemctl enable fail2ban
systemctl start fail2ban

# Create non-root user
adduser kaccy
# Set password: <strong-password>
# Press Enter for other prompts (defaults are fine)

# Add to sudo group
usermod -aG sudo kaccy

# Copy SSH key to new user
mkdir -p /home/kaccy/.ssh
cp /root/.ssh/authorized_keys /home/kaccy/.ssh/
chown -R kaccy:kaccy /home/kaccy/.ssh
chmod 700 /home/kaccy/.ssh
chmod 600 /home/kaccy/.ssh/authorized_keys

# Test new user (open new terminal on local machine)
# ssh -i ~/.ssh/kaccy_vps kaccy@$VPS_IP
# If successful, continue. Otherwise debug SSH config.

# Disable root SSH login (security)
vim /etc/ssh/sshd_config
# Find line: PermitRootLogin yes
# Change to: PermitRootLogin no
# Save and exit (:wq)

systemctl restart sshd

# From now on, always login as kaccy user
# Exit root session
exit

# Re-login as kaccy
ssh -i ~/.ssh/kaccy_vps kaccy@$VPS_IP

```

### Step 1.3: Install PostgreSQL

```bash
Copy# ============================================
# ON VPS (as kaccy user)
# ============================================

# Install PostgreSQL 15
sudo apt install -y postgresql postgresql-contrib

# Start and enable
sudo systemctl start postgresql
sudo systemctl enable postgresql

# Check status
sudo systemctl status postgresql
# Should show: active (running)

# Switch to postgres user
sudo -i -u postgres

# Create database and user
psql

```

```sql
Copy-- Inside PostgreSQL prompt (postgres=#)

-- Create user
CREATE USER kaccy_admin WITH PASSWORD 'CHANGE_THIS_STRONG_PASSWORD';

-- Create database
CREATE DATABASE kaccy_db OWNER kaccy_admin;

-- Grant privileges
GRANT ALL PRIVILEGES ON DATABASE kaccy_db TO kaccy_admin;

-- Exit psql
\q

```

```bash
Copy# Exit postgres user
exit

# Test connection
psql -U kaccy_admin -d kaccy_db -h localhost
# Enter password when prompted
# You should see: kaccy_db=>

# Exit psql
\q

# Configure PostgreSQL to allow password authentication
sudo vim /etc/postgresql/15/main/pg_hba.conf

# Find line:
# local   all             all                                     peer
# Change to:
# local   all             all                                     md5

# Restart PostgreSQL
sudo systemctl restart postgresql

```

### Step 1.4: Install Rust

```bash
Copy# ============================================
# ON VPS (as kaccy user)
# ============================================

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Choose: 1) Proceed with installation (default)

# Load Rust environment
source "$HOME/.cargo/env"

# Verify installation
rustc --version
# Should output: rustc 1.75.0 (or newer)

cargo --version
# Should output: cargo 1.75.0 (or newer)

# Install SQLx CLI (for database migrations)
cargo install sqlx-cli --no-default-features --features postgres

```

---

### **Afternoon Session (3-4 hours): Backend Project Setup**

### Step 2.1: Create Backend Project

```bash
Copy# ============================================
# ON VPS (or LOCAL - I recommend starting locally for development)
# ============================================

# Create project directory
mkdir -p ~/kaccy
cd ~/kaccy

# Initialize backend project
cargo new kaccy-backend --name kaccy_backend
cd kaccy-backend

# Project structure
# kaccy-backend/
# ├── Cargo.toml
# ├── src/
# │   └── main.rs
# └── ...

```

### Step 2.2: Configure Dependencies

```toml
Copy# File: Cargo.toml

[package]name = "kaccy_backend"
version = "0.1.0"
edition = "2021"

[dependencies]# Web framework
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "migrate"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Authentication
jsonwebtoken = "9"
argon2 = "0.5"

# Utilities
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
dotenv = "0.15"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
thiserror = "1.0"

[profile.release]opt-level = 3
lto = true
codegen-units = 1

```

### Step 2.3: Project Structure

```bash
Copy# Create folder structure
mkdir -p src/{routes,models,db,middleware,utils}

# Create files
touch src/routes/mod.rs
touch src/routes/auth.rs
touch src/routes/users.rs
touch src/models/mod.rs
touch src/models/user.rs
touch src/db/mod.rs
touch src/db/pool.rs
touch src/middleware/mod.rs
touch src/middleware/auth.rs
touch src/utils/mod.rs
touch src/utils/error.rs
touch src/utils/jwt.rs

# Create .env file
touch .env

# Final structure:
# kaccy-backend/
# ├── Cargo.toml
# ├── .env
# ├── migrations/          (will be created by sqlx)
# └── src/
#     ├── main.rs
#     ├── routes/
#     │   ├── mod.rs
#     │   ├── auth.rs
#     │   └── users.rs
#     ├── models/
#     │   ├── mod.rs
#     │   └── user.rs
#     ├── db/
#     │   ├── mod.rs
#     │   └── pool.rs
#     ├── middleware/
#     │   ├── mod.rs
#     │   └── auth.rs
#     └── utils/
#         ├── mod.rs
#         ├── error.rs
#         └── jwt.rs

```

### Step 2.4: Environment Configuration

```bash
Copy# File: .env

DATABASE_URL=postgresql://kaccy_admin:CHANGE_THIS_STRONG_PASSWORD@localhost/kaccy_db
JWT_SECRET=CHANGE_THIS_TO_RANDOM_64_CHAR_STRING_GENERATE_WITH_openssl_rand_base64_48
RUST_LOG=debug
HOST=0.0.0.0
PORT=8080

```

```bash
Copy# Generate JWT secret
openssl rand -base64 48
# Copy output and paste into .env as JWT_SECRET

```

### Step 2.5: Database Migration Setup

```bash
Copy# Initialize SQLx migrations
sqlx migrate add create_users_table

# This creates: migrations/YYYYMMDDHHMMSS_create_users_table.sql

```

```sql
Copy-- File: migrations/YYYYMMDDHHMMSS_create_users_table.sql

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
    user_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    username VARCHAR(50) UNIQUE NOT NULL,
    display_name VARCHAR(100),
    bio TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    kyc_status VARCHAR(20) DEFAULT 'pending',
    reputation_score NUMERIC(10, 2) DEFAULT 500.0,
    role VARCHAR(20) DEFAULT 'user'
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);

```

```bash
Copy# Run migration
sqlx migrate run

# Verify table created
psql -U kaccy_admin -d kaccy_db -c "\dt"
# Should show: users table

```

---

## ⚙️ Day 1 Code Implementation

### File 1: Database Pool

```rust
Copy// File: src/db/pool.rs

use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

```

```rust
Copy// File: src/db/mod.rs

pub mod pool;

```

---

### File 2: User Model

```rust
Copy// File: src/models/user.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]  // Never send password_hash to client
    pub password_hash: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub kyc_status: String,
    pub reputation_score: f64,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserPublic {
    pub user_id: Uuid,
    pub email: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub kyc_status: String,
    pub reputation_score: f64,
}

impl From<User> for UserPublic {
    fn from(user: User) -> Self {
        UserPublic {
            user_id: user.user_id,
            email: user.email,
            username: user.username,
            display_name: user.display_name,
            bio: user.bio,
            avatar_url: user.avatar_url,
            created_at: user.created_at,
            kyc_status: user.kyc_status,
            reputation_score: user.reputation_score,
        }
    }
}

```

```rust
Copy// File: src/models/mod.rs

pub mod user;

```

---

### File 3: Error Handling

```rust
Copy// File: src/utils/error.rs

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error")
            }
            AppError::Auth(ref msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            AppError::Validation(ref msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            AppError::NotFound(ref msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            AppError::Internal => {
                tracing::error!("Internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
            }
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

```

---

### File 4: JWT Utilities

```rust
Copy// File: src/utils/jwt.rs

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::{AppError, Result};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // user_id
    pub exp: i64,     // expiration time
    pub iat: i64,     // issued at
}

pub fn generate_token(user_id: Uuid, jwt_secret: &str) -> Result<String> {
    let now = Utc::now();
    let expires_at = now + Duration::hours(1); // 1 hour expiration

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expires_at.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| {
        tracing::error!("JWT encoding error: {:?}", e);
        AppError::Internal
    })
}

pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| {
        tracing::error!("JWT decoding error: {:?}", e);
        AppError::Auth("Invalid or expired token".to_string())
    })
}

```

```rust
Copy// File: src/utils/mod.rs

pub mod error;
pub mod jwt;

```

---

### File 5: Auth Routes

```rust
Copy// File: src/routes/auth.rs

use axum::{extract::State, Json};
use sqlx::PgPool;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::{
    models::user::{AuthResponse, LoginRequest, RegisterRequest, User, UserPublic},
    utils::{
        error::{AppError, Result},
        jwt::generate_token,
    },
};

pub async fn register(
    State(pool): State<PgPool>,
    State(jwt_secret): State<String>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    // Validate input
    if payload.email.is_empty() || payload.password.is_empty() || payload.username.is_empty() {
        return Err(AppError::Validation("All fields are required".to_string()));
    }

    if payload.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!("Password hashing error: {:?}", e);
            AppError::Internal
        })?
        .to_string();

    // Insert user into database
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, password_hash, username)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&payload.username)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return AppError::Validation("Email or username already exists".to_string());
            }
        }
        AppError::Database(e)
    })?;

    // Generate JWT token
    let token = generate_token(user.user_id, &jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

pub async fn login(
    State(pool): State<PgPool>,
    State(jwt_secret): State<String>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    // Fetch user from database
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT * FROM users
        WHERE email = $1
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::Auth("Invalid email or password".to_string()))?;

    // Verify password
    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
        tracing::error!("Password hash parsing error: {:?}", e);
        AppError::Internal
    })?;

    Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Auth("Invalid email or password".to_string()))?;

    // Generate JWT token
    let token = generate_token(user.user_id, &jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

```

```rust
Copy// File: src/routes/mod.rs

pub mod auth;
pub mod users;

```

---

### File 6: Auth Middleware

```rust
Copy// File: src/middleware/auth.rs

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::utils::{error::AppError, jwt::verify_token};

pub async fn require_auth(
    State(jwt_secret): State<String>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Extract token (format: "Bearer <token>")
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify token
    let claims = verify_token(token, &jwt_secret).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Parse user_id from claims
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Insert user_id into request extensions
    req.extensions_mut().insert(user_id);

    Ok(next.run(req).await)
}

```

```rust
Copy// File: src/middleware/mod.rs

pub mod auth;

```

---

### File 7: User Routes (Protected)

```rust
Copy// File: src/routes/users.rs

use axum::{extract::State, Extension, Json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::user::UserPublic,
    utils::error::{AppError, Result},
};

pub async fn get_me(
    State(pool): State<PgPool>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<UserPublic>> {
    let user = sqlx::query_as::<_, UserPublic>(
        r#"
        SELECT user_id, email, username, display_name, bio, avatar_url,
               created_at, kyc_status, reputation_score
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await?;

    Ok(Json(user))
}

```

---

### File 8: Main Application

```rust
Copy// File: src/main.rs

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use std::env;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db;
mod middleware as app_middleware;
mod models;
mod routes;
mod utils;

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kaccy_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get environment variables
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    // Create database connection pool
    let pool = db::pool::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");

    tracing::info!("Database connection pool created");

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build application router
    let app = Router::new()
        // Public routes
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login))
        // Protected routes
        .route(
            "/api/users/me",
            get(routes::users::get_me)
                .route_layer(middleware::from_fn_with_state(
                    jwt_secret.clone(),
                    app_middleware::auth::require_auth,
                )),
        )
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Add state
        .with_state(pool.clone())
        .with_state(jwt_secret)
        // Add CORS
        .layer(cors);

    // Start server
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

```

---

## 🧪 Day 1 Testing

```bash
Copy# Build the project
cargo build

# If errors, fix them (likely missing dependencies or typos)

# Run the server
cargo run

# You should see:
# Database connection pool created
# Server listening on 0.0.0.0:8080

```

### Test with curl

```bash
Copy# Open new terminal

# 1. Health check
curl http://localhost:8080/health
# Expected: OK

# 2. Register new user
curl -X POST http://localhost:8080/api/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "password123",
    "username": "testuser"
  }'

# Expected output (JSON):
# {
#   "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
#   "user": {
#     "user_id": "...",
#     "email": "test@example.com",
#     "username": "testuser",
#     ...
#   }
# }

# Copy the token from the response

# 3. Login
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "password123"
  }'

# Expected: Same as register response

# 4. Get current user (protected route)
TOKEN="<paste-token-here>"

curl http://localhost:8080/api/users/me \
  -H "Authorization: Bearer $TOKEN"

# Expected: User object (without password_hash)

```

---

## ✅ Day 1 Completion Checklist

```yaml
CopyInfrastructure:
  - [ ] VPS provisioned and accessible
  - [ ] Firewall configured
  - [ ] PostgreSQL installed and running
  - [ ] Database and user created

Backend:
  - [ ] Rust installed
  - [ ] Project initialized
  - [ ] Dependencies configured (Cargo.toml)
  - [ ] Folder structure created
  - [ ] .env file configured
  - [ ] Database migration created and run

Code:
  - [ ] Database pool module
  - [ ] User model
  - [ ] Error handling
  - [ ] JWT utilities
  - [ ] Auth routes (register, login)
  - [ ] Auth middleware
  - [ ] User routes (get_me)
  - [ ] Main application

Testing:
  - [ ] Server starts without errors
  - [ ] Health check works
  - [ ] User registration works
  - [ ] User login works
  - [ ] Protected route works (with token)
  - [ ] Protected route fails (without token)

```

---

## 📅 Day 2 Preview: Frontend Setup

Tomorrow we'll build:

- ✅ Next.js project initialization
- ✅ Login page
- ✅ Register page
- ✅ Dashboard (protected)
- ✅ API client with axios
- ✅ Auth state management (Zustand)

---

## 🎤 End of Day 1 Standup

**What we built today:**

1. ✅ Complete VPS setup (Ubuntu, PostgreSQL, Rust)
2. ✅ Backend API with authentication (register, login, get user)
3. ✅ Database schema and migrations
4. ✅ JWT-based authentication
5. ✅ Password hashing with Argon2
6. ✅ Protected routes with middleware

**Blockers:**

- None (or note any issues you encountered)

**Tomorrow:**

- Frontend setup (Next.js)
- Connect frontend to backend API
- Complete authentication flow (UI)