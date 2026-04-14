//! Authentication routes

use std::net::SocketAddr;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    extract::{ConnectInfo, State},
};

use kaccy_core::models::{AuthResponse, LoginRequest, RegisterRequest, User};

use crate::error::{ApiError, Result};
use crate::middleware::auth::generate_token;
use crate::middleware::metrics::{
    AUTH_LOGIN_ATTEMPTS_TOTAL, AUTH_TOKENS_ISSUED_TOTAL, USERS_REGISTERED_TOTAL,
};
use crate::state::AppState;

/// Register a new user account
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    // Validate input
    if payload.email.is_empty() || payload.password.is_empty() || payload.username.is_empty() {
        return Err(ApiError::Validation("All fields are required".to_string()));
    }

    if payload.password.len() < 8 {
        return Err(ApiError::Validation(
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
            ApiError::Internal
        })?
        .to_string();

    // Insert user
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
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return ApiError::Validation("Email or username already exists".to_string());
            }
        }
        ApiError::Database(e)
    })?;

    // Generate token
    let token = generate_token(user.user_id, &state.jwt_secret).map_err(|e| {
        tracing::error!("Token generation error: {:?}", e);
        ApiError::Internal
    })?;

    // Update metrics
    USERS_REGISTERED_TOTAL.inc();
    AUTH_TOKENS_ISSUED_TOTAL.inc();

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

/// Authenticate with email and password
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let ip = addr.ip();

    // Check for lockout
    if let Some(remaining) = state.check_login_lockout(ip) {
        tracing::warn!(
            ip = %ip,
            remaining_secs = remaining.as_secs(),
            "Login attempt from locked out IP"
        );
        return Err(ApiError::Auth(format!(
            "Too many failed login attempts. Please try again in {} seconds.",
            remaining.as_secs()
        )));
    }

    // Get user
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT * FROM users
        WHERE email = $1
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        state.record_failed_login(ip);
        AUTH_LOGIN_ATTEMPTS_TOTAL
            .with_label_values(&["failed"])
            .inc();
        ApiError::Auth("Invalid email or password".to_string())
    })?;

    // Verify password
    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
        tracing::error!("Password hash parsing error: {:?}", e);
        ApiError::Internal
    })?;

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        state.record_failed_login(ip);
        AUTH_LOGIN_ATTEMPTS_TOTAL
            .with_label_values(&["failed"])
            .inc();
        return Err(ApiError::Auth("Invalid email or password".to_string()));
    }

    // Successful login - clear any previous failed attempts
    state.clear_login_attempts(ip);

    // Generate token
    let token = generate_token(user.user_id, &state.jwt_secret).map_err(|e| {
        tracing::error!("Token generation error: {:?}", e);
        ApiError::Internal
    })?;

    // Update metrics
    AUTH_LOGIN_ATTEMPTS_TOTAL
        .with_label_values(&["success"])
        .inc();
    AUTH_TOKENS_ISSUED_TOTAL.inc();

    tracing::info!(
        user_id = %user.user_id,
        email = %user.email,
        "User logged in successfully"
    );

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}
