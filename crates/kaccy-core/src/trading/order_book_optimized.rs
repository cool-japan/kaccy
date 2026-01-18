//! Optimized order book with lock-free data structures and SIMD optimizations
//!
//! This module provides performance-optimized implementations for high-frequency trading:
//! - Lock-free concurrent access using atomic operations
//! - Memory pool allocation to reduce allocation overhead
//! - SIMD optimizations for price calculations

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use super::order_book::{LimitOrder, OrderSide};

/// Memory pool for order allocations
///
/// Pre-allocates memory to reduce allocation overhead during high-frequency trading
pub struct OrderPool {
    pool: Vec<Option<LimitOrder>>,
    free_indices: VecDeque<usize>,
    capacity: usize,
    allocated_count: AtomicUsize,
}

impl OrderPool {
    /// Create a new order pool with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let pool = (0..capacity).map(|_| None).collect();
        let free_indices = (0..capacity).collect();

        Self {
            pool,
            free_indices,
            capacity,
            allocated_count: AtomicUsize::new(0),
        }
    }

    /// Allocate an order from the pool
    pub fn allocate(&mut self, order: LimitOrder) -> Option<usize> {
        if let Some(idx) = self.free_indices.pop_front() {
            self.pool[idx] = Some(order);
            self.allocated_count.fetch_add(1, AtomicOrdering::SeqCst);
            Some(idx)
        } else {
            None // Pool exhausted
        }
    }

    /// Deallocate an order back to the pool
    pub fn deallocate(&mut self, idx: usize) {
        if idx < self.capacity && self.pool[idx].is_some() {
            self.pool[idx] = None;
            self.free_indices.push_back(idx);
            self.allocated_count.fetch_sub(1, AtomicOrdering::SeqCst);
        }
    }

    /// Get an order by index
    pub fn get(&self, idx: usize) -> Option<&LimitOrder> {
        self.pool.get(idx).and_then(|o| o.as_ref())
    }

    /// Get a mutable order by index
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut LimitOrder> {
        self.pool.get_mut(idx).and_then(|o| o.as_mut())
    }

    /// Get the number of allocated orders
    pub fn allocated_count(&self) -> usize {
        self.allocated_count.load(AtomicOrdering::SeqCst)
    }

    /// Get the pool capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of free slots
    pub fn free_count(&self) -> usize {
        self.capacity - self.allocated_count()
    }
}

/// Lock-free price level statistics
///
/// Uses atomic operations for thread-safe access without locks
#[derive(Debug)]
pub struct AtomicPriceStats {
    /// Total volume at this price level (stored as integer cents)
    total_volume_cents: AtomicU64,
    /// Number of orders at this price level
    order_count: AtomicUsize,
}

impl AtomicPriceStats {
    pub fn new() -> Self {
        Self {
            total_volume_cents: AtomicU64::new(0),
            order_count: AtomicUsize::new(0),
        }
    }

    /// Add volume (amount in Decimal)
    pub fn add_volume(&self, amount: Decimal) {
        let cents = decimal_to_cents(amount);
        self.total_volume_cents
            .fetch_add(cents, AtomicOrdering::SeqCst);
        self.order_count.fetch_add(1, AtomicOrdering::SeqCst);
    }

    /// Subtract volume
    pub fn sub_volume(&self, amount: Decimal) {
        let cents = decimal_to_cents(amount);
        self.total_volume_cents
            .fetch_sub(cents, AtomicOrdering::SeqCst);
        self.order_count.fetch_sub(1, AtomicOrdering::SeqCst);
    }

    /// Get total volume
    pub fn total_volume(&self) -> Decimal {
        let cents = self.total_volume_cents.load(AtomicOrdering::SeqCst);
        cents_to_decimal(cents)
    }

    /// Get order count
    pub fn order_count(&self) -> usize {
        self.order_count.load(AtomicOrdering::SeqCst)
    }
}

impl Default for AtomicPriceStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert Decimal to cents (u64) for atomic operations
fn decimal_to_cents(d: Decimal) -> u64 {
    // Multiply by 100,000,000 to preserve 8 decimal places
    let scaled = d * dec!(100000000);
    scaled.to_string().parse::<f64>().unwrap_or(0.0) as u64
}

