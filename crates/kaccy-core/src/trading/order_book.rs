//! Order book for limit orders
//!
//! Implements a price-time priority matching engine for limit orders.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, VecDeque};
use uuid::Uuid;

use crate::models::OrderType;

/// A limit order in the order book
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LimitOrder {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub side: OrderSide,
    pub price: Decimal,
    pub amount: Decimal,
    pub filled_amount: Decimal,
    pub timestamp: i64, // Unix timestamp for time priority
}

impl LimitOrder {
    pub fn new(
        user_id: Uuid,
        token_id: Uuid,
        side: OrderSide,
        price: Decimal,
        amount: Decimal,
    ) -> Self {
        Self {
            order_id: Uuid::new_v4(),
            user_id,
            token_id,
            side,
            price,
            amount,
            filled_amount: Decimal::ZERO,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn remaining(&self) -> Decimal {
        self.amount - self.filled_amount
    }

    pub fn is_filled(&self) -> bool {
        self.filled_amount >= self.amount
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl From<OrderType> for OrderSide {
    fn from(order_type: OrderType) -> Self {
        match order_type {
            OrderType::Buy => OrderSide::Buy,
            OrderType::Sell => OrderSide::Sell,
        }
    }
}

/// Result of a match operation
#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    pub trades: Vec<Trade>,
    pub remaining_amount: Decimal,
    pub fully_filled: bool,
}

/// A trade resulting from order matching
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub trade_id: Uuid,
    pub taker_order_id: Uuid,
    pub maker_order_id: Uuid,
    pub price: Decimal,
    pub amount: Decimal,
    pub taker_side: OrderSide,
}

/// Price level in the order book
#[derive(Debug, Clone)]
struct PriceLevel {
    orders: VecDeque<LimitOrder>,
    total_amount: Decimal,
}

impl PriceLevel {
    fn new() -> Self {
        Self {
            orders: VecDeque::new(),
            total_amount: Decimal::ZERO,
        }
    }

    fn add_order(&mut self, order: LimitOrder) {
        self.total_amount += order.remaining();
        self.orders.push_back(order);
    }

    fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

/// Order book for a single token pair
#[derive(Debug)]
pub struct OrderBook {
    pub token_id: Uuid,
    /// Buy orders sorted by price descending (highest first)
    bids: BTreeMap<PriceKey, PriceLevel>,
    /// Sell orders sorted by price ascending (lowest first)
    asks: BTreeMap<PriceKey, PriceLevel>,
    /// Order lookup by ID
    orders: HashMap<Uuid, OrderLocation>,
}

#[derive(Debug, Clone, Copy)]
struct OrderLocation {
    side: OrderSide,
    price: Decimal,
}

/// Key for price ordering (handles descending order for bids)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriceKey {
    price: Decimal,
    descending: bool,
}

impl PartialOrd for PriceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp = self.price.cmp(&other.price);
        if self.descending { cmp.reverse() } else { cmp }
    }
}

