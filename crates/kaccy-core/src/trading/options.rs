//! Options and derivatives trading system
//!
//! This module provides European-style options contracts with Black-Scholes pricing,
//! Greeks calculation, and cash settlement.

use crate::error::{CoreError, Result};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of option contract
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionType {
    /// Grants the holder the right to buy the underlying asset
    Call,
    /// Grants the holder the right to sell the underlying asset
    Put,
}

/// Status of an option contract
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionStatus {
    /// Active and tradeable
    Active,
    /// Exercised by holder
    Exercised,
    /// Expired without exercise
    Expired,
    /// Cancelled by issuer (if allowed)
    Cancelled,
}

/// European-style option contract
///
/// Cannot be exercised before expiration date
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    /// Unique identifier for this contract
    pub contract_id: Uuid,
    /// Token that is the underlying asset
    pub token_id: Uuid,
    /// Whether this is a call or put option
    pub option_type: OptionType,
    /// Strike price at which the option may be exercised
    pub strike_price: Decimal,
    /// Date and time when this option expires
    pub expiration_date: DateTime<Utc>,
    /// Premium paid by the buyer to the writer
    pub premium: Decimal,
    /// User who wrote (issued) this contract
    pub writer_id: Uuid,
    /// User who currently holds (owns) this contract, if sold
    pub holder_id: Option<Uuid>,
    /// Current lifecycle status of the contract
    pub status: OptionStatus,
    /// Timestamp when this contract was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when this contract was exercised, if applicable
    pub exercised_at: Option<DateTime<Utc>>,
    /// Amount of underlying tokens per contract
    pub contract_size: Decimal,
}

impl OptionContract {
    /// Create a new option contract
    pub fn new(
        token_id: Uuid,
        option_type: OptionType,
        strike_price: Decimal,
        expiration_date: DateTime<Utc>,
        premium: Decimal,
        writer_id: Uuid,
        contract_size: Decimal,
    ) -> Result<Self> {
        if strike_price <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Strike price must be positive".to_string(),
            ));
        }

        if premium < Decimal::ZERO {
            return Err(CoreError::Validation(
                "Premium cannot be negative".to_string(),
            ));
        }

        if contract_size <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Contract size must be positive".to_string(),
            ));
        }

        if expiration_date <= Utc::now() {
            return Err(CoreError::Validation(
                "Expiration date must be in the future".to_string(),
            ));
        }

        Ok(Self {
            contract_id: Uuid::new_v4(),
            token_id,
            option_type,
            strike_price,
            expiration_date,
            premium,
            writer_id,
            holder_id: None,
            status: OptionStatus::Active,
            created_at: Utc::now(),
            exercised_at: None,
            contract_size,
        })
    }

    /// Check if option is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiration_date
    }

    /// Check if option is in the money
    pub fn is_in_the_money(&self, current_price: Decimal) -> bool {
        match self.option_type {
            OptionType::Call => current_price > self.strike_price,
            OptionType::Put => current_price < self.strike_price,
        }
    }

    /// Calculate intrinsic value
    pub fn intrinsic_value(&self, current_price: Decimal) -> Decimal {
        match self.option_type {
            OptionType::Call => (current_price - self.strike_price).max(Decimal::ZERO),
            OptionType::Put => (self.strike_price - current_price).max(Decimal::ZERO),
        }
    }

    /// Get time to expiration in years
    pub fn time_to_expiration(&self) -> Result<f64> {
        let now = Utc::now();
        if now > self.expiration_date {
            return Ok(0.0);
        }

        let duration = self.expiration_date - now;
        let days = duration.num_days() as f64;
        Ok(days / 365.0)
    }

    /// Calculate payoff at expiration
    pub fn payoff(&self, settlement_price: Decimal) -> Decimal {
        self.intrinsic_value(settlement_price) * self.contract_size
    }
}

/// Greeks for option pricing sensitivity analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    /// Delta: rate of change of option price with respect to underlying price
    pub delta: f64,
    /// Gamma: rate of change of delta with respect to underlying price
    pub gamma: f64,
    /// Theta: rate of change of option price with respect to time
    pub theta: f64,
    /// Vega: rate of change of option price with respect to volatility
    pub vega: f64,
    /// Rho: rate of change of option price with respect to interest rate
    pub rho: f64,
}