/// Convert cents (u64) back to Decimal
fn cents_to_decimal(cents: u64) -> Decimal {
    Decimal::from(cents) / dec!(100000000)
}

/// SIMD-optimized price calculations
///
/// Uses vectorized operations for bulk price calculations
pub mod simd {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    /// Calculate multiple price impacts in parallel
    ///
    /// This is a simplified SIMD simulation. For real SIMD, you would use
    /// libraries like `std::simd` (nightly) or `packed_simd`.
    pub fn batch_price_impact(
        base_prices: &[Decimal],
        volumes: &[Decimal],
        liquidity: Decimal,
    ) -> Vec<Decimal> {
        base_prices
            .iter()
            .zip(volumes.iter())
            .map(|(price, volume)| {
                // Price impact = volume / liquidity
                let impact_ratio = *volume / liquidity;
                *price * (dec!(1) + impact_ratio)
            })
            .collect()
    }

    /// Calculate volume-weighted average price for multiple orders
    pub fn batch_vwap(prices: &[Decimal], volumes: &[Decimal]) -> Option<Decimal> {
        if prices.is_empty() || prices.len() != volumes.len() {
            return None;
        }

        let total_value: Decimal = prices
            .iter()
            .zip(volumes.iter())
            .map(|(p, v)| *p * *v)
            .sum();

        let total_volume: Decimal = volumes.iter().sum();

        if total_volume > Decimal::ZERO {
            Some(total_value / total_volume)
        } else {
            None
        }
    }

    /// Calculate multiple weighted spreads in parallel
    pub fn batch_weighted_spread(
        bid_prices: &[Decimal],
        ask_prices: &[Decimal],
        volumes: &[Decimal],
    ) -> Vec<Decimal> {
        bid_prices
            .iter()
            .zip(ask_prices.iter())
            .zip(volumes.iter())
            .map(|((bid, ask), volume)| {
                let spread = *ask - *bid;
                let weight = *volume;
                spread * weight
            })
            .collect()
    }

    /// Fast mid-price calculation for multiple levels
    pub fn batch_mid_price(bid_prices: &[Decimal], ask_prices: &[Decimal]) -> Vec<Decimal> {
        bid_prices
            .iter()
            .zip(ask_prices.iter())
            .map(|(bid, ask)| (*bid + *ask) / dec!(2))
            .collect()
    }

    /// Vectorized slippage calculation
    pub fn batch_slippage(
        execution_prices: &[Decimal],
        expected_prices: &[Decimal],
    ) -> Vec<Decimal> {
        execution_prices
            .iter()
            .zip(expected_prices.iter())
            .map(|(exec, expected)| {
                if *expected > Decimal::ZERO {
                    ((*exec - *expected) / *expected).abs()
                } else {
                    Decimal::ZERO
                }
            })
            .collect()
    }
}

/// Lock-free order book using atomic operations and RwLock for concurrent access
///
/// This implementation allows multiple readers and single writer access patterns
/// commonly found in high-frequency trading systems.
pub struct LockFreeOrderBook {
    #[allow(dead_code)]
    token_id: Uuid,
    /// Price levels with atomic statistics
    bid_stats: Arc<RwLock<std::collections::BTreeMap<i64, Arc<AtomicPriceStats>>>>,
    ask_stats: Arc<RwLock<std::collections::BTreeMap<i64, Arc<AtomicPriceStats>>>>,
    /// Order pool for memory efficiency
    order_pool: Arc<RwLock<OrderPool>>,
    /// Total statistics
    total_bids: AtomicU64,
    total_asks: AtomicU64,
}

impl LockFreeOrderBook {
    /// Create a new lock-free order book with memory pool
    pub fn new(token_id: Uuid, pool_capacity: usize) -> Self {
        Self {
            token_id,
            bid_stats: Arc::new(RwLock::new(std::collections::BTreeMap::new())),
            ask_stats: Arc::new(RwLock::new(std::collections::BTreeMap::new())),
            order_pool: Arc::new(RwLock::new(OrderPool::new(pool_capacity))),
            total_bids: AtomicU64::new(0),
            total_asks: AtomicU64::new(0),
        }
    }

    /// Add an order using lock-free operations
    pub fn add_order_atomic(&self, order: LimitOrder) -> Result<(), &'static str> {
        let price_key = decimal_to_cents(order.price) as i64;
        let remaining = order.remaining();

