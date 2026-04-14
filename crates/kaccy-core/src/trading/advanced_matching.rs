//! Advanced order matching algorithms
//!
//! Implements pro-rata, hybrid, and dark pool matching mechanisms.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide, Trade};

/// Matching algorithm type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchingAlgorithm {
    /// Pure price-time priority (FIFO)
    PriceTime,
    /// Pro-rata allocation
    ProRata,
    /// Hybrid: top-of-book gets priority, rest is pro-rata
    Hybrid {
        /// Percentage of fill allocated to top-of-book orders (0-100)
        top_priority_pct: u8,
    },
}

/// Configuration for pro-rata matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProRataConfig {
    /// Minimum fill guarantee per order (e.g., 0.01 = 1%)
    pub minimum_fill_guarantee: Decimal,
    /// Whether to give priority to the first order at each price level
    pub top_of_book_priority: bool,
}

impl Default for ProRataConfig {
    fn default() -> Self {
        Self {
            minimum_fill_guarantee: dec!(0.01), // 1%
            top_of_book_priority: true,
        }
    }
}

/// Pro-rata matcher
#[derive(Debug)]
pub struct ProRataMatcher {
    config: ProRataConfig,
}

impl ProRataMatcher {
    /// Creates a new `ProRataMatcher` with the given configuration.
    pub fn new(config: ProRataConfig) -> Self {
        Self { config }
    }

    /// Match orders using pro-rata allocation
    ///
    /// Distributes fills proportionally based on order size at each price level.
    pub fn match_orders(
        &self,
        _taker_order: &LimitOrder,
        mut maker_orders: VecDeque<LimitOrder>,
        available_amount: Decimal,
    ) -> Vec<(LimitOrder, Decimal)> {
        let mut fills = Vec::new();
        let mut remaining = available_amount;

        if maker_orders.is_empty() || remaining <= Decimal::ZERO {
            return fills;
        }

        // Calculate total liquidity at this price level (for potential future use)
        let _total_liquidity: Decimal = maker_orders.iter().map(|o| o.remaining()).sum();

        // Top-of-book priority: allocate a portion to the first order
        if self.config.top_of_book_priority {
            if let Some(top_order) = maker_orders.pop_front() {
                let top_fill =
                    (remaining * self.config.minimum_fill_guarantee).min(top_order.remaining());

                if top_fill > Decimal::ZERO {
                    fills.push((top_order.clone(), top_fill));
                    remaining -= top_fill;

                    // Put the order back if not fully filled
                    if top_fill < top_order.remaining() {
                        let mut updated = top_order;
                        updated.filled_amount += top_fill;
                        maker_orders.push_front(updated);
                    }
                }
            }
        }

        // Pro-rata allocation for remaining amount
        if remaining > Decimal::ZERO && !maker_orders.is_empty() {
            let remaining_liquidity: Decimal = maker_orders.iter().map(|o| o.remaining()).sum();

            let total_to_allocate = remaining;

            for order in maker_orders {
                if remaining <= Decimal::ZERO {
                    break;
                }

                let order_remaining = order.remaining();

                // Calculate pro-rata share based on original total
                let pro_rata_share = if remaining_liquidity > Decimal::ZERO {
                    (order_remaining / remaining_liquidity) * total_to_allocate
                } else {
                    Decimal::ZERO
                };

                // Apply minimum fill guarantee based on original total
                let minimum_fill = total_to_allocate * self.config.minimum_fill_guarantee;
                let fill_amount = pro_rata_share
                    .max(minimum_fill)
                    .min(order_remaining)
                    .min(remaining);

                if fill_amount > Decimal::ZERO {
                    fills.push((order, fill_amount));
                    remaining -= fill_amount;
                }
            }
        }

        fills
    }
}

/// Hybrid matcher combining price-time and pro-rata
#[derive(Debug)]
pub struct HybridMatcher {
    /// Percentage allocated to price-time priority (0-100)
    pub price_time_pct: u8,
    pro_rata_config: ProRataConfig,
}

impl HybridMatcher {
    /// Creates a new `HybridMatcher` with the given price-time percentage and pro-rata config.
    pub fn new(price_time_pct: u8, pro_rata_config: ProRataConfig) -> Self {
        assert!(price_time_pct <= 100, "Percentage must be 0-100");
        Self {
            price_time_pct,
            pro_rata_config,
        }
    }

