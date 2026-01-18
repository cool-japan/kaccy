//! Smart Order Routing (SOR) for optimal execution across venues
//!
//! This module provides intelligent routing of orders across multiple
//! liquidity venues to achieve best execution price and minimize costs.

use crate::error::{CoreError, Result};
use crate::models::{Order, OrderType};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trading venue information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    pub id: Uuid,
    pub name: String,
    pub fee_rate: Decimal,
    pub available_liquidity: Decimal,
    pub current_price: Decimal,
    pub estimated_slippage: Decimal,
    pub latency_ms: u64,
}

impl Venue {
    pub fn new(name: impl Into<String>, fee_rate: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            fee_rate,
            available_liquidity: Decimal::ZERO,
            current_price: Decimal::ZERO,
            estimated_slippage: Decimal::ZERO,
            latency_ms: 0,
        }
    }

    pub fn with_liquidity(mut self, liquidity: Decimal) -> Self {
        self.available_liquidity = liquidity;
        self
    }

    pub fn with_price(mut self, price: Decimal) -> Self {
        self.current_price = price;
        self
    }

    pub fn with_slippage(mut self, slippage: Decimal) -> Self {
        self.estimated_slippage = slippage;
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Calculate total cost for executing given amount
    pub fn calculate_cost(&self, amount: Decimal, order_type: OrderType) -> Decimal {
        let base_cost = amount * self.current_price;
        let fee = base_cost * self.fee_rate;
        let slippage_cost = base_cost * self.estimated_slippage;

        match order_type {
            OrderType::Buy => base_cost + fee + slippage_cost,
            OrderType::Sell => base_cost - fee - slippage_cost,
        }
    }

    /// Calculate effective price including fees and slippage
    pub fn effective_price(&self, order_type: OrderType) -> Decimal {
        match order_type {
            OrderType::Buy => {
                self.current_price * (dec!(1) + self.fee_rate + self.estimated_slippage)
            }
            OrderType::Sell => {
                self.current_price * (dec!(1) - self.fee_rate - self.estimated_slippage)
            }
        }
    }
}

/// Route allocation for an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAllocation {
    pub venue_id: Uuid,
    pub venue_name: String,
    pub amount: Decimal,
    pub estimated_price: Decimal,
    pub estimated_cost: Decimal,
}

/// Order routing strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Route to venue with best price
    BestPrice,
    /// Route to minimize slippage
    MinimizeSlippage,
    /// Route to minimize fees
    MinimizeFees,
    /// Balance between price, slippage, and fees
    Balanced,
    /// Distribute across multiple venues
    Distributed,
}

/// Smart order router
#[derive(Debug, Clone)]
pub struct SmartOrderRouter {
    venues: Vec<Venue>,
    strategy: RoutingStrategy,
}

impl SmartOrderRouter {
    pub fn new(strategy: RoutingStrategy) -> Self {
        Self {
            venues: Vec::new(),
            strategy,
        }
    }

    pub fn add_venue(&mut self, venue: Venue) {
        self.venues.push(venue);
    }

    pub fn set_strategy(&mut self, strategy: RoutingStrategy) {
        self.strategy = strategy;
    }

    /// Route an order across venues
    pub fn route(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        if self.venues.is_empty() {
            return Err(CoreError::Validation(
                "No venues available for routing".to_string(),
            ));
        }

        match self.strategy {
            RoutingStrategy::BestPrice => self.route_best_price(order),
            RoutingStrategy::MinimizeSlippage => self.route_min_slippage(order),
            RoutingStrategy::MinimizeFees => self.route_min_fees(order),
            RoutingStrategy::Balanced => self.route_balanced(order),
            RoutingStrategy::Distributed => self.route_distributed(order),
        }
    }

    fn route_best_price(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        // Find venue with best effective price
        let best_venue = match order.order_type {
            OrderType::Buy => self
                .venues
                .iter()
                .filter(|v| v.available_liquidity >= order.amount)
                .min_by(|a, b| {
                    a.effective_price(order.order_type)
                        .cmp(&b.effective_price(order.order_type))
                }),
            OrderType::Sell => self
                .venues
                .iter()
                .filter(|v| v.available_liquidity >= order.amount)
                .max_by(|a, b| {
                    a.effective_price(order.order_type)
                        .cmp(&b.effective_price(order.order_type))
                }),
        };

        if let Some(venue) = best_venue {
            Ok(vec![RouteAllocation {
                venue_id: venue.id,
                venue_name: venue.name.clone(),
                amount: order.amount,
                estimated_price: venue.effective_price(order.order_type),
                estimated_cost: venue.calculate_cost(order.amount, order.order_type),
            }])
        } else {
            // Split across multiple venues if no single venue has enough liquidity
            self.route_distributed(order)
        }
    }

