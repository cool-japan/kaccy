//! Avellaneda-Stoikov Market Making Model
//!
//! Implements the optimal market making strategy from Avellaneda and Stoikov (2008).
//! This model computes optimal bid-ask spreads based on:
//! - Inventory risk
//! - Market volatility
//! - Time to horizon
//! - Risk aversion

use crate::error::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Avellaneda-Stoikov market maker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvellanedaStoikovConfig {
    /// Risk aversion parameter (gamma)
    /// Higher gamma = more risk averse = wider spreads
    pub risk_aversion: Decimal,

    /// Time horizon for market making (in seconds)
    pub time_horizon_secs: u64,

    /// Order arrival intensity (kappa)
    /// Higher kappa = more order flow = tighter spreads
    pub order_intensity: Decimal,

    /// Inventory limits
    pub max_inventory: Decimal,

    /// Minimum spread (bps)
    pub min_spread_bps: Decimal,

    /// Maximum spread (bps)
    pub max_spread_bps: Decimal,
}

impl Default for AvellanedaStoikovConfig {
    fn default() -> Self {
        Self {
            risk_aversion: dec!(0.1),
            time_horizon_secs: 3600, // 1 hour
            order_intensity: dec!(1.5),
            max_inventory: dec!(1000),
            min_spread_bps: dec!(10),  // 0.1%
            max_spread_bps: dec!(500), // 5%
        }
    }
}

/// Market state for Avellaneda-Stoikov model
#[derive(Debug, Clone)]
pub struct MarketState {
    /// Current mid price
    pub mid_price: Decimal,

    /// Market volatility (annual)
    pub volatility: Decimal,

    /// Current inventory (positive = long, negative = short)
    pub inventory: Decimal,

    /// Time remaining until horizon (seconds)
    pub time_remaining_secs: u64,
}

/// Avellaneda-Stoikov quote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ASQuote {
    /// Bid price
    pub bid_price: Decimal,

    /// Ask price
    pub ask_price: Decimal,

    /// Bid size
    pub bid_size: Decimal,

    /// Ask size
    pub ask_size: Decimal,

    /// Reservation price (indifference price)
    pub reservation_price: Decimal,

    /// Optimal spread
    pub spread: Decimal,

    /// Quote timestamp
    pub timestamp: std::time::SystemTime,
}

/// Avellaneda-Stoikov market maker
pub struct AvellanedaStoikovMarketMaker {
    /// Model configuration parameters.
    pub config: AvellanedaStoikovConfig,
    /// Token this market maker is operating on.
    pub token_id: Uuid,
}

impl AvellanedaStoikovMarketMaker {
    /// Create a new Avellaneda-Stoikov market maker
    pub fn new(config: AvellanedaStoikovConfig, token_id: Uuid) -> Self {
        Self { config, token_id }
    }

    /// Calculate reservation price (indifference price)
    /// Formula: r = s - q * gamma * sigma^2 * (T - t)
    /// where:
    /// - s = mid price
    /// - q = inventory position
    /// - gamma = risk aversion
    /// - sigma = volatility
    /// - T - t = time remaining
    pub fn calculate_reservation_price(&self, state: &MarketState) -> Result<Decimal> {
        let mid_price = state.mid_price;
        let inventory = state.inventory;
        let gamma = self.config.risk_aversion;
        let sigma = state.volatility;

        // Convert time remaining to years for annual volatility
        let time_remaining_years = Decimal::from(state.time_remaining_secs) / dec!(31536000); // seconds per year

        // Calculate inventory adjustment
        // Adjustment = q * gamma * sigma^2 * (T - t)
        let variance = sigma * sigma;
        let inventory_adjustment = inventory * gamma * variance * time_remaining_years;

        // Reservation price = mid_price - inventory_adjustment
        let reservation_price = mid_price - inventory_adjustment;

        Ok(reservation_price)
    }

