//! Bonding curve implementations for token pricing

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Trait for bonding curve implementations
pub trait BondingCurve: Send + Sync {
    /// Calculate the cost to buy `amount` tokens at current `supply`
    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal;

    /// Calculate the proceeds from selling `amount` tokens at current `supply`
    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal;

    /// Get the current spot price at given supply
    fn spot_price(&self, current_supply: Decimal) -> Decimal;

    /// Calculate price impact percentage for a buy order
    fn buy_price_impact(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount == dec!(0) {
            return dec!(0);
        }
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);
        if spot_before == dec!(0) {
            return dec!(0);
        }
        ((spot_after - spot_before) / spot_before) * dec!(100)
    }

    /// Calculate price impact percentage for a sell order
    fn sell_price_impact(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount == dec!(0) || current_supply <= amount {
            return dec!(0);
        }
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply - amount);
        if spot_before == dec!(0) {
            return dec!(0);
        }
        ((spot_before - spot_after) / spot_before) * dec!(100)
    }

    /// Calculate average price per token for a buy order
    fn avg_buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount == dec!(0) {
            return dec!(0);
        }
        self.buy_price(current_supply, amount) / amount
    }

    /// Calculate average price per token for a sell order
    fn avg_sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount == dec!(0) {
            return dec!(0);
        }
        self.sell_price(current_supply, amount) / amount
    }

    /// Check if price is within slippage tolerance
    fn check_buy_slippage(
        &self,
        current_supply: Decimal,
        amount: Decimal,
        max_price: Decimal,
    ) -> bool {
        let avg_price = self.avg_buy_price(current_supply, amount);
        avg_price <= max_price
    }

    /// Check if sell price is within slippage tolerance
    fn check_sell_slippage(
        &self,
        current_supply: Decimal,
        amount: Decimal,
        min_price: Decimal,
    ) -> bool {
        let avg_price = self.avg_sell_price(current_supply, amount);
        avg_price >= min_price
    }
}

/// Price quote for an order
#[derive(Debug, Clone, Serialize)]
pub struct PriceQuote {
    /// Amount of tokens
    pub amount: Decimal,
    /// Total cost/proceeds in BTC
    pub total_btc: Decimal,
    /// Average price per token
    pub avg_price_btc: Decimal,
    /// Current spot price before trade
    pub spot_price_before: Decimal,
    /// Spot price after trade
    pub spot_price_after: Decimal,
    /// Price impact percentage
    pub price_impact_percent: Decimal,
}

impl PriceQuote {
    /// Check if price impact exceeds a threshold
    pub fn is_high_impact(&self, threshold_percent: Decimal) -> bool {
        self.price_impact_percent.abs() >= threshold_percent
    }

    /// Get effective spread (difference between spot before and average price)
    pub fn effective_spread_percent(&self) -> Decimal {
        if self.spot_price_before == dec!(0) {
            return dec!(0);
        }
        ((self.avg_price_btc - self.spot_price_before) / self.spot_price_before).abs() * dec!(100)
    }
}

/// Curve parameter validator
pub struct CurveValidator;

impl CurveValidator {
    /// Validate initial price (must be positive and reasonable)
    pub fn validate_initial_price(price: Decimal) -> Result<(), String> {
        if price <= dec!(0) {
            return Err("Initial price must be positive".to_string());
        }
        if price < dec!(0.00000001) {
            return Err("Initial price is too small (min 0.00000001 BTC)".to_string());
        }
        if price > dec!(100) {
            return Err("Initial price is too high (max 100 BTC)".to_string());
        }
        Ok(())
    }