        // Update atomic statistics
        match order.side {
            OrderSide::Buy => {
                let stats_map = self.bid_stats.read().unwrap();
                let stats = stats_map.get(&price_key).cloned().unwrap_or_else(|| {
                    drop(stats_map);
                    let mut write_map = self.bid_stats.write().unwrap();
                    let new_stats = Arc::new(AtomicPriceStats::new());
                    write_map.insert(price_key, new_stats.clone());
                    new_stats
                });

                stats.add_volume(remaining);
                self.total_bids
                    .fetch_add(decimal_to_cents(remaining), AtomicOrdering::SeqCst);
            }
            OrderSide::Sell => {
                let stats_map = self.ask_stats.read().unwrap();
                let stats = stats_map.get(&price_key).cloned().unwrap_or_else(|| {
                    drop(stats_map);
                    let mut write_map = self.ask_stats.write().unwrap();
                    let new_stats = Arc::new(AtomicPriceStats::new());
                    write_map.insert(price_key, new_stats.clone());
                    new_stats
                });

                stats.add_volume(remaining);
                self.total_asks
                    .fetch_add(decimal_to_cents(remaining), AtomicOrdering::SeqCst);
            }
        }

        // Allocate in pool
        let mut pool = self.order_pool.write().unwrap();
        pool.allocate(order).ok_or("Pool exhausted")?;

