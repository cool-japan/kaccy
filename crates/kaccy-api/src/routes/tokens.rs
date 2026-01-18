//! Token routes

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use kaccy_core::models::{
    CreateTokenRequest, SortOrder, Token, TokenListQuery, TokenSortBy, TokenWithIssuer,
};
use kaccy_core::pricing::{BondingCurve, LinearBondingCurve};

use crate::error::{ApiError, Result};
use crate::middleware::metrics::TOKENS_CREATED_TOTAL;
use crate::state::AppState;

/// Response for paginated token list
#[derive(Debug, Serialize)]
pub struct TokenListResponse {
    pub tokens: Vec<TokenWithIssuer>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub total_pages: u32,
}

/// Token row with joined issuer data
#[derive(Debug, sqlx::FromRow)]
struct TokenWithIssuerRow {
    // Token fields
    pub token_id: Uuid,
    pub issuer_user_id: Uuid,
    pub symbol: String,
    pub name: String,
    pub description: Option<String>,
    pub total_supply: Decimal,
    pub circulating_supply: Decimal,
    pub initial_price_btc: Decimal,
    pub price_increment_btc: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: kaccy_core::models::TokenStatus,
    // Issuer fields
    pub issuer_username: String,
    pub issuer_display_name: Option<String>,
    pub issuer_reputation_score: Decimal,
}

pub async fn list_tokens(
    State(state): State<AppState>,
    Query(query): Query<TokenListQuery>,
) -> Result<Json<TokenListResponse>> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = (page - 1) * limit;
    let sort_by = query.sort_by.unwrap_or_default();
    let order = query.order.unwrap_or_default();

    // Build dynamic ORDER BY clause
    let order_column = match sort_by {
        TokenSortBy::CreatedAt => "t.created_at",
        TokenSortBy::Symbol => "t.symbol",
        TokenSortBy::Name => "t.name",
        TokenSortBy::TotalSupply => "t.total_supply",
        TokenSortBy::CirculatingSupply => "t.circulating_supply",
        TokenSortBy::InitialPrice => "t.initial_price_btc",
    };
    let order_dir = match order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };

    // Build WHERE conditions
    let status_filter = query
        .status
        .map(|s| s.to_string())
        .unwrap_or_else(|| "active".to_string());

    // Build dynamic query with all filters
    let sql = format!(
        r#"
        SELECT
            t.token_id, t.issuer_user_id, t.symbol, t.name, t.description,
            t.total_supply, t.circulating_supply, t.initial_price_btc,
            t.price_increment_btc, t.created_at, t.status,
            u.username AS issuer_username,
            u.display_name AS issuer_display_name,
            u.reputation_score AS issuer_reputation_score
        FROM tokens t
        JOIN users u ON t.issuer_user_id = u.user_id
        WHERE t.status = $1
        AND ($2::text IS NULL OR t.symbol ILIKE '%' || $2 || '%' OR t.name ILIKE '%' || $2 || '%')
        AND ($3::numeric IS NULL OR t.initial_price_btc >= $3)
        AND ($4::numeric IS NULL OR t.initial_price_btc <= $4)
        ORDER BY {} {}
        LIMIT $5 OFFSET $6
        "#,
        order_column, order_dir
    );

    let rows = sqlx::query_as::<_, TokenWithIssuerRow>(&sql)
        .bind(&status_filter)
        .bind(&query.search)
        .bind(query.min_price)
        .bind(query.max_price)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await?;

    // Get total count for pagination
    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM tokens t
        WHERE t.status = $1
        AND ($2::text IS NULL OR t.symbol ILIKE '%' || $2 || '%' OR t.name ILIKE '%' || $2 || '%')
        AND ($3::numeric IS NULL OR t.initial_price_btc >= $3)
        AND ($4::numeric IS NULL OR t.initial_price_btc <= $4)
        "#,
    )
    .bind(&status_filter)
    .bind(&query.search)
    .bind(query.min_price)
    .bind(query.max_price)
    .fetch_one(&state.pool)
    .await?;

    // Calculate current prices and build response
    let tokens: Vec<TokenWithIssuer> = rows
        .into_iter()
        .map(|row| {
            let curve = LinearBondingCurve::new(row.initial_price_btc, row.price_increment_btc);
            let current_price = curve.spot_price(row.circulating_supply);

            TokenWithIssuer {
                token: Token {
                    token_id: row.token_id,
                    issuer_user_id: row.issuer_user_id,
                    symbol: row.symbol,
                    name: row.name,
                    description: row.description,
                    total_supply: row.total_supply,
                    circulating_supply: row.circulating_supply,
                    initial_price_btc: row.initial_price_btc,
                    price_increment_btc: row.price_increment_btc,
                    created_at: row.created_at,
                    status: row.status,
                },
                issuer_username: row.issuer_username,
                issuer_display_name: row.issuer_display_name,
                issuer_reputation_score: row.issuer_reputation_score,
                current_price,
            }
        })
        .collect();

    let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;

    Ok(Json(TokenListResponse {
        tokens,
        total,
        page,
        limit,
        total_pages,
    }))
}

