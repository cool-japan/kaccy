//! User routes

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kaccy_core::models::{UpdateUserRequest, UserPublic};

use crate::error::{ApiError, Result};
use crate::state::AppState;

/// Get current user's profile
pub async fn get_me(
    State(state): State<AppState>,
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
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(user))
}

/// Update current user's profile
pub async fn update_me(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<UserPublic>> {
    // Validate input
    if let Err(e) = payload.validate() {
        return Err(ApiError::Validation(e.to_string()));
    }

    let user = sqlx::query_as::<_, UserPublic>(
        r#"
        UPDATE users
        SET display_name = COALESCE($2, display_name),
            bio = COALESCE($3, bio),
            avatar_url = COALESCE($4, avatar_url),
            btc_withdrawal_address = COALESCE($5, btc_withdrawal_address)
        WHERE user_id = $1
        RETURNING user_id, email, username, display_name, bio, avatar_url,
                  created_at, kyc_status, reputation_score
        "#,
    )
    .bind(user_id)
    .bind(&payload.display_name)
    .bind(&payload.bio)
    .bind(&payload.avatar_url)
    .bind(&payload.btc_withdrawal_address)
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(user_id = %user_id, "User profile updated");
    Ok(Json(user))
}

/// Get a user's public profile by ID
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<PublicUserProfile>> {
    let user = sqlx::query_as::<_, PublicUserProfile>(
        r#"
        SELECT user_id, username, display_name, bio, avatar_url,
               created_at, reputation_score
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    Ok(Json(user))
}

/// Public user profile (without email)
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PublicUserProfile {
    /// Unique user ID
    pub user_id: Uuid,
    /// Username
    pub username: String,
    /// Display name
    pub display_name: Option<String>,
    /// Profile bio
    pub bio: Option<String>,
    /// Avatar image URL
    pub avatar_url: Option<String>,
    /// Account creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Reputation score (0-1000)
    pub reputation_score: Decimal,
}

/// Get user's portfolio
pub async fn get_portfolio(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Portfolio>> {
    let balances = sqlx::query_as::<_, PortfolioItem>(
        r#"
        SELECT
            b.token_id,
            b.amount,
            t.symbol,
            t.name,
            t.initial_price_btc,
            t.price_increment_btc,
            t.circulating_supply
        FROM balances b
        JOIN tokens t ON b.token_id = t.token_id
        WHERE b.user_id = $1 AND b.amount > 0
        ORDER BY b.amount DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    // Calculate current prices and total value
    let mut total_value_btc = Decimal::ZERO;
    let mut items: Vec<PortfolioItemResponse> = Vec::new();

    for balance in balances {
        let curve = kaccy_core::pricing::LinearBondingCurve::new(
            balance.initial_price_btc,
            balance.price_increment_btc,
        );
        let current_price =
            kaccy_core::pricing::BondingCurve::spot_price(&curve, balance.circulating_supply);
        let value = balance.amount * current_price;
        total_value_btc += value;

        items.push(PortfolioItemResponse {
            token_id: balance.token_id,
            symbol: balance.symbol,
            name: balance.name,
            amount: balance.amount,
            current_price_btc: current_price,
            value_btc: value,
        });
    }

    Ok(Json(Portfolio {
        total_value_btc,
        items,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct PortfolioItem {
    token_id: Uuid,
    amount: Decimal,
    symbol: String,
    name: String,
    initial_price_btc: Decimal,
    price_increment_btc: Decimal,
    circulating_supply: Decimal,
}

/// A single portfolio holding
#[derive(Debug, Serialize)]
pub struct PortfolioItemResponse {
    /// Token ID
    pub token_id: Uuid,
    /// Token symbol
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Amount of tokens held
    pub amount: Decimal,
    /// Current spot price in BTC
    pub current_price_btc: Decimal,
    /// Total value of holding in BTC
    pub value_btc: Decimal,
}

/// User's token portfolio
#[derive(Debug, Serialize)]
pub struct Portfolio {
    /// Total portfolio value in BTC
    pub total_value_btc: Decimal,
    /// Individual token holdings
    pub items: Vec<PortfolioItemResponse>,
}

/// Get tokens issued by a user
pub async fn get_user_tokens(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<UserTokenSummary>>> {
    let tokens = sqlx::query_as::<_, UserTokenSummary>(
        r#"
        SELECT
            t.token_id,
            t.symbol,
            t.name,
            t.circulating_supply,
            t.total_supply,
            t.initial_price_btc,
            t.price_increment_btc,
            t.created_at,
            t.status,
            (SELECT COUNT(*) FROM balances WHERE token_id = t.token_id AND amount > 0) as holder_count,
            (SELECT COALESCE(SUM(total_btc), 0) FROM trades WHERE token_id = t.token_id) as total_volume_btc
        FROM tokens t
        WHERE t.issuer_user_id = $1
        ORDER BY t.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(tokens))
}

/// Summary of a token issued by a user
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserTokenSummary {
    /// Unique token ID
    pub token_id: Uuid,
    /// Token symbol
    pub symbol: String,
    /// Token name
    pub name: String,
    /// Current circulating supply
    pub circulating_supply: Decimal,
    /// Maximum total supply
    pub total_supply: Decimal,
    /// Initial price in BTC
    pub initial_price_btc: Decimal,
    /// Price increment per token in BTC
    pub price_increment_btc: Decimal,
    /// Token creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Token status (active, paused, etc.)
    pub status: String,
    /// Number of unique token holders
    pub holder_count: i64,
    /// All-time total trading volume in BTC
    pub total_volume_btc: Decimal,
}

/// Query parameters for trade history
#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    /// Maximum number of trades to return
    pub limit: Option<i64>,
    /// Number of records to skip for pagination
    pub offset: Option<i64>,
}

/// Get the authenticated user's trade history
pub async fn get_my_trades(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    axum::extract::Query(query): axum::extract::Query<TradeHistoryQuery>,
) -> Result<Json<Vec<TradeHistoryItem>>> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let trades = sqlx::query_as::<_, TradeHistoryItem>(
        r#"
        SELECT
            tr.trade_id,
            tr.token_id,
            t.symbol as token_symbol,
            CASE WHEN tr.buyer_user_id = $1 THEN 'buy' ELSE 'sell' END as trade_type,
            tr.amount,
            tr.price_btc,
            tr.total_btc,
            tr.platform_fee_btc,
            tr.executed_at
        FROM trades tr
        JOIN tokens t ON tr.token_id = t.token_id
        WHERE tr.buyer_user_id = $1 OR tr.seller_user_id = $1
        ORDER BY tr.executed_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(trades))
}

/// A trade history entry for a user
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TradeHistoryItem {
    /// Unique trade ID
    pub trade_id: Uuid,
    /// Token ID that was traded
    pub token_id: Uuid,
    /// Symbol of the traded token
    pub token_symbol: String,
    /// Direction of the trade (buy or sell)
    pub trade_type: String,
    /// Amount of tokens traded
    pub amount: Decimal,
    /// Price per token in BTC
    pub price_btc: Decimal,
    /// Total trade value in BTC
    pub total_btc: Decimal,
    /// Platform fee paid in BTC
    pub platform_fee_btc: Decimal,
    /// Timestamp when trade was executed
    pub executed_at: chrono::DateTime<chrono::Utc>,
}
