//! OpenAPI/Swagger documentation for Kaccy Protocol API
//!
//! This module generates OpenAPI 3.1 specification from route handlers.

use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

/// Main API documentation struct
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Kaccy Protocol API",
        version = "1.0.0",
        description = "REST API for Kaccy Protocol - Bitcoin-native creator token platform",
        contact(
            name = "COOLJAPAN OU (Team KitaSan)",
            url = "https://kaccy.io",
            email = "support@kaccy.io"
        ),
        license(
            name = "Proprietary",
            url = "https://kaccy.io/license"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development"),
        (url = "https://api.kaccy.io", description = "Production")
    ),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "users", description = "User management"),
        (name = "tokens", description = "Token operations"),
        (name = "orders", description = "Order management"),
        (name = "trades", description = "Trade history"),
        (name = "commitments", description = "Output commitments"),
        (name = "admin", description = "Admin operations"),
        (name = "health", description = "Health check endpoints")
    ),
    components(
        schemas(
            // Auth schemas
            RegisterRequestSchema,
            LoginRequestSchema,
            AuthResponseSchema,
            // User schemas
            UserPublicSchema,
            UserProfileSchema,
            UpdateProfileRequestSchema,
            // Token schemas
            TokenSchema,
            TokenListSchema,
            CreateTokenRequestSchema,
            TokenDetailsSchema,
            // Order schemas
            BuyOrderRequestSchema,
            SellOrderRequestSchema,
            OrderSchema,
            OrderListSchema,
            // Trade schemas
            TradeSchema,
            TradeListSchema,
            TradeSummarySchema,
            // Commitment schemas
            CommitmentSchema,
            CreateCommitmentRequestSchema,
            SubmitEvidenceRequestSchema,
            // Admin schemas
            AdminStatsSchema,
            VerifyCommitmentRequestSchema,
            // Common schemas
            PaginationSchema,
            ErrorResponseSchema,
            // Health schemas
            HealthResponseSchema,
            DetailedHealthSchema
        )
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

/// Add security scheme to OpenAPI spec
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("JWT token from /api/auth/login or /api/auth/register"))
                    .build(),
            ),
        );
    }
}

// ============== Auth Schemas ==============

/// User registration request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "email": "user@example.com",
    "password": "SecurePass123!",
    "username": "creator_alice"
}))]
pub struct RegisterRequestSchema {
    /// User's email address
    pub email: String,
    /// Password (min 8 characters)
    pub password: String,
    /// Unique username
    pub username: String,
}

/// User login request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "email": "user@example.com",
    "password": "SecurePass123!"
}))]
pub struct LoginRequestSchema {
    /// User's email address
    pub email: String,
    /// User's password
    pub password: String,
}

/// Authentication response with JWT token
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "user": {
        "user_id": "550e8400-e29b-41d4-a716-446655440000",
        "username": "creator_alice",
        "email": "user@example.com",
        "reputation_score": 500.00
    }
}))]
pub struct AuthResponseSchema {
    /// JWT token for authentication
    pub token: String,
    /// User information
    pub user: UserPublicSchema,
}

// ============== User Schemas ==============

/// Public user information
#[derive(utoipa::ToSchema)]
pub struct UserPublicSchema {
    /// Unique user identifier
    pub user_id: String,
    /// Username
    pub username: String,
    /// Display name (optional)
    pub display_name: Option<String>,
    /// Profile bio (optional)
    pub bio: Option<String>,
    /// Avatar URL (optional)
    pub avatar_url: Option<String>,
    /// Reputation score (0-1000)
    pub reputation_score: f64,
    /// Number of tokens issued
    pub tokens_issued: i32,
}

/// User profile with sensitive data (for authenticated user)
#[derive(utoipa::ToSchema)]
pub struct UserProfileSchema {
    /// Unique user identifier
    pub user_id: String,
    /// Username
    pub username: String,
    /// Email address
    pub email: String,
    /// Display name
    pub display_name: Option<String>,
    /// Profile bio
    pub bio: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Reputation score
    pub reputation_score: f64,
    /// User role (user, admin)
    pub role: String,
    /// Account creation time
    pub created_at: String,
}

/// Profile update request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "display_name": "Alice Creator",
    "bio": "Building amazing things on Bitcoin",
    "avatar_url": "https://example.com/avatar.jpg"
}))]
pub struct UpdateProfileRequestSchema {
    /// New display name
    pub display_name: Option<String>,
    /// New bio
    pub bio: Option<String>,
    /// New avatar URL
    pub avatar_url: Option<String>,
}

// ============== Token Schemas ==============