    /// Validate increment/growth rate parameters
    pub fn validate_growth_rate(rate: Decimal, curve_type: &str) -> Result<(), String> {
        match curve_type {
            "linear" => {
                if rate < dec!(0) {
                    return Err("Linear increment cannot be negative".to_string());
                }
                if rate > dec!(1) {
                    return Err("Linear increment is too high (max 1 BTC)".to_string());
                }
            }
            "exponential" => {
                if rate <= dec!(0) {
                    return Err("Exponential growth rate must be positive".to_string());
                }
                if rate > dec!(0.1) {
                    return Err("Exponential growth rate is too high (max 0.1)".to_string());
                }
            }
            "bancor" if (rate <= dec!(0) || rate > dec!(1)) => {
                return Err("Reserve ratio must be between 0 and 1".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Validate scale factor (must be positive)
    pub fn validate_scale_factor(factor: Decimal) -> Result<(), String> {
        if factor <= dec!(0) {
            return Err("Scale factor must be positive".to_string());
        }
        if factor < dec!(0.1) {
            return Err("Scale factor is too small (min 0.1)".to_string());
        }
        Ok(())
    }
}

/// Curve comparison utilities
pub struct CurveComparator;

impl CurveComparator {
    /// Compare two curves at a given supply level
    pub fn compare_spot_prices<C1: BondingCurve, C2: BondingCurve>(
        curve1: &C1,
        curve2: &C2,
        supply: Decimal,
    ) -> CurveComparison {
        let price1 = curve1.spot_price(supply);
        let price2 = curve2.spot_price(supply);

        let difference = price1 - price2;
        let percent_difference = if price2 != dec!(0) {
            (difference / price2) * dec!(100)
        } else {
            dec!(0)
        };

        CurveComparison {
            supply,
            price1,
            price2,
            difference,
            percent_difference,
        }
    }

    /// Calculate price elasticity (% change in price / % change in supply)
    pub fn calculate_elasticity<C: BondingCurve>(
        curve: &C,
        supply: Decimal,
        delta_percent: Decimal,
    ) -> Decimal {
        let delta = supply * delta_percent / dec!(100);
        let new_supply = supply + delta;

        let price_before = curve.spot_price(supply);
        let price_after = curve.spot_price(new_supply);

        if price_before == dec!(0) {
            return dec!(0);
        }

        let price_change_percent = ((price_after - price_before) / price_before) * dec!(100);
        if delta_percent != dec!(0) {
            price_change_percent / delta_percent
        } else {
            dec!(0)
        }
    }

    /// Find the supply level where a curve reaches a target price
    pub fn find_price_target<C: BondingCurve>(
        curve: &C,
        target_price: Decimal,
        max_supply: Decimal,
        tolerance: Decimal,
    ) -> Option<Decimal> {
        // Binary search for supply level
        let mut low = dec!(0);
        let mut high = max_supply;
        let mut iterations = 0;
        let max_iterations = 100;

        while iterations < max_iterations && (high - low) > tolerance {
            let mid = (low + high) / dec!(2);
            let price = curve.spot_price(mid);

            if (price - target_price).abs() < tolerance {
                return Some(mid);
            }

            if price < target_price {
                low = mid;
            } else {
                high = mid;
            }

            iterations += 1;
        }

        None
    }
}

/// Result of comparing two curves
#[derive(Debug, Clone, Serialize)]
pub struct CurveComparison {
    /// Supply level at which the comparison was made
    pub supply: Decimal,
    /// Spot price from the first curve
    pub price1: Decimal,
    /// Spot price from the second curve
    pub price2: Decimal,
    /// Absolute price difference (price1 - price2)
    pub difference: Decimal,
    /// Percentage difference relative to price2
    pub percent_difference: Decimal,
}

/// Linear bonding curve: P(n) = initial_price + (n * increment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearBondingCurve {
    /// Starting price at zero supply
    pub initial_price: Decimal,
    /// Price increase per unit of additional supply
    pub increment: Decimal,
}

impl LinearBondingCurve {
    /// Create a new linear bonding curve
    pub fn new(initial_price: Decimal, increment: Decimal) -> Self {
        Self {
            initial_price,
            increment,
        }
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply - amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }
}

impl BondingCurve for LinearBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        self.initial_price + (current_supply * self.increment)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        // Integral of P(n) from current_supply to current_supply + amount
        // = amount * initial_price + increment * (amount * (2*supply + amount - 1) / 2)
        let base_cost = amount * self.initial_price;
        let incremental_cost =
            self.increment * amount * (dec!(2) * current_supply + amount - dec!(1)) / dec!(2);
        base_cost + incremental_cost
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        // Same as buy but from (supply - amount) to supply, with 5% spread
        let new_supply = current_supply - amount;
        let base_value = amount * self.initial_price;
        let incremental_value =
            self.increment * amount * (dec!(2) * new_supply + amount - dec!(1)) / dec!(2);
        (base_value + incremental_value) * dec!(0.95) // 5% spread
    }
}

/// Bancor bonding curve using reserve ratio formula
/// P(n) = reserve_balance / (supply * reserve_ratio)
/// This is an approximation using polynomial expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BancorBondingCurve {
    /// Reserve balance in BTC (virtual reserve)
    pub reserve_balance: Decimal,
    /// Reserve ratio (0-1), typically 0.1-0.5
    /// Higher ratio = lower price volatility
    pub reserve_ratio: Decimal,
    /// Initial supply for calculations
    pub initial_supply: Decimal,
}

impl BancorBondingCurve {
    /// Create a new Bancor bonding curve
    pub fn new(reserve_balance: Decimal, reserve_ratio: Decimal, initial_supply: Decimal) -> Self {
        Self {
            reserve_balance,
            reserve_ratio: reserve_ratio.clamp(dec!(0.05), dec!(1)),
            initial_supply,
        }
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }

    /// Calculate power approximation: (1 + x)^n using Taylor series
    /// For small x, this gives a good approximation
    fn power_approx(base: Decimal, exponent: Decimal) -> Decimal {
        // For (1+x)^n where x is small, use binomial expansion
        // (1+x)^n ≈ 1 + nx + n(n-1)x²/2 + ...
        let x = base - dec!(1);
        if x.abs() < dec!(0.5) {
            let term1 = dec!(1);
            let term2 = exponent * x;
            let term3 = exponent * (exponent - dec!(1)) * x * x / dec!(2);
            let term4 =
                exponent * (exponent - dec!(1)) * (exponent - dec!(2)) * x * x * x / dec!(6);
            (term1 + term2 + term3 + term4).max(dec!(0.001))
        } else {
            // For larger values, use simpler approximation
            // Convert to f64 for pow, then back
            let base_f64: f64 = base.try_into().unwrap_or(1.0);
            let exp_f64: f64 = exponent.try_into().unwrap_or(1.0);
            let result = base_f64.powf(exp_f64);
            Decimal::try_from(result).unwrap_or(dec!(1))
        }
    }
}

impl BondingCurve for BancorBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        let effective_supply = self.initial_supply + current_supply;
        if effective_supply == dec!(0) || self.reserve_ratio == dec!(0) {
            return dec!(0);
        }
        self.reserve_balance / (effective_supply * self.reserve_ratio)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) {
            return dec!(0);
        }
        let effective_supply = self.initial_supply + current_supply;
        if effective_supply == dec!(0) {
            return dec!(0);
        }

        // Bancor formula: cost = reserve * ((1 + amount/supply)^(1/rr) - 1)
        let supply_ratio = dec!(1) + amount / effective_supply;
        let exponent = dec!(1) / self.reserve_ratio;
        let power = Self::power_approx(supply_ratio, exponent);

        self.reserve_balance * (power - dec!(1))
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply {
            return dec!(0);
        }
        let effective_supply = self.initial_supply + current_supply;

        // Bancor formula: proceeds = reserve * (1 - (1 - amount/supply)^(1/rr))
        let supply_ratio = dec!(1) - amount / effective_supply;
        let exponent = dec!(1) / self.reserve_ratio;
        let power = Self::power_approx(supply_ratio, exponent);

        // Apply 5% spread
        self.reserve_balance * (dec!(1) - power) * dec!(0.95)
    }
}