    fn route_min_slippage(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        let best_venue = self
            .venues
            .iter()
            .filter(|v| v.available_liquidity >= order.amount)
            .min_by(|a, b| a.estimated_slippage.cmp(&b.estimated_slippage));

        if let Some(venue) = best_venue {
            Ok(vec![RouteAllocation {
                venue_id: venue.id,
                venue_name: venue.name.clone(),
                amount: order.amount,
                estimated_price: venue.effective_price(order.order_type),
                estimated_cost: venue.calculate_cost(order.amount, order.order_type),
            }])
        } else {
            self.route_distributed(order)
        }
    }

    fn route_min_fees(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        let best_venue = self
            .venues
            .iter()
            .filter(|v| v.available_liquidity >= order.amount)
            .min_by(|a, b| a.fee_rate.cmp(&b.fee_rate));

        if let Some(venue) = best_venue {
            Ok(vec![RouteAllocation {
                venue_id: venue.id,
                venue_name: venue.name.clone(),
                amount: order.amount,
                estimated_price: venue.effective_price(order.order_type),
                estimated_cost: venue.calculate_cost(order.amount, order.order_type),
            }])
        } else {
            self.route_distributed(order)
        }
    }

    fn route_balanced(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        // Score each venue based on multiple factors
        let mut scored_venues: Vec<_> = self
            .venues
            .iter()
            .map(|venue| {
                let price_score = self.calculate_price_score(venue, order.order_type);
                let slippage_score = dec!(1) - venue.estimated_slippage;
                let fee_score = dec!(1) - venue.fee_rate;
                let liquidity_score = (venue.available_liquidity / order.amount).min(dec!(1));

                // Weighted average
                let total_score = price_score * dec!(0.4)
                    + slippage_score * dec!(0.3)
                    + fee_score * dec!(0.2)
                    + liquidity_score * dec!(0.1);

                (venue, total_score)
            })
            .collect();

        scored_venues.sort_by(|a, b| b.1.cmp(&a.1));

        if let Some((best_venue, _)) = scored_venues.first() {
            if best_venue.available_liquidity >= order.amount {
                Ok(vec![RouteAllocation {
                    venue_id: best_venue.id,
                    venue_name: best_venue.name.clone(),
                    amount: order.amount,
                    estimated_price: best_venue.effective_price(order.order_type),
                    estimated_cost: best_venue.calculate_cost(order.amount, order.order_type),
                }])
            } else {
                self.route_distributed(order)
            }
        } else {
            Err(CoreError::Validation(
                "No suitable venues found".to_string(),
            ))
        }
    }

    fn route_distributed(&self, order: &Order) -> Result<Vec<RouteAllocation>> {
        let mut allocations = Vec::new();
        let mut remaining = order.amount;

        // Sort venues by effective price
        let mut sorted_venues = self.venues.clone();
        match order.order_type {
            OrderType::Buy => {
                sorted_venues.sort_by(|a, b| {
                    a.effective_price(order.order_type)
                        .cmp(&b.effective_price(order.order_type))
                });
            }
            OrderType::Sell => {
                sorted_venues.sort_by(|a, b| {
                    b.effective_price(order.order_type)
                        .cmp(&a.effective_price(order.order_type))
                });
            }
        }

        for venue in sorted_venues {
            if remaining == Decimal::ZERO {
                break;
            }

            let allocation_amount = remaining.min(venue.available_liquidity);
            if allocation_amount > Decimal::ZERO {
                allocations.push(RouteAllocation {
                    venue_id: venue.id,
                    venue_name: venue.name.clone(),
                    amount: allocation_amount,
                    estimated_price: venue.effective_price(order.order_type),
                    estimated_cost: venue.calculate_cost(allocation_amount, order.order_type),
                });
                remaining -= allocation_amount;
            }
        }

        if remaining > Decimal::ZERO {
            return Err(CoreError::InsufficientLiquidity(format!(
                "Cannot fulfill order: {} remaining after distribution",
                remaining
            )));
        }

        Ok(allocations)
    }

    fn calculate_price_score(&self, venue: &Venue, order_type: OrderType) -> Decimal {
        let all_prices: Vec<Decimal> = self
            .venues
            .iter()
            .map(|v| v.effective_price(order_type))
            .collect();

        let min_price = all_prices.iter().min().copied().unwrap_or(Decimal::ZERO);
        let max_price = all_prices.iter().max().copied().unwrap_or(Decimal::ONE);

        if max_price == min_price {
            return dec!(1);
        }

        match order_type {
            OrderType::Buy => {
                // Lower price is better for buy
                (max_price - venue.effective_price(order_type)) / (max_price - min_price)
            }
            OrderType::Sell => {
                // Higher price is better for sell
                (venue.effective_price(order_type) - min_price) / (max_price - min_price)
            }
        }
    }

