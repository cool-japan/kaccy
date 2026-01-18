//! Trade execution logic

use crate::error::{CoreError, Result};
use crate::models::{Balance, Order, OrderType, Trade};
use crate::pricing::{FeeBreakdown, FeeSchedule};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use uuid::Uuid;

/// Trade executor handles atomic trade execution with ACID guarantees
pub struct TradeExecutor {
    pool: PgPool,
    fee_schedule: FeeSchedule,
}

impl TradeExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            fee_schedule: FeeSchedule::default(),
        }
    }

    pub fn with_fee_schedule(pool: PgPool, fee_schedule: FeeSchedule) -> Self {
        Self { pool, fee_schedule }
    }

    /// Calculate fees for a trade
    pub fn calculate_fees(&self, amount_btc: Decimal) -> FeeBreakdown {
        self.fee_schedule.calculate(amount_btc)
    }

    /// Execute a buy order after payment is confirmed
    pub async fn execute_buy_order(&self, order_id: Uuid) -> Result<Trade> {
        let mut tx = self.pool.begin().await?;

        // 1. Get and lock the order
        let order: Order = sqlx::query_as(
            r#"
            SELECT * FROM orders
            WHERE order_id = $1 AND status = 'pending'
            FOR UPDATE
            "#,
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CoreError::NotFound("Order not found".to_string()))?;

        if order.order_type != OrderType::Buy {
            return Err(CoreError::Validation(
                "Order is not a buy order".to_string(),
            ));
        }

        // 2. Get and lock the token, also get issuer_user_id for royalty
        let (circulating_supply, total_supply, issuer_user_id): (Decimal, Decimal, Uuid) =
            sqlx::query_as(
                r#"
            SELECT circulating_supply, total_supply, issuer_user_id FROM tokens
            WHERE token_id = $1
            FOR UPDATE
            "#,
            )
            .bind(order.token_id)
            .fetch_one(&mut *tx)
            .await?;

        // Check supply limit
        if circulating_supply + order.amount > total_supply {
            return Err(CoreError::Validation(
                "Would exceed total supply".to_string(),
            ));
        }

        // 3. Calculate fees
        let fees = self.calculate_fees(order.total_btc);

        // 4. Update token circulating supply
        sqlx::query(
            r#"
            UPDATE tokens
            SET circulating_supply = circulating_supply + $1
            WHERE token_id = $2
            "#,
        )
        .bind(order.amount)
        .bind(order.token_id)
        .execute(&mut *tx)
        .await?;

        // 5. Update or insert user balance
        sqlx::query(
            r#"
            INSERT INTO balances (balance_id, user_id, token_id, amount, locked_amount, updated_at)
            VALUES ($1, $2, $3, $4, 0, NOW())
            ON CONFLICT (user_id, token_id)
            DO UPDATE SET amount = balances.amount + $4, updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(order.user_id)
        .bind(order.token_id)
        .bind(order.amount)
        .execute(&mut *tx)
        .await?;

        // 6. Create trade record
        let trade_id = Uuid::new_v4();
        let trade = Trade {
            trade_id,
            buyer_user_id: order.user_id,
            seller_user_id: None, // Bonding curve buy
            token_id: order.token_id,
            amount: order.amount,
            price_btc: order.price_btc,
            total_btc: order.total_btc,
            platform_fee_btc: fees.platform_fee_btc,
            issuer_royalty_btc: fees.issuer_royalty_btc,
            executed_at: Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO trades (trade_id, buyer_user_id, seller_user_id, token_id, amount, price_btc, total_btc, platform_fee_btc, issuer_royalty_btc, executed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(trade.trade_id)
        .bind(trade.buyer_user_id)
        .bind(trade.seller_user_id)
        .bind(trade.token_id)
        .bind(trade.amount)
        .bind(trade.price_btc)
        .bind(trade.total_btc)
        .bind(trade.platform_fee_btc)
        .bind(trade.issuer_royalty_btc)
        .bind(trade.executed_at)
        .execute(&mut *tx)
        .await?;

        // 7. Update order status
        sqlx::query(
            r#"
            UPDATE orders
            SET status = 'completed', completed_at = NOW()
            WHERE order_id = $1
            "#,
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

        // 8. Record issuer royalty (credit to issuer's BTC balance)
        if fees.issuer_royalty_btc > dec!(0) {
            sqlx::query(
                r#"
                INSERT INTO issuer_earnings (earning_id, user_id, token_id, trade_id, amount_btc, created_at)
                VALUES ($1, $2, $3, $4, $5, NOW())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(issuer_user_id)
            .bind(order.token_id)
            .bind(trade.trade_id)
            .bind(fees.issuer_royalty_btc)
            .execute(&mut *tx)
            .await
            .ok(); // Don't fail if table doesn't exist yet
        }

        tx.commit().await?;

        tracing::info!(
            order_id = %order_id,
            trade_id = %trade_id,
            amount = %order.amount,
            fees = %fees.total_fees_btc,
            "Buy order executed successfully"
        );

        Ok(trade)
    }

    /// Execute a sell order (immediate execution against bonding curve)
    pub async fn execute_sell_order(&self, order_id: Uuid) -> Result<Trade> {
        let mut tx = self.pool.begin().await?;

        // 1. Get and lock the order
        let order: Order = sqlx::query_as(
            r#"
            SELECT * FROM orders
            WHERE order_id = $1 AND status = 'pending'
            FOR UPDATE
            "#,
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CoreError::NotFound("Order not found".to_string()))?;

        if order.order_type != OrderType::Sell {
            return Err(CoreError::Validation(
                "Order is not a sell order".to_string(),
            ));
        }

        // 2. Get and lock user balance
        let balance: Balance = sqlx::query_as(
            r#"
            SELECT * FROM balances
            WHERE user_id = $1 AND token_id = $2
            FOR UPDATE
            "#,
        )
        .bind(order.user_id)
        .bind(order.token_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| CoreError::InsufficientBalance {
            required: order.amount,
            available: dec!(0),
        })?;

        // Check sufficient balance
        if balance.available() < order.amount {
            return Err(CoreError::InsufficientBalance {
                required: order.amount,
                available: balance.available(),
            });
        }

        // 3. Get and lock the token
        let (circulating_supply, issuer_user_id): (Decimal, Uuid) = sqlx::query_as(
            r#"
            SELECT circulating_supply, issuer_user_id FROM tokens
            WHERE token_id = $1
            FOR UPDATE
            "#,
        )
        .bind(order.token_id)
        .fetch_one(&mut *tx)
        .await?;

        // Check we're not selling more than circulating
        if order.amount > circulating_supply {
            return Err(CoreError::Validation(
                "Cannot sell more than circulating supply".to_string(),
            ));
        }

        // 4. Calculate fees on proceeds
        let fees = self.calculate_fees(order.total_btc);

        // 5. Deduct from user balance
        sqlx::query(
            r#"
            UPDATE balances
            SET amount = amount - $1, updated_at = NOW()
            WHERE user_id = $2 AND token_id = $3
            "#,
        )
        .bind(order.amount)
        .bind(order.user_id)
        .bind(order.token_id)
        .execute(&mut *tx)
        .await?;

        // 6. Update token circulating supply
        sqlx::query(
            r#"
            UPDATE tokens
            SET circulating_supply = circulating_supply - $1
            WHERE token_id = $2
            "#,
        )
        .bind(order.amount)
        .bind(order.token_id)
        .execute(&mut *tx)
        .await?;

        // 7. Create trade record
        let trade_id = Uuid::new_v4();
        let trade = Trade {
            trade_id,
            buyer_user_id: Uuid::nil(), // Bonding curve is the "buyer"
            seller_user_id: Some(order.user_id),
            token_id: order.token_id,
            amount: order.amount,
            price_btc: order.price_btc,
            total_btc: order.total_btc,
            platform_fee_btc: fees.platform_fee_btc,
            issuer_royalty_btc: fees.issuer_royalty_btc,
            executed_at: Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO trades (trade_id, buyer_user_id, seller_user_id, token_id, amount, price_btc, total_btc, platform_fee_btc, issuer_royalty_btc, executed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(trade.trade_id)
        .bind(trade.buyer_user_id)
        .bind(trade.seller_user_id)
        .bind(trade.token_id)
        .bind(trade.amount)
        .bind(trade.price_btc)
        .bind(trade.total_btc)
        .bind(trade.platform_fee_btc)
        .bind(trade.issuer_royalty_btc)
        .bind(trade.executed_at)
        .execute(&mut *tx)
        .await?;

        // 8. Update order status
        sqlx::query(
            r#"
            UPDATE orders
            SET status = 'completed', completed_at = NOW()
            WHERE order_id = $1
            "#,
        )
        .bind(order_id)
        .execute(&mut *tx)
        .await?;

        // 9. Record pending BTC payout to seller
        let net_proceeds = fees.net_amount_btc;
        sqlx::query(
            r#"
            INSERT INTO pending_payouts (payout_id, user_id, trade_id, amount_btc, status, created_at)
            VALUES ($1, $2, $3, $4, 'pending', NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(order.user_id)
        .bind(trade.trade_id)
        .bind(net_proceeds)
        .execute(&mut *tx)
        .await
        .ok(); // Don't fail if table doesn't exist yet

        // 10. Record issuer royalty
        if fees.issuer_royalty_btc > dec!(0) {
            sqlx::query(
                r#"
                INSERT INTO issuer_earnings (earning_id, user_id, token_id, trade_id, amount_btc, created_at)
                VALUES ($1, $2, $3, $4, $5, NOW())
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(issuer_user_id)
            .bind(order.token_id)
            .bind(trade.trade_id)
            .bind(fees.issuer_royalty_btc)
            .execute(&mut *tx)
            .await
            .ok();
        }

        tx.commit().await?;

        tracing::info!(
            order_id = %order_id,
            trade_id = %trade_id,
            amount = %order.amount,
            net_proceeds = %net_proceeds,
            "Sell order executed successfully"
        );

        Ok(trade)
    }

    /// Cancel a pending order
    pub async fn cancel_order(&self, order_id: Uuid, user_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE orders
            SET status = 'cancelled', completed_at = NOW()
            WHERE order_id = $1 AND user_id = $2 AND status = 'pending'
            "#,
        )
        .bind(order_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound(
                "Order not found or already processed".to_string(),
            ));
        }

        tracing::info!(order_id = %order_id, "Order cancelled");
        Ok(())
    }

    /// Expire old pending orders
    pub async fn expire_stale_orders(&self, max_age_hours: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE orders
            SET status = 'expired', completed_at = NOW()
            WHERE status = 'pending'
            AND created_at < NOW() - INTERVAL '1 hour' * $1
            "#,
        )
        .bind(max_age_hours)
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::info!(count = count, "Expired stale orders");
        }
        Ok(count)
    }

    /// Execute multiple buy orders in a single transaction
    /// Returns successful trades and failed order IDs
    pub async fn execute_buy_orders_batch(
        &self,
        order_ids: Vec<Uuid>,
    ) -> Result<BatchExecutionResult> {
        let mut successful_trades = Vec::new();
        let mut failed_orders = Vec::new();

        for order_id in order_ids {
            match self.execute_buy_order(order_id).await {
                Ok(trade) => successful_trades.push(trade),
                Err(e) => {
                    tracing::warn!(order_id = %order_id, error = %e, "Failed to execute buy order in batch");
                    failed_orders.push((order_id, e.to_string()));
                }
            }
        }

        tracing::info!(
            successful = successful_trades.len(),
            failed = failed_orders.len(),
            "Batch buy order execution completed"
        );

        Ok(BatchExecutionResult {
            successful_trades,
            failed_orders,
        })
    }

    /// Execute multiple sell orders in a single transaction
    /// Returns successful trades and failed order IDs
    pub async fn execute_sell_orders_batch(
        &self,
        order_ids: Vec<Uuid>,
    ) -> Result<BatchExecutionResult> {
        let mut successful_trades = Vec::new();
        let mut failed_orders = Vec::new();

        for order_id in order_ids {
            match self.execute_sell_order(order_id).await {
                Ok(trade) => successful_trades.push(trade),
                Err(e) => {
                    tracing::warn!(order_id = %order_id, error = %e, "Failed to execute sell order in batch");
                    failed_orders.push((order_id, e.to_string()));
                }
            }
        }

        tracing::info!(
            successful = successful_trades.len(),
            failed = failed_orders.len(),
            "Batch sell order execution completed"
        );

        Ok(BatchExecutionResult {
            successful_trades,
            failed_orders,
        })
    }

    /// Get pending orders for a specific token
    pub async fn get_pending_orders_by_token(&self, token_id: Uuid) -> Result<Vec<Order>> {
        let orders = sqlx::query_as::<_, Order>(
            r#"
            SELECT * FROM orders
            WHERE token_id = $1 AND status = 'pending'
            ORDER BY created_at ASC
            "#,
        )
        .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Get pending orders for a specific user
    pub async fn get_pending_orders_by_user(&self, user_id: Uuid) -> Result<Vec<Order>> {
        let orders = sqlx::query_as::<_, Order>(
            r#"
            SELECT * FROM orders
            WHERE user_id = $1 AND status = 'pending'
            ORDER BY created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }
}

/// Result of batch order execution
#[derive(Debug)]
pub struct BatchExecutionResult {
    pub successful_trades: Vec<Trade>,
    pub failed_orders: Vec<(Uuid, String)>,
}

impl BatchExecutionResult {
    /// Get total number of orders processed
    pub fn total_processed(&self) -> usize {
        self.successful_trades.len() + self.failed_orders.len()
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> Decimal {
        if self.total_processed() == 0 {
            return dec!(0);
        }
        let successful = Decimal::from(self.successful_trades.len());
        let total = Decimal::from(self.total_processed());
        (successful / total) * dec!(100)
    }

    /// Check if all orders succeeded
    pub fn all_successful(&self) -> bool {
        self.failed_orders.is_empty()
    }
}
