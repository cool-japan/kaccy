//! Smart order routing for optimal trade execution across multiple venues

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Trading venue (bonding curve, AMM, order book, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Venue {
    /// Bonding curve
    BondingCurve,
    /// AMM pool
    Amm,
    /// Order book
    OrderBook,
    /// External exchange
    External,
}

/// Quote from a venue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueQuote {
    /// Venue
    pub venue: Venue,
    /// Token ID
    pub token_id: Uuid,
    /// Amount available
    pub available_amount: Decimal,
    /// Execution price
    pub price: Decimal,
    /// Expected slippage
    pub slippage: Decimal,
    /// Estimated gas/fees
    pub fees: Decimal,
    /// Liquidity depth
    pub depth: Decimal,
}

impl VenueQuote {
    /// Calculate effective price including fees
    pub fn effective_price(&self) -> Decimal {
        self.price * (dec!(1) + self.slippage) + self.fees
    }

    /// Calculate total cost for an amount
    pub fn total_cost(&self, amount: Decimal) -> Decimal {
        amount * self.effective_price()
    }
}

/// Execution route across venues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Splits across venues
    pub splits: Vec<RouteSplit>,
    /// Total amount
    pub total_amount: Decimal,
    /// Average execution price
    pub avg_price: Decimal,
    /// Total slippage
    pub total_slippage: Decimal,
    /// Total fees
    pub total_fees: Decimal,
}

/// Route split for a venue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSplit {
    /// Venue
    pub venue: Venue,
    /// Amount to execute on this venue
    pub amount: Decimal,
    /// Execution price
    pub price: Decimal,
    /// Percentage of total order
    pub percentage: Decimal,
}

/// Order routing optimizer
pub struct SmartRouter {
    /// Venue quotes cache
    venue_quotes: HashMap<(Uuid, Venue), VenueQuote>,
}

impl SmartRouter {
    /// Create a new smart router
    pub fn new() -> Self {
        Self {
            venue_quotes: HashMap::new(),
        }
    }

    /// Add a venue quote
    pub fn add_quote(&mut self, quote: VenueQuote) {
        self.venue_quotes
            .insert((quote.token_id, quote.venue), quote);
    }

    /// Find best single venue
    pub fn find_best_venue(&self, token_id: Uuid, amount: Decimal) -> Option<Venue> {
        let mut best_venue = None;
        let mut best_price = Decimal::MAX;

        for ((tid, venue), quote) in &self.venue_quotes {
            if *tid == token_id && quote.available_amount >= amount {
                let price = quote.effective_price();
                if price < best_price {
                    best_price = price;
                    best_venue = Some(*venue);
                }
            }
        }

        best_venue
    }

    /// Find optimal route (split across multiple venues)
    pub fn find_optimal_route(&self, token_id: Uuid, amount: Decimal) -> Option<Route> {
        let mut available_quotes: Vec<&VenueQuote> = self
            .venue_quotes
            .values()
            .filter(|q| q.token_id == token_id && q.available_amount > Decimal::ZERO)
            .collect();

        if available_quotes.is_empty() {
            return None;
        }

        // Sort by effective price (best first)
        available_quotes.sort_by(|a, b| {
            a.effective_price()
                .partial_cmp(&b.effective_price())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut splits = Vec::new();
        let mut remaining = amount;
        let mut total_cost = Decimal::ZERO;
        let mut total_fees = Decimal::ZERO;

        // Greedy algorithm: fill from best priced venue first
        for quote in available_quotes {
            if remaining.is_zero() {
                break;
            }

            let fill_amount = remaining.min(quote.available_amount);
            let cost = quote.total_cost(fill_amount);
            let percentage = fill_amount / amount;

            splits.push(RouteSplit {
                venue: quote.venue,
                amount: fill_amount,
                price: quote.price,
                percentage,
            });

            total_cost += cost;
            total_fees += quote.fees * fill_amount;
            remaining -= fill_amount;
        }

        if !remaining.is_zero() {
            return None; // Can't fill entire order
        }

        let avg_price = total_cost / amount;
        let initial_price = splits[0].price;
        let total_slippage = if initial_price.is_zero() {
            Decimal::ZERO
        } else {
            (avg_price - initial_price) / initial_price
        };

        Some(Route {
            splits,
            total_amount: amount,
            avg_price,
            total_slippage,
            total_fees,
        })
    }

    /// Get execution quality metrics
    pub fn get_execution_quality(
        &self,
        token_id: Uuid,
        amount: Decimal,
        executed_price: Decimal,
    ) -> ExecutionQuality {
        let best_venue = self.find_best_venue(token_id, amount);
        let optimal_route = self.find_optimal_route(token_id, amount);

        let benchmark_price = optimal_route
            .as_ref()
            .map(|r| r.avg_price)
            .unwrap_or(executed_price);

        let price_improvement = if benchmark_price > Decimal::ZERO {
            (benchmark_price - executed_price) / benchmark_price
        } else {
            Decimal::ZERO
        };

        ExecutionQuality {
            executed_price,
            benchmark_price,
            price_improvement,
            best_venue,
            used_routing: optimal_route.is_some(),
        }
    }
}

impl Default for SmartRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionQuality {
    /// Actual execution price
    pub executed_price: Decimal,
    /// Benchmark price (best available)
    pub benchmark_price: Decimal,
    /// Price improvement (positive = better than benchmark)
    pub price_improvement: Decimal,
    /// Best venue that was available
    pub best_venue: Option<Venue>,
    /// Whether routing was used
    pub used_routing: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_venue_quote_effective_price() {
        let quote = VenueQuote {
            venue: Venue::Amm,
            token_id: Uuid::new_v4(),
            available_amount: dec!(1000),
            price: dec!(100),
            slippage: dec!(0.01), // 1%
            fees: dec!(0.5),
            depth: dec!(10000),
        };

        // 100 * (1 + 0.01) + 0.5 = 101.5
        assert_eq!(quote.effective_price(), dec!(101.5));
    }

    #[test]
    fn test_find_best_venue() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        router.add_quote(VenueQuote {
            venue: Venue::BondingCurve,
            token_id,
            available_amount: dec!(1000),
            price: dec!(105),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(5000),
        });

        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(1000),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(10000),
        });

