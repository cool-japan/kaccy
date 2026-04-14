//! Rust SDK generator

use std::collections::HashMap;

use super::types::{GeneratedSdk, SdkConfig, SdkLanguage, SdkMetadata};

/// Rust SDK generator
pub struct RustGenerator;

impl RustGenerator {
    /// Generate Rust SDK
    pub fn generate(config: &SdkConfig) -> GeneratedSdk {
        let mut files = HashMap::new();

        // Generate lib.rs
        files.insert("lib.rs".to_string(), Self::generate_lib());

        // Generate client.rs
        files.insert("client.rs".to_string(), Self::generate_client(config));

        // Generate types.rs
        files.insert("types.rs".to_string(), Self::generate_types());

        // Generate error.rs
        files.insert("error.rs".to_string(), Self::generate_error());

        // Generate Cargo.toml
        files.insert("Cargo.toml".to_string(), Self::generate_cargo_toml(config));

        // Generate README
        files.insert("README.md".to_string(), Self::generate_readme(config));

        GeneratedSdk {
            language: SdkLanguage::Rust,
            files,
            metadata: SdkMetadata {
                name: config.package_name.clone(),
                version: config.version.clone(),
                description: "Rust SDK for Kaccy Protocol API".to_string(),
                dependencies: vec![
                    "reqwest".to_string(),
                    "serde".to_string(),
                    "serde_json".to_string(),
                    "tokio".to_string(),
                ],
            },
        }
    }

    fn generate_lib() -> String {
        r#"//! Kaccy Protocol Rust SDK
//!
//! Official Rust client library for the Kaccy Protocol API

pub mod client;
pub mod error;
pub mod types;

pub use client::KaccyClient;
pub use error::{Error, Result};
pub use types::*;
"#
        .to_string()
    }