    /// Match using hybrid algorithm: price-time for top portion, pro-rata for rest
    pub fn match_orders(
        &self,
        taker_order: &LimitOrder,
        maker_orders: VecDeque<LimitOrder>,
        available_amount: Decimal,
    ) -> Vec<(LimitOrder, Decimal)> {
        let mut fills = Vec::new();

        if maker_orders.is_empty() || available_amount <= Decimal::ZERO {
            return fills;
        }

        // Calculate allocation
        let price_time_amount = available_amount * Decimal::from(self.price_time_pct) / dec!(100);
        let pro_rata_amount = available_amount - price_time_amount;

        // Phase 1: Price-time priority allocation
        let mut remaining_pt = price_time_amount;
        let mut orders_for_prorata = VecDeque::new();

        for order in maker_orders {
            if remaining_pt <= Decimal::ZERO {
                orders_for_prorata.push_back(order);
                continue;
            }

            let fill = remaining_pt.min(order.remaining());

            if fill > Decimal::ZERO {
                fills.push((order.clone(), fill));
                remaining_pt -= fill;

                // If order has remaining, add to pro-rata pool
                if fill < order.remaining() {
                    let mut updated = order;
                    updated.filled_amount += fill;
                    orders_for_prorata.push_back(updated);
                }
            } else {
                orders_for_prorata.push_back(order);
            }
        }

        // Phase 2: Pro-rata allocation
        if pro_rata_amount > Decimal::ZERO && !orders_for_prorata.is_empty() {
            let pro_rata_matcher = ProRataMatcher::new(self.pro_rata_config.clone());
            let pro_rata_fills =
                pro_rata_matcher.match_orders(taker_order, orders_for_prorata, pro_rata_amount);
            fills.extend(pro_rata_fills);
        }

        fills
    }
}

/// Dark pool for hidden liquidity
#[derive(Debug)]
pub struct DarkPool {
    /// Token ID for this dark pool
    pub token_id: Uuid,
    /// Hidden orders indexed by price
    hidden_orders: HashMap<(OrderSide, Decimal), VecDeque<LimitOrder>>,
    /// Minimum execution quantity (MEQ) to prevent information leakage
    pub minimum_execution_qty: Decimal,
    /// Whether to allow price improvement
    pub allow_price_improvement: bool,
}

impl DarkPool {
    /// Creates a new `DarkPool` for the given token with the specified minimum execution quantity.
    pub fn new(token_id: Uuid, minimum_execution_qty: Decimal) -> Self {
        Self {
            token_id,
            hidden_orders: HashMap::new(),
            minimum_execution_qty,
            allow_price_improvement: true,
        }
    }

    /// Add a hidden order to the dark pool
    pub fn add_hidden_order(&mut self, order: LimitOrder) {
        let key = (order.side, order.price);
        self.hidden_orders.entry(key).or_default().push_back(order);
    }

    /// Try to match an order in the dark pool
    ///
    /// Returns trades if sufficient liquidity exists, None if MEQ not met
    pub fn try_match(&mut self, order: LimitOrder) -> Option<Vec<Trade>> {
        // Find matching orders on opposite side
        let opposite_side = match order.side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        let mut trades = Vec::new();
        let mut remaining = order.amount;
        let mut keys_to_remove = Vec::new();

        // Iterate through all price levels
        let mut matching_keys: Vec<_> = self
            .hidden_orders
            .keys()
            .filter(|(side, price)| *side == opposite_side && self.price_crosses(&order, *price))
            .cloned()
            .collect();

        // Sort by best price
        matching_keys.sort_by(|a, b| match order.side {
            OrderSide::Buy => a.1.cmp(&b.1),  // Lowest ask first
            OrderSide::Sell => b.1.cmp(&a.1), // Highest bid first
        });

        for key in matching_keys {
            if remaining <= Decimal::ZERO {
                break;
            }

            if let Some(orders) = self.hidden_orders.get_mut(&key) {
                while let Some(mut maker_order) = orders.pop_front() {
                    if remaining <= Decimal::ZERO {
                        orders.push_front(maker_order);
                        break;
                    }

                    let fill_amount = remaining.min(maker_order.remaining());

                    // Determine execution price with possible improvement
                    let execution_price = if self.allow_price_improvement {
                        (order.price + maker_order.price) / dec!(2) // Midpoint price
                    } else {
                        maker_order.price
                    };

                    trades.push(Trade {
                        trade_id: Uuid::new_v4(),
                        taker_order_id: order.order_id,
                        maker_order_id: maker_order.order_id,
                        price: execution_price,
                        amount: fill_amount,
                        taker_side: order.side,
                    });

                    maker_order.filled_amount += fill_amount;
                    remaining -= fill_amount;

                    // Put back if not fully filled
                    if !maker_order.is_filled() {
                        orders.push_front(maker_order);
                    }
                }

                if orders.is_empty() {
                    keys_to_remove.push(key);
                }
            }
        }

        // Clean up empty price levels
        for key in keys_to_remove {
            self.hidden_orders.remove(&key);
        }

        // Check MEQ requirement
        let total_filled = order.amount - remaining;
        if total_filled >= self.minimum_execution_qty {
            Some(trades)
        } else {
            // MEQ not met - no execution
            None
        }
    }

