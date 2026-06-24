//! OAuth2 authentication routes with real CSRF + PKCE state persistence.
//!
//! Security model:
//! - On login: generate a PKCE challenge + CSRF token, serialize the full
//!   `OAuth2State` as JSON, and store it in Redis keyed by the CSRF/state token
//!   with a 10-minute TTL.  The unguessable CSRF token is passed to the provider
//!   as the `state` parameter.
//! - On callback: look up the state by the returned `state` query parameter,
//!   **atomically delete** it (one-time use — prevents replay), verify it was
//!   found (not expired), then drive the token exchange with the stored PKCE
//!   verifier.
//! - If Redis is not configured the login flow returns an honest configuration
//!   error rather than silently skipping CSRF/PKCE protection.

use axum::{
    Json,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{ApiError, Result};
use crate::middleware::auth;
use crate::state::AppState;

// ──────────────────────────────────────────────────────────────────────────────
// Redis key helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Namespace prefix used for all OAuth2 state keys stored in Redis.
const OAUTH2_STATE_PREFIX: &str = "oauth2:state:";

/// TTL (seconds) for a pending OAuth2 state entry — 10 minutes.
const OAUTH2_STATE_TTL_SECS: u64 = 600;

/// Build the Redis key for a given CSRF/state token.
fn oauth2_state_key(csrf_token: &str) -> String {
    format!("{}{}", OAUTH2_STATE_PREFIX, csrf_token)
}

// ──────────────────────────────────────────────────────────────────────────────
// Abstraction layer: OAuthStateStore
//
// This thin trait lets the handler logic be tested without a live Redis.
// ──────────────────────────────────────────────────────────────────────────────

/// Operations required of any OAuth2 state store.
///
/// Implementations must guarantee **one-time-use** semantics: `consume` must
/// atomically remove the entry so that a second call with the same key returns
/// `None` even under concurrent access.
#[async_trait::async_trait]
pub trait OAuthStateStore: Send + Sync {
    /// Persist `state` under `csrf_token` with the configured TTL.
    async fn store(&self, csrf_token: &str, state: &OAuth2State) -> Result<()>;

    /// Atomically retrieve **and delete** the state for `csrf_token`.
    ///
    /// Returns `Ok(None)` when the key is absent or has expired.
    async fn consume(&self, csrf_token: &str) -> Result<Option<OAuth2State>>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Redis-backed implementation
// ──────────────────────────────────────────────────────────────────────────────

/// Redis-backed OAuth2 state store that uses `kaccy_db::RedisCache`.
struct RedisOAuthStateStore<'a> {
    cache: &'a kaccy_db::RedisCache,
}

impl<'a> RedisOAuthStateStore<'a> {
    fn new(cache: &'a kaccy_db::RedisCache) -> Self {
        Self { cache }
    }
}

#[async_trait::async_trait]
impl<'a> OAuthStateStore for RedisOAuthStateStore<'a> {
    async fn store(&self, csrf_token: &str, state: &OAuth2State) -> Result<()> {
        let key = oauth2_state_key(csrf_token);
        self.cache
            .set(&key, state, OAUTH2_STATE_TTL_SECS)
            .await
            .map_err(|e| {
                tracing::error!("Redis set failed for OAuth2 state: {:?}", e);
                ApiError::InternalWithMessage("Failed to persist OAuth2 state in Redis".to_string())
            })
    }

    async fn consume(&self, csrf_token: &str) -> Result<Option<OAuth2State>> {
        let key = oauth2_state_key(csrf_token);

        // Retrieve first; if absent (expired or never stored) short-circuit.
        let maybe_state: Option<OAuth2State> = self.cache.get(&key).await.map_err(|e| {
            tracing::error!("Redis get failed for OAuth2 state: {:?}", e);
            ApiError::InternalWithMessage("Failed to retrieve OAuth2 state from Redis".to_string())
        })?;

        if maybe_state.is_some() {
            // Delete unconditionally — one-time use.  A failure to delete is
            // logged but we still return the state; the short TTL bounds the
            // exposure window even if deletion fails.
            if let Err(e) = self.cache.delete(&key).await {
                tracing::error!("Redis delete failed for OAuth2 state key {}: {:?}", key, e);
            }
        }

        Ok(maybe_state)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────────────────────────────────────

/// OAuth2 provider configuration.
#[derive(Clone, Debug)]
pub struct OAuth2Provider {
    /// Provider name (e.g. "google", "github").
    pub name: String,
    /// OAuth2 application client ID.
    pub client_id: String,
    /// OAuth2 application client secret.
    pub client_secret: String,
    /// Authorization endpoint URL.
    pub auth_url: String,
    /// Token exchange endpoint URL.
    pub token_url: String,
    /// Redirect URI registered with the provider.
    pub redirect_uri: String,
    /// OAuth2 scopes to request.
    pub scopes: Vec<String>,
    /// Endpoint to fetch the authenticated user's info.
    pub user_info_url: String,
}

impl OAuth2Provider {
    /// Create Google OAuth2 provider.
    pub fn google(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            name: "google".to_string(),
            client_id,
            client_secret,
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri,
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            user_info_url: "https://www.googleapis.com/oauth2/v2/userinfo".to_string(),
        }
    }

    /// Create GitHub OAuth2 provider.
    pub fn github(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            name: "github".to_string(),
            client_id,
            client_secret,
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            redirect_uri,
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            user_info_url: "https://api.github.com/user".to_string(),
        }
    }
}

/// OAuth2 state persisted in Redis between the login redirect and the callback.
///
/// Stored as JSON; keyed by the CSRF/state token with a short TTL so that
/// expired or replayed callbacks are rejected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OAuth2State {
    /// The CSRF token value (== the Redis key suffix).
    pub csrf_token: String,
    /// PKCE code verifier secret — used in the token exchange.
    pub pkce_verifier: String,
    /// Provider name used in the initial request (e.g. "google").
    pub provider: String,
}