impl OrderBook {
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
        }
    }

    /// Add a limit order to the book
    pub fn add_order(&mut self, order: LimitOrder) {
        let location = OrderLocation {
            side: order.side,
            price: order.price,
        };
        self.orders.insert(order.order_id, location);

        match order.side {
            OrderSide::Buy => {
                let key = PriceKey {
                    price: order.price,
                    descending: true,
                };
                self.bids
                    .entry(key)
                    .or_insert_with(PriceLevel::new)
                    .add_order(order);
            }
            OrderSide::Sell => {
                let key = PriceKey {
                    price: order.price,
                    descending: false,
                };
                self.asks
                    .entry(key)
                    .or_insert_with(PriceLevel::new)
                    .add_order(order);
            }
        }
    }

    /// Cancel an order by ID
    pub fn cancel_order(&mut self, order_id: Uuid) -> Option<LimitOrder> {
        let location = self.orders.remove(&order_id)?;

        let orders_map = match location.side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        let key = PriceKey {
            price: location.price,
            descending: location.side == OrderSide::Buy,
        };

        let level = orders_map.get_mut(&key)?;
        let idx = level.orders.iter().position(|o| o.order_id == order_id)?;
        let order = level.orders.remove(idx)?;
        level.total_amount -= order.remaining();

        // Remove empty price levels
        if level.is_empty() {
            orders_map.remove(&key);
        }

        Some(order)
    }

    /// Match an incoming order against the book
    pub fn match_order(&mut self, order: LimitOrder) -> MatchResult {
        let mut trades = Vec::new();
        let mut remaining = order.amount;

        let opposite_side = match order.side {
            OrderSide::Buy => &mut self.asks,
            OrderSide::Sell => &mut self.bids,
        };

        // Keys to remove after matching
        let mut empty_keys = Vec::new();

        for (key, level) in opposite_side.iter_mut() {
            // Check if price crosses
            let price_crosses = match order.side {
                OrderSide::Buy => order.price >= key.price,
                OrderSide::Sell => order.price <= key.price,
            };

            if !price_crosses {
                break; // No more matches possible
            }

            // Match against orders at this price level (time priority)
            while let Some(maker_order) = level.orders.front_mut() {
                if remaining <= Decimal::ZERO {
                    break;
                }

                let fill_amount = remaining.min(maker_order.remaining());

                // Create trade
                trades.push(Trade {
                    trade_id: Uuid::new_v4(),
                    taker_order_id: order.order_id,
                    maker_order_id: maker_order.order_id,
                    price: maker_order.price, // Execute at maker's price
                    amount: fill_amount,
                    taker_side: order.side,
                });

                maker_order.filled_amount += fill_amount;
                remaining -= fill_amount;
                level.total_amount -= fill_amount;

                // Remove filled orders
                if maker_order.is_filled() {
                    self.orders.remove(&maker_order.order_id);
                    level.orders.pop_front();
                }
            }

            if level.is_empty() {
                empty_keys.push(*key);
            }
        }

        // Clean up empty price levels
        for key in empty_keys {
            opposite_side.remove(&key);
        }

        MatchResult {
            trades,
            remaining_amount: remaining,
            fully_filled: remaining <= Decimal::ZERO,
        }
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first_key_value().map(|(k, _)| k.price)
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first_key_value().map(|(k, _)| k.price)
    }

    /// Get the spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get the mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask + bid) / dec!(2)),
            _ => None,
        }
    }

    /// Get total bid volume
    pub fn total_bid_volume(&self) -> Decimal {
        self.bids.values().map(|l| l.total_amount).sum()
    }

    /// Get total ask volume
    pub fn total_ask_volume(&self) -> Decimal {
        self.asks.values().map(|l| l.total_amount).sum()
    }

    /// Get order book depth (top N levels)
    pub fn get_depth(&self, levels: usize) -> OrderBookDepth {
        let bids: Vec<_> = self
            .bids
            .iter()
            .take(levels)
            .map(|(k, l)| DepthLevel {
                price: k.price,
                amount: l.total_amount,
                order_count: l.orders.len(),
            })
            .collect();

        let asks: Vec<_> = self
            .asks
            .iter()
            .take(levels)
            .map(|(k, l)| DepthLevel {
                price: k.price,
                amount: l.total_amount,
                order_count: l.orders.len(),
            })
            .collect();

        OrderBookDepth {
            token_id: self.token_id,
            bids,
            asks,
            spread: self.spread(),
            mid_price: self.mid_price(),
        }
    }

    /// Check if an order exists
    pub fn has_order(&self, order_id: Uuid) -> bool {
        self.orders.contains_key(&order_id)
    }

    /// Get total number of orders
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }
}

/// Order book depth snapshot
#[derive(Debug, Clone, Serialize)]
pub struct OrderBookDepth {
    pub token_id: Uuid,
    pub bids: Vec<DepthLevel>,
    pub asks: Vec<DepthLevel>,
    pub spread: Option<Decimal>,
    pub mid_price: Option<Decimal>,
}

/// A single depth level
#[derive(Debug, Clone, Serialize)]
pub struct DepthLevel {
    pub price: Decimal,
    pub amount: Decimal,
    pub order_count: usize,
}

/// Manager for multiple order books
pub struct OrderBookManager {
    books: HashMap<Uuid, OrderBook>,
}

impl OrderBookManager {
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
        }
    }

    /// Get or create an order book for a token
    pub fn get_or_create(&mut self, token_id: Uuid) -> &mut OrderBook {
        self.books
            .entry(token_id)
            .or_insert_with(|| OrderBook::new(token_id))
    }

    /// Get an order book if it exists
    pub fn get(&self, token_id: Uuid) -> Option<&OrderBook> {
        self.books.get(&token_id)
    }

    /// Get mutable order book if it exists
    pub fn get_mut(&mut self, token_id: Uuid) -> Option<&mut OrderBook> {
        self.books.get_mut(&token_id)
    }

    /// Remove an order book
    pub fn remove(&mut self, token_id: Uuid) -> Option<OrderBook> {
        self.books.remove(&token_id)
    }

    /// Get all token IDs with order books
    pub fn token_ids(&self) -> Vec<Uuid> {
        self.books.keys().cloned().collect()
    }
}

impl Default for OrderBookManager {
    fn default() -> Self {
        Self::new()
    }
}