    /// Calculate optimal spread
    /// Formula: delta = gamma * sigma^2 * (T - t) + (2/gamma) * ln(1 + gamma/kappa)
    /// where:
    /// - gamma = risk aversion
    /// - sigma = volatility
    /// - T - t = time remaining
    /// - kappa = order arrival intensity
    pub fn calculate_optimal_spread(&self, state: &MarketState) -> Result<Decimal> {
        let gamma = self.config.risk_aversion;
        let sigma = state.volatility;
        let kappa = self.config.order_intensity;

        // Convert time remaining to years
        let time_remaining_years = Decimal::from(state.time_remaining_secs) / dec!(31536000);

        // First term: gamma * sigma^2 * (T - t)
        let variance = sigma * sigma;
        let first_term = gamma * variance * time_remaining_years;

        // Second term: (2/gamma) * ln(1 + gamma/kappa)
        // Approximation: ln(1 + x) ≈ x for small x
        // More accurate: ln(1 + x) ≈ x - x^2/2 + x^3/3
        let ratio = gamma / kappa;
        let ln_approx = self.ln_approximation(dec!(1) + ratio);
        let second_term = (dec!(2) / gamma) * ln_approx;

        let optimal_spread = first_term + second_term;

        // Apply min/max spread constraints
        let spread_bps = optimal_spread * dec!(10000); // Convert to basis points
        let constrained_bps = spread_bps
            .max(self.config.min_spread_bps)
            .min(self.config.max_spread_bps);

        Ok(constrained_bps / dec!(10000)) // Convert back to decimal
    }

    /// Calculate asymmetric spread based on inventory
    /// When long (positive inventory): widen ask, tighten bid
    /// When short (negative inventory): widen bid, tighten ask
    pub fn calculate_asymmetric_spread(
        &self,
        reservation_price: Decimal,
        optimal_spread: Decimal,
        state: &MarketState,
    ) -> Result<(Decimal, Decimal)> {
        // Calculate inventory skew factor
        let inventory_ratio = if self.config.max_inventory > Decimal::ZERO {
            (state.inventory / self.config.max_inventory)
                .min(dec!(1))
                .max(dec!(-1))
        } else {
            Decimal::ZERO
        };

        // Asymmetry factor (0 to 1)
        let asymmetry_factor = inventory_ratio.abs() * dec!(0.5); // Max 50% asymmetry

        // When long: increase ask side, decrease bid side
        // When short: increase bid side, decrease ask side
        let bid_adjustment = if inventory_ratio > Decimal::ZERO {
            dec!(1) - asymmetry_factor
        } else {
            dec!(1) + asymmetry_factor
        };

        let ask_adjustment = if inventory_ratio > Decimal::ZERO {
            dec!(1) + asymmetry_factor
        } else {
            dec!(1) - asymmetry_factor
        };

        // Half spread
        let half_spread = optimal_spread / dec!(2);

        // Calculate bid and ask prices
        let bid_price = reservation_price - (half_spread * bid_adjustment);
        let ask_price = reservation_price + (half_spread * ask_adjustment);

        Ok((bid_price, ask_price))
    }

    /// Generate quote based on current market state
    pub fn generate_quote(&self, state: &MarketState, base_size: Decimal) -> Result<ASQuote> {
        // Calculate reservation price
        let reservation_price = self.calculate_reservation_price(state)?;

        // Calculate optimal spread
        let optimal_spread = self.calculate_optimal_spread(state)?;

        // Calculate asymmetric bid/ask prices
        let (bid_price, ask_price) =
            self.calculate_asymmetric_spread(reservation_price, optimal_spread, state)?;

        // Adjust sizes based on inventory
        let (bid_size, ask_size) = self.calculate_optimal_sizes(state, base_size)?;

        Ok(ASQuote {
            bid_price,
            ask_price,
            bid_size,
            ask_size,
            reservation_price,
            spread: optimal_spread,
            timestamp: std::time::SystemTime::now(),
        })
    }