/// Black-Scholes option pricing model
pub struct BlackScholesModel {
    /// Risk-free interest rate (annualized)
    risk_free_rate: f64,
}

impl BlackScholesModel {
    /// Create a new Black-Scholes model with the given annualized risk-free rate
    pub fn new(risk_free_rate: f64) -> Result<Self> {
        if risk_free_rate < 0.0 {
            return Err(CoreError::Validation(
                "Risk-free rate cannot be negative".to_string(),
            ));
        }
        Ok(Self { risk_free_rate })
    }

    /// Calculate option price using Black-Scholes formula
    ///
    /// # Arguments
    /// * `spot_price` - Current price of underlying asset
    /// * `strike_price` - Strike price of option
    /// * `time_to_expiration` - Time to expiration in years
    /// * `volatility` - Annualized volatility (standard deviation)
    /// * `option_type` - Call or Put
    pub fn price(
        &self,
        spot_price: Decimal,
        strike_price: Decimal,
        time_to_expiration: f64,
        volatility: f64,
        option_type: OptionType,
    ) -> Result<Decimal> {
        if spot_price <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Spot price must be positive".to_string(),
            ));
        }

        if strike_price <= Decimal::ZERO {
            return Err(CoreError::Validation(
                "Strike price must be positive".to_string(),
            ));
        }

        if time_to_expiration <= 0.0 {
            // At expiration, return intrinsic value
            return Ok(match option_type {
                OptionType::Call => (spot_price - strike_price).max(Decimal::ZERO),
                OptionType::Put => (strike_price - spot_price).max(Decimal::ZERO),
            });
        }

        if volatility <= 0.0 {
            return Err(CoreError::Validation(
                "Volatility must be positive".to_string(),
            ));
        }

        let s = spot_price
            .to_f64()
            .expect("spot_price is a validated positive Decimal");
        let k = strike_price
            .to_f64()
            .expect("strike_price is a validated positive Decimal");
        let t = time_to_expiration;
        let r = self.risk_free_rate;
        let sigma = volatility;

        let d1 = (s.ln() - k.ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();

        let price = match option_type {
            OptionType::Call => s * Self::norm_cdf(d1) - k * (-r * t).exp() * Self::norm_cdf(d2),
            OptionType::Put => k * (-r * t).exp() * Self::norm_cdf(-d2) - s * Self::norm_cdf(-d1),
        };

        Decimal::from_f64(price)
            .ok_or_else(|| CoreError::Validation("Failed to convert price".to_string()))
    }

    /// Calculate Greeks for an option
    pub fn calculate_greeks(
        &self,
        spot_price: Decimal,
        strike_price: Decimal,
        time_to_expiration: f64,
        volatility: f64,
        option_type: OptionType,
    ) -> Result<Greeks> {
        if time_to_expiration <= 0.0 {
            return Ok(Greeks {
                delta: 0.0,
                gamma: 0.0,
                theta: 0.0,
                vega: 0.0,
                rho: 0.0,
            });
        }

        let s = spot_price
            .to_f64()
            .expect("spot_price is a validated positive Decimal");
        let k = strike_price
            .to_f64()
            .expect("strike_price is a validated positive Decimal");
        let t = time_to_expiration;
        let r = self.risk_free_rate;
        let sigma = volatility;

        let d1 = (s.ln() - k.ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();

        let norm_d1 = Self::norm_cdf(d1);
        let norm_d2 = Self::norm_cdf(d2);
        let norm_pdf_d1 = Self::norm_pdf(d1);

        let delta = match option_type {
            OptionType::Call => norm_d1,
            OptionType::Put => norm_d1 - 1.0,
        };

        let gamma = norm_pdf_d1 / (s * sigma * t.sqrt());

        let theta = match option_type {
            OptionType::Call => {
                -s * norm_pdf_d1 * sigma / (2.0 * t.sqrt()) - r * k * (-r * t).exp() * norm_d2
            }
            OptionType::Put => {
                -s * norm_pdf_d1 * sigma / (2.0 * t.sqrt())
                    + r * k * (-r * t).exp() * Self::norm_cdf(-d2)
            }
        };

        let vega = s * norm_pdf_d1 * t.sqrt();

        let rho = match option_type {
            OptionType::Call => k * t * (-r * t).exp() * norm_d2,
            OptionType::Put => -k * t * (-r * t).exp() * Self::norm_cdf(-d2),
        };

        Ok(Greeks {
            delta,
            gamma,
            theta: theta / 365.0, // Convert to daily theta
            vega: vega / 100.0,   // Per 1% change in volatility
            rho: rho / 100.0,     // Per 1% change in interest rate
        })
    }

    /// Calculate implied volatility using Newton-Raphson method
    pub fn implied_volatility(
        &self,
        market_price: Decimal,
        spot_price: Decimal,
        strike_price: Decimal,
        time_to_expiration: f64,
        option_type: OptionType,
    ) -> Result<f64> {
        if time_to_expiration <= 0.0 {
            return Err(CoreError::Validation(
                "Cannot calculate IV for expired option".to_string(),
            ));
        }

        let target = market_price
            .to_f64()
            .expect("market_price is a validated Decimal");
        let mut vol = 0.3; // Initial guess: 30% volatility
        let max_iterations = 100;
        let tolerance = 1e-6;

        for _ in 0..max_iterations {
            let price = self.price(
                spot_price,
                strike_price,
                time_to_expiration,
                vol,
                option_type,
            )?;
            let price_f64 = price
                .to_f64()
                .expect("price calculated from validated Decimal inputs");

            if (price_f64 - target).abs() < tolerance {
                return Ok(vol);
            }

            // Calculate vega for Newton-Raphson
            let greeks = self.calculate_greeks(
                spot_price,
                strike_price,
                time_to_expiration,
                vol,
                option_type,
            )?;

            if greeks.vega.abs() < 1e-10 {
                return Err(CoreError::Validation(
                    "Vega too small, cannot converge".to_string(),
                ));
            }

            // Newton-Raphson step: vol_new = vol_old - f(vol) / f'(vol)
            // where f(vol) = price(vol) - market_price
            // and f'(vol) = vega
            vol -= (price_f64 - target) / (greeks.vega * 100.0); // Vega is scaled

            if vol <= 0.0 {
                vol = 0.01; // Prevent negative volatility
            }
        }

        Err(CoreError::Validation(
            "Failed to converge on implied volatility".to_string(),
        ))
    }

    /// Standard normal cumulative distribution function
    fn norm_cdf(x: f64) -> f64 {
        0.5 * (1.0 + Self::erf(x / std::f64::consts::SQRT_2))
    }

    /// Standard normal probability density function
    fn norm_pdf(x: f64) -> f64 {
        (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
    }

    /// Error function approximation
    fn erf(x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }
}

/// Options trading manager
pub struct OptionsManager {
    /// All option contracts by ID
    contracts: HashMap<Uuid, OptionContract>,
    /// Black-Scholes pricing model
    pricing_model: BlackScholesModel,
}

impl OptionsManager {
    /// Create a new options manager with the given annualized risk-free rate
    pub fn new(risk_free_rate: f64) -> Result<Self> {
        Ok(Self {
            contracts: HashMap::new(),
            pricing_model: BlackScholesModel::new(risk_free_rate)?,
        })
    }

    /// Create and list a new option contract
    #[allow(clippy::too_many_arguments)]
    pub fn create_option(
        &mut self,
        token_id: Uuid,
        option_type: OptionType,
        strike_price: Decimal,
        expiration_date: DateTime<Utc>,
        premium: Decimal,
        writer_id: Uuid,
        contract_size: Decimal,
    ) -> Result<Uuid> {
        let contract = OptionContract::new(
            token_id,
            option_type,
            strike_price,
            expiration_date,
            premium,
            writer_id,
            contract_size,
        )?;

        let contract_id = contract.contract_id;
        self.contracts.insert(contract_id, contract);
        Ok(contract_id)
    }

    /// Purchase an option contract
    pub fn buy_option(&mut self, contract_id: Uuid, buyer_id: Uuid) -> Result<Decimal> {
        let contract = self
            .contracts
            .get_mut(&contract_id)
            .ok_or_else(|| CoreError::NotFound("Option contract not found".to_string()))?;

        if contract.status != OptionStatus::Active {
            return Err(CoreError::InvalidState(
                "Option contract is not active".to_string(),
            ));
        }

        if contract.holder_id.is_some() {
            return Err(CoreError::InvalidState(
                "Option contract already sold".to_string(),
            ));
        }

        if contract.is_expired() {
            contract.status = OptionStatus::Expired;
            return Err(CoreError::OrderExpired);
        }

        contract.holder_id = Some(buyer_id);
        Ok(contract.premium)
    }

    /// Exercise an option contract (European style - only at expiration)
    pub fn exercise_option(
        &mut self,
        contract_id: Uuid,
        user_id: Uuid,
        settlement_price: Decimal,
    ) -> Result<Decimal> {
        let contract = self
            .contracts
            .get_mut(&contract_id)
            .ok_or_else(|| CoreError::NotFound("Option contract not found".to_string()))?;

        if contract.status != OptionStatus::Active {
            return Err(CoreError::InvalidState(
                "Option contract is not active".to_string(),
            ));
        }

        // Verify holder
        if contract.holder_id != Some(user_id) {
            return Err(CoreError::Unauthorized);
        }

        // European style: can only exercise at expiration
        if !contract.is_expired() {
            return Err(CoreError::InvalidState(
                "European option can only be exercised at expiration".to_string(),
            ));
        }

        // Check if in the money
        if !contract.is_in_the_money(settlement_price) {
            contract.status = OptionStatus::Expired;
            return Err(CoreError::InvalidState(
                "Option is not in the money".to_string(),
            ));
        }

        let payoff = contract.payoff(settlement_price);
        contract.status = OptionStatus::Exercised;
        contract.exercised_at = Some(Utc::now());

        Ok(payoff)
    }

    /// Calculate option price
    pub fn calculate_price(
        &self,
        contract_id: Uuid,
        spot_price: Decimal,
        volatility: f64,
    ) -> Result<Decimal> {
        let contract = self
            .contracts
            .get(&contract_id)
            .ok_or_else(|| CoreError::NotFound("Option contract not found".to_string()))?;

        let time_to_expiration = contract.time_to_expiration()?;

        self.pricing_model.price(
            spot_price,
            contract.strike_price,
            time_to_expiration,
            volatility,
            contract.option_type,
        )
    }

    /// Calculate Greeks for an option
    pub fn calculate_greeks(
        &self,
        contract_id: Uuid,
        spot_price: Decimal,
        volatility: f64,
    ) -> Result<Greeks> {
        let contract = self
            .contracts
            .get(&contract_id)
            .ok_or_else(|| CoreError::NotFound("Option contract not found".to_string()))?;

        let time_to_expiration = contract.time_to_expiration()?;

        self.pricing_model.calculate_greeks(
            spot_price,
            contract.strike_price,
            time_to_expiration,
            volatility,
            contract.option_type,
        )
    }

    /// Calculate implied volatility from market price
    pub fn calculate_implied_volatility(
        &self,
        contract_id: Uuid,
        market_price: Decimal,
        spot_price: Decimal,
    ) -> Result<f64> {
        let contract = self
            .contracts
            .get(&contract_id)
            .ok_or_else(|| CoreError::NotFound("Option contract not found".to_string()))?;

        let time_to_expiration = contract.time_to_expiration()?;

        self.pricing_model.implied_volatility(
            market_price,
            spot_price,
            contract.strike_price,
            time_to_expiration,
            contract.option_type,
        )
    }

    /// Get option contract
    pub fn get_contract(&self, contract_id: Uuid) -> Option<&OptionContract> {
        self.contracts.get(&contract_id)
    }

    /// Get all active contracts for a token
    pub fn get_active_contracts(&self, token_id: Uuid) -> Vec<&OptionContract> {
        self.contracts
            .values()
            .filter(|c| c.token_id == token_id && c.status == OptionStatus::Active)
            .collect()
    }

    /// Mark expired contracts
    pub fn mark_expired_contracts(&mut self) -> usize {
        let mut count = 0;
        for contract in self.contracts.values_mut() {
            if contract.status == OptionStatus::Active && contract.is_expired() {
                contract.status = OptionStatus::Expired;
                count += 1;
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_option_contract_creation() {
        let token_id = Uuid::new_v4();
        let writer_id = Uuid::new_v4();
        let expiration = Utc::now() + chrono::Duration::days(30);

        let contract = OptionContract::new(
            token_id,
            OptionType::Call,
            dec!(100),
            expiration,
            dec!(5),
            writer_id,
            dec!(1),
        );

        assert!(contract.is_ok());
        let contract = contract.unwrap();
        assert_eq!(contract.option_type, OptionType::Call);
        assert_eq!(contract.strike_price, dec!(100));
        assert_eq!(contract.premium, dec!(5));
    }

    #[test]
    fn test_option_intrinsic_value() {
        let token_id = Uuid::new_v4();
        let writer_id = Uuid::new_v4();
        let expiration = Utc::now() + chrono::Duration::days(30);

        let call = OptionContract::new(
            token_id,
            OptionType::Call,
            dec!(100),
            expiration,
            dec!(5),
            writer_id,
            dec!(1),
        )
        .unwrap();

        assert_eq!(call.intrinsic_value(dec!(110)), dec!(10));
        assert_eq!(call.intrinsic_value(dec!(90)), dec!(0));

        let put = OptionContract::new(
            token_id,
            OptionType::Put,
            dec!(100),
            expiration,
            dec!(5),
            writer_id,
            dec!(1),
        )
        .unwrap();

        assert_eq!(put.intrinsic_value(dec!(90)), dec!(10));
        assert_eq!(put.intrinsic_value(dec!(110)), dec!(0));
    }

    #[test]
    fn test_black_scholes_call_pricing() {
        let model = BlackScholesModel::new(0.05).unwrap();

        let price = model
            .price(dec!(100), dec!(100), 1.0, 0.2, OptionType::Call)
            .unwrap();

        // Call option at-the-money should have positive value
        assert!(price > dec!(0));
        assert!(price < dec!(20)); // Reasonable range
    }

    #[test]
    fn test_black_scholes_put_pricing() {
        let model = BlackScholesModel::new(0.05).unwrap();

        let price = model
            .price(dec!(100), dec!(100), 1.0, 0.2, OptionType::Put)
            .unwrap();

        // Put option at-the-money should have positive value
        assert!(price > dec!(0));
        assert!(price < dec!(20)); // Reasonable range
    }

    #[test]
    fn test_greeks_calculation() {
        let model = BlackScholesModel::new(0.05).unwrap();

        let greeks = model
            .calculate_greeks(dec!(100), dec!(100), 1.0, 0.2, OptionType::Call)
            .unwrap();

        // Delta for ATM call should be around 0.5 (range from 0.3 to 0.7 is reasonable)
        assert!(greeks.delta > 0.3 && greeks.delta < 0.7);
        // Gamma should be positive
        assert!(greeks.gamma > 0.0);
        // Vega should be positive
        assert!(greeks.vega > 0.0);
    }

    #[test]
    fn test_options_manager() {
        let mut manager = OptionsManager::new(0.05).unwrap();
        let token_id = Uuid::new_v4();
        let writer_id = Uuid::new_v4();
        let expiration = Utc::now() + chrono::Duration::days(30);

        let contract_id = manager
            .create_option(
                token_id,
                OptionType::Call,
                dec!(100),
                expiration,
                dec!(5),
                writer_id,
                dec!(1),
            )
            .unwrap();

        let contract = manager.get_contract(contract_id).unwrap();
        assert_eq!(contract.strike_price, dec!(100));

        // Buy option
        let buyer_id = Uuid::new_v4();
        let premium = manager.buy_option(contract_id, buyer_id).unwrap();
        assert_eq!(premium, dec!(5));
    }

    #[test]
    fn test_option_expiration() {
        let mut manager = OptionsManager::new(0.05).unwrap();
        let token_id = Uuid::new_v4();
        let writer_id = Uuid::new_v4();
        let expiration = Utc::now() - chrono::Duration::days(1); // Expired

        let result = manager.create_option(
            token_id,
            OptionType::Call,
            dec!(100),
            expiration,
            dec!(5),
            writer_id,
            dec!(1),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_put_call_parity() {
        // Put-Call Parity: C - P = S - K*e^(-r*T)
        let model = BlackScholesModel::new(0.05).unwrap();
        let spot = dec!(100);
        let strike = dec!(100);
        let time = 1.0;
        let vol = 0.2;

        let call = model
            .price(spot, strike, time, vol, OptionType::Call)
            .unwrap();
        let put = model
            .price(spot, strike, time, vol, OptionType::Put)
            .unwrap();

        let left = call - put;
        let right = spot - strike * Decimal::from_f64((-0.05 * time).exp()).unwrap();

        // Should be approximately equal
        let diff = (left - right).abs();
        assert!(diff < dec!(0.01));
    }
}