/// Token information
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "issuer_id": "550e8400-e29b-41d4-a716-446655440000",
    "symbol": "ALICE",
    "name": "Alice Token",
    "description": "Support Alice's creative work",
    "current_supply": 1000.0,
    "current_price_btc": 0.00001,
    "status": "active",
    "created_at": "2024-01-15T10:00:00Z"
}))]
pub struct TokenSchema {
    /// Unique token ID
    pub token_id: String,
    /// User ID of the token issuer
    pub issuer_id: String,
    /// Token symbol
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Token description
    pub description: Option<String>,
    /// Current circulating supply
    pub current_supply: f64,
    /// Current price in BTC
    pub current_price_btc: f64,
    /// Token status
    pub status: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
}

/// Paginated token list
#[derive(utoipa::ToSchema)]
pub struct TokenListSchema {
    /// List of tokens
    pub tokens: Vec<TokenSchema>,
    /// Total number of tokens
    pub total: i64,
    /// Current page number
    pub page: i32,
    /// Number of items per page
    pub per_page: i32,
}

/// Token creation request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "symbol": "ALICE",
    "name": "Alice Token",
    "description": "Support Alice's creative work",
    "base_price_btc": 0.000001,
    "price_increment": 0.0000001
}))]
pub struct CreateTokenRequestSchema {
    /// Token symbol (3-10 chars, uppercase)
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Token description
    pub description: Option<String>,
    /// Base price in BTC
    pub base_price_btc: f64,
    /// Price increment per token
    pub price_increment: f64,
}

/// Detailed token information with issuer
#[derive(utoipa::ToSchema)]
pub struct TokenDetailsSchema {
    /// Token information
    pub token: TokenSchema,
    /// Issuer information
    pub issuer: UserPublicSchema,
    /// 24h trading volume in BTC
    pub volume_24h_btc: f64,
    /// Number of holders
    pub holder_count: i64,
}

// ============== Order Schemas ==============

/// Buy order request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount": 100.0,
    "max_price_btc": 0.001
}))]
pub struct BuyOrderRequestSchema {
    /// Token to buy
    pub token_id: String,
    /// Amount of tokens to buy
    pub amount: f64,
    /// Maximum total price in BTC (slippage protection)
    pub max_price_btc: Option<f64>,
}

/// Sell order request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount": 50.0,
    "min_price_btc": 0.0005
}))]
pub struct SellOrderRequestSchema {
    /// Token to sell
    pub token_id: String,
    /// Amount of tokens to sell
    pub amount: f64,
    /// Minimum total price in BTC (slippage protection)
    pub min_price_btc: Option<f64>,
}

/// Order information
#[derive(utoipa::ToSchema)]
pub struct OrderSchema {
    /// Unique order ID
    pub order_id: String,
    /// ID of the user who placed the order
    pub user_id: String,
    /// ID of the token being ordered
    pub token_id: String,
    /// Order type (buy/sell)
    pub order_type: String,
    /// Amount of tokens
    pub amount: f64,
    /// Price in BTC
    pub price_btc: f64,
    /// Status (pending, completed, cancelled, expired)
    pub status: String,
    /// BTC payment address (for buy orders)
    pub btc_address: Option<String>,
    /// Order creation timestamp (ISO 8601)
    pub created_at: String,
}

/// Paginated order list
#[derive(utoipa::ToSchema)]
pub struct OrderListSchema {
    /// List of orders
    pub orders: Vec<OrderSchema>,
    /// Total number of orders
    pub total: i64,
}

// ============== Trade Schemas ==============

/// Trade record
#[derive(utoipa::ToSchema)]
pub struct TradeSchema {
    /// Trade ID
    pub trade_id: String,
    /// Token ID
    pub token_id: String,
    /// Buyer ID
    pub buyer_id: String,
    /// Seller ID (bonding curve for primary market)
    pub seller_id: Option<String>,
    /// Amount traded
    pub amount: f64,
    /// Price per token in BTC
    pub price_btc: f64,
    /// Total value in BTC
    pub total_btc: f64,
    /// Platform fee
    pub platform_fee_btc: f64,
    /// Issuer royalty
    pub issuer_royalty_btc: f64,
    /// Trade time
    pub executed_at: String,
}

/// Paginated trade list
#[derive(utoipa::ToSchema)]
pub struct TradeListSchema {
    /// List of trades
    pub trades: Vec<TradeSchema>,
    /// Total number of trades
    pub total: i64,
}