    /// Cancel a hidden order
    pub fn cancel_order(&mut self, order_id: Uuid) -> Option<LimitOrder> {
        for orders in self.hidden_orders.values_mut() {
            if let Some(idx) = orders.iter().position(|o| o.order_id == order_id) {
                return orders.remove(idx);
            }
        }
        None
    }

    /// Get total hidden liquidity
    pub fn total_liquidity(&self) -> (Decimal, Decimal) {
        let mut bid_liquidity = Decimal::ZERO;
        let mut ask_liquidity = Decimal::ZERO;

        for ((side, _), orders) in &self.hidden_orders {
            let total: Decimal = orders.iter().map(|o| o.remaining()).sum();
            match side {
                OrderSide::Buy => bid_liquidity += total,
                OrderSide::Sell => ask_liquidity += total,
            }
        }

        (bid_liquidity, ask_liquidity)
    }

    /// Check if price crosses for matching
    fn price_crosses(&self, taker: &LimitOrder, maker_price: Decimal) -> bool {
        match taker.side {
            OrderSide::Buy => taker.price >= maker_price,
            OrderSide::Sell => taker.price <= maker_price,
        }
    }

    /// Get order count
    pub fn order_count(&self) -> usize {
        self.hidden_orders.values().map(|orders| orders.len()).sum()
    }
}

/// Manager for multiple dark pools
#[derive(Debug)]
pub struct DarkPoolManager {
    pools: HashMap<Uuid, DarkPool>,
    default_meq: Decimal,
}

impl DarkPoolManager {
    /// Creates a new `DarkPoolManager` with the given default minimum execution quantity.
    pub fn new(default_meq: Decimal) -> Self {
        Self {
            pools: HashMap::new(),
            default_meq,
        }
    }

    /// Get or create a dark pool for a token
    pub fn get_or_create(&mut self, token_id: Uuid) -> &mut DarkPool {
        let meq = self.default_meq;
        self.pools
            .entry(token_id)
            .or_insert_with(|| DarkPool::new(token_id, meq))
    }

    /// Get a dark pool if it exists
    pub fn get(&self, token_id: Uuid) -> Option<&DarkPool> {
        self.pools.get(&token_id)
    }

    /// Get mutable dark pool if it exists
    pub fn get_mut(&mut self, token_id: Uuid) -> Option<&mut DarkPool> {
        self.pools.get_mut(&token_id)
    }

    /// Get total hidden liquidity across all pools
    pub fn total_liquidity(&self) -> HashMap<Uuid, (Decimal, Decimal)> {
        self.pools
            .iter()
            .map(|(id, pool)| (*id, pool.total_liquidity()))
            .collect()
    }
}

impl Default for DarkPoolManager {
    fn default() -> Self {
        Self::new(dec!(1.0)) // Default MEQ of 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_order(side: OrderSide, price: Decimal, amount: Decimal) -> LimitOrder {
        LimitOrder::new(Uuid::new_v4(), Uuid::new_v4(), side, price, amount)
    }

