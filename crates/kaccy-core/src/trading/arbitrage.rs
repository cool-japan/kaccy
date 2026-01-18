//! Cross-token arbitrage prevention

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::LiquidityPool;

/// Arbitrage opportunity detector
#[derive(Debug, Clone)]
pub struct ArbitrageDetector {
    /// Maximum allowed price discrepancy (0-1)
    pub max_price_discrepancy: Decimal,
    /// Minimum profit threshold to flag as arbitrage
    pub min_profit_threshold: Decimal,
    /// Time window for rate limiting arbitrage trades
    pub rate_limit_window_seconds: i64,
    /// Maximum arbitrage trades per user per window
    pub max_trades_per_window: i32,
}

impl Default for ArbitrageDetector {
    fn default() -> Self {
        Self {
            max_price_discrepancy: dec!(0.05), // 5%
            min_profit_threshold: dec!(0.01),  // 1%
            rate_limit_window_seconds: 3600,   // 1 hour
            max_trades_per_window: 10,
        }
    }
}

/// Arbitrage opportunity found between pools
#[derive(Debug, Clone, Serialize)]
pub struct ArbitrageOpportunity {
    /// Token pair involved
    pub token_a_id: Uuid,
    pub token_b_id: Uuid,
    /// Pool to buy from
    pub buy_pool_id: Uuid,
    /// Pool to sell to
    pub sell_pool_id: Uuid,
    /// Price in buy pool
    pub buy_price: Decimal,
    /// Price in sell pool
    pub sell_price: Decimal,
    /// Price discrepancy percentage
    pub price_discrepancy: Decimal,
    /// Estimated profit percentage
    pub profit_percentage: Decimal,
    /// Recommended trade amount
    pub recommended_amount: Decimal,
    /// When this opportunity was detected
    pub detected_at: DateTime<Utc>,
}

/// Arbitrage trade tracking
#[derive(Debug, Clone)]
pub struct ArbitrageTrade {
    pub user_id: Uuid,
    pub token_a_id: Uuid,
    pub token_b_id: Uuid,
    pub amount: Decimal,
    pub profit: Decimal,
    pub executed_at: DateTime<Utc>,
}

/// Arbitrage prevention state
pub struct ArbitragePreventor {
    detector: ArbitrageDetector,
    /// Recent arbitrage trades per user
    recent_trades: HashMap<Uuid, Vec<ArbitrageTrade>>,
    /// Flagged users for excessive arbitrage
    flagged_users: HashSet<Uuid>,
}

impl Default for ArbitragePreventor {
    fn default() -> Self {
        Self::new(ArbitrageDetector::default())
    }
}

impl ArbitragePreventor {
    /// Create new arbitrage preventor
    pub fn new(detector: ArbitrageDetector) -> Self {
        Self {
            detector,
            recent_trades: HashMap::new(),
            flagged_users: HashSet::new(),
        }
    }

    /// Detect arbitrage opportunities between pools
    pub fn detect_opportunities(&self, pools: &[LiquidityPool]) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();
        let now = Utc::now();

        // Build a map of token pairs to pools
        let mut pair_pools: HashMap<(Uuid, Uuid), Vec<&LiquidityPool>> = HashMap::new();

        for pool in pools {
            // Normalize pair (smaller UUID first)
            let pair = if pool.token_a_id < pool.token_b_id {
                (pool.token_a_id, pool.token_b_id)
            } else {
                (pool.token_b_id, pool.token_a_id)
            };

            pair_pools.entry(pair).or_default().push(pool);
        }