/// Exponential bonding curve: P(n) = initial_price * e^(k * n)
/// Uses polynomial approximation for e^x
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExponentialBondingCurve {
    /// Starting price at zero supply
    pub initial_price: Decimal,
    /// Growth rate (k). Smaller = gentler curve.
    pub growth_rate: Decimal,
}

impl ExponentialBondingCurve {
    /// Create a new exponential bonding curve
    pub fn new(initial_price: Decimal, growth_rate: Decimal) -> Self {
        Self {
            initial_price,
            growth_rate: growth_rate.clamp(dec!(0.0001), dec!(0.1)),
        }
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }

    /// Approximate e^x using Taylor series
    fn exp_approx(x: Decimal) -> Decimal {
        // e^x ≈ 1 + x + x²/2 + x³/6 + x⁴/24
        let x2 = x * x;
        let x3 = x2 * x;
        let x4 = x3 * x;
        let x5 = x4 * x;

        let result = dec!(1) + x + x2 / dec!(2) + x3 / dec!(6) + x4 / dec!(24) + x5 / dec!(120);
        result.max(dec!(0.0001))
    }
}

impl BondingCurve for ExponentialBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        let exponent = self.growth_rate * current_supply;
        // Clamp exponent to avoid overflow
        let clamped_exp = exponent.clamp(dec!(-10), dec!(10));
        self.initial_price * Self::exp_approx(clamped_exp)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || self.growth_rate == dec!(0) {
            return self.initial_price * amount;
        }

        // Integral of P * e^(k*n) from supply to supply+amount
        // = (P/k) * (e^(k*(supply+amount)) - e^(k*supply))
        let k = self.growth_rate;
        let exp_start = Self::exp_approx((k * current_supply).clamp(dec!(-10), dec!(10)));
        let exp_end = Self::exp_approx((k * (current_supply + amount)).clamp(dec!(-10), dec!(10)));

        let integral = (self.initial_price / k) * (exp_end - exp_start);

        // Floor the integral at `amount * spot_price(current_supply)`.
        //
        // For a monotonically increasing curve the true cost of buying `amount`
        // tokens is ∫ P(n) dn over [s, s+amount], and since P(n) >= P(s) on that
        // interval the integral is ALWAYS >= amount * P(s). When the exponent
        // saturates (both endpoints clamp to 10) the computed integral collapses
        // to 0; this floor — which is strictly positive for valid inputs — then
        // correctly supplies the lower bound. In the non-saturated regime the
        // integral dominates and the floor is inert, so normal pricing is
        // unchanged.
        let lower_bound = amount * self.spot_price(current_supply);
        integral.max(lower_bound)
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply || self.growth_rate == dec!(0) {
            return self.initial_price * amount * dec!(0.95);
        }

        let new_supply = current_supply - amount;
        let k = self.growth_rate;
        let exp_start = Self::exp_approx((k * new_supply).clamp(dec!(-10), dec!(10)));
        let exp_end = Self::exp_approx((k * current_supply).clamp(dec!(-10), dec!(10)));

        let integral = (self.initial_price / k) * (exp_end - exp_start);

        // Floor the pre-spread integral at `amount * spot_price(new_supply)`.
        //
        // The proceeds of selling `amount` tokens is ∫ P(n) dn over
        // [s-amount, s] = [new_supply, current_supply], and since P(n) >=
        // P(new_supply) on that interval the integral is ALWAYS >=
        // amount * P(new_supply). Under exponent saturation the integral
        // collapses to 0; the floor (strictly positive for valid inputs) then
        // supplies the lower bound. The existing 5% spread is applied AFTER the
        // floor so sell proceeds stay below buy cost while remaining > 0.
        let lower_bound = amount * self.spot_price(new_supply);
        integral.max(lower_bound) * dec!(0.95)
    }
}

/// Sigmoid bonding curve: P(n) = max_price / (1 + e^(-k*(n - midpoint)))
/// Creates S-curve pricing that caps at max_price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigmoidBondingCurve {
    /// Maximum price the curve approaches
    pub max_price: Decimal,
    /// Growth rate (steepness of curve)
    pub growth_rate: Decimal,
    /// Supply midpoint where price = max_price/2
    pub midpoint: Decimal,
}

impl SigmoidBondingCurve {
    /// Create a new sigmoid bonding curve
    pub fn new(max_price: Decimal, growth_rate: Decimal, midpoint: Decimal) -> Self {
        Self {
            max_price,
            growth_rate: growth_rate.clamp(dec!(0.001), dec!(0.1)),
            midpoint,
        }
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }

    /// Sigmoid function: 1 / (1 + e^-x)
    fn sigmoid(x: Decimal) -> Decimal {
        let clamped = x.clamp(dec!(-10), dec!(10));
        let exp_neg_x = ExponentialBondingCurve::exp_approx(-clamped);
        dec!(1) / (dec!(1) + exp_neg_x)
    }
}