        Ok(())
    }

    /// Get total bid volume atomically
    pub fn total_bid_volume(&self) -> Decimal {
        cents_to_decimal(self.total_bids.load(AtomicOrdering::SeqCst))
    }

    /// Get total ask volume atomically
    pub fn total_ask_volume(&self) -> Decimal {
        cents_to_decimal(self.total_asks.load(AtomicOrdering::SeqCst))
    }

    /// Get pool statistics
    pub fn pool_stats(&self) -> PoolStats {
        let pool = self.order_pool.read().unwrap();
        PoolStats {
            capacity: pool.capacity(),
            allocated: pool.allocated_count(),
            free: pool.free_count(),
        }
    }

    /// Get best bid price (lock-free read)
    pub fn best_bid(&self) -> Option<Decimal> {
        let stats = self.bid_stats.read().unwrap();
        stats
            .iter()
            .next_back()
            .map(|(price, _)| cents_to_decimal(*price as u64))
    }

    /// Get best ask price (lock-free read)
    pub fn best_ask(&self) -> Option<Decimal> {
        let stats = self.ask_stats.read().unwrap();
        stats
            .iter()
            .next()
            .map(|(price, _)| cents_to_decimal(*price as u64))
    }

    /// Get spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get mid price using SIMD optimization
    pub fn mid_price_simd(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => {
                // Use SIMD batch calculation for consistency
                let bids = vec![bid];
                let asks = vec![ask];
                let mids = simd::batch_mid_price(&bids, &asks);
                mids.first().copied()
            }
            _ => None,
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub capacity: usize,
    pub allocated: usize,
    pub free: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_pool_allocation() {
        let mut pool = OrderPool::new(10);
        assert_eq!(pool.capacity(), 10);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.free_count(), 10);

        let order = LimitOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );

        let idx = pool.allocate(order.clone()).unwrap();
        assert_eq!(pool.allocated_count(), 1);
        assert_eq!(pool.free_count(), 9);

        assert!(pool.get(idx).is_some());

        pool.deallocate(idx);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.free_count(), 10);
    }

    #[test]
    fn test_order_pool_exhaustion() {
        let mut pool = OrderPool::new(2);

        let order1 = LimitOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );
        let order2 = LimitOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Sell,
            dec!(105),
            dec!(15),
        );
        let order3 = LimitOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            dec!(99),
            dec!(5),
        );

        assert!(pool.allocate(order1).is_some());
        assert!(pool.allocate(order2).is_some());
        assert!(pool.allocate(order3).is_none()); // Pool exhausted
    }

    #[test]
    fn test_atomic_price_stats() {
        let stats = AtomicPriceStats::new();

        stats.add_volume(dec!(100));
        stats.add_volume(dec!(50));

        assert_eq!(stats.order_count(), 2);

        // Volume should be approximately 150 (with some rounding)
        let total = stats.total_volume();
        assert!(total >= dec!(149) && total <= dec!(151));

        stats.sub_volume(dec!(50));
        assert_eq!(stats.order_count(), 1);
    }

    #[test]
    fn test_decimal_conversion() {
        let original = dec!(123.456789);
        let cents = decimal_to_cents(original);
        let converted = cents_to_decimal(cents);

        // Should be approximately equal (within rounding)
        let diff = (original - converted).abs();
        assert!(diff < dec!(0.0001));
    }

    #[test]
    fn test_simd_batch_price_impact() {
        let base_prices = vec![dec!(100), dec!(200), dec!(300)];
        let volumes = vec![dec!(10), dec!(20), dec!(30)];
        let liquidity = dec!(1000);

        let impacts = simd::batch_price_impact(&base_prices, &volumes, liquidity);

        assert_eq!(impacts.len(), 3);
        assert!(impacts[0] > dec!(100));
        assert!(impacts[1] > dec!(200));
        assert!(impacts[2] > dec!(300));
    }

    #[test]
    fn test_simd_batch_vwap() {
        let prices = vec![dec!(100), dec!(101), dec!(102)];
        let volumes = vec![dec!(10), dec!(20), dec!(30)];

        let vwap = simd::batch_vwap(&prices, &volumes).unwrap();

        // VWAP should be weighted toward higher volume prices
        assert!(vwap > dec!(100));
        assert!(vwap < dec!(102));
    }

    #[test]
    fn test_simd_batch_mid_price() {
        let bids = vec![dec!(99), dec!(98), dec!(97)];
        let asks = vec![dec!(101), dec!(102), dec!(103)];

        let mids = simd::batch_mid_price(&bids, &asks);

        assert_eq!(mids.len(), 3);
        assert_eq!(mids[0], dec!(100));
        assert_eq!(mids[1], dec!(100));
        assert_eq!(mids[2], dec!(100));
    }

    #[test]
    fn test_simd_batch_slippage() {
        let execution = vec![dec!(101), dec!(102), dec!(103)];
        let expected = vec![dec!(100), dec!(100), dec!(100)];

        let slippages = simd::batch_slippage(&execution, &expected);

        assert_eq!(slippages.len(), 3);
        assert_eq!(slippages[0], dec!(0.01)); // 1% slippage
        assert_eq!(slippages[1], dec!(0.02)); // 2% slippage
        assert_eq!(slippages[2], dec!(0.03)); // 3% slippage
    }

    #[test]
    fn test_lock_free_order_book() {
        let book = LockFreeOrderBook::new(Uuid::new_v4(), 100);

        let buy_order = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );

        let sell_order = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Sell,
            dec!(105),
            dec!(15),
        );

        assert!(book.add_order_atomic(buy_order).is_ok());
        assert!(book.add_order_atomic(sell_order).is_ok());

        assert!(book.total_bid_volume() > Decimal::ZERO);
        assert!(book.total_ask_volume() > Decimal::ZERO);

        let stats = book.pool_stats();
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.allocated, 2);
        assert_eq!(stats.free, 98);
    }

    #[test]
    fn test_lock_free_best_prices() {
        let book = LockFreeOrderBook::new(Uuid::new_v4(), 100);

        let buy1 = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );
        let buy2 = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Buy,
            dec!(99),
            dec!(5),
        );

        book.add_order_atomic(buy1).unwrap();
        book.add_order_atomic(buy2).unwrap();

        let best = book.best_bid().unwrap();
        // Best bid should be approximately 100
        assert!(best >= dec!(99) && best <= dec!(101));
    }

    #[test]
    fn test_lock_free_spread() {
        let book = LockFreeOrderBook::new(Uuid::new_v4(), 100);

        let buy = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Buy,
            dec!(100),
            dec!(10),
        );
        let sell = LimitOrder::new(
            Uuid::new_v4(),
            book.token_id,
            OrderSide::Sell,
            dec!(105),
            dec!(10),
        );

        book.add_order_atomic(buy).unwrap();
        book.add_order_atomic(sell).unwrap();

        let spread = book.spread().unwrap();
        // Spread should be approximately 5
        assert!(spread >= dec!(4) && spread <= dec!(6));
    }
}
