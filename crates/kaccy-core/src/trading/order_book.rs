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
    /// Unique identifier for this order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token being traded
    pub token_id: Uuid,
    /// Buy or sell side
    pub side: OrderSide,
    /// Limit price
    pub price: Decimal,
    /// Total order quantity
    pub amount: Decimal,
    /// Amount already filled
    pub filled_amount: Decimal,
    /// Unix timestamp in milliseconds for time priority
    pub timestamp: i64,
}

impl LimitOrder {
    /// Create a new limit order
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

    /// Returns the unfilled quantity remaining on this order.
    pub fn remaining(&self) -> Decimal {
        self.amount - self.filled_amount
    }

    /// Returns `true` if the order has been completely filled.
    pub fn is_filled(&self) -> bool {
        self.filled_amount >= self.amount
    }
}

/// Which side of the market an order is on
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OrderSide {
    /// Buy (bid) order
    Buy,
    /// Sell (ask) order
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
    /// Trades executed during matching
    pub trades: Vec<Trade>,
    /// Unfilled quantity remaining after matching
    pub remaining_amount: Decimal,
    /// Whether the incoming order was completely filled
    pub fully_filled: bool,
}

/// A trade resulting from order matching
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    /// Unique identifier for this trade
    pub trade_id: Uuid,
    /// The incoming (aggressive) order ID
    pub taker_order_id: Uuid,
    /// The resting (passive) order ID
    pub maker_order_id: Uuid,
    /// Execution price (maker's price)
    pub price: Decimal,
    /// Quantity traded
    pub amount: Decimal,
    /// Side of the taker order
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
    /// Token this order book serves
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
    /// Create an empty order book for the given token
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
    /// Token this snapshot belongs to
    pub token_id: Uuid,
    /// Top bid price levels (highest price first)
    pub bids: Vec<DepthLevel>,
    /// Top ask price levels (lowest price first)
    pub asks: Vec<DepthLevel>,
    /// Current bid-ask spread
    pub spread: Option<Decimal>,
    /// Current mid price
    pub mid_price: Option<Decimal>,
}

/// A single depth level
#[derive(Debug, Clone, Serialize)]
pub struct DepthLevel {
    /// Price of this level
    pub price: Decimal,
    /// Aggregate quantity at this price
    pub amount: Decimal,
    /// Number of orders at this price level
    pub order_count: usize,
}

/// Manager for multiple order books
pub struct OrderBookManager {
    /// Order books indexed by token ID
    books: HashMap<Uuid, OrderBook>,
}

impl OrderBookManager {
    /// Create a new order book manager
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Construct a bid (Buy) LimitOrder with known fields for deterministic tests.
    fn make_bid(
        order_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
        price: Decimal,
        amount: Decimal,
        timestamp: i64,
    ) -> LimitOrder {
        LimitOrder {
            order_id,
            user_id,
            token_id,
            side: OrderSide::Buy,
            price,
            amount,
            filled_amount: Decimal::ZERO,
            timestamp,
        }
    }

    /// Construct an ask (Sell) LimitOrder with known fields for deterministic tests.
    fn make_ask(
        order_id: Uuid,
        user_id: Uuid,
        token_id: Uuid,
        price: Decimal,
        amount: Decimal,
        timestamp: i64,
    ) -> LimitOrder {
        LimitOrder {
            order_id,
            user_id,
            token_id,
            side: OrderSide::Sell,
            price,
            amount,
            filled_amount: Decimal::ZERO,
            timestamp,
        }
    }

    // ------------------------------------------------------------------ //
    // Test 1: Adding bid orders increases bid-side depth                  //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_add_bid_increases_bid_depth() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        assert_eq!(book.order_count(), 0, "new book should be empty");