    fn generate_client(config: &SdkConfig) -> String {
        let base_url = &config.base_url;
        format!(
            r#"//! Kaccy API Client

use crate::error::{{Error, Result}};
use crate::types::*;
use reqwest::Client;
use serde::{{Deserialize, Serialize}};

/// Kaccy API client
pub struct KaccyClient {{
    client: Client,
    base_url: String,
    token: Option<String>,
}}

impl KaccyClient {{
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {{
        Self {{
            client: Client::new(),
            base_url: base_url.into(),
            token: None,
        }}
    }}

    /// Create a new client with default base URL
    pub fn default_client() -> Self {{
        Self::new("{base_url}")
    }}

    /// Set authentication token
    pub fn set_token(&mut self, token: String) {{
        self.token = Some(token);
    }}

    /// Register a new user
    pub async fn register(
        &mut self,
        email: impl Into<String>,
        password: impl Into<String>,
        username: impl Into<String>,
    ) -> Result<AuthResponse> {{
        let request = RegisterRequest {{
            email: email.into(),
            password: password.into(),
            username: username.into(),
        }};

        let response: AuthResponse = self
            .client
            .post(format!("{{}}/api/auth/register", self.base_url))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        self.token = Some(response.token.clone());
        Ok(response)
    }}

    /// Login with email and password
    pub async fn login(
        &mut self,
        email: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<AuthResponse> {{
        let request = LoginRequest {{
            email: email.into(),
            password: password.into(),
        }};

        let response: AuthResponse = self
            .client
            .post(format!("{{}}/api/auth/login", self.base_url))
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        self.token = Some(response.token.clone());
        Ok(response)
    }}

    /// Get current user profile
    pub async fn get_profile(&self) -> Result<User> {{
        self.authenticated_request("GET", "/api/users/me", None::<&()>)
            .await
    }}

    /// Update user profile
    pub async fn update_profile(&self, updates: UpdateProfileRequest) -> Result<User> {{
        self.authenticated_request("PUT", "/api/users/me", Some(&updates))
            .await
    }}

    /// List tokens
    pub async fn list_tokens(&self, page: i32, per_page: i32) -> Result<TokenListResponse> {{
        let url = format!(
            "{{}}/api/tokens?page={{}}&per_page={{}}",
            self.base_url, page, per_page
        );

        self.client.get(&url).send().await?.json().await.map_err(Into::into)
    }}

    /// Get token details
    pub async fn get_token(&self, token_id: &str) -> Result<Token> {{
        let url = format!("{{}}/api/tokens/{{}}", self.base_url, token_id);
        self.client.get(&url).send().await?.json().await.map_err(Into::into)
    }}

    /// Create a new token
    pub async fn create_token(&self, request: CreateTokenRequest) -> Result<Token> {{
        self.authenticated_request("POST", "/api/tokens", Some(&request))
            .await
    }}

    /// Place a buy order
    pub async fn buy_tokens(&self, request: BuyOrderRequest) -> Result<Order> {{
        self.authenticated_request("POST", "/api/orders/buy", Some(&request))
            .await
    }}

    /// Place a sell order
    pub async fn sell_tokens(&self, request: SellOrderRequest) -> Result<Order> {{
        self.authenticated_request("POST", "/api/orders/sell", Some(&request))
            .await
    }}

    /// Get user's portfolio
    pub async fn get_portfolio(&self) -> Result<serde_json::Value> {{
        self.authenticated_request("GET", "/api/portfolio", None::<&()>)
            .await
    }}

    /// Make an authenticated request
    async fn authenticated_request<T, B>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        B: Serialize,
    {{
        let token = self.token.as_ref().ok_or(Error::NotAuthenticated)?;
        let url = format!("{{}}{{}}", self.base_url, path);

        let mut request = match method {{
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            _ => return Err(Error::InvalidMethod),
        }};

        request = request.header("Authorization", format!("Bearer {{}}", token));

        if let Some(body) = body {{
            request = request.json(body);
        }}

        let response = request.send().await?;

        if !response.status().is_success() {{
            return Err(Error::ApiError(response.status().as_u16()));
        }}

        response.json().await.map_err(Into::into)
    }}
}}
"#,
            base_url = base_url
        )
    }

    fn generate_types() -> String {
        r#"//! Type definitions

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub reputation_score: f64,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub token_id: String,
    pub issuer_id: String,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub current_supply: f64,
    pub current_price_btc: f64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub user_id: String,
    pub token_id: String,
    pub order_type: String,
    pub amount: f64,
    pub price_btc: f64,
    pub status: String,
    pub btc_address: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub base_price_btc: f64,
    pub price_increment: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyOrderRequest {
    pub token_id: String,
    pub amount: f64,
    pub max_price_btc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellOrderRequest {
    pub token_id: String,
    pub amount: f64,
    pub min_price_btc: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenListResponse {
    pub tokens: Vec<Token>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}
"#
        .to_string()
    }

    fn generate_error() -> String {
        r#"//! Error types

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// SDK error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Invalid HTTP method")]
    InvalidMethod,

    #[error("API error: {0}")]
    ApiError(u16),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
"#
        .to_string()
    }

    fn generate_cargo_toml(config: &SdkConfig) -> String {
        format!(
            r#"[package]
name = "{}"
version = "{}"
edition = "2021"
description = "Rust SDK for Kaccy Protocol API"
license = "MIT"
repository = "https://github.com/kaccy/kaccy-rust"
keywords = ["kaccy", "bitcoin", "tokens", "api", "sdk"]

[dependencies]
reqwest = {{ version = "0.12", features = ["json"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tokio = {{ version = "1.0", features = ["full"] }}
thiserror = "2.0"

[dev-dependencies]
tokio-test = "0.4"
"#,
            config.package_name, config.version
        )
    }

    fn generate_readme(config: &SdkConfig) -> String {
        format!(
            r#"# {} - Kaccy Protocol Rust SDK

Official Rust client library for the Kaccy Protocol API.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
{} = "{}"
```

## Quick Start

```rust
use kaccy::KaccyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    // Initialize client
    let mut client = KaccyClient::default();

    // Register a new user
    let auth = client.register(
        "user@example.com",
        "SecurePass123!",
        "alice"
    ).await?;

    println!("Authenticated as: {{}}", auth.user.username);

    // Create a token
    let token = client.create_token(CreateTokenRequest {{
        symbol: "ALICE".to_string(),
        name: "Alice Token".to_string(),
        description: Some("Support Alice's creative work".to_string()),
        base_price_btc: 0.000001,
        price_increment: 0.0000001,
    }}).await?;

    // Buy tokens
    let order = client.buy_tokens(BuyOrderRequest {{
        token_id: token.token_id.clone(),
        amount: 100.0,
        max_price_btc: Some(0.001),
    }}).await?;

    // Get portfolio
    let portfolio = client.get_portfolio().await?;

    Ok(())
}}
```

## API Reference

### Authentication

- `register()` - Register a new user
- `login()` - Login with email/password
- `set_token()` - Set authentication token

### Users

- `get_profile()` - Get current user profile
- `update_profile()` - Update user profile

### Tokens

- `list_tokens()` - List all tokens
- `get_token()` - Get token details
- `create_token()` - Create a new token

### Orders

- `buy_tokens()` - Place buy order
- `sell_tokens()` - Place sell order

### Portfolio

- `get_portfolio()` - Get user's portfolio

## Error Handling

```rust
match client.create_token(request).await {{
    Ok(token) => println!("Created: {{}}", token.symbol),
    Err(e) => eprintln!("Error: {{}}", e),
}}
```

## License

MIT
"#,
            config.package_name, config.package_name, config.version
        )
    }
}