/// Trade summary for a token
#[derive(utoipa::ToSchema)]
pub struct TradeSummarySchema {
    /// Total volume in BTC
    pub total_volume_btc: f64,
    /// 24h volume in BTC
    pub volume_24h_btc: f64,
    /// Average price in BTC
    pub avg_price_btc: f64,
    /// Total trades
    pub total_trades: i64,
}

// ============== Commitment Schemas ==============

/// Output commitment
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "commitment_id": "550e8400-e29b-41d4-a716-446655440002",
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "title": "Weekly Newsletter",
    "description": "Publish a weekly newsletter for token holders",
    "deadline": "2024-02-01T00:00:00Z",
    "status": "pending",
    "evidence_url": null,
    "created_at": "2024-01-15T10:00:00Z"
}))]
pub struct CommitmentSchema {
    /// Unique commitment ID
    pub commitment_id: String,
    /// ID of the user who created the commitment
    pub user_id: String,
    /// ID of the associated token
    pub token_id: String,
    /// Commitment title
    pub title: String,
    /// Detailed description
    pub description: Option<String>,
    /// Commitment deadline (ISO 8601)
    pub deadline: String,
    /// Current status (pending, completed, verified, failed)
    pub status: String,
    /// HTTPS URL to submitted evidence
    pub evidence_url: Option<String>,
    /// Description of submitted evidence
    pub evidence_description: Option<String>,
    /// Timestamp when commitment was verified (ISO 8601)
    pub verified_at: Option<String>,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
}

/// Create commitment request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "token_id": "550e8400-e29b-41d4-a716-446655440001",
    "title": "Weekly Newsletter",
    "description": "Publish a weekly newsletter for token holders",
    "deadline": "2024-02-01T00:00:00Z"
}))]
pub struct CreateCommitmentRequestSchema {
    /// Token ID
    pub token_id: String,
    /// Commitment title
    pub title: String,
    /// Detailed description
    pub description: Option<String>,
    /// Deadline (ISO 8601)
    pub deadline: String,
}

/// Submit evidence request
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "evidence_url": "https://example.com/newsletter-issue-1",
    "evidence_description": "Published the first newsletter issue"
}))]
pub struct SubmitEvidenceRequestSchema {
    /// HTTPS URL to evidence
    pub evidence_url: String,
    /// Optional description
    pub evidence_description: Option<String>,
}

// ============== Admin Schemas ==============

/// Admin statistics
#[derive(utoipa::ToSchema)]
pub struct AdminStatsSchema {
    /// Total registered users
    pub total_users: i64,
    /// Total tokens created
    pub total_tokens: i64,
    /// Total trades executed
    pub total_trades: i64,
    /// Total volume in BTC
    pub total_volume_btc: f64,
    /// Pending commitments awaiting verification
    pub pending_commitments: i64,
    /// Platform fees collected
    pub total_platform_fees_btc: f64,
}

/// Verify commitment request (admin)
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "fulfilled": true,
    "quality_score": 85,
    "notes": "Great quality newsletter with valuable content"
}))]
pub struct VerifyCommitmentRequestSchema {
    /// Whether commitment was fulfilled
    pub fulfilled: bool,
    /// Quality score (0-100)
    pub quality_score: Option<i32>,
    /// Admin notes
    pub notes: Option<String>,
}

// ============== Common Schemas ==============

/// Pagination parameters
#[derive(utoipa::ToSchema)]
pub struct PaginationSchema {
    /// Page number (1-indexed)
    pub page: i32,
    /// Items per page (max 100)
    pub per_page: i32,
}

/// Standard error response
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "error": "validation_error",
    "message": "Email is required",
    "details": null
}))]
pub struct ErrorResponseSchema {
    /// Error type
    pub error: String,
    /// Human-readable message
    pub message: String,
    /// Additional details
    pub details: Option<serde_json::Value>,
}

// ============== Health Schemas ==============

/// Basic health check response
#[derive(utoipa::ToSchema)]
#[schema(example = json!({
    "status": "healthy"
}))]
pub struct HealthResponseSchema {
    /// Health status
    pub status: String,
}

/// Detailed health check response
#[derive(utoipa::ToSchema)]
pub struct DetailedHealthSchema {
    /// Overall status
    pub status: String,
    /// Database connection status
    pub database: String,
    /// Database latency in ms
    pub db_latency_ms: i64,
    /// Active DB connections
    pub db_connections_active: u32,
    /// Idle DB connections
    pub db_connections_idle: u32,
}

/// Get the OpenAPI JSON
#[allow(dead_code)]
pub fn openapi_json() -> String {
    ApiDoc::openapi()
        .to_json()
        .expect("OpenAPI JSON generation failed")
}