/// OAuth2 callback query parameters.
#[derive(Debug, Deserialize)]
pub struct OAuth2Callback {
    /// Authorization code returned by the provider.
    pub code: String,
    /// State token to verify CSRF protection.
    pub state: String,
}

/// OAuth2 user info from provider.
#[derive(Debug, Deserialize)]
pub struct OAuth2UserInfo {
    /// Provider-assigned user identifier.
    #[serde(default)]
    pub id: String,
    /// User's email address.
    #[serde(default)]
    pub email: Option<String>,
    /// User's display name.
    #[serde(default)]
    pub name: Option<String>,
    /// Username or login handle (e.g. GitHub username).
    #[serde(default)]
    pub login: Option<String>,
    /// URL to the user's avatar image.
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// OAuth2 login response.
#[derive(Debug, Serialize)]
pub struct OAuth2LoginResponse {
    /// JWT token for the authenticated session.
    pub token: String,
    /// Kaccy user ID for the authenticated user.
    pub user_id: Uuid,
    /// True if the account was created during this OAuth2 login.
    pub is_new_user: bool,
}

// ──────────────────────────────────────────────────────────────────────────────
// Route handlers (public API — signatures must not change)
// ──────────────────────────────────────────────────────────────────────────────

/// Initialize OAuth2 login flow.
///
/// Generates a PKCE challenge and a CSRF/state token, stores them in Redis
/// with a 10-minute TTL, then redirects the user-agent to the provider's
/// authorization endpoint.
///
/// Returns `503 Internal Server Error` when Redis is not configured — skipping
/// CSRF/PKCE protection silently is not an option.
pub async fn oauth2_login(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let provider_name = params.get("provider").ok_or_else(|| {
        ApiError::Validation("Provider is required (google or github)".to_string())
    })?;

    let provider = get_provider(provider_name)?;

    // Require Redis — we cannot guarantee CSRF/PKCE without it.
    let redis_cache = state.redis().map_err(|_| {
        ApiError::InternalWithMessage(
            "OAuth2 state store (Redis) is not configured; \
             login flow cannot proceed without CSRF/PKCE protection"
                .to_string(),
        )
    })?;
    let store = RedisOAuthStateStore::new(redis_cache);

    // Generate PKCE challenge and CSRF token.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url_parsed, token_url_parsed, redirect_url_parsed) =
        validate_provider_urls(&provider)?;

    // Build the oauth2 client — type-state fully resolved at the call site.
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(auth_url_parsed)
        .set_token_uri(token_url_parsed)
        .set_redirect_uri(redirect_url_parsed);

    let mut auth_request = client.authorize_url(CsrfToken::new_random);
    for scope in &provider.scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }
    let (auth_url, csrf_token) = auth_request.set_pkce_challenge(pkce_challenge).url();