impl BondingCurve for SigmoidBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        let x = self.growth_rate * (current_supply - self.midpoint);
        self.max_price * Self::sigmoid(x)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) {
            return dec!(0);
        }

        // Numerical integration using trapezoidal rule
        let steps = 100i32;
        let step_size = amount / Decimal::from(steps);
        let mut total = dec!(0);

        for i in 0..steps {
            let n = current_supply + step_size * Decimal::from(i);
            let price_start = self.spot_price(n);
            let price_end = self.spot_price(n + step_size);
            total += (price_start + price_end) / dec!(2) * step_size;
        }

        total
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply {
            return dec!(0);
        }

        let new_supply = current_supply - amount;

        // Numerical integration
        let steps = 100i32;
        let step_size = amount / Decimal::from(steps);
        let mut total = dec!(0);

        for i in 0..steps {
            let n = new_supply + step_size * Decimal::from(i);
            let price_start = self.spot_price(n);
            let price_end = self.spot_price(n + step_size);
            total += (price_start + price_end) / dec!(2) * step_size;
        }

        // Apply 5% spread
        total * dec!(0.95)
    }
}

/// Phase of the adaptive bonding curve
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CurvePhase {
    /// Early adopter phase: exponential growth (rewards early buyers)
    EarlyAdopter,
    /// Growth phase: linear growth (fair distribution)
    Growth,
    /// Maturity phase: sigmoid (price stabilization)
    Maturity,
}

/// Adaptive bonding curve that transitions through phases
/// - Early Adopter (0 - phase1_threshold): Exponential growth
/// - Growth (phase1_threshold - phase2_threshold): Linear growth
/// - Maturity (> phase2_threshold): Sigmoid approach to max
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveBondingCurve {
    /// Base initial price
    pub initial_price: Decimal,
    /// Maximum price the curve can reach
    pub max_price: Decimal,
    /// Supply threshold to transition from Early to Growth
    pub phase1_threshold: Decimal,
    /// Supply threshold to transition from Growth to Maturity
    pub phase2_threshold: Decimal,
    /// Total supply cap
    pub total_supply: Decimal,
}

impl AdaptiveBondingCurve {
    /// Create a new adaptive bonding curve with explicit phase thresholds
    pub fn new(
        initial_price: Decimal,
        max_price: Decimal,
        phase1_threshold: Decimal,
        phase2_threshold: Decimal,
        total_supply: Decimal,
    ) -> Self {
        Self {
            initial_price,
            max_price,
            phase1_threshold: phase1_threshold.max(dec!(1)),
            phase2_threshold: phase2_threshold.max(phase1_threshold + dec!(1)),
            total_supply,
        }
    }

    /// Create with default thresholds based on total supply
    /// Phase 1 at 10%, Phase 2 at 50%
    pub fn with_defaults(
        initial_price: Decimal,
        max_price: Decimal,
        total_supply: Decimal,
    ) -> Self {
        Self::new(
            initial_price,
            max_price,
            total_supply * dec!(0.1), // 10% for early adopter
            total_supply * dec!(0.5), // 50% for growth phase
            total_supply,
        )
    }

    /// Determine current phase based on supply
    pub fn get_phase(&self, current_supply: Decimal) -> CurvePhase {
        if current_supply < self.phase1_threshold {
            CurvePhase::EarlyAdopter
        } else if current_supply < self.phase2_threshold {
            CurvePhase::Growth
        } else {
            CurvePhase::Maturity
        }
    }

    /// Get price at phase 1 threshold (for continuity)
    fn price_at_phase1(&self) -> Decimal {
        self.early_adopter_price(self.phase1_threshold)
    }

    /// Get price at phase 2 threshold (for continuity)
    fn price_at_phase2(&self) -> Decimal {
        self.growth_price(self.phase2_threshold)
    }

    /// Early adopter phase pricing: exponential growth
    fn early_adopter_price(&self, supply: Decimal) -> Decimal {
        // Use simple exponential growth: P(n) = initial_price * (1 + growth_rate)^n
        // This provides reliable increasing prices
        if supply <= dec!(0) {
            return self.initial_price;
        }

        // Calculate growth rate to reach 3x at phase1_threshold
        // (1 + r)^threshold = 3, so r = 3^(1/threshold) - 1
        // For small r, approximate: r ≈ ln(3) / threshold ≈ 1.1 / threshold
        let growth_per_unit = dec!(1.1) / self.phase1_threshold.max(dec!(1));

        // P(n) = initial * (1 + n * growth_per_unit)
        // This is linearized exponential growth that's simpler and always increasing
        let multiplier = dec!(1) + supply * growth_per_unit;
        self.initial_price * multiplier.min(dec!(3))
    }

    /// Growth phase pricing: linear growth
    fn growth_price(&self, supply: Decimal) -> Decimal {
        // Linear from price_at_phase1 to a target price at phase2
        let start_price = self.price_at_phase1();
        let target_price = self.max_price * dec!(0.7); // Target 70% of max at end of growth

        let supply_in_phase = supply - self.phase1_threshold;
        let phase_length = self.phase2_threshold - self.phase1_threshold;

        if phase_length <= dec!(0) {
            return start_price;
        }

        let progress = supply_in_phase / phase_length;
        start_price + (target_price - start_price) * progress.min(dec!(1))
    }

    /// Maturity phase pricing: sigmoid approach to max
    fn maturity_price(&self, supply: Decimal) -> Decimal {
        // Sigmoid from price_at_phase2 approaching max_price
        let start_price = self.price_at_phase2();
        let remaining_supply = self.total_supply - self.phase2_threshold;

        if remaining_supply <= dec!(0) {
            return start_price;
        }

        let supply_in_phase = supply - self.phase2_threshold;
        let progress = supply_in_phase / remaining_supply;

        // Sigmoid transformation for smooth approach
        // Using a simple S-curve approximation
        let sigmoid_progress = if progress < dec!(0.5) {
            // First half: accelerating
            dec!(2) * progress * progress
        } else {
            // Second half: decelerating
            dec!(1) - dec!(2) * (dec!(1) - progress) * (dec!(1) - progress)
        };

        start_price + (self.max_price - start_price) * sigmoid_progress.min(dec!(1))
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }
}