    #[test]
    fn test_pro_rata_basic_allocation() {
        let config = ProRataConfig {
            minimum_fill_guarantee: dec!(0.01),
            top_of_book_priority: false, // Disable for pure pro-rata test
        };
        let matcher = ProRataMatcher::new(config);

        let taker = create_test_order(OrderSide::Buy, dec!(100), dec!(10));

        let mut makers = VecDeque::new();
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(5)));
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(5)));

        let fills = matcher.match_orders(&taker, makers, dec!(6));

        // Should allocate proportionally
        assert_eq!(fills.len(), 2);

        // Total should equal available
        let total: Decimal = fills.iter().map(|(_, amt)| *amt).sum();
        assert_eq!(total, dec!(6));
    }

    #[test]
    fn test_pro_rata_top_priority() {
        let config = ProRataConfig {
            minimum_fill_guarantee: dec!(0.1), // 10%
            top_of_book_priority: true,
        };
        let matcher = ProRataMatcher::new(config);

        let taker = create_test_order(OrderSide::Buy, dec!(100), dec!(10));

        let mut makers = VecDeque::new();
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(10)));
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(10)));

        let fills = matcher.match_orders(&taker, makers, dec!(10));

        // First order should get at least 10% priority
        assert!(fills[0].1 >= dec!(1)); // 10% of 10
    }

    #[test]
    fn test_hybrid_matching() {
        let config = ProRataConfig::default();
        let matcher = HybridMatcher::new(70, config); // 70% price-time, 30% pro-rata

        let taker = create_test_order(OrderSide::Buy, dec!(100), dec!(10));

        let mut makers = VecDeque::new();
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(5)));
        makers.push_back(create_test_order(OrderSide::Sell, dec!(100), dec!(10)));

        let fills = matcher.match_orders(&taker, makers, dec!(10));

        assert!(!fills.is_empty());

        // Total should equal available
        let total: Decimal = fills.iter().map(|(_, amt)| *amt).sum();
        assert_eq!(total, dec!(10));
    }

    #[test]
    fn test_dark_pool_meq_enforcement() {
        let mut pool = DarkPool::new(Uuid::new_v4(), dec!(5)); // MEQ = 5

        // Add sell orders
        pool.add_hidden_order(create_test_order(OrderSide::Sell, dec!(100), dec!(3)));
        pool.add_hidden_order(create_test_order(OrderSide::Sell, dec!(100), dec!(3)));

        // Try to buy 4 units (below MEQ)
        let buy_order = create_test_order(OrderSide::Buy, dec!(100), dec!(4));
        let result = pool.try_match(buy_order);

        // Should fail MEQ check even though liquidity exists
        assert!(result.is_none());
    }

    #[test]
    fn test_dark_pool_successful_match() {
        let mut pool = DarkPool::new(Uuid::new_v4(), dec!(5)); // MEQ = 5

        // Add sell orders
        pool.add_hidden_order(create_test_order(OrderSide::Sell, dec!(100), dec!(10)));

        // Try to buy 6 units (above MEQ)
        let buy_order = create_test_order(OrderSide::Buy, dec!(100), dec!(6));
        let result = pool.try_match(buy_order);

        assert!(result.is_some());
        let trades = result.unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].amount, dec!(6));
    }

    #[test]
    fn test_dark_pool_price_improvement() {
        let mut pool = DarkPool::new(Uuid::new_v4(), dec!(1));
        pool.allow_price_improvement = true;

        // Sell at 100
        pool.add_hidden_order(create_test_order(OrderSide::Sell, dec!(100), dec!(10)));

        // Buy at 102 (willing to pay more)
        let buy_order = create_test_order(OrderSide::Buy, dec!(102), dec!(5));
        let result = pool.try_match(buy_order);

        assert!(result.is_some());
        let trades = result.unwrap();

        // Should execute at midpoint (101)
        assert_eq!(trades[0].price, dec!(101));
    }

    #[test]
    fn test_dark_pool_cancel_order() {
        let mut pool = DarkPool::new(Uuid::new_v4(), dec!(1));

        let order = create_test_order(OrderSide::Sell, dec!(100), dec!(10));
        let order_id = order.order_id;

        pool.add_hidden_order(order);
        assert_eq!(pool.order_count(), 1);

        let cancelled = pool.cancel_order(order_id);
        assert!(cancelled.is_some());
        assert_eq!(pool.order_count(), 0);
    }

    #[test]
    fn test_dark_pool_liquidity() {
        let mut pool = DarkPool::new(Uuid::new_v4(), dec!(1));

        pool.add_hidden_order(create_test_order(OrderSide::Buy, dec!(100), dec!(5)));
        pool.add_hidden_order(create_test_order(OrderSide::Sell, dec!(102), dec!(8)));

        let (bid_liq, ask_liq) = pool.total_liquidity();
        assert_eq!(bid_liq, dec!(5));
        assert_eq!(ask_liq, dec!(8));
    }

    #[test]
    fn test_dark_pool_manager() {
        let mut manager = DarkPoolManager::new(dec!(1));

        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        let pool1 = manager.get_or_create(token1);
        pool1.add_hidden_order(create_test_order(OrderSide::Buy, dec!(100), dec!(5)));

        let pool2 = manager.get_or_create(token2);
        pool2.add_hidden_order(create_test_order(OrderSide::Sell, dec!(200), dec!(10)));

        let liquidity = manager.total_liquidity();
        assert_eq!(liquidity.len(), 2);
    }
}