pub async fn create_token(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateTokenRequest>,
) -> Result<Json<Token>> {
    // Validate symbol format
    if !payload.symbol.starts_with('$') || payload.symbol.len() < 3 || payload.symbol.len() > 11 {
        return Err(ApiError::Validation(
            "Symbol must start with $ and be 2-10 characters".to_string(),
        ));
    }

    // Validate prices
    if payload.initial_price_btc <= Decimal::ZERO || payload.price_increment_btc <= Decimal::ZERO {
        return Err(ApiError::Validation("Prices must be positive".to_string()));
    }

    // Check user reputation (minimum 400 for token issuance)
    let (reputation_score,): (Decimal,) = sqlx::query_as(
        r#"
        SELECT reputation_score FROM users WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    if reputation_score < Decimal::from(400) {
        return Err(ApiError::Forbidden(
            "Minimum reputation score of 400 required to issue tokens".to_string(),
        ));
    }

    // Create token
    let token = sqlx::query_as::<_, Token>(
        r#"
        INSERT INTO tokens (issuer_user_id, symbol, name, description, total_supply, initial_price_btc, price_increment_btc)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(&payload.symbol)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(payload.total_supply)
    .bind(payload.initial_price_btc)
    .bind(payload.price_increment_btc)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return ApiError::Validation("Token symbol already exists".to_string());
            }
        }
        ApiError::Database(e)
    })?;

    // Update metrics
    TOKENS_CREATED_TOTAL.inc();

    tracing::info!(
        token_id = %token.token_id,
        symbol = %token.symbol,
        issuer = %user_id,
        "Token created"
    );

    Ok(Json(token))
}

pub async fn get_token(
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
) -> Result<Json<TokenWithIssuer>> {
    let token = sqlx::query_as::<_, Token>(
        r#"
        SELECT * FROM tokens WHERE token_id = $1
        "#,
    )
    .bind(token_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("Token not found".to_string()))?;

    // Get issuer info
    let (issuer_username, issuer_display_name, issuer_reputation_score): (
        String,
        Option<String>,
        Decimal,
    ) = sqlx::query_as(
        r#"
            SELECT username, display_name, reputation_score
            FROM users WHERE user_id = $1
            "#,
    )
    .bind(token.issuer_user_id)
    .fetch_one(&state.pool)
    .await?;

    // Calculate current price
    let curve = LinearBondingCurve::new(token.initial_price_btc, token.price_increment_btc);
    let current_price =
        kaccy_core::pricing::BondingCurve::spot_price(&curve, token.circulating_supply);

    Ok(Json(TokenWithIssuer {
        token,
        issuer_username,
        issuer_display_name,
        issuer_reputation_score,
        current_price,
    }))
}