impl BondingCurve for AdaptiveBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        match self.get_phase(current_supply) {
            CurvePhase::EarlyAdopter => self.early_adopter_price(current_supply),
            CurvePhase::Growth => self.growth_price(current_supply),
            CurvePhase::Maturity => self.maturity_price(current_supply),
        }
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) {
            return dec!(0);
        }

        // Numerical integration using trapezoidal rule
        // This handles phase transitions smoothly
        let steps = 100i32;
        let step_size = amount / Decimal::from(steps);
        let mut total = dec!(0);

        for i in 0..steps {
            let n = current_supply + step_size * Decimal::from(i);
            let price_start = self.spot_price(n);
            let price_end = self.spot_price(n + step_size);
            total += (price_start + price_end) / dec!(2) * step_size;
        }

        total
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply {
            return dec!(0);
        }

        let new_supply = current_supply - amount;

        // Numerical integration
        let steps = 100i32;
        let step_size = amount / Decimal::from(steps);
        let mut total = dec!(0);

        for i in 0..steps {
            let n = new_supply + step_size * Decimal::from(i);
            let price_start = self.spot_price(n);
            let price_end = self.spot_price(n + step_size);
            total += (price_start + price_end) / dec!(2) * step_size;
        }

        // Apply 5% spread
        total * dec!(0.95)
    }
}

/// Square root bonding curve: P(n) = initial_price * sqrt(1 + n/scale_factor)
/// Provides gentler price increases than exponential but still rewards early adopters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquareRootBondingCurve {
    /// Starting price at zero supply
    pub initial_price: Decimal,
    /// Scale factor controlling curve steepness. Higher = gentler curve.
    pub scale_factor: Decimal,
}

impl SquareRootBondingCurve {
    /// Create a new square root bonding curve
    pub fn new(initial_price: Decimal, scale_factor: Decimal) -> Self {
        Self {
            initial_price,
            scale_factor: scale_factor.max(dec!(1)),
        }
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }

    /// Approximate square root using Newton's method
    fn sqrt_approx(x: Decimal) -> Decimal {
        if x <= dec!(0) {
            return dec!(0);
        }
        if x == dec!(1) {
            return dec!(1);
        }

        // Convert to f64 for sqrt, then back
        let x_f64: f64 = x.try_into().unwrap_or(1.0);
        let result = x_f64.sqrt();
        Decimal::try_from(result).unwrap_or(dec!(1))
    }

    /// Closed-form integral of P(s) = init * sqrt(1 + s/k) from `supply` to `supply + amount`.
    ///
    /// Antiderivative: F(s) = init * (2k/3) * (1 + s/k)^(3/2)
    /// Definite integral = F(s0 + amount) - F(s0)
    fn sqrt_integral(&self, supply: Decimal, amount: Decimal) -> Decimal {
        let init: f64 = self.initial_price.try_into().unwrap_or(0.0);
        let k: f64 = self.scale_factor.try_into().unwrap_or(1.0);
        let s0: f64 = supply.try_into().unwrap_or(0.0);
        let amt: f64 = amount.try_into().unwrap_or(0.0);

        if k <= 0.0 || amt <= 0.0 {
            return dec!(0);
        }

        let f_upper = (1.0 + (s0 + amt) / k).powf(1.5);
        let f_lower = (1.0 + s0 / k).powf(1.5);
        let total = init * (2.0 * k / 3.0) * (f_upper - f_lower);

        Decimal::try_from(total.max(0.0)).unwrap_or(dec!(0))
    }
}

impl BondingCurve for SquareRootBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        let normalized = dec!(1) + current_supply / self.scale_factor;
        self.initial_price * Self::sqrt_approx(normalized)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) {
            return dec!(0);
        }

        // Exact closed-form antiderivative of P(s) = init * sqrt(1 + s/k)
        self.sqrt_integral(current_supply, amount)
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply {
            return dec!(0);
        }

        // Integrate from (current_supply - amount) to current_supply, then apply 5% spread
        self.sqrt_integral(current_supply - amount, amount) * dec!(0.95)
    }
}

/// Logarithmic bonding curve: P(n) = initial_price * (1 + log_base(1 + n/scale_factor))
/// Provides diminishing price growth - good for long-term stability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogarithmicBondingCurve {
    /// Starting price at zero supply
    pub initial_price: Decimal,
    /// Scale factor controlling curve steepness
    pub scale_factor: Decimal,
    /// Logarithm base (typically e=2.71828 or 10)
    pub log_base: Decimal,
}

impl LogarithmicBondingCurve {
    /// Create a new logarithmic bonding curve
    pub fn new(initial_price: Decimal, scale_factor: Decimal, log_base: Decimal) -> Self {
        Self {
            initial_price,
            scale_factor: scale_factor.max(dec!(1)),
            log_base: log_base.clamp(dec!(1.1), dec!(10)),
        }
    }

    /// Create with natural logarithm (base e)
    pub fn new_natural_log(initial_price: Decimal, scale_factor: Decimal) -> Self {
        // Use Euler's number (e ≈ 2.71828182845904523536...)
        let e = Decimal::try_from(std::f64::consts::E).unwrap_or(dec!(2.71828182845904523536));
        Self::new(initial_price, scale_factor, e)
    }

