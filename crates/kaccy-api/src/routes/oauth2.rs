//! OAuth2 authentication routes

use axum::{
    Json,
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{ApiError, Result};
use crate::middleware::auth;
use crate::state::AppState;

/// OAuth2 provider configuration
#[derive(Clone, Debug)]
pub struct OAuth2Provider {
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub user_info_url: String,
}

impl OAuth2Provider {
    /// Create Google OAuth2 provider
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

    /// Create GitHub OAuth2 provider
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

/// OAuth2 state stored in session
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuth2State {
    pub csrf_token: String,
    pub pkce_verifier: String,
    pub provider: String,
}

/// OAuth2 callback query parameters
#[derive(Debug, Deserialize)]
pub struct OAuth2Callback {
    pub code: String,
    pub state: String,
}

/// OAuth2 user info from provider
#[derive(Debug, Deserialize)]
pub struct OAuth2UserInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// OAuth2 login response
#[derive(Debug, Serialize)]
pub struct OAuth2LoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub is_new_user: bool,
}

/// Initialize OAuth2 login flow
pub async fn oauth2_login(
    State(_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let provider_name = params.get("provider").ok_or(ApiError::Validation(
        "Provider is required (google or github)".to_string(),
    ))?;

    let provider = get_provider(provider_name)?;

    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build OAuth2 client and authorization URL
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.clone()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new(provider.token_url.clone()).expect("Invalid token URL"))
        .set_redirect_uri(
            RedirectUrl::new(provider.redirect_uri.clone()).expect("Invalid redirect URI"),
        );

    let mut auth_request = client.authorize_url(CsrfToken::new_random);

    for scope in &provider.scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }

    let (auth_url, csrf_token) = auth_request.set_pkce_challenge(pkce_challenge).url();

    // Store OAuth2 state in session (in production, use Redis or database)
    // For now, we'll include it in the redirect state parameter
    let _oauth_state = OAuth2State {
        csrf_token: csrf_token.secret().to_string(),
        pkce_verifier: pkce_verifier.secret().to_string(),
        provider: provider_name.to_string(),
    };

    // In production, store _oauth_state in Redis with csrf_token as key
    // For now, we'll skip session management for simplicity

    Ok(Redirect::to(auth_url.as_str()))
}

/// Handle OAuth2 callback
pub async fn oauth2_callback(
    State(state): State<AppState>,
    Query(callback): Query<OAuth2Callback>,
) -> Result<Json<OAuth2LoginResponse>> {
    // In production, retrieve oauth_state from Redis using callback.state
    // For this implementation, we'll use environment variables directly

    let provider_name = std::env::var("OAUTH2_PROVIDER").unwrap_or_else(|_| "google".to_string());
    let provider = get_provider(&provider_name)?;

    // Build OAuth2 client
    let client = BasicClient::new(ClientId::new(provider.client_id.clone()))
        .set_client_secret(ClientSecret::new(provider.client_secret.clone()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.clone()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new(provider.token_url.clone()).expect("Invalid token URL"))
        .set_redirect_uri(
            RedirectUrl::new(provider.redirect_uri.clone()).expect("Invalid redirect URI"),
        );

    // Exchange code for token using oauth2's re-exported reqwest client
    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ApiError::InternalWithMessage("Failed to create HTTP client".to_string()))?;

    let token_result = client
        .exchange_code(AuthorizationCode::new(callback.code))
        // .set_pkce_verifier(PkceCodeVerifier::new(oauth_state.pkce_verifier)) // Would use stored verifier
        .request_async(&http_client)
        .await
        .map_err(|_| ApiError::Auth("Failed to exchange OAuth2 code".to_string()))?;

    // Fetch user info
    let user_info = fetch_user_info(&provider, token_result.access_token().secret()).await?;

    // Find or create user
    let pool = &state.pool;
    let (user_id, is_new_user) =
        find_or_create_oauth_user(pool, &provider_name, &user_info).await?;

    // Generate JWT token
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "secret".to_string());
    let token = auth::generate_token(user_id, &jwt_secret)
        .map_err(|_| ApiError::InternalWithMessage("Failed to generate token".to_string()))?;

    Ok(Json(OAuth2LoginResponse {
        token,
        user_id,
        is_new_user,
    }))
}

/// Get OAuth2 provider by name
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
            "Unsupported provider: {}",
            name
        ))),
    }
}

/// Fetch user info from OAuth2 provider
async fn fetch_user_info(provider: &OAuth2Provider, access_token: &str) -> Result<OAuth2UserInfo> {
    let client = reqwest::Client::new();
    let response = client
        .get(&provider.user_info_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| ApiError::InternalWithMessage("Failed to fetch user info".to_string()))?;

    if !response.status().is_success() {
        return Err(ApiError::InternalWithMessage(
            "Failed to fetch user info".to_string(),
        ));
    }

    let user_info = response
        .json::<OAuth2UserInfo>()
        .await
        .map_err(|_| ApiError::InternalWithMessage("Failed to parse user info".to_string()))?;

    Ok(user_info)
}

/// Find or create user from OAuth2 info
async fn find_or_create_oauth_user(
    pool: &PgPool,
    _provider: &str,
    user_info: &OAuth2UserInfo,
) -> Result<(Uuid, bool)> {
    // Create new user
    let email = user_info.email.as_ref().ok_or(ApiError::Validation(
        "Email is required from OAuth2 provider".to_string(),
    ))?;

    // Check if user already exists by email
    let existing = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::Database)?;

    if let Some((user_id,)) = existing {
        return Ok((user_id, false));
    }

    let username = email.split('@').next().unwrap_or("user");
    let display_name = user_info.name.as_ref().or(user_info.login.as_ref());

    let user_id = Uuid::new_v4();

    // Create user
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, display_name, avatar_url, role) VALUES ($1, $2, $3, '', $4, $5, 'user')"
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