    /// Calculate total execution quality score
    pub fn calculate_execution_quality(&self, allocations: &[RouteAllocation]) -> Decimal {
        if allocations.is_empty() {
            return Decimal::ZERO;
        }

        let total_amount: Decimal = allocations.iter().map(|a| a.amount).sum();
        if total_amount == Decimal::ZERO {
            return Decimal::ZERO;
        }

        let weighted_price: Decimal = allocations
            .iter()
            .map(|a| a.estimated_price * a.amount)
            .sum::<Decimal>()
            / total_amount;

        // Score based on how close to best available price
        let best_price = allocations
            .iter()
            .map(|a| a.estimated_price)
            .min()
            .unwrap_or(Decimal::ZERO);

        if weighted_price == Decimal::ZERO {
            return Decimal::ZERO;
        }

        best_price / weighted_price
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_order(amount: Decimal, order_type: OrderType) -> Order {
        Order {
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            order_type,
            amount,
            price_btc: dec!(100),
            total_btc: amount * dec!(100),
            status: crate::models::OrderStatus::Pending,
            btc_address: None,
            btc_txid: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    #[test]
    fn test_venue_cost_calculation() {
        let venue = Venue::new("Test Venue", dec!(0.001))
            .with_liquidity(dec!(1000))
            .with_price(dec!(100))
            .with_slippage(dec!(0.002));

        let buy_cost = venue.calculate_cost(dec!(10), OrderType::Buy);
        let sell_cost = venue.calculate_cost(dec!(10), OrderType::Sell);

        assert!(buy_cost > dec!(1000)); // Base + fees + slippage
        assert!(sell_cost < dec!(1000)); // Base - fees - slippage
    }

    #[test]
    fn test_best_price_routing() {
        let mut router = SmartOrderRouter::new(RoutingStrategy::BestPrice);

        router.add_venue(
            Venue::new("Venue A", dec!(0.003))
                .with_liquidity(dec!(1000))
                .with_price(dec!(100))
                .with_slippage(dec!(0.001)),
        );

        router.add_venue(
            Venue::new("Venue B", dec!(0.002))
                .with_liquidity(dec!(1000))
                .with_price(dec!(99))
                .with_slippage(dec!(0.001)),
        );

        let order = create_test_order(dec!(100), OrderType::Buy);
        let routes = router.route(&order).unwrap();

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].venue_name, "Venue B"); // Better price
    }

    #[test]
    fn test_distributed_routing() {
        let mut router = SmartOrderRouter::new(RoutingStrategy::Distributed);

        router.add_venue(
            Venue::new("Venue A", dec!(0.001))
                .with_liquidity(dec!(100))
                .with_price(dec!(100))
                .with_slippage(dec!(0.001)),
        );

        router.add_venue(
            Venue::new("Venue B", dec!(0.001))
                .with_liquidity(dec!(150))
                .with_price(dec!(101))
                .with_slippage(dec!(0.001)),
        );

        let order = create_test_order(dec!(200), OrderType::Buy);
        let routes = router.route(&order).unwrap();

        assert_eq!(routes.len(), 2);
        let total: Decimal = routes.iter().map(|r| r.amount).sum();
        assert_eq!(total, order.amount);
    }

    #[test]
    fn test_insufficient_liquidity() {
        let mut router = SmartOrderRouter::new(RoutingStrategy::BestPrice);

        router.add_venue(
            Venue::new("Venue A", dec!(0.001))
                .with_liquidity(dec!(50))
                .with_price(dec!(100))
                .with_slippage(dec!(0.001)),
        );

        let order = create_test_order(dec!(200), OrderType::Buy);
        let result = router.route(&order);

        assert!(result.is_err());
    }

    #[test]
    fn test_min_slippage_routing() {
        let mut router = SmartOrderRouter::new(RoutingStrategy::MinimizeSlippage);

        router.add_venue(
            Venue::new("Venue A", dec!(0.001))
                .with_liquidity(dec!(1000))
                .with_price(dec!(100))
                .with_slippage(dec!(0.005)),
        );

        router.add_venue(
            Venue::new("Venue B", dec!(0.001))
                .with_liquidity(dec!(1000))
                .with_price(dec!(100))
                .with_slippage(dec!(0.001)),
        );

        let order = create_test_order(dec!(100), OrderType::Buy);
        let routes = router.route(&order).unwrap();

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].venue_name, "Venue B"); // Lower slippage
    }

    #[test]
    fn test_execution_quality_score() {
        let router = SmartOrderRouter::new(RoutingStrategy::Balanced);

        let allocations = vec![
            RouteAllocation {
                venue_id: Uuid::new_v4(),
                venue_name: "Venue A".to_string(),
                amount: dec!(50),
                estimated_price: dec!(100),
                estimated_cost: dec!(5000),
            },
            RouteAllocation {
                venue_id: Uuid::new_v4(),
                venue_name: "Venue B".to_string(),
                amount: dec!(50),
                estimated_price: dec!(102),
                estimated_cost: dec!(5100),
            },
        ];

        let quality = router.calculate_execution_quality(&allocations);
        assert!(quality > Decimal::ZERO);
        assert!(quality <= dec!(1));
    }
}
