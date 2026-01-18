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
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
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

#[derive(Debug, Serialize)]
pub struct PortfolioItemResponse {
    pub token_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub amount: Decimal,
    pub current_price_btc: Decimal,
    pub value_btc: Decimal,
}

#[derive(Debug, Serialize)]
pub struct Portfolio {
    pub total_value_btc: Decimal,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserTokenSummary {
    pub token_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub circulating_supply: Decimal,
    pub total_supply: Decimal,
    pub initial_price_btc: Decimal,
    pub price_increment_btc: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub holder_count: i64,
    pub total_volume_btc: Decimal,
}

/// Get user's trade history
#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TradeHistoryItem {
    pub trade_id: Uuid,
    pub token_id: Uuid,
    pub token_symbol: String,
    pub trade_type: String,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub platform_fee_btc: Decimal,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}