        // Check for price discrepancies in each pair
        for ((token_a, token_b), pair_pools) in pair_pools {
            if pair_pools.len() < 2 {
                continue; // Need at least 2 pools for arbitrage
            }

            // Find min and max prices
            for i in 0..pair_pools.len() {
                for j in (i + 1)..pair_pools.len() {
                    let pool1 = pair_pools[i];
                    let pool2 = pair_pools[j];

                    // Get prices (token_a in terms of token_b)
                    let price1 = self.get_normalized_price(pool1, token_a, token_b);
                    let price2 = self.get_normalized_price(pool2, token_a, token_b);

                    if price1 == dec!(0) || price2 == dec!(0) {
                        continue;
                    }

                    // Calculate discrepancy
                    let discrepancy = ((price2 - price1).abs() / price1).abs();

                    if discrepancy > self.detector.max_price_discrepancy {
                        // Arbitrage opportunity detected
                        let (buy_pool, sell_pool, buy_price, sell_price) = if price1 < price2 {
                            (pool1, pool2, price1, price2)
                        } else {
                            (pool2, pool1, price2, price1)
                        };

                        let profit_percentage = (sell_price - buy_price) / buy_price;

                        if profit_percentage >= self.detector.min_profit_threshold {
                            opportunities.push(ArbitrageOpportunity {
                                token_a_id: token_a,
                                token_b_id: token_b,
                                buy_pool_id: buy_pool.pool_id,
                                sell_pool_id: sell_pool.pool_id,
                                buy_price,
                                sell_price,
                                price_discrepancy: discrepancy,
                                profit_percentage,
                                recommended_amount: self.calculate_optimal_arbitrage_amount(
                                    buy_pool, sell_pool, token_a,
                                ),
                                detected_at: now,
                            });
                        }
                    }
                }
            }
        }