    /// Get a price quote for buying tokens
    pub fn get_buy_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.buy_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = self.spot_price(current_supply + amount);

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.buy_price_impact(current_supply, amount),
        }
    }

    /// Get a price quote for selling tokens
    pub fn get_sell_quote(&self, current_supply: Decimal, amount: Decimal) -> PriceQuote {
        let total = self.sell_price(current_supply, amount);
        let spot_before = self.spot_price(current_supply);
        let spot_after = if current_supply > amount {
            self.spot_price(current_supply - amount)
        } else {
            dec!(0)
        };

        PriceQuote {
            amount,
            total_btc: total,
            avg_price_btc: if amount > dec!(0) {
                total / amount
            } else {
                dec!(0)
            },
            spot_price_before: spot_before,
            spot_price_after: spot_after,
            price_impact_percent: self.sell_price_impact(current_supply, amount),
        }
    }

    /// Approximate logarithm: log_base(x) = ln(x) / ln(base)
    fn log_approx(x: Decimal, base: Decimal) -> Decimal {
        if x <= dec!(0) {
            return dec!(0);
        }
        if x == dec!(1) {
            return dec!(0);
        }

        // Convert to f64 for log calculation
        let x_f64: f64 = x.try_into().unwrap_or(1.0);
        let base_f64: f64 = base.try_into().unwrap_or(std::f64::consts::E);
        let result = x_f64.ln() / base_f64.ln();
        Decimal::try_from(result).unwrap_or(dec!(0))
    }

    /// Closed-form integral of P(s) = init * (1 + log_b(1 + s/k)) from `supply` to `supply + amount`.
    ///
    /// Antiderivative: F(s) = init * [s + (s+k)/ln(b) * (ln(1 + s/k) - 1)]
    /// Definite integral = init * { amount + (1/ln(b)) *
    ///     [(s0+amt+k)*(ln(1+(s0+amt)/k) - 1) - (s0+k)*(ln(1+s0/k) - 1)] }
    fn log_integral(&self, supply: Decimal, amount: Decimal) -> Decimal {
        let init: f64 = self.initial_price.try_into().unwrap_or(0.0);
        let k: f64 = self.scale_factor.try_into().unwrap_or(1.0);
        let s0: f64 = supply.try_into().unwrap_or(0.0);
        let amt: f64 = amount.try_into().unwrap_or(0.0);
        let log_b: f64 = self.log_base.try_into().unwrap_or(std::f64::consts::E);

        if k <= 0.0 || amt <= 0.0 || log_b <= 0.0 || log_b == 1.0 {
            return dec!(0);
        }

        let ln_b = log_b.ln();

        // Antiderivative evaluated at s: s + (s+k)/ln_b * (ln(1 + s/k) - 1)
        let antiderivative = |s: f64| -> f64 {
            let arg = 1.0 + s / k;
            if arg <= 0.0 {
                return 0.0;
            }
            s + (s + k) / ln_b * (arg.ln() - 1.0)
        };

        let total = init * (antiderivative(s0 + amt) - antiderivative(s0));
        Decimal::try_from(total.max(0.0)).unwrap_or(dec!(0))
    }
}

impl BondingCurve for LogarithmicBondingCurve {
    fn spot_price(&self, current_supply: Decimal) -> Decimal {
        let normalized = dec!(1) + current_supply / self.scale_factor;
        let log_term = Self::log_approx(normalized, self.log_base);
        self.initial_price * (dec!(1) + log_term)
    }