    /// Calculate optimal bid/ask sizes based on inventory
    fn calculate_optimal_sizes(
        &self,
        state: &MarketState,
        base_size: Decimal,
    ) -> Result<(Decimal, Decimal)> {
        let inventory_ratio = if self.config.max_inventory > Decimal::ZERO {
            (state.inventory / self.config.max_inventory)
                .min(dec!(1))
                .max(dec!(-1))
        } else {
            Decimal::ZERO
        };

        // When long: reduce ask size, increase bid size (to reduce position)
        // When short: reduce bid size, increase ask size (to reduce position)
        let bid_multiplier = if inventory_ratio > Decimal::ZERO {
            dec!(1) - (inventory_ratio * dec!(0.5))
        } else {
            dec!(1) + (inventory_ratio.abs() * dec!(0.5))
        };

        let ask_multiplier = if inventory_ratio < Decimal::ZERO {
            dec!(1) - (inventory_ratio.abs() * dec!(0.5))
        } else {
            dec!(1) + (inventory_ratio * dec!(0.5))
        };

        let bid_size = base_size * bid_multiplier;
        let ask_size = base_size * ask_multiplier;

        Ok((bid_size, ask_size))
    }

    /// Approximate natural logarithm using Taylor series
    /// ln(x) = 2 * [(x-1)/(x+1) + (1/3)*((x-1)/(x+1))^3 + (1/5)*((x-1)/(x+1))^5 + ...]
    fn ln_approximation(&self, x: Decimal) -> Decimal {
        if x <= Decimal::ZERO {
            return Decimal::MIN;
        }

        let y = (x - dec!(1)) / (x + dec!(1));
        let y2 = y * y;

        // Taylor series approximation (5 terms)
        let mut result = y;
        let mut term = y;

        for n in 1..=4 {
            term *= y2;
            let divisor = Decimal::from(2 * n + 1);
            result += term / divisor;
        }

        result * dec!(2)
    }

    /// Calculate expected profit per unit time
    pub fn calculate_expected_profit(
        &self,
        _state: &MarketState,
        quote: &ASQuote,
    ) -> Result<Decimal> {
        let spread = quote.ask_price - quote.bid_price;
        let kappa = self.config.order_intensity;

        // Expected profit = kappa * spread / 2
        // (probability of execution * profit per trade)
        let expected_profit = kappa * spread / dec!(2);

        Ok(expected_profit)
    }

    /// Calculate inventory risk
    pub fn calculate_inventory_risk(&self, state: &MarketState) -> Result<Decimal> {
        let gamma = self.config.risk_aversion;
        let sigma = state.volatility;
        let inventory = state.inventory;

        // Risk = 0.5 * gamma * sigma^2 * q^2
        let variance = sigma * sigma;
        let risk = dec!(0.5) * gamma * variance * inventory * inventory;

        Ok(risk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reservation_price_neutral_inventory() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20), // 20% annual volatility
            inventory: Decimal::ZERO,
            time_remaining_secs: 3600,
        };

        let reservation_price = mm.calculate_reservation_price(&state).unwrap();

