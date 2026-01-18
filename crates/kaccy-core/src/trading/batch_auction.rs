//! Batch auction mechanism for MEV protection
//!
//! This module implements batch auctions (also called call auctions) where orders are collected
//! during a batch period and executed simultaneously at a uniform clearing price. This prevents
//! front-running and provides fairer price discovery.
//!
//! # How It Works
//!
//! 1. **Collection Phase**: Orders are submitted during the batch period
//! 2. **Price Discovery**: Find the price that maximizes matched volume
//! 3. **Execution**: All matched orders execute at the uniform clearing price
//! 4. **Settlement**: Update balances and create trade records
//!
//! # Benefits
//!
//! - Prevents front-running and sandwich attacks
//! - Fair price discovery (everyone gets same price)
//! - Reduces MEV extraction
//! - Better for large trades (less price impact)

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::Trade;
use crate::trading::order_book::{LimitOrder, OrderSide};

/// Batch auction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAuctionConfig {
    /// Duration of each batch period (e.g., 10 seconds, 1 minute)
    pub batch_duration: Duration,
    /// Minimum orders required to execute a batch
    pub min_orders: usize,
    /// Maximum price deviation from reference price (as percentage)
    pub max_price_deviation_pct: Decimal,
    /// Whether to allow partial fills
    pub allow_partial_fills: bool,
}

impl Default for BatchAuctionConfig {
    fn default() -> Self {
        Self {
            batch_duration: Duration::seconds(10),
            min_orders: 2,
            max_price_deviation_pct: dec!(20.0), // 20% max deviation
            allow_partial_fills: true,
        }
    }
}

/// Status of a batch auction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Collecting orders
    Collecting,
    /// Computing clearing price
    Computing,
    /// Executing matched orders
    Executing,
    /// Batch completed
    Completed,
    /// Batch cancelled (e.g., insufficient orders)
    Cancelled,
}

/// A batch auction instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAuction {
    /// Unique batch ID
    pub id: Uuid,
    /// Token being traded
    pub token_id: Uuid,
    /// Batch start time
    pub start_time: DateTime<Utc>,
    /// Batch end time
    pub end_time: DateTime<Utc>,
    /// Current status
    pub status: BatchStatus,
    /// Collected buy orders
    pub buy_orders: Vec<LimitOrder>,
    /// Collected sell orders
    pub sell_orders: Vec<LimitOrder>,
    /// Clearing price (if computed)
    pub clearing_price: Option<Decimal>,
    /// Total volume matched
    pub matched_volume: Decimal,
    /// Configuration
    pub config: BatchAuctionConfig,
}

impl BatchAuction {
    /// Create a new batch auction
    pub fn new(token_id: Uuid, config: BatchAuctionConfig) -> Self {
        let start_time = Utc::now();
        let end_time = start_time + config.batch_duration;

        Self {
            id: Uuid::new_v4(),
            token_id,
            start_time,
            end_time,
            status: BatchStatus::Collecting,
            buy_orders: Vec::new(),
            sell_orders: Vec::new(),
            clearing_price: None,
            matched_volume: dec!(0),
            config,
        }
    }

    /// Check if the batch period has ended
    pub fn is_ended(&self) -> bool {
        Utc::now() >= self.end_time
    }

    /// Get remaining time in the batch period
    pub fn remaining_time(&self) -> Duration {
        if self.is_ended() {
            Duration::zero()
        } else {
            self.end_time - Utc::now()
        }
    }

    /// Add an order to the batch
    pub fn add_order(&mut self, order: LimitOrder) -> Result<()> {
        // Validate batch is collecting
        if self.status != BatchStatus::Collecting {
            return Err(CoreError::InvalidState(format!(
                "Batch is not collecting orders (status: {:?})",
                self.status
            )));
        }

        // Validate token matches
        if order.token_id != self.token_id {
            return Err(CoreError::Validation(format!(
                "Order token {} does not match batch token {}",
                order.token_id, self.token_id
            )));
        }

        // Add to appropriate side
        match order.side {
            OrderSide::Buy => self.buy_orders.push(order),
            OrderSide::Sell => self.sell_orders.push(order),
        }

        Ok(())
    }