    fn buy_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) {
            return dec!(0);
        }

        // Exact closed-form antiderivative of P(s) = init * (1 + log_b(1 + s/k))
        self.log_integral(current_supply, amount)
    }

    fn sell_price(&self, current_supply: Decimal, amount: Decimal) -> Decimal {
        if amount <= dec!(0) || amount > current_supply {
            return dec!(0);
        }

        // Integrate from (current_supply - amount) to current_supply, then apply 5% spread
        self.log_integral(current_supply - amount, amount) * dec!(0.95)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_spot_price() {
        let curve = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));

        assert_eq!(curve.spot_price(dec!(0)), dec!(0.0001));
        assert_eq!(curve.spot_price(dec!(100)), dec!(0.0011));
    }

    #[test]
    fn test_linear_buy_price() {
        let curve = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));

        // Buy 10 tokens starting from supply 0
        let cost = curve.buy_price(dec!(0), dec!(10));

        // Expected: 10 * 0.0001 + 0.00001 * 10 * (0 + 10 - 1) / 2
        // = 0.001 + 0.00001 * 10 * 4.5
        // = 0.001 + 0.00045
        // = 0.00145
        assert_eq!(cost, dec!(0.00145));
    }

    #[test]
    fn test_linear_sell_price_less_than_buy() {
        let curve = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));

        let buy_cost = curve.buy_price(dec!(0), dec!(10));
        let sell_proceeds = curve.sell_price(dec!(10), dec!(10));

        // Sell proceeds should be less than buy cost due to spread
        assert!(sell_proceeds < buy_cost);
        assert!(sell_proceeds > buy_cost * dec!(0.9)); // But not too much less
    }

    #[test]
    fn test_bancor_spot_price() {
        let curve = BancorBondingCurve::new(dec!(0.1), dec!(0.5), dec!(1000));

        // At supply 0: price = 0.1 / (1000 * 0.5) = 0.0002
        let price = curve.spot_price(dec!(0));
        assert_eq!(price, dec!(0.0002));
    }

    #[test]
    fn test_exponential_increasing_price() {
        let curve = ExponentialBondingCurve::new(dec!(0.0001), dec!(0.001));

        let price_0 = curve.spot_price(dec!(0));
        let price_100 = curve.spot_price(dec!(100));
        let price_1000 = curve.spot_price(dec!(1000));

        // Price should increase as supply increases
        assert!(price_100 > price_0);
        assert!(price_1000 > price_100);
    }

    /// Regression for the exponent-saturation zero bug found by
    /// `prop_exponential_prices_positive` (minimal failing input
    /// supply=10000, amount=1). With growth_rate=0.001 the exponent
    /// k*supply reaches the clamp boundary (10) at supply=10000, so both
    /// integral endpoints clamp to the same value and the raw integral
    /// collapses to 0. The lower-bound floor must keep both buy and sell
    /// strictly positive.
    #[test]
    fn test_exponential_prices_positive_at_saturation() {
        let curve = ExponentialBondingCurve::new(dec!(0.0001), dec!(0.001));

        // The exact saved regression seed: supply=10000, amount=1.
        let buy = curve.buy_price(dec!(10000), dec!(1));
        let sell = curve.sell_price(dec!(10000), dec!(1));
        let spot = curve.spot_price(dec!(10000));

        assert!(spot > dec!(0), "spot must stay positive: {spot}");
        assert!(
            buy > dec!(0),
            "buy_price must be positive at saturation: buy={buy}"
        );
        assert!(
            sell > dec!(0),
            "sell_price must be positive at saturation: sell={sell}"
        );
        // The floor is exactly amount * spot at saturation (raw integral == 0).
        assert!(
            buy >= spot,
            "buy must be >= amount*spot floor: buy={buy} spot={spot}"
        );
        // Spread keeps sell below buy but still positive.
        assert!(
            sell < buy,
            "sell must stay below buy: sell={sell} buy={buy}"
        );

        // Mid-supply sanity (non-saturated regime: the integral dominates and
        // the floor is inert). buy must exceed amount*spot due to price impact.
        let s_mid = dec!(5000);
        let a_mid = dec!(10);
        let buy_mid = curve.buy_price(s_mid, a_mid);
        let sell_mid = curve.sell_price(s_mid, a_mid);
        assert!(buy_mid > dec!(0), "mid buy must be positive: {buy_mid}");
        assert!(sell_mid > dec!(0), "mid sell must be positive: {sell_mid}");
        assert!(
            sell_mid < buy_mid,
            "mid sell must stay below buy: sell={sell_mid} buy={buy_mid}"
        );

        // Low-supply sanity (well away from the clamp).
        let buy_low = curve.buy_price(dec!(100), dec!(5));
        let sell_low = curve.sell_price(dec!(100), dec!(5));
        assert!(buy_low > dec!(0), "low buy must be positive: {buy_low}");
        assert!(sell_low > dec!(0), "low sell must be positive: {sell_low}");
        assert!(
            sell_low < buy_low,
            "low sell must stay below buy: sell={sell_low} buy={buy_low}"
        );
    }

    #[test]
    fn test_sigmoid_approaches_max() {
        let curve = SigmoidBondingCurve::new(dec!(1), dec!(0.01), dec!(500));

        let price_0 = curve.spot_price(dec!(0));
        let price_500 = curve.spot_price(dec!(500));
        let price_1000 = curve.spot_price(dec!(1000));

        // At midpoint, price should be ~0.5 of max
        assert!(price_500 > dec!(0.4));
        assert!(price_500 < dec!(0.6));

        // At high supply, should approach max
        assert!(price_1000 > price_500);
        assert!(price_1000 < dec!(1));

        // At 0, should be low
        assert!(price_0 < dec!(0.1));
    }

    #[test]
    fn test_adaptive_phase_transitions() {
        // Supply 1000, phases at 100 (10%) and 500 (50%)
        let curve = AdaptiveBondingCurve::with_defaults(
            dec!(0.0001), // initial price
            dec!(0.01),   // max price
            dec!(1000),   // total supply
        );

        // Check phases
        assert_eq!(curve.get_phase(dec!(50)), CurvePhase::EarlyAdopter);
        assert_eq!(curve.get_phase(dec!(100)), CurvePhase::Growth);
        assert_eq!(curve.get_phase(dec!(300)), CurvePhase::Growth);
        assert_eq!(curve.get_phase(dec!(500)), CurvePhase::Maturity);
        assert_eq!(curve.get_phase(dec!(800)), CurvePhase::Maturity);
    }

    #[test]
    fn test_adaptive_price_increases() {
        let curve = AdaptiveBondingCurve::with_defaults(
            dec!(0.0001), // initial price
            dec!(0.01),   // max price
            dec!(1000),   // total supply
        );

        let price_0 = curve.spot_price(dec!(0));
        let price_50 = curve.spot_price(dec!(50));
        let price_100 = curve.spot_price(dec!(100));
        let price_300 = curve.spot_price(dec!(300));
        let price_500 = curve.spot_price(dec!(500));
        let price_900 = curve.spot_price(dec!(900));

        // Price should generally increase
        assert!(price_50 > price_0);
        assert!(price_100 > price_50);
        assert!(price_300 > price_100);
        assert!(price_500 > price_300);
        assert!(price_900 > price_500);

        // Maturity phase should approach but not exceed max
        assert!(price_900 < dec!(0.01));
    }

    #[test]
    fn test_adaptive_sell_less_than_buy() {
        let curve = AdaptiveBondingCurve::with_defaults(dec!(0.0001), dec!(0.01), dec!(1000));

        let buy_cost = curve.buy_price(dec!(100), dec!(50));
        let sell_proceeds = curve.sell_price(dec!(150), dec!(50));

        // Sell proceeds should be less due to 5% spread
        assert!(sell_proceeds < buy_cost);
        assert!(sell_proceeds > buy_cost * dec!(0.9));
    }

    #[test]
    fn test_sqrt_curve_gentler_growth() {
        let curve = SquareRootBondingCurve::new(dec!(0.0001), dec!(100));

        let price_0 = curve.spot_price(dec!(0));
        let price_100 = curve.spot_price(dec!(100));
        let price_400 = curve.spot_price(dec!(400));

        // Price should increase but more gently than exponential
        assert!(price_100 > price_0);
        assert!(price_400 > price_100);

        // sqrt(1) = 1, sqrt(2) ≈ 1.414, sqrt(5) ≈ 2.236
        // So price growth should be sublinear
        let growth_100 = price_100 / price_0;
        let growth_400 = price_400 / price_0;

        // sqrt relationship: 400/100 = 4, but sqrt(5)/sqrt(2) ≈ 1.58
        assert!(growth_400 < growth_100 * dec!(2));
    }

    #[test]
    fn test_sqrt_sell_less_than_buy() {
        let curve = SquareRootBondingCurve::new(dec!(0.0001), dec!(100));

        let buy_cost = curve.buy_price(dec!(100), dec!(50));
        let sell_proceeds = curve.sell_price(dec!(150), dec!(50));

        // Sell should be less than buy due to spread
        assert!(sell_proceeds < buy_cost);
        assert!(sell_proceeds > buy_cost * dec!(0.9));
    }

    #[test]
    fn test_log_curve_basic_properties() {
        let curve = LogarithmicBondingCurve::new_natural_log(dec!(0.0001), dec!(100));

        let price_0 = curve.spot_price(dec!(0));
        let price_100 = curve.spot_price(dec!(100));
        let price_1000 = curve.spot_price(dec!(1000));

        // Price should increase monotonically
        assert!(price_100 > price_0);
        assert!(price_1000 > price_100);

        // All prices should be positive
        assert!(price_0 > dec!(0));
        assert!(price_100 > dec!(0));
        assert!(price_1000 > dec!(0));
    }

    #[test]
    fn test_log_sell_less_than_buy() {
        let curve = LogarithmicBondingCurve::new_natural_log(dec!(0.0001), dec!(100));

        let buy_cost = curve.buy_price(dec!(100), dec!(50));
        let sell_proceeds = curve.sell_price(dec!(150), dec!(50));

        // Sell should be less than buy due to spread
        assert!(sell_proceeds < buy_cost);
        assert!(sell_proceeds > buy_cost * dec!(0.9));
    }

    #[test]
    fn test_sqrt_curve_basic_properties() {
        let curve = SquareRootBondingCurve::new(dec!(0.0001), dec!(100));

        let price_0 = curve.spot_price(dec!(0));
        let price_100 = curve.spot_price(dec!(100));
        let price_400 = curve.spot_price(dec!(400));

        // Price should increase monotonically
        assert!(price_100 > price_0);
        assert!(price_400 > price_100);

        // All prices should be positive
        assert!(price_0 > dec!(0));
        assert!(price_100 > dec!(0));
        assert!(price_400 > dec!(0));
    }

    /// Verify closed-form SquareRoot integral matches 100-step trapezoidal rule within 0.01%.
    #[test]
    fn test_closed_form_matches_numerical_sqrt() {
        let curve = SquareRootBondingCurve::new(dec!(0.0001), dec!(100));

        let test_cases: &[(Decimal, Decimal)] = &[
            (dec!(0), dec!(50)),
            (dec!(100), dec!(50)),
            (dec!(500), dec!(100)),
            (dec!(1000), dec!(200)),
        ];

        for &(supply, amount) in test_cases {
            // Reference: 100-step trapezoidal rule
            let steps = 100i32;
            let step_size = amount / Decimal::from(steps);
            let mut numerical = dec!(0);
            for i in 0..steps {
                let n = supply + step_size * Decimal::from(i);
                let price_start = curve.spot_price(n);
                let price_end = curve.spot_price(n + step_size);
                numerical += (price_start + price_end) / dec!(2) * step_size;
            }

            let closed_form = curve.buy_price(supply, amount);

            // Relative error must be under 0.01%
            let relative_error = if numerical > dec!(0) {
                ((closed_form - numerical) / numerical).abs()
            } else {
                dec!(0)
            };
            assert!(
                relative_error < dec!(0.0001),
                "sqrt closed-form vs numerical: supply={supply}, amount={amount}, \
                 closed={closed_form}, numerical={numerical}, rel_err={relative_error}"
            );
        }
    }

    /// Verify closed-form Logarithmic integral matches 100-step trapezoidal rule within 0.01%.
    #[test]
    fn test_closed_form_matches_numerical_log() {
        let curve = LogarithmicBondingCurve::new_natural_log(dec!(0.0001), dec!(100));

        let test_cases: &[(Decimal, Decimal)] = &[
            (dec!(0), dec!(50)),
            (dec!(100), dec!(50)),
            (dec!(500), dec!(100)),
            (dec!(1000), dec!(200)),
        ];

        for &(supply, amount) in test_cases {
            // Reference: 100-step trapezoidal rule
            let steps = 100i32;
            let step_size = amount / Decimal::from(steps);
            let mut numerical = dec!(0);
            for i in 0..steps {
                let n = supply + step_size * Decimal::from(i);
                let price_start = curve.spot_price(n);
                let price_end = curve.spot_price(n + step_size);
                numerical += (price_start + price_end) / dec!(2) * step_size;
            }

            let closed_form = curve.buy_price(supply, amount);

            // Relative error must be under 0.01%
            let relative_error = if numerical > dec!(0) {
                ((closed_form - numerical) / numerical).abs()
            } else {
                dec!(0)
            };
            assert!(
                relative_error < dec!(0.0001),
                "log closed-form vs numerical: supply={supply}, amount={amount}, \
                 closed={closed_form}, numerical={numerical}, rel_err={relative_error}"
            );
        }
    }
}