        let best = router.find_best_venue(token_id, dec!(500));
        assert_eq!(best, Some(Venue::Amm)); // AMM has lower price
    }

    #[test]
    fn test_optimal_route_single_venue() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(1000),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(10000),
        });

        let route = router.find_optimal_route(token_id, dec!(500)).unwrap();

        assert_eq!(route.splits.len(), 1);
        assert_eq!(route.splits[0].venue, Venue::Amm);
        assert_eq!(route.splits[0].amount, dec!(500));
        assert_eq!(route.total_amount, dec!(500));
    }

    #[test]
    fn test_optimal_route_multi_venue() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        // AMM: cheaper but limited liquidity
        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(300),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(5000),
        });

        // Order book: more expensive but more liquidity
        router.add_quote(VenueQuote {
            venue: Venue::OrderBook,
            token_id,
            available_amount: dec!(1000),
            price: dec!(102),
            slippage: dec!(0.005),
            fees: dec!(0.3),
            depth: dec!(15000),
        });

        let route = router.find_optimal_route(token_id, dec!(500)).unwrap();

        assert_eq!(route.splits.len(), 2);
        // First 300 from AMM (cheaper)
        assert_eq!(route.splits[0].venue, Venue::Amm);
        assert_eq!(route.splits[0].amount, dec!(300));
        // Remaining 200 from OrderBook
        assert_eq!(route.splits[1].venue, Venue::OrderBook);
        assert_eq!(route.splits[1].amount, dec!(200));
        assert_eq!(route.total_amount, dec!(500));
    }

    #[test]
    fn test_route_insufficient_liquidity() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(100),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(1000),
        });

        let route = router.find_optimal_route(token_id, dec!(500));
        assert!(route.is_none()); // Not enough liquidity
    }

    #[test]
    fn test_execution_quality() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(1000),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(10000),
        });

        // Executed at better price than available
        let quality = router.get_execution_quality(token_id, dec!(500), dec!(99));

        assert_eq!(quality.executed_price, dec!(99));
        assert!(quality.price_improvement > Decimal::ZERO);
        assert_eq!(quality.best_venue, Some(Venue::Amm));
    }

    #[test]
    fn test_route_percentages() {
        let mut router = SmartRouter::new();
        let token_id = Uuid::new_v4();

        router.add_quote(VenueQuote {
            venue: Venue::Amm,
            token_id,
            available_amount: dec!(250),
            price: dec!(100),
            slippage: dec!(0.01),
            fees: dec!(0.5),
            depth: dec!(5000),
        });

        router.add_quote(VenueQuote {
            venue: Venue::OrderBook,
            token_id,
            available_amount: dec!(750),
            price: dec!(101),
            slippage: dec!(0.005),
            fees: dec!(0.3),
            depth: dec!(15000),
        });

        let route = router.find_optimal_route(token_id, dec!(1000)).unwrap();

        assert_eq!(route.splits[0].percentage, dec!(0.25)); // 250/1000
        assert_eq!(route.splits[1].percentage, dec!(0.75)); // 750/1000
    }
}