    /// Get total buy volume at a given price
    fn buy_volume_at_price(&self, price: Decimal) -> Decimal {
        self.buy_orders
            .iter()
            .filter(|order| order.price >= price)
            .map(|order| order.amount)
            .sum()
    }

    /// Get total sell volume at a given price
    fn sell_volume_at_price(&self, price: Decimal) -> Decimal {
        self.sell_orders
            .iter()
            .filter(|order| order.price <= price)
            .map(|order| order.amount)
            .sum()
    }

    /// Compute the clearing price that maximizes matched volume
    pub fn compute_clearing_price(&mut self, reference_price: Option<Decimal>) -> Result<Decimal> {
        if self.status != BatchStatus::Collecting {
            return Err(CoreError::InvalidState(
                "Batch must be in Collecting state".to_string(),
            ));
        }

        // Check minimum orders
        let total_orders = self.buy_orders.len() + self.sell_orders.len();
        if total_orders < self.config.min_orders {
            self.status = BatchStatus::Cancelled;
            return Err(CoreError::InsufficientLiquidity(format!(
                "Insufficient orders: {} < {}",
                total_orders, self.config.min_orders
            )));
        }

        self.status = BatchStatus::Computing;

        // Collect all unique prices from orders
        let mut prices: Vec<Decimal> = self
            .buy_orders
            .iter()
            .chain(self.sell_orders.iter())
            .map(|order| order.price)
            .collect();
        prices.sort();
        prices.dedup();

        // If we have a reference price, filter by max deviation
        if let Some(ref_price) = reference_price {
            let max_deviation = (self.config.max_price_deviation_pct / dec!(100)) * ref_price;
            let min_price = ref_price - max_deviation;
            let max_price = ref_price + max_deviation;
            prices.retain(|&p| p >= min_price && p <= max_price);

            if prices.is_empty() {
                self.status = BatchStatus::Cancelled;
                return Err(CoreError::Validation(
                    "No prices within acceptable deviation".to_string(),
                ));
            }
        }

        // Find price that maximizes matched volume
        let mut best_price = dec!(0);
        let mut max_volume = dec!(0);

        for &price in &prices {
            let buy_vol = self.buy_volume_at_price(price);
            let sell_vol = self.sell_volume_at_price(price);
            let matched_vol = buy_vol.min(sell_vol);

            if matched_vol > max_volume {
                max_volume = matched_vol;
                best_price = price;
            }
        }

        if max_volume.is_zero() {
            self.status = BatchStatus::Cancelled;
            return Err(CoreError::InsufficientLiquidity(
                "No overlapping buy/sell orders".to_string(),
            ));
        }

        self.clearing_price = Some(best_price);
        self.matched_volume = max_volume;

        Ok(best_price)
    }

