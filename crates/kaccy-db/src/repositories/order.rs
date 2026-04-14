//! Order repository

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Order data from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRow {
    /// Unique identifier for the order.
    pub order_id: Uuid,
    /// User who placed the order.
    pub user_id: Uuid,
    /// Token being bought or sold.
    pub token_id: Uuid,
    /// Order type (e.g., "buy", "sell").
    pub order_type: String,
    /// Token amount specified in the order.
    pub amount: Decimal,
    /// Price per token in BTC.
    pub price_btc: Decimal,
    /// Total BTC value of the order.
    pub total_btc: Decimal,
    /// Current order status (e.g., "pending", "completed", "cancelled").
    pub status: String,
    /// BTC address for payment or receipt.
    pub btc_address: Option<String>,
    /// BTC transaction ID once settled.
    pub btc_txid: Option<String>,
    /// Timestamp when the order was placed.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp when the order was completed or cancelled.
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Repository for order operations
pub struct OrderRepository {
    pool: PgPool,
}

impl OrderRepository {
    /// Create a new order repository.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new order
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: Uuid,
        token_id: Uuid,
        order_type: &str,
        amount: Decimal,
        price_btc: Decimal,
        total_btc: Decimal,
        btc_address: Option<&str>,
    ) -> Result<OrderRow> {
        let order = sqlx::query_as::<_, OrderRow>(
            r#"
            INSERT INTO orders (user_id, token_id, order_type, amount, price_btc, total_btc, btc_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(token_id)
        .bind(order_type)
        .bind(amount)
        .bind(price_btc)
        .bind(total_btc)
        .bind(btc_address)
        .fetch_one(&self.pool)
        .await?;

        Ok(order)
    }

    /// Find order by ID
    pub async fn find_by_id(&self, order_id: Uuid) -> Result<Option<OrderRow>> {
        let order = sqlx::query_as::<_, OrderRow>(r#"SELECT * FROM orders WHERE order_id = $1"#)
            .bind(order_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(order)
    }

    /// Find order by BTC address
    pub async fn find_by_btc_address(&self, btc_address: &str) -> Result<Option<OrderRow>> {
        let order = sqlx::query_as::<_, OrderRow>(
            r#"SELECT * FROM orders WHERE btc_address = $1 AND status = 'pending'"#,
        )
        .bind(btc_address)
        .fetch_optional(&self.pool)
        .await?;

        Ok(order)
    }

    /// Get user's orders
    pub async fn get_user_orders(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OrderRow>> {
        let orders = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM orders
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Get pending orders
    pub async fn get_pending_orders(&self, limit: i64) -> Result<Vec<OrderRow>> {
        let orders = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM orders
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Update order status
    pub async fn update_status(&self, order_id: Uuid, status: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE orders
            SET status = $2, completed_at = CASE WHEN $2 IN ('completed', 'cancelled', 'expired') THEN NOW() ELSE completed_at END
            WHERE order_id = $1
            "#,
        )
        .bind(order_id)
        .bind(status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update order with BTC transaction ID
    pub async fn update_btc_txid(&self, order_id: Uuid, btc_txid: &str) -> Result<()> {
        sqlx::query(r#"UPDATE orders SET btc_txid = $2 WHERE order_id = $1"#)
            .bind(order_id)
            .bind(btc_txid)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Cancel order
    pub async fn cancel(&self, order_id: Uuid, user_id: Uuid) -> Result<bool> {
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

        Ok(result.rows_affected() > 0)
    }

    /// Expire stale orders
    pub async fn expire_stale(&self, max_age_hours: i64) -> Result<u64> {
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

        Ok(result.rows_affected())
    }

    /// Get order count by status
    pub async fn count_by_status(&self, status: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM orders WHERE status = $1
            "#,
        )
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Get order volume by token
    pub async fn get_token_volume(&self, token_id: Uuid) -> Result<Decimal> {
        let row: (Option<Decimal>,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(total_btc), 0) FROM orders
            WHERE token_id = $1 AND status = 'completed'
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(Decimal::ZERO))
    }

    /// Get order completion rate for a user
    pub async fn get_user_completion_rate(&self, user_id: Uuid) -> Result<Decimal> {
        let row: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) as total
            FROM orders
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let (completed, total) = row;
        if total == 0 {
            return Ok(Decimal::ZERO);
        }

        let rate = Decimal::from(completed) / Decimal::from(total) * Decimal::from(100);
        Ok(rate)
    }

    /// Get average order size for a token
    pub async fn get_average_order_size(&self, token_id: Uuid) -> Result<Decimal> {
        let row: (Option<Decimal>,) = sqlx::query_as(
            r#"
            SELECT AVG(amount) FROM orders
            WHERE token_id = $1 AND status = 'completed'
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(Decimal::ZERO))
    }

    /// Batch cancel orders (useful for emergency situations or mass cancellations)
    pub async fn batch_cancel(&self, order_ids: &[Uuid], user_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE orders
            SET status = 'cancelled', completed_at = NOW()
            WHERE order_id = ANY($1) AND user_id = $2 AND status = 'pending'
            "#,
        )
        .bind(order_ids)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get orders by status and date range
    pub async fn get_orders_by_status_and_date(
        &self,
        status: &str,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OrderRow>> {
        let orders = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM orders
            WHERE status = $1
            AND created_at BETWEEN $2 AND $3
            ORDER BY created_at DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(status)
        .bind(start_date)
        .bind(end_date)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Get orders by token with optional status filter
    pub async fn get_token_orders(
        &self,
        token_id: Uuid,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OrderRow>> {
        let orders = if let Some(status_filter) = status {
            sqlx::query_as::<_, OrderRow>(
                r#"
                SELECT * FROM orders
                WHERE token_id = $1 AND status = $2
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(token_id)
            .bind(status_filter)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, OrderRow>(
                r#"
                SELECT * FROM orders
                WHERE token_id = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(token_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(orders)
    }

    /// Get orders within a price range
    pub async fn get_orders_by_price_range(
        &self,
        token_id: Uuid,
        min_price: Decimal,
        max_price: Decimal,
        limit: i64,
    ) -> Result<Vec<OrderRow>> {
        let orders = sqlx::query_as::<_, OrderRow>(
            r#"
            SELECT * FROM orders
            WHERE token_id = $1
            AND price_btc BETWEEN $2 AND $3
            AND status = 'pending'
            ORDER BY price_btc ASC
            LIMIT $4
            "#,
        )
        .bind(token_id)
        .bind(min_price)
        .bind(max_price)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(orders)
    }

    /// Get order book depth (grouped by price levels)
    pub async fn get_order_book_depth(
        &self,
        token_id: Uuid,
        depth_levels: i64,
    ) -> Result<OrderBookDepth> {
        // Get buy orders (grouped by price, highest first)
        let buy_orders: Vec<PriceLevel> = sqlx::query_as(
            r#"
            SELECT
                price_btc,
                SUM(amount) as total_amount,
                COUNT(*) as order_count
            FROM orders
            WHERE token_id = $1
            AND order_type = 'buy'
            AND status = 'pending'
            GROUP BY price_btc
            ORDER BY price_btc DESC
            LIMIT $2
            "#,
        )
        .bind(token_id)
        .bind(depth_levels)
        .fetch_all(&self.pool)
        .await?;

        // Get sell orders (grouped by price, lowest first)
        let sell_orders: Vec<PriceLevel> = sqlx::query_as(
            r#"
            SELECT
                price_btc,
                SUM(amount) as total_amount,
                COUNT(*) as order_count
            FROM orders
            WHERE token_id = $1
            AND order_type = 'sell'
            AND status = 'pending'
            GROUP BY price_btc
            ORDER BY price_btc ASC
            LIMIT $2
            "#,
        )
        .bind(token_id)
        .bind(depth_levels)
        .fetch_all(&self.pool)
        .await?;

        Ok(OrderBookDepth {
            buy_orders,
            sell_orders,
        })
    }

    /// Get best bid (highest buy price) and ask (lowest sell price)
    pub async fn get_best_bid_ask(&self, token_id: Uuid) -> Result<BidAsk> {
        let row: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
            r#"
            SELECT
                (SELECT MAX(price_btc) FROM orders
                 WHERE token_id = $1 AND order_type = 'buy' AND status = 'pending') as best_bid,
                (SELECT MIN(price_btc) FROM orders
                 WHERE token_id = $1 AND order_type = 'sell' AND status = 'pending') as best_ask
            "#,
        )
        .bind(token_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(BidAsk {
            best_bid: row.0,
            best_ask: row.1,
        })
    }

    /// Get order spread (difference between best ask and best bid)
    pub async fn get_spread(&self, token_id: Uuid) -> Result<Option<Decimal>> {
        let bid_ask = self.get_best_bid_ask(token_id).await?;

        match (bid_ask.best_bid, bid_ask.best_ask) {
            (Some(bid), Some(ask)) => Ok(Some(ask - bid)),
            _ => Ok(None),
        }
    }

    /// Get total order count for a user
    pub async fn count_user_orders(&self, user_id: Uuid) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM orders WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }
}

/// Order book depth with buy and sell orders grouped by price
#[derive(Debug, Clone)]
pub struct OrderBookDepth {
    /// Aggregated buy orders sorted by price descending.
    pub buy_orders: Vec<PriceLevel>,
    /// Aggregated sell orders sorted by price ascending.
    pub sell_orders: Vec<PriceLevel>,
}

/// Price level in the order book
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PriceLevel {
    /// Price in BTC at this level.
    pub price_btc: Decimal,
    /// Total token amount available at this price.
    pub total_amount: Decimal,
    /// Number of open orders at this price.
    pub order_count: i64,
}

/// Best bid and ask prices
#[derive(Debug, Clone)]
pub struct BidAsk {
    /// Highest open buy price in BTC, or None if no open buys.
    pub best_bid: Option<Decimal>,
    /// Lowest open sell price in BTC, or None if no open sells.
    pub best_ask: Option<Decimal>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_depth_creation() {
        let depth = OrderBookDepth {
            buy_orders: vec![],
            sell_orders: vec![],
        };

        assert_eq!(depth.buy_orders.len(), 0);
        assert_eq!(depth.sell_orders.len(), 0);
    }

    #[test]
    fn test_price_level_creation() {
        let level = PriceLevel {
            price_btc: Decimal::new(50000, 0),
            total_amount: Decimal::new(100, 0),
            order_count: 5,
        };

        assert_eq!(level.price_btc, Decimal::new(50000, 0));
        assert_eq!(level.total_amount, Decimal::new(100, 0));
        assert_eq!(level.order_count, 5);
    }

    #[test]
    fn test_bid_ask_creation() {
        let bid_ask = BidAsk {
            best_bid: Some(Decimal::new(50000, 0)),
            best_ask: Some(Decimal::new(51000, 0)),
        };

        assert!(bid_ask.best_bid.is_some());
        assert!(bid_ask.best_ask.is_some());
        assert_eq!(bid_ask.best_bid.unwrap(), Decimal::new(50000, 0));
        assert_eq!(bid_ask.best_ask.unwrap(), Decimal::new(51000, 0));
    }

    #[test]
    fn test_bid_ask_empty() {
        let bid_ask = BidAsk {
            best_bid: None,
            best_ask: None,
        };

        assert!(bid_ask.best_bid.is_none());
        assert!(bid_ask.best_ask.is_none());
    }

    #[test]
    fn test_spread_calculation() {
        let bid = Some(Decimal::new(50000, 0));
        let ask = Some(Decimal::new(51000, 0));

        if let (Some(b), Some(a)) = (bid, ask) {
            let spread = a - b;
            assert_eq!(spread, Decimal::new(1000, 0));
        }
    }

    #[test]
    fn test_order_book_with_multiple_levels() {
        let buy_orders = vec![
            PriceLevel {
                price_btc: Decimal::new(50000, 0),
                total_amount: Decimal::new(100, 0),
                order_count: 5,
            },
            PriceLevel {
                price_btc: Decimal::new(49000, 0),
                total_amount: Decimal::new(200, 0),
                order_count: 10,
            },
        ];

        let sell_orders = vec![
            PriceLevel {
                price_btc: Decimal::new(51000, 0),
                total_amount: Decimal::new(150, 0),
                order_count: 7,
            },
            PriceLevel {
                price_btc: Decimal::new(52000, 0),
                total_amount: Decimal::new(250, 0),
                order_count: 12,
            },
        ];

        let depth = OrderBookDepth {
            buy_orders: buy_orders.clone(),
            sell_orders: sell_orders.clone(),
        };

        assert_eq!(depth.buy_orders.len(), 2);
        assert_eq!(depth.sell_orders.len(), 2);
        assert_eq!(depth.buy_orders[0].price_btc, Decimal::new(50000, 0));
        assert_eq!(depth.sell_orders[0].price_btc, Decimal::new(51000, 0));
    }

    #[test]
    fn test_completion_rate_zero_orders() {
        let completed = 0i64;
        let total = 0i64;

        let rate = if total == 0 {
            Decimal::ZERO
        } else {
            Decimal::from(completed) / Decimal::from(total) * Decimal::from(100)
        };

        assert_eq!(rate, Decimal::ZERO);
    }

    #[test]
    fn test_completion_rate_calculation() {
        let completed = 75i64;
        let total = 100i64;

        let rate = Decimal::from(completed) / Decimal::from(total) * Decimal::from(100);

        assert_eq!(rate, Decimal::new(75, 0));
    }

    #[test]
    fn test_completion_rate_perfect() {
        let completed = 100i64;
        let total = 100i64;

        let rate = Decimal::from(completed) / Decimal::from(total) * Decimal::from(100);

        assert_eq!(rate, Decimal::new(100, 0));
    }

    #[test]
    fn test_completion_rate_partial() {
        let completed = 50i64;
        let total = 200i64;

        let rate = Decimal::from(completed) / Decimal::from(total) * Decimal::from(100);

        assert_eq!(rate, Decimal::new(25, 0));
    }
}