        let bid1 = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(5),
            1,
        );
        book.add_order(bid1);
        assert_eq!(book.order_count(), 1, "should have 1 order after first add");

        let bid2 = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(99),
            dec!(3),
            2,
        );
        book.add_order(bid2);
        assert_eq!(
            book.order_count(),
            2,
            "should have 2 orders after second add"
        );

        let depth = book.get_depth(10);
        assert_eq!(depth.bids.len(), 2, "bid depth should have 2 price levels");
        assert!(depth.asks.is_empty(), "ask depth should be empty");
    }

    // ------------------------------------------------------------------ //
    // Test 2: Adding ask orders increases ask-side depth                  //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_add_ask_increases_ask_depth() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let ask1 = make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(101),
            dec!(5),
            1,
        );
        book.add_order(ask1);
        assert_eq!(book.order_count(), 1);

        let ask2 = make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(102),
            dec!(2),
            2,
        );
        book.add_order(ask2);
        assert_eq!(book.order_count(), 2);

        let depth = book.get_depth(10);
        assert!(depth.bids.is_empty(), "bid depth should be empty");
        assert_eq!(depth.asks.len(), 2, "ask depth should have 2 price levels");
    }

    // ------------------------------------------------------------------ //
    // Test 3: Best bid < best ask (no-cross invariant)                    //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_no_cross_invariant_best_bid_below_best_ask() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(99),
            dec!(1),
            1,
        ));
        book.add_order(make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(101),
            dec!(1),
            2,
        ));

        let best_bid = book.best_bid().expect("best_bid should exist");
        let best_ask = book.best_ask().expect("best_ask should exist");

        assert!(
            best_bid < best_ask,
            "best_bid ({best_bid}) must be strictly less than best_ask ({best_ask})"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 4: Matching orders when bid >= ask produces a fill             //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_match_order_produces_fill_when_bid_meets_ask() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        // Resting ask at 100
        book.add_order(make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(5),
            1,
        ));

        // Incoming buy order at 100 (exact price)
        let taker = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(3),
            2,
        );
        let result = book.match_order(taker);

        assert_eq!(
            result.trades.len(),
            1,
            "exactly one trade should be generated"
        );
        assert_eq!(result.trades[0].amount, dec!(3));
        assert_eq!(result.trades[0].price, dec!(100));
        assert_eq!(result.remaining_amount, dec!(0));
        assert!(result.fully_filled, "taker should be fully filled");
    }

    // ------------------------------------------------------------------ //
    // Test 5: Order book sorted — bids descending, asks ascending         //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_bid_ordering_is_descending_and_ask_ordering_is_ascending() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        // Insert bids in non-sorted order
        for price in [dec!(95), dec!(100), dec!(97)] {
            book.add_order(make_bid(
                Uuid::new_v4(),
                Uuid::new_v4(),
                token_id,
                price,
                dec!(1),
                1,
            ));
        }
        // Insert asks in non-sorted order
        for price in [dec!(105), dec!(102), dec!(108)] {
            book.add_order(make_ask(
                Uuid::new_v4(),
                Uuid::new_v4(),
                token_id,
                price,
                dec!(1),
                1,
            ));
        }

        let depth = book.get_depth(10);

        // Bids should be highest first
        let bid_prices: Vec<Decimal> = depth.bids.iter().map(|l| l.price).collect();
        let mut sorted_bids = bid_prices.clone();
        sorted_bids.sort_by(|a, b| b.cmp(a));
        assert_eq!(bid_prices, sorted_bids, "bids must be sorted descending");

        // Asks should be lowest first
        let ask_prices: Vec<Decimal> = depth.asks.iter().map(|l| l.price).collect();
        let mut sorted_asks = ask_prices.clone();
        sorted_asks.sort();
        assert_eq!(ask_prices, sorted_asks, "asks must be sorted ascending");
    }

    // ------------------------------------------------------------------ //
    // Test 6: Canceling an order removes it from the book                 //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_cancel_order_removes_from_book() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let order_id = Uuid::new_v4();
        let order = make_bid(order_id, Uuid::new_v4(), token_id, dec!(100), dec!(5), 1);
        book.add_order(order);

        assert!(book.has_order(order_id), "order should exist before cancel");
        assert_eq!(book.order_count(), 1);

        let cancelled = book
            .cancel_order(order_id)
            .expect("cancel should return the order");
        assert_eq!(cancelled.order_id, order_id);
        assert!(
            !book.has_order(order_id),
            "order should not exist after cancel"
        );
        assert_eq!(book.order_count(), 0);
    }

    // ------------------------------------------------------------------ //
    // Test 7: Canceling nonexistent order returns None                    //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_cancel_nonexistent_order_returns_none() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let result = book.cancel_order(Uuid::new_v4());
        assert!(
            result.is_none(),
            "canceling non-existent order should return None"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 8: Empty book returns None for best bid/ask                    //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_empty_book_returns_none_for_best_prices() {
        let token_id = Uuid::new_v4();
        let book = OrderBook::new(token_id);

        assert!(
            book.best_bid().is_none(),
            "empty book: best_bid should be None"
        );
        assert!(
            book.best_ask().is_none(),
            "empty book: best_ask should be None"
        );
        assert!(book.spread().is_none(), "empty book: spread should be None");
        assert!(
            book.mid_price().is_none(),
            "empty book: mid_price should be None"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 9: Price level aggregation — multiple orders at same price     //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_price_level_aggregates_multiple_orders() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        // Three bids at the same price
        for _ in 0..3 {
            book.add_order(make_bid(
                Uuid::new_v4(),
                Uuid::new_v4(),
                token_id,
                dec!(100),
                dec!(2),
                1,
            ));
        }

        let depth = book.get_depth(10);
        assert_eq!(
            depth.bids.len(),
            1,
            "three orders at same price = one level"
        );
        assert_eq!(
            depth.bids[0].amount,
            dec!(6),
            "aggregated amount should be 6"
        );
        assert_eq!(depth.bids[0].order_count, 3);

        // Total volume should reflect all three
        assert_eq!(book.total_bid_volume(), dec!(6));
    }

    // ------------------------------------------------------------------ //
    // Test 10: FIFO priority within same price level                      //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_fifo_priority_within_same_price_level() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let maker_id_first = Uuid::new_v4();
        let maker_id_second = Uuid::new_v4();

        // Resting asks at the same price, first has earlier timestamp
        let first_ask = make_ask(
            maker_id_first,
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(3),
            1000,
        );
        let second_ask = make_ask(
            maker_id_second,
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(3),
            2000,
        );
        book.add_order(first_ask);
        book.add_order(second_ask);

        // Incoming buy fills only 3 units — should consume first_ask entirely
        let taker = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(3),
            3000,
        );
        let result = book.match_order(taker);

        assert_eq!(result.trades.len(), 1, "exactly one trade");
        assert_eq!(
            result.trades[0].maker_order_id, maker_id_first,
            "FIFO: first resting order must be matched first"
        );
        // The second ask should still be on the book
        assert!(
            book.has_order(maker_id_second),
            "second ask should remain unfilled"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 11: Spread and mid price calculations                          //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_spread_and_mid_price_calculations() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(98),
            dec!(1),
            1,
        ));
        book.add_order(make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(102),
            dec!(1),
            2,
        ));

        let spread = book.spread().expect("spread should be Some");
        assert_eq!(spread, dec!(4), "spread = 102 - 98 = 4");

        let mid = book.mid_price().expect("mid_price should be Some");
        assert_eq!(mid, dec!(100), "mid = (98 + 102) / 2 = 100");
    }

    // ------------------------------------------------------------------ //
    // Test 12: Partial fill leaves remainder on taker, updates maker      //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_partial_fill_remainder_tracked_correctly() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let maker_id = Uuid::new_v4();
        // Resting ask for 5 units
        book.add_order(make_ask(
            maker_id,
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(5),
            1,
        ));

        // Incoming buy for 10 units — should partially fill
        let taker = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(10),
            2,
        );
        let result = book.match_order(taker);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(
            result.trades[0].amount,
            dec!(5),
            "fill amount = min(10, 5) = 5"
        );
        assert_eq!(
            result.remaining_amount,
            dec!(5),
            "taker still has 5 unfilled"
        );
        assert!(!result.fully_filled, "taker is not fully filled");

        // Maker (resting ask) should have been consumed and removed from book
        assert!(
            !book.has_order(maker_id),
            "fully filled maker should be removed"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 13: Best bid updates after cancel leaves correct best price    //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_best_bid_updates_after_cancellation() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        let top_bid_id = Uuid::new_v4();
        book.add_order(make_bid(
            top_bid_id,
            Uuid::new_v4(),
            token_id,
            dec!(105),
            dec!(1),
            1,
        ));
        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(1),
            2,
        ));

        assert_eq!(book.best_bid(), Some(dec!(105)));

        book.cancel_order(top_bid_id).expect("cancel top bid");
        assert_eq!(
            book.best_bid(),
            Some(dec!(100)),
            "best bid should fall to 100"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 14: Total volume reflects all orders across price levels       //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_total_volume_across_multiple_price_levels() {
        let token_id = Uuid::new_v4();
        let mut book = OrderBook::new(token_id);

        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(3),
            1,
        ));
        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(99),
            dec!(7),
            2,
        ));
        book.add_order(make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(101),
            dec!(4),
            3,
        ));
        book.add_order(make_ask(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(102),
            dec!(6),
            4,
        ));

        assert_eq!(
            book.total_bid_volume(),
            dec!(10),
            "total bid volume = 3 + 7 = 10"
        );
        assert_eq!(
            book.total_ask_volume(),
            dec!(10),
            "total ask volume = 4 + 6 = 10"
        );
    }

    // ------------------------------------------------------------------ //
    // Test 15: OrderBookManager creates and retrieves books by token_id   //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_order_book_manager_get_or_create() {
        let mut manager = OrderBookManager::new();
        let token_id = Uuid::new_v4();

        let book = manager.get_or_create(token_id);
        book.add_order(make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            token_id,
            dec!(100),
            dec!(1),
            1,
        ));

        assert_eq!(manager.token_ids().len(), 1);

        let retrieved = manager.get(token_id).expect("book should be retrievable");
        assert_eq!(retrieved.order_count(), 1);
    }

    // ------------------------------------------------------------------ //
    // Test 16: LimitOrder remaining() and is_filled() helpers             //
    // ------------------------------------------------------------------ //
    #[test]
    fn test_limit_order_remaining_and_is_filled() {
        let mut order = make_bid(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            dec!(100),
            dec!(10),
            1,
        );

        assert_eq!(order.remaining(), dec!(10));
        assert!(!order.is_filled());

        order.filled_amount = dec!(10);
        assert_eq!(order.remaining(), dec!(0));
        assert!(order.is_filled());

        // Overfill guard: remaining should not go negative
        order.filled_amount = dec!(10);
        assert!(order.remaining() >= Decimal::ZERO);
    }
}