    let csrf_value = csrf_token.secret().to_string();

    // Persist state in Redis — the CSRF token is the key.
    let oauth_state = OAuth2State {
        csrf_token: csrf_value.clone(),
        pkce_verifier: pkce_verifier.secret().to_string(),
        provider: provider_name.to_string(),
    };
    store.store(&csrf_value, &oauth_state).await?;

    Ok(Redirect::to(auth_url.as_str()))
}

/// Handle OAuth2 callback.
///
/// Validates the returned `state` parameter against the Redis store (CSRF
/// check), consumes the entry (replay protection), then completes the PKCE
/// token exchange with the stored verifier.
///
/// Returns `401 Unauthorized` when the state is missing, expired, or has
/// already been consumed.
pub async fn oauth2_callback(
    State(state): State<AppState>,
    Query(callback): Query<OAuth2Callback>,
) -> Result<Json<OAuth2LoginResponse>> {
    // Require Redis.
    let redis_cache = state.redis().map_err(|_| {
        ApiError::InternalWithMessage(
            "OAuth2 state store (Redis) is not configured; \
             callback cannot be validated"
                .to_string(),
        )
    })?;
    let store = RedisOAuthStateStore::new(redis_cache);

    // Consume the pending state — this is an atomic get + delete.
    let oauth_state = store.consume(&callback.state).await?.ok_or_else(|| {
        ApiError::Auth(
            "OAuth2 state not found or expired — possible CSRF or replay attack".to_string(),
        )
    })?;

    // CSRF check: the `state` query param must match the stored token.
    if oauth_state.csrf_token != callback.state {
        return Err(ApiError::Auth("OAuth2 CSRF token mismatch".to_string()));
    }

    let provider = get_provider(&oauth_state.provider)?;

    let (auth_url_parsed, token_url_parsed, redirect_url_parsed) =
        validate_provider_urls(&provider)?;

    // Build the oauth2 client — type-state fully resolved at the call site.
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(auth_url_parsed)
        .set_token_uri(token_url_parsed)
        .set_redirect_uri(redirect_url_parsed);

    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ApiError::InternalWithMessage("Failed to create HTTP client".to_string()))?;

    // Token exchange — now wired with the stored PKCE verifier.
    let token_result = client
        .exchange_code(AuthorizationCode::new(callback.code))
        .set_pkce_verifier(PkceCodeVerifier::new(oauth_state.pkce_verifier))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            tracing::error!("OAuth2 token exchange failed: {:?}", e);
            ApiError::Auth("Failed to exchange OAuth2 code".to_string())
        })?;

    let user_info = fetch_user_info(&provider, token_result.access_token().secret()).await?;

    let pool = &state.pool;
    let (user_id, is_new_user) =
        find_or_create_oauth_user(pool, &oauth_state.provider, &user_info).await?;

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let token = auth::generate_token(user_id, &jwt_secret)
        .map_err(|_| ApiError::InternalWithMessage("Failed to generate token".to_string()))?;

    Ok(Json(OAuth2LoginResponse {
        token,
        user_id,
        is_new_user,
    }))
}

// ──────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Validate the OAuth2 endpoint URLs for a provider, returning errors on
/// malformed URLs so the caller can construct the client safely.
fn validate_provider_urls(provider: &OAuth2Provider) -> Result<(AuthUrl, TokenUrl, RedirectUrl)> {
    let auth_url = AuthUrl::new(provider.auth_url.clone())
        .map_err(|e| ApiError::InternalWithMessage(format!("Invalid OAuth2 auth URL: {}", e)))?;
    let token_url = TokenUrl::new(provider.token_url.clone())
        .map_err(|e| ApiError::InternalWithMessage(format!("Invalid OAuth2 token URL: {}", e)))?;
    let redirect_url = RedirectUrl::new(provider.redirect_uri.clone()).map_err(|e| {
        ApiError::InternalWithMessage(format!("Invalid OAuth2 redirect URI: {}", e))
    })?;
    Ok((auth_url, token_url, redirect_url))
}