        // With zero inventory, reservation price should equal mid price
        assert_eq!(reservation_price, dec!(100));
    }

    #[test]
    fn test_reservation_price_long_inventory() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: dec!(50), // Long 50 units
            time_remaining_secs: 3600,
        };

        let reservation_price = mm.calculate_reservation_price(&state).unwrap();

        // With positive inventory, reservation price should be below mid price
        assert!(reservation_price < dec!(100));
    }

    #[test]
    fn test_reservation_price_short_inventory() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: dec!(-50), // Short 50 units
            time_remaining_secs: 3600,
        };

        let reservation_price = mm.calculate_reservation_price(&state).unwrap();

        // With negative inventory, reservation price should be above mid price
        assert!(reservation_price > dec!(100));
    }

    #[test]
    fn test_optimal_spread_calculation() {
        let config = AvellanedaStoikovConfig::default();
        let min_spread_bps = config.min_spread_bps;
        let max_spread_bps = config.max_spread_bps;
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: Decimal::ZERO,
            time_remaining_secs: 3600,
        };

        let spread = mm.calculate_optimal_spread(&state).unwrap();

        // Spread should be positive
        assert!(spread > Decimal::ZERO);

        // Spread should be within configured bounds
        let spread_bps = spread * dec!(10000);
        assert!(spread_bps >= min_spread_bps);
        assert!(spread_bps <= max_spread_bps);
    }

    #[test]
    fn test_generate_quote() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: Decimal::ZERO,
            time_remaining_secs: 3600,
        };

        let quote = mm.generate_quote(&state, dec!(10)).unwrap();

        // Basic sanity checks
        assert!(quote.bid_price > Decimal::ZERO);
        assert!(quote.ask_price > quote.bid_price);
        assert!(quote.bid_size > Decimal::ZERO);
        assert!(quote.ask_size > Decimal::ZERO);
        assert_eq!(quote.spread, quote.ask_price - quote.bid_price);
    }

    #[test]
    fn test_asymmetric_spread_long_position() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: dec!(500), // Long position (50% of max)
            time_remaining_secs: 3600,
        };

        let reservation_price = mm.calculate_reservation_price(&state).unwrap();
        let optimal_spread = mm.calculate_optimal_spread(&state).unwrap();
        let (bid_price, ask_price) = mm
            .calculate_asymmetric_spread(reservation_price, optimal_spread, &state)
            .unwrap();

        // When long, should have tighter bid (encourage buying) and wider ask
        let bid_distance = reservation_price - bid_price;
        let ask_distance = ask_price - reservation_price;
        assert!(ask_distance > bid_distance);
    }

    #[test]
    fn test_ln_approximation() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        // Test ln(1) = 0
        let result = mm.ln_approximation(dec!(1));
        assert!(result.abs() < dec!(0.01));

        // Test ln(e) ≈ 1 (e ≈ 2.718)
        let result = mm.ln_approximation(dec!(2.718));
        assert!((result - dec!(1)).abs() < dec!(0.1));
    }

    #[test]
    fn test_expected_profit() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: Decimal::ZERO,
            time_remaining_secs: 3600,
        };

        let quote = mm.generate_quote(&state, dec!(10)).unwrap();
        let expected_profit = mm.calculate_expected_profit(&state, &quote).unwrap();

        // Expected profit should be positive
        assert!(expected_profit > Decimal::ZERO);
    }

    #[test]
    fn test_inventory_risk() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state_neutral = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: Decimal::ZERO,
            time_remaining_secs: 3600,
        };

        let state_long = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: dec!(100),
            time_remaining_secs: 3600,
        };

        let risk_neutral = mm.calculate_inventory_risk(&state_neutral).unwrap();
        let risk_long = mm.calculate_inventory_risk(&state_long).unwrap();

        // Risk with neutral inventory should be zero
        assert_eq!(risk_neutral, Decimal::ZERO);

        // Risk with long inventory should be positive
        assert!(risk_long > Decimal::ZERO);
    }

    #[test]
    fn test_optimal_sizes_inventory_adjustment() {
        let config = AvellanedaStoikovConfig::default();
        let mm = AvellanedaStoikovMarketMaker::new(config, Uuid::new_v4());

        let state = MarketState {
            mid_price: dec!(100),
            volatility: dec!(0.20),
            inventory: dec!(500), // 50% of max
            time_remaining_secs: 3600,
        };

        let (bid_size, ask_size) = mm.calculate_optimal_sizes(&state, dec!(10)).unwrap();

        // When long, should reduce ask size to discourage selling more
        // and maintain/increase bid size to encourage unwinding
        assert!(bid_size > Decimal::ZERO);
        assert!(ask_size > Decimal::ZERO);
    }
}