    /// Execute the batch at the clearing price
    pub fn execute(&mut self) -> Result<Vec<BatchMatch>> {
        let clearing_price = self
            .clearing_price
            .ok_or_else(|| CoreError::InvalidState("Clearing price not computed".to_string()))?;

        if self.status != BatchStatus::Computing {
            return Err(CoreError::InvalidState(
                "Batch must be in Computing state".to_string(),
            ));
        }

        self.status = BatchStatus::Executing;

        // Get eligible buy orders (price >= clearing price)
        let mut eligible_buys: Vec<&LimitOrder> = self
            .buy_orders
            .iter()
            .filter(|order| order.price >= clearing_price)
            .collect();

        // Get eligible sell orders (price <= clearing price)
        let mut eligible_sells: Vec<&LimitOrder> = self
            .sell_orders
            .iter()
            .filter(|order| order.price <= clearing_price)
            .collect();

        // Sort by price-time priority
        eligible_buys.sort_by(|a, b| {
            b.price
                .cmp(&a.price)
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });
        eligible_sells.sort_by(|a, b| {
            a.price
                .cmp(&b.price)
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });

        let mut matches = Vec::new();
        let mut buy_idx = 0;
        let mut sell_idx = 0;
        let mut buy_filled = dec!(0);
        let mut sell_filled = dec!(0);

        // Match orders
        while buy_idx < eligible_buys.len() && sell_idx < eligible_sells.len() {
            let buy_order = eligible_buys[buy_idx];
            let sell_order = eligible_sells[sell_idx];

            let buy_remaining = buy_order.amount - buy_filled;
            let sell_remaining = sell_order.amount - sell_filled;

            if buy_remaining.is_zero() {
                buy_idx += 1;
                buy_filled = dec!(0);
                continue;
            }

            if sell_remaining.is_zero() {
                sell_idx += 1;
                sell_filled = dec!(0);
                continue;
            }

            let match_amount = buy_remaining.min(sell_remaining);

            matches.push(BatchMatch {
                buy_order_id: buy_order.order_id,
                sell_order_id: sell_order.order_id,
                buyer_id: buy_order.user_id,
                seller_id: sell_order.user_id,
                amount: match_amount,
                price: clearing_price,
                timestamp: Utc::now(),
            });

            buy_filled += match_amount;
            sell_filled += match_amount;

            if buy_filled >= buy_order.amount {
                buy_idx += 1;
                buy_filled = dec!(0);
            }

            if sell_filled >= sell_order.amount {
                sell_idx += 1;
                sell_filled = dec!(0);
            }
        }

        self.status = BatchStatus::Completed;

        Ok(matches)
    }

    /// Get statistics about the batch
    pub fn stats(&self) -> BatchStats {
        BatchStats {
            batch_id: self.id,
            token_id: self.token_id,
            total_buy_orders: self.buy_orders.len(),
            total_sell_orders: self.sell_orders.len(),
            total_buy_volume: self.buy_orders.iter().map(|o| o.amount).sum(),
            total_sell_volume: self.sell_orders.iter().map(|o| o.amount).sum(),
            clearing_price: self.clearing_price,
            matched_volume: self.matched_volume,
            status: self.status,
            start_time: self.start_time,
            end_time: self.end_time,
        }
    }
}

/// A matched trade from a batch auction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMatch {
    /// Buy order ID
    pub buy_order_id: Uuid,
    /// Sell order ID
    pub sell_order_id: Uuid,
    /// Buyer user ID
    pub buyer_id: Uuid,
    /// Seller user ID
    pub seller_id: Uuid,
    /// Matched amount
    pub amount: Decimal,
    /// Clearing price
    pub price: Decimal,
    /// Match timestamp
    pub timestamp: DateTime<Utc>,
}

impl BatchMatch {
    /// Convert to a Trade record
    pub fn to_trade(&self, token_id: Uuid) -> Trade {
        Trade {
            trade_id: Uuid::new_v4(),
            buyer_user_id: self.buyer_id,
            seller_user_id: Some(self.seller_id),
            token_id,
            amount: self.amount,
            price_btc: self.price,
            total_btc: self.amount * self.price,
            platform_fee_btc: dec!(0), // Fees calculated separately
            issuer_royalty_btc: dec!(0),
            executed_at: self.timestamp,
        }
    }
}

/// Batch auction statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStats {
    /// Batch ID
    pub batch_id: Uuid,
    /// Token ID
    pub token_id: Uuid,
    /// Total buy orders submitted
    pub total_buy_orders: usize,
    /// Total sell orders submitted
    pub total_sell_orders: usize,
    /// Total buy volume
    pub total_buy_volume: Decimal,
    /// Total sell volume
    pub total_sell_volume: Decimal,
    /// Clearing price (if computed)
    pub clearing_price: Option<Decimal>,
    /// Matched volume
    pub matched_volume: Decimal,
    /// Batch status
    pub status: BatchStatus,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
}

