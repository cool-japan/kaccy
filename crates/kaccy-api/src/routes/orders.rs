//! Order routes

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use kaccy_core::models::{
    BuyOrderResponse, CreateBuyOrderRequest, CreateSellOrderRequest, Order, SellOrderResponse,
    Token,
};
use kaccy_core::pricing::LinearBondingCurve;

use crate::error::{ApiError, Result};
use crate::middleware::metrics::ORDERS_CREATED_TOTAL;
use crate::state::AppState;

/// Create a buy order
pub async fn create_buy_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateBuyOrderRequest>,
) -> Result<Json<BuyOrderResponse>> {
    // Validate input
    if payload.amount <= Decimal::ZERO {
        return Err(ApiError::Validation("Amount must be positive".to_string()));
    }

    // Get token
    let token: Token =
        sqlx::query_as(r#"SELECT * FROM tokens WHERE token_id = $1 AND status = 'active'"#)
            .bind(payload.token_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("Token not found or not active".to_string()))?;

    // Check available supply
    let remaining_supply = token.total_supply - token.circulating_supply;
    if payload.amount > remaining_supply {
        return Err(ApiError::Validation(format!(
            "Insufficient supply. Available: {}",
            remaining_supply
        )));
    }

    // Calculate price using bonding curve
    let curve = LinearBondingCurve::new(token.initial_price_btc, token.price_increment_btc);
    let quote = curve.get_buy_quote(token.circulating_supply, payload.amount);

    // Check slippage if max_price specified
    if let Some(max_price) = payload.max_price_btc {
        if quote.avg_price_btc > max_price {
            return Err(ApiError::Validation(format!(
                "Price exceeds max. Avg: {}, Max: {}",
                quote.avg_price_btc, max_price
            )));
        }
    }

    // Generate BTC address for payment (placeholder - would integrate with kaccy-bitcoin)
    let btc_address = format!("bc1q_order_{}", Uuid::new_v4());

    // Create order
    let order: Order = sqlx::query_as(
        r#"
        INSERT INTO orders (user_id, token_id, order_type, amount, price_btc, total_btc, btc_address)
        VALUES ($1, $2, 'buy', $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(payload.token_id)
    .bind(payload.amount)
    .bind(quote.avg_price_btc)
    .bind(quote.total_btc)
    .bind(&btc_address)
    .fetch_one(&state.pool)
    .await?;

    let expires_at = Utc::now() + Duration::hours(1);

    // Update metrics
    ORDERS_CREATED_TOTAL.with_label_values(&["buy"]).inc();

    tracing::info!(
        order_id = %order.order_id,
        user_id = %user_id,
        token_id = %payload.token_id,
        amount = %payload.amount,
        total_btc = %quote.total_btc,
        "Buy order created"
    );

    Ok(Json(BuyOrderResponse {
        order_id: order.order_id,
        btc_address,
        total_btc: quote.total_btc,
        price_per_token: quote.avg_price_btc,
        price_impact_percent: quote.price_impact_percent,
        expires_at,
    }))
}

/// Create a sell order
pub async fn create_sell_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(payload): Json<CreateSellOrderRequest>,
) -> Result<Json<SellOrderResponse>> {
    // Validate input
    if payload.amount <= Decimal::ZERO {
        return Err(ApiError::Validation("Amount must be positive".to_string()));
    }

    let mut tx = state.pool.begin().await?;

    // Get token
    let token: Token = sqlx::query_as(
        r#"SELECT * FROM tokens WHERE token_id = $1 AND status = 'active' FOR UPDATE"#,
    )
    .bind(payload.token_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Token not found or not active".to_string()))?;

    // Check user balance
    let (balance, locked): (Decimal, Decimal) = sqlx::query_as(
        r#"
        SELECT amount, locked_amount
        FROM balances
        WHERE user_id = $1 AND token_id = $2
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(payload.token_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or((Decimal::ZERO, Decimal::ZERO));

    let available = balance - locked;
    if available < payload.amount {
        return Err(ApiError::Validation(format!(
            "Insufficient balance. Available: {}",
            available
        )));
    }

    // Calculate sell price using bonding curve
    let curve = LinearBondingCurve::new(token.initial_price_btc, token.price_increment_btc);
    let quote = curve.get_sell_quote(token.circulating_supply, payload.amount);

    // Check slippage if min_price specified
    if let Some(min_price) = payload.min_price_btc {
        if quote.avg_price_btc < min_price {
            return Err(ApiError::Validation(format!(
                "Price below min. Avg: {}, Min: {}",
                quote.avg_price_btc, min_price
            )));
        }
    }

    // Lock the tokens
    sqlx::query(
        r#"
        UPDATE balances
        SET locked_amount = locked_amount + $3, updated_at = NOW()
        WHERE user_id = $1 AND token_id = $2
        "#,
    )
    .bind(user_id)
    .bind(payload.token_id)
    .bind(payload.amount)
    .execute(&mut *tx)
    .await?;

    // Create sell order
    let order: Order = sqlx::query_as(
        r#"
        INSERT INTO orders (user_id, token_id, order_type, amount, price_btc, total_btc)
        VALUES ($1, $2, 'sell', $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(user_id)
    .bind(payload.token_id)
    .bind(payload.amount)
    .bind(quote.avg_price_btc)
    .bind(quote.total_btc)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    // Update metrics
    ORDERS_CREATED_TOTAL.with_label_values(&["sell"]).inc();

    tracing::info!(
        order_id = %order.order_id,
        user_id = %user_id,
        token_id = %payload.token_id,
        amount = %payload.amount,
        total_btc = %quote.total_btc,
        "Sell order created"
    );

    Ok(Json(SellOrderResponse {
        order_id: order.order_id,
        total_btc: quote.total_btc,
        price_per_token: quote.avg_price_btc,
        price_impact_percent: quote.price_impact_percent,
    }))
}

/// Get user's orders
pub async fn get_my_orders(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<Vec<Order>>> {
    let orders = sqlx::query_as::<_, Order>(
        r#"
        SELECT * FROM orders
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(orders))
}

/// Get a specific order
pub async fn get_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<Order>> {
    let order =
        sqlx::query_as::<_, Order>(r#"SELECT * FROM orders WHERE order_id = $1 AND user_id = $2"#)
            .bind(order_id)
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::NotFound("Order not found".to_string()))?;

    Ok(Json(order))
}

/// Cancel an order
pub async fn cancel_order(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let mut tx = state.pool.begin().await?;

    // Get the order
    let order: Order = sqlx::query_as(
        r#"
        SELECT * FROM orders
        WHERE order_id = $1 AND user_id = $2 AND status = 'pending'
        FOR UPDATE
        "#,
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("Order not found or not cancelable".to_string()))?;

    // If it's a sell order, unlock the tokens
    if order.order_type.to_string() == "sell" {
        sqlx::query(
            r#"
            UPDATE balances
            SET locked_amount = locked_amount - $3, updated_at = NOW()
            WHERE user_id = $1 AND token_id = $2
            "#,
        )
        .bind(user_id)
        .bind(order.token_id)
        .bind(order.amount)
        .execute(&mut *tx)
        .await?;
    }

    // Update order status
    sqlx::query(
        r#"
        UPDATE orders
        SET status = 'cancelled', completed_at = NOW()
        WHERE order_id = $1
        "#,
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    tracing::info!(order_id = %order_id, "Order cancelled");

    Ok(Json(serde_json::json!({ "success": true })))
}