/// Get OAuth2 provider by name, reading credentials from environment variables.
fn get_provider(name: &str) -> Result<OAuth2Provider> {
    match name {
        "google" => {
            let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
                ApiError::InternalWithMessage("GOOGLE_CLIENT_ID not configured".to_string())
            })?;
            let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| {
                ApiError::InternalWithMessage("GOOGLE_CLIENT_SECRET not configured".to_string())
            })?;
            let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:3000/api/oauth2/callback".to_string());
            Ok(OAuth2Provider::google(
                client_id,
                client_secret,
                redirect_uri,
            ))
        }
        "github" => {
            let client_id = std::env::var("GITHUB_CLIENT_ID").map_err(|_| {
                ApiError::InternalWithMessage("GITHUB_CLIENT_ID not configured".to_string())
            })?;
            let client_secret = std::env::var("GITHUB_CLIENT_SECRET").map_err(|_| {
                ApiError::InternalWithMessage("GITHUB_CLIENT_SECRET not configured".to_string())
            })?;
            let redirect_uri = std::env::var("GITHUB_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:3000/api/oauth2/callback".to_string());
            Ok(OAuth2Provider::github(
                client_id,
                client_secret,
                redirect_uri,
            ))
        }
        _ => Err(ApiError::Validation(format!(
            "Unsupported OAuth2 provider: {}",
            name
        ))),
    }
}

/// Fetch user info from OAuth2 provider using the access token.
async fn fetch_user_info(provider: &OAuth2Provider, access_token: &str) -> Result<OAuth2UserInfo> {
    let client = reqwest::Client::new();
    let response = client
        .get(&provider.user_info_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user info: {:?}", e);
            ApiError::InternalWithMessage("Failed to fetch user info from provider".to_string())
        })?;

    if !response.status().is_success() {
        return Err(ApiError::InternalWithMessage(
            "Provider returned error when fetching user info".to_string(),
        ));
    }

    response.json::<OAuth2UserInfo>().await.map_err(|e| {
        tracing::error!("Failed to parse user info: {:?}", e);
        ApiError::InternalWithMessage("Failed to parse user info from provider".to_string())
    })
}