impl BatchStats {
    /// Get match rate (matched volume / total volume)
    pub fn match_rate(&self) -> Decimal {
        let total_volume = self.total_buy_volume.min(self.total_sell_volume);
        if total_volume.is_zero() {
            dec!(0)
        } else {
            (self.matched_volume / total_volume) * dec!(100)
        }
    }

    /// Check if batch was successful
    pub fn is_successful(&self) -> bool {
        self.status == BatchStatus::Completed && !self.matched_volume.is_zero()
    }
}

/// Manager for running batch auctions
pub struct BatchAuctionManager {
    /// Active batches by token
    active_batches: HashMap<Uuid, BatchAuction>,
    /// Completed batches (kept for history)
    completed_batches: Vec<BatchAuction>,
    /// Default configuration
    default_config: BatchAuctionConfig,
}

impl BatchAuctionManager {
    /// Create a new batch auction manager
    pub fn new(config: BatchAuctionConfig) -> Self {
        Self {
            active_batches: HashMap::new(),
            completed_batches: Vec::new(),
            default_config: config,
        }
    }

    /// Start a new batch for a token
    pub fn start_batch(&mut self, token_id: Uuid) -> Result<Uuid> {
        if self.active_batches.contains_key(&token_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Batch already active for token {}",
                token_id
            )));
        }

        let batch = BatchAuction::new(token_id, self.default_config.clone());
        let batch_id = batch.id;
        self.active_batches.insert(token_id, batch);

        Ok(batch_id)
    }

    /// Add an order to the active batch for a token
    pub fn add_order(&mut self, order: LimitOrder) -> Result<()> {
        let batch = self
            .active_batches
            .get_mut(&order.token_id)
            .ok_or_else(|| {
                CoreError::NotFound(format!("No active batch for token {}", order.token_id))
            })?;

        batch.add_order(order)
    }

    /// Execute a batch for a token
    pub fn execute_batch(
        &mut self,
        token_id: Uuid,
        reference_price: Option<Decimal>,
    ) -> Result<Vec<BatchMatch>> {
        let mut batch = self.active_batches.remove(&token_id).ok_or_else(|| {
            CoreError::NotFound(format!("No active batch for token {}", token_id))
        })?;

        batch.compute_clearing_price(reference_price)?;
        let matches = batch.execute()?;

        self.completed_batches.push(batch);

        Ok(matches)
    }

    /// Get active batch for a token
    pub fn get_active_batch(&self, token_id: &Uuid) -> Option<&BatchAuction> {
        self.active_batches.get(token_id)
    }

    /// Get statistics for all completed batches
    pub fn get_completed_stats(&self) -> Vec<BatchStats> {
        self.completed_batches.iter().map(|b| b.stats()).collect()
    }

    /// Clean up old completed batches (keep last N)
    pub fn cleanup_old_batches(&mut self, keep_last: usize) {
        if self.completed_batches.len() > keep_last {
            self.completed_batches
                .drain(0..self.completed_batches.len() - keep_last);
        }
    }
}

