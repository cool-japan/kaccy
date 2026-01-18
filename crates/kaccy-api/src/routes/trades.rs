//! Trade routes

use axum::{
    Json,
    extract::{Path, Query, State},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Get trades for a specific token
pub async fn get_token_trades(
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
    Query(query): Query<TradesQuery>,
) -> Result<Json<TokenTradesResponse>> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    // Verify token exists
    let exists: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT token_id FROM tokens WHERE token_id = $1"#)
            .bind(token_id)
            .fetch_optional(&state.pool)
            .await?;

    if exists.is_none() {
        return Err(ApiError::NotFound("Token not found".to_string()));
    }

    // Get trades
    let trades = sqlx::query_as::<_, TokenTrade>(
        r#"
        SELECT
            tr.trade_id,
            tr.buyer_user_id,
            buyer.username as buyer_username,
            tr.seller_user_id,
            seller.username as seller_username,
            tr.amount,
            tr.price_btc,
            tr.total_btc,
            tr.executed_at
        FROM trades tr
        JOIN users buyer ON tr.buyer_user_id = buyer.user_id
        LEFT JOIN users seller ON tr.seller_user_id = seller.user_id
        WHERE tr.token_id = $1
        ORDER BY tr.executed_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(token_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    // Get summary stats
    let summary: TradeSummary = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total_trades,
            COALESCE(SUM(total_btc), 0) as total_volume_btc,
            COALESCE(AVG(price_btc), 0) as avg_price_btc,
            COALESCE(MAX(price_btc), 0) as high_price_btc,
            COALESCE(MIN(price_btc), 0) as low_price_btc
        FROM trades
        WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .fetch_one(&state.pool)
    .await?;

    // Get 24h stats
    let volume_24h: (Decimal,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(total_btc), 0)
        FROM trades
        WHERE token_id = $1
        AND executed_at > NOW() - INTERVAL '24 hours'
        "#,
    )
    .bind(token_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(TokenTradesResponse {
        trades,
        summary,
        volume_24h_btc: volume_24h.0,
    }))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TokenTrade {
    pub trade_id: Uuid,
    pub buyer_user_id: Uuid,
    pub buyer_username: String,
    pub seller_user_id: Option<Uuid>,
    pub seller_username: Option<String>,
    pub amount: Decimal,
    pub price_btc: Decimal,
    pub total_btc: Decimal,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TradeSummary {
    pub total_trades: i64,
    pub total_volume_btc: Decimal,
    pub avg_price_btc: Decimal,
    pub high_price_btc: Decimal,
    pub low_price_btc: Decimal,
}

#[derive(Debug, Serialize)]
pub struct TokenTradesResponse {
    pub trades: Vec<TokenTrade>,
    pub summary: TradeSummary,
    pub volume_24h_btc: Decimal,
}

/// Get token holders
pub async fn get_token_holders(
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
    Query(query): Query<TradesQuery>,
) -> Result<Json<TokenHoldersResponse>> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    // Verify token exists
    let token: Option<TokenInfo> =
        sqlx::query_as(r#"SELECT token_id, circulating_supply FROM tokens WHERE token_id = $1"#)
            .bind(token_id)
            .fetch_optional(&state.pool)
            .await?;

    let token = token.ok_or_else(|| ApiError::NotFound("Token not found".to_string()))?;

    // Get holders
    let holders = sqlx::query_as::<_, TokenHolder>(
        r#"
        SELECT
            b.user_id,
            u.username,
            u.display_name,
            b.amount,
            (b.amount / NULLIF($2, 0) * 100) as percentage
        FROM balances b
        JOIN users u ON b.user_id = u.user_id
        WHERE b.token_id = $1 AND b.amount > 0
        ORDER BY b.amount DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(token_id)
    .bind(token.circulating_supply)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    // Get holder count
    let (holder_count,): (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM balances WHERE token_id = $1 AND amount > 0"#)
            .bind(token_id)
            .fetch_one(&state.pool)
            .await?;

    Ok(Json(TokenHoldersResponse {
        holders,
        total_holders: holder_count,
        circulating_supply: token.circulating_supply,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct TokenInfo {
    #[allow(dead_code)]
    token_id: Uuid,
    circulating_supply: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TokenHolder {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub amount: Decimal,
    pub percentage: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct TokenHoldersResponse {
    pub holders: Vec<TokenHolder>,
    pub total_holders: i64,
    pub circulating_supply: Decimal,
}