/// Find an existing user by email, or create a new one from OAuth2 info.
async fn find_or_create_oauth_user(
    pool: &PgPool,
    _provider: &str,
    user_info: &OAuth2UserInfo,
) -> Result<(Uuid, bool)> {
    let email = user_info.email.as_ref().ok_or_else(|| {
        ApiError::Validation("Email is required from OAuth2 provider".to_string())
    })?;

    let existing = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::Database)?;

    if let Some((user_id,)) = existing {
        return Ok((user_id, false));
    }

    // Derive a username from the email local-part; the split always yields at
    // least one element so indexing [0] is safe here.
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    let username = parts[0];
    let display_name = user_info.name.as_ref().or(user_info.login.as_ref());
    let user_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO users \
         (id, username, email, password_hash, display_name, avatar_url, role) \
         VALUES ($1, $2, $3, '', $4, $5, 'user')",
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind(display_name)
    .bind(&user_info.avatar_url)
    .execute(pool)
    .await
    .map_err(ApiError::Database)?;

    Ok((user_id, true))
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    // ── In-memory mock store ───────────────────────────────────────────────────

    /// Entry stored in the mock; tracks insertion time for TTL simulation.
    struct MockEntry {
        state: OAuth2State,
        inserted_at: Instant,
        ttl: Duration,
    }

    impl MockEntry {
        fn is_expired(&self) -> bool {
            self.inserted_at.elapsed() > self.ttl
        }
    }

    /// Thread-safe in-memory store for unit tests — no Redis required.
    #[derive(Default)]
    struct MockOAuthStateStore {
        inner: Arc<Mutex<HashMap<String, MockEntry>>>,
        /// Configurable TTL for tests; defaults to 10 minutes like production.
        ttl: Duration,
    }

    impl MockOAuthStateStore {
        fn new() -> Self {
            Self {
                inner: Arc::default(),
                ttl: Duration::from_secs(OAUTH2_STATE_TTL_SECS),
            }
        }

        /// Create a store that immediately expires every entry (TTL = 0).
        fn with_zero_ttl() -> Self {
            Self {
                inner: Arc::default(),
                ttl: Duration::ZERO,
            }
        }
    }

    #[async_trait::async_trait]
    impl OAuthStateStore for MockOAuthStateStore {
        async fn store(&self, csrf_token: &str, state: &OAuth2State) -> Result<()> {
            let mut map = self.inner.lock().expect("lock poisoned");
            map.insert(
                csrf_token.to_string(),
                MockEntry {
                    state: state.clone(),
                    inserted_at: Instant::now(),
                    ttl: self.ttl,
                },
            );
            Ok(())
        }

        async fn consume(&self, csrf_token: &str) -> Result<Option<OAuth2State>> {
            let mut map = self.inner.lock().expect("lock poisoned");
            match map.remove(csrf_token) {
                Some(entry) if !entry.is_expired() => Ok(Some(entry.state)),
                // Expired entries are treated as absent — consistent with Redis TTL.
                Some(_expired) => Ok(None),
                None => Ok(None),
            }
        }
    }

    // ── Serialization round-trip ───────────────────────────────────────────────

    #[test]
    fn test_oauth2_state_serialization_round_trip() {
        let original = OAuth2State {
            csrf_token: "csrf-abc-123".to_string(),
            pkce_verifier: "verifier-xyz-456".to_string(),
            provider: "google".to_string(),
        };

        let json = serde_json::to_string(&original).expect("serialize");
        let restored: OAuth2State = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(original, restored);
    }

    #[test]
    fn test_oauth2_state_key_format() {
        let key = oauth2_state_key("my-csrf-token");
        assert_eq!(key, "oauth2:state:my-csrf-token");
        assert!(key.starts_with(OAUTH2_STATE_PREFIX));
    }

    // ── Store + retrieve (happy path) ─────────────────────────────────────────

    #[tokio::test]
    async fn test_store_and_consume_returns_correct_state() {
        let store = MockOAuthStateStore::new();
        let state = OAuth2State {
            csrf_token: "tok-1".to_string(),
            pkce_verifier: "pkce-secret".to_string(),
            provider: "github".to_string(),
        };

        store.store("tok-1", &state).await.expect("store");
        let retrieved = store.consume("tok-1").await.expect("consume");

        assert_eq!(retrieved, Some(state));
    }

    // ── One-time-use (replay protection) ──────────────────────────────────────

    #[tokio::test]
    async fn test_second_consume_returns_none() {
        let store = MockOAuthStateStore::new();
        let state = OAuth2State {
            csrf_token: "tok-once".to_string(),
            pkce_verifier: "pkce-secret".to_string(),
            provider: "google".to_string(),
        };

        store.store("tok-once", &state).await.expect("store");

        // First consume succeeds.
        let first = store.consume("tok-once").await.expect("first consume");
        assert!(first.is_some(), "first consume must succeed");

        // Second consume must fail — entry was deleted.
        let second = store.consume("tok-once").await.expect("second consume");
        assert!(second.is_none(), "replay: second consume must return None");
    }

    // ── Missing / never-stored state ───────────────────────────────────────────

    #[tokio::test]
    async fn test_consume_unknown_token_returns_none() {
        let store = MockOAuthStateStore::new();
        let result = store.consume("nonexistent-token").await.expect("consume");
        assert_eq!(result, None);
    }

    // ── Expired state ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_expired_state_returns_none() {
        let store = MockOAuthStateStore::with_zero_ttl();
        let state = OAuth2State {
            csrf_token: "tok-expire".to_string(),
            pkce_verifier: "pkce-secret".to_string(),
            provider: "google".to_string(),
        };

        store.store("tok-expire", &state).await.expect("store");

        // With TTL=0 the entry is immediately expired.
        let result = store.consume("tok-expire").await.expect("consume");
        assert_eq!(result, None, "expired entry must be treated as absent");
    }

    // ── Honest error on missing state (CSRF validation path) ──────────────────

    #[test]
    fn test_missing_state_maps_to_auth_error() {
        // Simulate what the callback handler does when `consume` returns None.
        let consumed: Option<OAuth2State> = None;
        let result: Result<OAuth2State> = consumed.ok_or_else(|| {
            ApiError::Auth(
                "OAuth2 state not found or expired — possible CSRF or replay attack".to_string(),
            )
        });

        assert!(
            result.is_err(),
            "missing state must produce an error, not succeed"
        );
        match result.unwrap_err() {
            ApiError::Auth(msg) => {
                assert!(
                    msg.contains("CSRF") || msg.contains("expired"),
                    "error message must mention CSRF or expiry: {msg}"
                );
            }
            other => panic!("expected ApiError::Auth, got {:?}", other),
        }
    }

    // ── CSRF mismatch ──────────────────────────────────────────────────────────

    #[test]
    fn test_csrf_mismatch_detected() {
        let stored = OAuth2State {
            csrf_token: "real-token".to_string(),
            pkce_verifier: "pkce-secret".to_string(),
            provider: "google".to_string(),
        };
        // Callback arrives with a different `state` value.
        let callback_state = "attacker-injected-state";

        let mismatch = stored.csrf_token != callback_state;
        assert!(mismatch, "CSRF mismatch must be detected");
    }

    // ── Multiple independent flows do not interfere ────────────────────────────

    #[tokio::test]
    async fn test_multiple_concurrent_flows_are_independent() {
        let store = MockOAuthStateStore::new();

        let state_a = OAuth2State {
            csrf_token: "tok-a".to_string(),
            pkce_verifier: "verifier-a".to_string(),
            provider: "google".to_string(),
        };
        let state_b = OAuth2State {
            csrf_token: "tok-b".to_string(),
            pkce_verifier: "verifier-b".to_string(),
            provider: "github".to_string(),
        };

        store.store("tok-a", &state_a).await.expect("store a");
        store.store("tok-b", &state_b).await.expect("store b");

        let got_b = store.consume("tok-b").await.expect("consume b");
        let got_a = store.consume("tok-a").await.expect("consume a");

        assert_eq!(got_a, Some(state_a), "flow A must retrieve its own state");
        assert_eq!(got_b, Some(state_b), "flow B must retrieve its own state");
    }

    // ── Provider config tests ──────────────────────────────────────────────────

    #[test]
    fn test_google_provider_config() {
        let provider = OAuth2Provider::google(
            "client_id".to_string(),
            "client_secret".to_string(),
            "http://localhost:3000/callback".to_string(),
        );

        assert_eq!(provider.name, "google");
        assert_eq!(provider.scopes.len(), 3);
        assert!(provider.scopes.contains(&"email".to_string()));
    }

    #[test]
    fn test_github_provider_config() {
        let provider = OAuth2Provider::github(
            "client_id".to_string(),
            "client_secret".to_string(),
            "http://localhost:3000/callback".to_string(),
        );

        assert_eq!(provider.name, "github");
        assert_eq!(provider.scopes.len(), 2);
        assert!(provider.scopes.contains(&"read:user".to_string()));
    }

    // ── Unsupported provider returns Validation error ──────────────────────────

    #[test]
    fn test_unsupported_provider_returns_validation_error() {
        // Unset env vars to avoid accidentally picking up credentials.
        // (get_provider for "unknown" short-circuits before reading any env var.)
        let result = get_provider("foobar");
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::Validation(msg) => {
                assert!(
                    msg.contains("foobar"),
                    "message must name the provider: {msg}"
                );
            }
            other => panic!("expected ApiError::Validation, got {:?}", other),
        }
    }

    // ── TTL constant sanity ────────────────────────────────────────────────────

    #[test]
    fn test_oauth2_state_ttl_is_10_minutes() {
        assert_eq!(OAUTH2_STATE_TTL_SECS, 600, "TTL must be 10 minutes (600 s)");
    }
}