impl Default for BatchAuctionManager {
    fn default() -> Self {
        Self::new(BatchAuctionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_order(
        token_id: Uuid,
        user_id: Uuid,
        side: OrderSide,
        price: Decimal,
        amount: Decimal,
    ) -> LimitOrder {
        LimitOrder {
            order_id: Uuid::new_v4(),
            token_id,
            user_id,
            side,
            price,
            amount,
            filled_amount: dec!(0),
            timestamp: Utc::now().timestamp_millis(),
        }
    }

    #[test]
    fn test_batch_auction_creation() {
        let token_id = Uuid::new_v4();
        let config = BatchAuctionConfig::default();
        let batch = BatchAuction::new(token_id, config);

        assert_eq!(batch.token_id, token_id);
        assert_eq!(batch.status, BatchStatus::Collecting);
        assert!(batch.buy_orders.is_empty());
        assert!(batch.sell_orders.is_empty());
    }

    #[test]
    fn test_add_order() {
        let token_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut batch = BatchAuction::new(token_id, BatchAuctionConfig::default());

        let buy_order = create_test_order(token_id, user_id, OrderSide::Buy, dec!(100), dec!(10));
        let sell_order = create_test_order(token_id, user_id, OrderSide::Sell, dec!(105), dec!(5));

        batch.add_order(buy_order).unwrap();
        batch.add_order(sell_order).unwrap();

        assert_eq!(batch.buy_orders.len(), 1);
        assert_eq!(batch.sell_orders.len(), 1);
    }

    #[test]
    fn test_clearing_price_computation() {
        let token_id = Uuid::new_v4();
        let mut batch = BatchAuction::new(token_id, BatchAuctionConfig::default());

        // Add buy orders
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Buy,
                dec!(105),
                dec!(10),
            ))
            .unwrap();
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Buy,
                dec!(100),
                dec!(15),
            ))
            .unwrap();

        // Add sell orders
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Sell,
                dec!(95),
                dec!(8),
            ))
            .unwrap();
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Sell,
                dec!(102),
                dec!(12),
            ))
            .unwrap();

        let clearing_price = batch.compute_clearing_price(None).unwrap();
        assert!(clearing_price >= dec!(95) && clearing_price <= dec!(105));
        assert!(batch.matched_volume > dec!(0));
    }

    #[test]
    fn test_batch_execution() {
        let token_id = Uuid::new_v4();
        let mut batch = BatchAuction::new(token_id, BatchAuctionConfig::default());

        // Add overlapping orders
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Buy,
                dec!(100),
                dec!(10),
            ))
            .unwrap();
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Sell,
                dec!(100),
                dec!(10),
            ))
            .unwrap();

        batch.compute_clearing_price(None).unwrap();
        let matches = batch.execute().unwrap();

        assert!(!matches.is_empty());
        assert_eq!(batch.status, BatchStatus::Completed);
    }

    #[test]
    fn test_batch_manager() {
        let mut manager = BatchAuctionManager::default();
        let token_id = Uuid::new_v4();

        // Start batch
        let batch_id = manager.start_batch(token_id).unwrap();
        assert!(batch_id != Uuid::nil());

        // Add orders
        let order = create_test_order(
            token_id,
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );
        manager.add_order(order).unwrap();

        // Check active batch
        assert!(manager.get_active_batch(&token_id).is_some());
    }

    #[test]
    fn test_insufficient_orders() {
        let token_id = Uuid::new_v4();
        let mut batch = BatchAuction::new(token_id, BatchAuctionConfig::default());

        // Only one order
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Buy,
                dec!(100),
                dec!(10),
            ))
            .unwrap();

        let result = batch.compute_clearing_price(None);
        assert!(result.is_err());
        assert_eq!(batch.status, BatchStatus::Cancelled);
    }

    #[test]
    fn test_batch_stats() {
        let token_id = Uuid::new_v4();
        let mut batch = BatchAuction::new(token_id, BatchAuctionConfig::default());

        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Buy,
                dec!(100),
                dec!(10),
            ))
            .unwrap();
        batch
            .add_order(create_test_order(
                token_id,
                Uuid::new_v4(),
                OrderSide::Sell,
                dec!(100),
                dec!(5),
            ))
            .unwrap();

        let stats = batch.stats();
        assert_eq!(stats.total_buy_orders, 1);
        assert_eq!(stats.total_sell_orders, 1);
        assert_eq!(stats.total_buy_volume, dec!(10));
        assert_eq!(stats.total_sell_volume, dec!(5));
    }
}