        opportunities
    }

    /// Get normalized price for a token pair
    fn get_normalized_price(&self, pool: &LiquidityPool, token_a: Uuid, token_b: Uuid) -> Decimal {
        if pool.token_a_id == token_a && pool.token_b_id == token_b {
            pool.price_a_in_b()
        } else if pool.token_b_id == token_a && pool.token_a_id == token_b {
            pool.price_b_in_a()
        } else {
            dec!(0)
        }
    }

    /// Calculate optimal arbitrage amount to maximize profit
    fn calculate_optimal_arbitrage_amount(
        &self,
        buy_pool: &LiquidityPool,
        sell_pool: &LiquidityPool,
        token_id: Uuid,
    ) -> Decimal {
        // Simplified: use 1% of the smaller reserve
        let buy_reserve = if buy_pool.token_a_id == token_id {
            buy_pool.reserve_a
        } else {
            buy_pool.reserve_b
        };

        let sell_reserve = if sell_pool.token_a_id == token_id {
            sell_pool.reserve_a
        } else {
            sell_pool.reserve_b
        };

        buy_reserve.min(sell_reserve) * dec!(0.01)
    }

    /// Check if a trade is likely arbitrage
    pub fn is_likely_arbitrage(
        &self,
        _user_id: Uuid,
        token_a_id: Uuid,
        token_b_id: Uuid,
        amount: Decimal,
        pools: &[LiquidityPool],
    ) -> bool {
        // Check if there are arbitrage opportunities for this pair
        let opportunities = self.detect_opportunities(pools);

        for opp in opportunities {
            if (opp.token_a_id == token_a_id && opp.token_b_id == token_b_id)
                || (opp.token_a_id == token_b_id && opp.token_b_id == token_a_id)
            {
                // Check if amount is close to recommended arbitrage amount
                let amount_ratio = (amount - opp.recommended_amount).abs() / opp.recommended_amount;
                if amount_ratio < dec!(0.2) {
                    // Within 20% of optimal arbitrage amount
                    return true;
                }
            }
        }

        false
    }

    /// Record an arbitrage trade
    pub fn record_trade(&mut self, trade: ArbitrageTrade) {
        let user_trades = self.recent_trades.entry(trade.user_id).or_default();
        user_trades.push(trade);
    }

    /// Clean up old trades outside the rate limit window
    pub fn cleanup_old_trades(&mut self) {
        let cutoff = Utc::now() - Duration::seconds(self.detector.rate_limit_window_seconds);

        for trades in self.recent_trades.values_mut() {
            trades.retain(|t| t.executed_at > cutoff);
        }

        // Remove empty entries
        self.recent_trades.retain(|_, trades| !trades.is_empty());
    }

    /// Check if user has exceeded arbitrage rate limit
    pub fn is_rate_limited(&mut self, user_id: Uuid) -> bool {
        self.cleanup_old_trades();

        if let Some(trades) = self.recent_trades.get(&user_id) {
            trades.len() as i32 >= self.detector.max_trades_per_window
        } else {
            false
        }
    }

    /// Check if trade should be allowed
    pub fn check_trade(
        &mut self,
        user_id: Uuid,
        token_a_id: Uuid,
        token_b_id: Uuid,
        amount: Decimal,
        pools: &[LiquidityPool],
    ) -> Result<()> {
        // Check if user is flagged
        if self.flagged_users.contains(&user_id) {
            return Err(CoreError::Validation(
                "User flagged for excessive arbitrage".to_string(),
            ));
        }

        // Check rate limit
        if self.is_rate_limited(user_id) {
            return Err(CoreError::Validation(
                "Arbitrage rate limit exceeded".to_string(),
            ));
        }

        // Check if likely arbitrage
        if self.is_likely_arbitrage(user_id, token_a_id, token_b_id, amount, pools) {
            // Allow but track
            return Ok(());
        }

        Ok(())
    }

    /// Flag a user for excessive arbitrage
    pub fn flag_user(&mut self, user_id: Uuid) {
        self.flagged_users.insert(user_id);
    }

    /// Unflag a user
    pub fn unflag_user(&mut self, user_id: Uuid) {
        self.flagged_users.remove(&user_id);
    }

    /// Get user's recent arbitrage stats
    pub fn get_user_stats(&mut self, user_id: Uuid) -> ArbitrageUserStats {
        self.cleanup_old_trades();

        let trades = self.recent_trades.get(&user_id);
        let trade_count = trades.map_or(0, |t| t.len() as i32);
        let total_profit = trades.map_or(dec!(0), |t| t.iter().map(|trade| trade.profit).sum());

        ArbitrageUserStats {
            user_id,
            trade_count_in_window: trade_count,
            total_profit_in_window: total_profit,
            is_rate_limited: trade_count >= self.detector.max_trades_per_window,
            is_flagged: self.flagged_users.contains(&user_id),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ArbitrageUserStats {
    pub user_id: Uuid,
    pub trade_count_in_window: i32,
    pub total_profit_in_window: Decimal,
    pub is_rate_limited: bool,
    pub is_flagged: bool,
}

/// Configuration for arbitrage prevention
#[derive(Debug, Deserialize)]
pub struct ArbitragePreventionConfig {
    pub max_price_discrepancy: Option<Decimal>,
    pub min_profit_threshold: Option<Decimal>,
    pub rate_limit_window_seconds: Option<i64>,
    pub max_trades_per_window: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pool(
        token_a: Uuid,
        token_b: Uuid,
        reserve_a: Decimal,
        reserve_b: Decimal,
    ) -> LiquidityPool {
        LiquidityPool {
            pool_id: Uuid::new_v4(),
            token_a_id: token_a,
            token_b_id: token_b,
            reserve_a,
            reserve_b,
            total_lp_tokens: dec!(1000),
            fee_percentage: dec!(0.003),
            status: crate::models::PoolStatus::Active,
            cumulative_volume_a: dec!(0),
            cumulative_volume_b: dec!(0),
            total_fees_a: dec!(0),
            total_fees_b: dec!(0),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_detect_arbitrage_opportunity() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();

        // Pool 1: 1 token_a = 2 token_b
        let pool1 = create_test_pool(token_a, token_b, dec!(1000), dec!(2000));

        // Pool 2: 1 token_a = 2.5 token_b (25% more expensive)
        let pool2 = create_test_pool(token_a, token_b, dec!(1000), dec!(2500));

        let detector = ArbitrageDetector {
            max_price_discrepancy: dec!(0.05), // 5%
            min_profit_threshold: dec!(0.01),  // 1%
            rate_limit_window_seconds: 3600,
            max_trades_per_window: 10,
        };

        let preventor = ArbitragePreventor::new(detector);
        let opportunities = preventor.detect_opportunities(&[pool1, pool2]);

        assert_eq!(opportunities.len(), 1);

        let opp = &opportunities[0];
        // Token IDs might be normalized (smaller first), so check both combinations
        assert!(
            (opp.token_a_id == token_a && opp.token_b_id == token_b)
                || (opp.token_a_id == token_b && opp.token_b_id == token_a)
        );
        assert!(opp.profit_percentage > dec!(0.2)); // > 20% profit
    }

    #[test]
    fn test_no_arbitrage_small_discrepancy() {
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();

        // Pool 1: 1 token_a = 2 token_b
        let pool1 = create_test_pool(token_a, token_b, dec!(1000), dec!(2000));

        // Pool 2: 1 token_a = 2.02 token_b (only 1% difference)
        let pool2 = create_test_pool(token_a, token_b, dec!(1000), dec!(2020));

        let detector = ArbitrageDetector {
            max_price_discrepancy: dec!(0.05), // 5%
            min_profit_threshold: dec!(0.01),  // 1%
            rate_limit_window_seconds: 3600,
            max_trades_per_window: 10,
        };

        let preventor = ArbitragePreventor::new(detector);
        let opportunities = preventor.detect_opportunities(&[pool1, pool2]);

        assert_eq!(opportunities.len(), 0); // No arbitrage (under threshold)
    }

    #[test]
    fn test_rate_limiting() {
        let mut preventor = ArbitragePreventor::default();
        let user_id = Uuid::new_v4();

        // Record 10 trades (at the limit)
        for _ in 0..10 {
            preventor.record_trade(ArbitrageTrade {
                user_id,
                token_a_id: Uuid::new_v4(),
                token_b_id: Uuid::new_v4(),
                amount: dec!(100),
                profit: dec!(1),
                executed_at: Utc::now(),
            });
        }

        // Should be rate limited
        assert!(preventor.is_rate_limited(user_id));

        // Different user should not be rate limited
        let other_user = Uuid::new_v4();
        assert!(!preventor.is_rate_limited(other_user));
    }

    #[test]
    fn test_user_flagging() {
        let mut preventor = ArbitragePreventor::default();
        let user_id = Uuid::new_v4();
        let token_a = Uuid::new_v4();
        let token_b = Uuid::new_v4();

        // Flag user
        preventor.flag_user(user_id);

        // Check trade should fail
        let result = preventor.check_trade(user_id, token_a, token_b, dec!(100), &[]);
        assert!(result.is_err());

        // Unflag user
        preventor.unflag_user(user_id);

        // Check trade should succeed
        let result = preventor.check_trade(user_id, token_a, token_b, dec!(100), &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cleanup_old_trades() {
        let detector = ArbitrageDetector {
            max_price_discrepancy: dec!(0.05),
            min_profit_threshold: dec!(0.01),
            rate_limit_window_seconds: 60, // 1 minute window
            max_trades_per_window: 10,
        };

        let mut preventor = ArbitragePreventor::new(detector);
        let user_id = Uuid::new_v4();

        // Record old trade (2 minutes ago)
        preventor.record_trade(ArbitrageTrade {
            user_id,
            token_a_id: Uuid::new_v4(),
            token_b_id: Uuid::new_v4(),
            amount: dec!(100),
            profit: dec!(1),
            executed_at: Utc::now() - Duration::seconds(120),
        });

        // Record recent trade
        preventor.record_trade(ArbitrageTrade {
            user_id,
            token_a_id: Uuid::new_v4(),
            token_b_id: Uuid::new_v4(),
            amount: dec!(100),
            profit: dec!(1),
            executed_at: Utc::now(),
        });

        // Cleanup should remove old trade
        preventor.cleanup_old_trades();

        let stats = preventor.get_user_stats(user_id);
        assert_eq!(stats.trade_count_in_window, 1); // Only recent trade remains
    }
}
