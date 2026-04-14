//! Counterparty Risk Management
//!
//! This module provides tools for managing counterparty risk, including credit exposure
//! calculation, default probability estimation, exposure aggregation, and collateral management.

use crate::error::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Credit rating categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreditRating {
    /// AAA rating - highest credit quality
    AAA,
    /// AA rating - very high credit quality
    AA,
    /// A rating - high credit quality
    A,
    /// BBB rating - good credit quality
    BBB,
    /// BB rating - speculative
    BB,
    /// B rating - highly speculative
    B,
    /// CCC rating - substantial credit risk
    CCC,
    /// CC rating - very high credit risk
    CC,
    /// C rating - extremely high credit risk
    C,
    /// D rating - default
    D,
}

impl CreditRating {
    /// Get the default probability for this credit rating (annual)
    pub fn default_probability(&self) -> Decimal {
        match self {
            CreditRating::AAA => dec!(0.0001), // 0.01%
            CreditRating::AA => dec!(0.0005),  // 0.05%
            CreditRating::A => dec!(0.001),    // 0.1%
            CreditRating::BBB => dec!(0.005),  // 0.5%
            CreditRating::BB => dec!(0.02),    // 2%
            CreditRating::B => dec!(0.05),     // 5%
            CreditRating::CCC => dec!(0.15),   // 15%
            CreditRating::CC => dec!(0.30),    // 30%
            CreditRating::C => dec!(0.50),     // 50%
            CreditRating::D => dec!(1.00),     // 100%
        }
    }

    /// Get the recovery rate for this credit rating (percentage of exposure recovered in default)
    pub fn recovery_rate(&self) -> Decimal {
        match self {
            CreditRating::AAA => dec!(0.80),
            CreditRating::AA => dec!(0.75),
            CreditRating::A => dec!(0.70),
            CreditRating::BBB => dec!(0.60),
            CreditRating::BB => dec!(0.50),
            CreditRating::B => dec!(0.40),
            CreditRating::CCC => dec!(0.30),
            CreditRating::CC => dec!(0.20),
            CreditRating::C => dec!(0.10),
            CreditRating::D => dec!(0.05),
        }
    }
}

/// Counterparty information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counterparty {
    /// Counterparty ID
    pub id: i64,
    /// Counterparty name
    pub name: String,
    /// Credit rating
    pub credit_rating: CreditRating,
    /// Maximum allowed exposure
    pub credit_limit: Decimal,
}

/// Exposure to a counterparty
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exposure {
    /// Counterparty ID
    pub counterparty_id: i64,
    /// Current exposure amount
    pub current_exposure: Decimal,
    /// Potential future exposure (estimated)
    pub potential_future_exposure: Decimal,
    /// Collateral posted by counterparty
    pub collateral: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl Exposure {
    /// Calculate net exposure (current + potential - collateral)
    pub fn net_exposure(&self) -> Decimal {
        (self.current_exposure + self.potential_future_exposure - self.collateral).max(dec!(0))
    }

    /// Calculate exposure at default (EAD)
    pub fn exposure_at_default(&self) -> Decimal {
        self.net_exposure()
    }
}

/// Expected loss calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedLoss {
    /// Exposure at default (EAD)
    pub exposure_at_default: Decimal,
    /// Probability of default (PD)
    pub probability_of_default: Decimal,
    /// Loss given default (LGD)
    pub loss_given_default: Decimal,
    /// Expected loss (EL = EAD × PD × LGD)
    pub expected_loss: Decimal,
}

impl ExpectedLoss {
    /// Calculate expected loss
    pub fn calculate(
        exposure_at_default: Decimal,
        probability_of_default: Decimal,
        recovery_rate: Decimal,
    ) -> Self {
        let loss_given_default = dec!(1) - recovery_rate;
        let expected_loss = exposure_at_default * probability_of_default * loss_given_default;

        Self {
            exposure_at_default,
            probability_of_default,
            loss_given_default,
            expected_loss,
        }
    }
}

/// Collateral position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralPosition {
    /// Collateral ID
    pub id: i64,
    /// Counterparty ID
    pub counterparty_id: i64,
    /// Token ID (collateral asset)
    pub token_id: i64,
    /// Quantity of collateral
    pub quantity: Decimal,
    /// Current market value
    pub market_value: Decimal,
    /// Haircut percentage (discount applied to market value)
    pub haircut: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl CollateralPosition {
    /// Calculate collateral value after haircut
    pub fn adjusted_value(&self) -> Decimal {
        self.market_value * (dec!(1) - self.haircut)
    }

    /// Check if collateral needs revaluation (older than 1 hour)
    pub fn needs_revaluation(&self) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.timestamp);
        age.num_hours() >= 1
    }
}

/// Counterparty risk manager
pub struct CounterpartyRiskManager {
    /// Counterparty information
    counterparties: HashMap<i64, Counterparty>,
    /// Current exposures
    exposures: HashMap<i64, Exposure>,
    /// Collateral positions
    collateral: HashMap<i64, Vec<CollateralPosition>>,
}

impl CounterpartyRiskManager {
    /// Create a new counterparty risk manager
    pub fn new() -> Self {
        Self {
            counterparties: HashMap::new(),
            exposures: HashMap::new(),
            collateral: HashMap::new(),
        }
    }

    /// Add a counterparty
    pub fn add_counterparty(&mut self, counterparty: Counterparty) {
        self.counterparties.insert(counterparty.id, counterparty);
    }

    /// Update exposure for a counterparty
    pub fn update_exposure(&mut self, exposure: Exposure) {
        self.exposures.insert(exposure.counterparty_id, exposure);
    }

    /// Add collateral position
    pub fn add_collateral(&mut self, collateral: CollateralPosition) {
        self.collateral
            .entry(collateral.counterparty_id)
            .or_default()
            .push(collateral);
    }

    /// Get total collateral value for a counterparty
    pub fn get_total_collateral(&self, counterparty_id: i64) -> Decimal {
        self.collateral
            .get(&counterparty_id)
            .map(|positions| positions.iter().map(|p| p.adjusted_value()).sum())
            .unwrap_or(dec!(0))
    }

    /// Calculate expected loss for a counterparty
    pub fn calculate_expected_loss(&self, counterparty_id: i64) -> Option<ExpectedLoss> {
        let counterparty = self.counterparties.get(&counterparty_id)?;
        let exposure = self.exposures.get(&counterparty_id)?;

        let ead = exposure.exposure_at_default();
        let pd = counterparty.credit_rating.default_probability();
        let recovery_rate = counterparty.credit_rating.recovery_rate();

        Some(ExpectedLoss::calculate(ead, pd, recovery_rate))
    }

    /// Calculate total expected loss across all counterparties
    pub fn calculate_total_expected_loss(&self) -> Decimal {
        self.counterparties
            .keys()
            .filter_map(|&id| self.calculate_expected_loss(id))
            .map(|el| el.expected_loss)
            .sum()
    }

    /// Check if a counterparty exceeds credit limit
    pub fn check_credit_limit(&self, counterparty_id: i64) -> Result<bool> {
        let counterparty = self.counterparties.get(&counterparty_id).ok_or_else(|| {
            crate::error::CoreError::NotFound(format!("Counterparty {}", counterparty_id))
        })?;

        let exposure = self.exposures.get(&counterparty_id).ok_or_else(|| {
            crate::error::CoreError::NotFound(format!(
                "Exposure for counterparty {}",
                counterparty_id
            ))
        })?;

        Ok(exposure.net_exposure() > counterparty.credit_limit)
    }

    /// Get counterparties exceeding credit limits
    pub fn get_limit_breaches(&self) -> Vec<i64> {
        self.counterparties
            .keys()
            .filter(|&&id| self.check_credit_limit(id).unwrap_or(false))
            .copied()
            .collect()
    }

    /// Calculate aggregate exposure by credit rating
    pub fn aggregate_by_rating(&self) -> HashMap<CreditRating, Decimal> {
        let mut aggregated: HashMap<CreditRating, Decimal> = HashMap::new();

        for (id, counterparty) in &self.counterparties {
            if let Some(exposure) = self.exposures.get(id) {
                let net_exp = exposure.net_exposure();
                *aggregated
                    .entry(counterparty.credit_rating)
                    .or_insert(dec!(0)) += net_exp;
            }
        }

        aggregated
    }

    /// Calculate concentration risk (percentage of total exposure to top N counterparties)
    pub fn calculate_concentration(&self, top_n: usize) -> Decimal {
        let total_exposure: Decimal = self.exposures.values().map(|e| e.net_exposure()).sum();

        if total_exposure == dec!(0) {
            return dec!(0);
        }

        let mut exposures: Vec<Decimal> =
            self.exposures.values().map(|e| e.net_exposure()).collect();

        exposures.sort_by(|a, b| b.cmp(a));

        let top_exposure: Decimal = exposures.iter().take(top_n).sum();

        (top_exposure / total_exposure) * dec!(100)
    }

    /// Get collateral positions that need revaluation
    pub fn get_stale_collateral(&self) -> Vec<&CollateralPosition> {
        self.collateral
            .values()
            .flatten()
            .filter(|c| c.needs_revaluation())
            .collect()
    }
}

impl Default for CounterpartyRiskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Credit Value Adjustment (CVA) calculator
pub struct CVACalculator;

impl CVACalculator {
    /// Calculate CVA for a counterparty
    ///
    /// CVA = (1 - Recovery Rate) × Exposure × Probability of Default
    pub fn calculate(
        exposure: Decimal,
        probability_of_default: Decimal,
        recovery_rate: Decimal,
    ) -> Decimal {
        let loss_given_default = dec!(1) - recovery_rate;
        exposure * probability_of_default * loss_given_default
    }

    /// Calculate CVA for a time series of exposures (discounted)
    pub fn calculate_time_series(
        exposures: &[(Decimal, Decimal)], // (time in years, exposure)
        probability_of_default: Decimal,
        recovery_rate: Decimal,
        risk_free_rate: Decimal,
    ) -> Decimal {
        let loss_given_default = dec!(1) - recovery_rate;
        let mut cva = dec!(0);

        for &(time, exposure) in exposures {
            // Discount factor = e^(-r*t)
            let discount_factor = Self::exp_approx(-risk_free_rate * time);
            let survival_prob = dec!(1) - probability_of_default * time;

            cva += exposure
                * probability_of_default
                * loss_given_default
                * discount_factor
                * survival_prob;
        }

        cva
    }

    /// Approximation of e^x using Taylor series
    #[allow(dead_code)]
    fn exp_approx(x: Decimal) -> Decimal {
        // e^x ≈ 1 + x + x²/2! + x³/3! + x⁴/4! + x⁵/5!
        let x2 = x * x;
        let x3 = x2 * x;
        let x4 = x3 * x;
        let x5 = x4 * x;

        dec!(1) + x + x2 / dec!(2) + x3 / dec!(6) + x4 / dec!(24) + x5 / dec!(120)
    }
}

/// Credit Default Swap (CDS) pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CDSContract {
    /// Notional amount
    pub notional: Decimal,
    /// Credit rating of reference entity
    pub credit_rating: CreditRating,
    /// Contract maturity in years
    pub maturity_years: Decimal,
    /// Annual premium (spread) paid by protection buyer
    pub premium_rate: Decimal,
    /// Recovery rate assumption
    pub recovery_rate: Decimal,
}

impl CDSContract {
    /// Create a new CDS contract
    pub fn new(notional: Decimal, credit_rating: CreditRating, maturity_years: Decimal) -> Self {
        let recovery_rate = credit_rating.recovery_rate();
        // Default premium rate based on credit rating and maturity
        let premium_rate = Self::default_premium(credit_rating, maturity_years);

        Self {
            notional,
            credit_rating,
            maturity_years,
            premium_rate,
            recovery_rate,
        }
    }

    /// Calculate default premium rate based on credit rating and maturity
    fn default_premium(credit_rating: CreditRating, maturity_years: Decimal) -> Decimal {
        let pd = credit_rating.default_probability();
        let recovery = credit_rating.recovery_rate();
        let lgd = dec!(1) - recovery;

        // Premium ≈ PD × LGD / (1 - PD)
        // Adjusted for maturity (longer maturity = higher risk)
        let maturity_factor = dec!(1) + (maturity_years - dec!(1)) * dec!(0.1);
        (pd * lgd / (dec!(1) - pd)) * maturity_factor
    }

    /// Calculate present value of premium leg (buyer pays)
    pub fn premium_leg_pv(&self, risk_free_rate: Decimal) -> Decimal {
        let mut pv = dec!(0);
        let n_periods = (self.maturity_years * dec!(4)).to_i64().unwrap_or(4); // Quarterly payments

        for i in 1..=n_periods {
            let t = Decimal::from(i) / dec!(4); // Time in years
            let discount = Self::exp_approx(-risk_free_rate * t);
            let survival = dec!(1) - self.credit_rating.default_probability() * t;
            pv += self.premium_rate * self.notional * dec!(0.25) * discount * survival;
        }

        pv
    }

    /// Calculate present value of default leg (seller pays if default occurs)
    pub fn default_leg_pv(&self, risk_free_rate: Decimal) -> Decimal {
        let lgd = dec!(1) - self.recovery_rate;
        let pd = self.credit_rating.default_probability();

        // Approximate PV of default payment
        let expected_time_to_default = self.maturity_years / dec!(2);
        let discount = Self::exp_approx(-risk_free_rate * expected_time_to_default);

        self.notional * lgd * pd * self.maturity_years * discount
    }

    /// Calculate fair CDS spread (breakeven premium rate)
    pub fn fair_spread(&self, risk_free_rate: Decimal) -> Decimal {
        let default_leg = self.default_leg_pv(risk_free_rate);

        // Calculate denominator for fair spread
        let mut risky_annuity = dec!(0);
        let n_periods = (self.maturity_years * dec!(4)).to_i64().unwrap_or(4);

        for i in 1..=n_periods {
            let t = Decimal::from(i) / dec!(4);
            let discount = Self::exp_approx(-risk_free_rate * t);
            let survival = dec!(1) - self.credit_rating.default_probability() * t;
            risky_annuity += dec!(0.25) * discount * survival;
        }

        if risky_annuity > dec!(0) {
            default_leg / (self.notional * risky_annuity)
        } else {
            dec!(0)
        }
    }

    /// Approximation of e^x using Taylor series
    fn exp_approx(x: Decimal) -> Decimal {
        let x2 = x * x;
        let x3 = x2 * x;
        let x4 = x3 * x;
        let x5 = x4 * x;
        dec!(1) + x + x2 / dec!(2) + x3 / dec!(6) + x4 / dec!(24) + x5 / dec!(120)
    }
}

/// Netting agreement for bilateral exposure reduction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NettingAgreement {
    /// Agreement ID
    pub id: i64,
    /// Counterparty A
    pub counterparty_a: i64,
    /// Counterparty B
    pub counterparty_b: i64,
    /// Netting type
    pub netting_type: NettingType,
    /// Active status
    pub active: bool,
}

/// Type of netting agreement between counterparties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NettingType {
    /// Close-out netting (upon default)
    CloseOut,
    /// Payment netting (periodic settlement)
    Payment,
    /// Cross-product netting (across different instruments)
    CrossProduct,
}

impl NettingAgreement {
    /// Create a new netting agreement
    pub fn new(
        id: i64,
        counterparty_a: i64,
        counterparty_b: i64,
        netting_type: NettingType,
    ) -> Self {
        Self {
            id,
            counterparty_a,
            counterparty_b,
            netting_type,
            active: true,
        }
    }

    /// Calculate netted exposure between two counterparties
    pub fn calculate_netted_exposure(
        &self,
        exposures_a: &[Exposure],
        exposures_b: &[Exposure],
    ) -> Decimal {
        let total_a: Decimal = exposures_a.iter().map(|e| e.current_exposure).sum();
        let total_b: Decimal = exposures_b.iter().map(|e| e.current_exposure).sum();

        (total_a - total_b).abs()
    }

    /// Calculate netting benefit (gross exposure - net exposure)
    pub fn calculate_netting_benefit(
        &self,
        exposures_a: &[Exposure],
        exposures_b: &[Exposure],
    ) -> Decimal {
        let gross_a: Decimal = exposures_a.iter().map(|e| e.current_exposure).sum();
        let gross_b: Decimal = exposures_b.iter().map(|e| e.current_exposure).sum();
        let gross_exposure = gross_a + gross_b;

        let net_exposure = self.calculate_netted_exposure(exposures_a, exposures_b);

        gross_exposure - net_exposure
    }
}

/// Collateral optimization engine
pub struct CollateralOptimizer;

impl CollateralOptimizer {
    /// Calculate optimal collateral allocation to minimize costs
    pub fn optimize_allocation(
        required_collateral: Decimal,
        available_assets: &[(i64, Decimal, Decimal, Decimal)], // (token_id, quantity, value, haircut)
    ) -> Vec<(i64, Decimal)> {
        // Sort by effective cost (haircut rate) - prefer assets with lower haircuts
        let mut sorted_assets: Vec<_> = available_assets
            .iter()
            .map(|&(id, qty, val, haircut)| {
                let adjusted_val = val * (dec!(1) - haircut);
                (id, qty, adjusted_val, haircut)
            })
            .collect();

        sorted_assets.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());

        let mut allocation = Vec::new();
        let mut remaining = required_collateral;

        for (id, qty, adj_val, _) in sorted_assets {
            if remaining <= dec!(0) {
                break;
            }

            let value_per_unit = adj_val / qty;
            let units_needed = remaining / value_per_unit;
            let units_to_use = units_needed.min(qty);

            allocation.push((id, units_to_use));
            remaining -= units_to_use * value_per_unit;
        }

        allocation
    }

    /// Calculate collateral efficiency (value delivered / value posted)
    pub fn calculate_efficiency(
        collateral_posted: Decimal,
        collateral_value_after_haircut: Decimal,
    ) -> Decimal {
        if collateral_posted > dec!(0) {
            collateral_value_after_haircut / collateral_posted
        } else {
            dec!(0)
        }
    }

    /// Suggest collateral rebalancing to improve efficiency
    pub fn suggest_rebalancing(
        current_positions: &[CollateralPosition],
        target_efficiency: Decimal,
    ) -> Vec<(i64, Decimal)> {
        // Calculate current efficiency
        let total_value: Decimal = current_positions.iter().map(|p| p.market_value).sum();
        let adjusted_value: Decimal = current_positions.iter().map(|p| p.adjusted_value()).sum();
        let current_efficiency = Self::calculate_efficiency(total_value, adjusted_value);

        if current_efficiency >= target_efficiency {
            return Vec::new(); // Already efficient
        }

        // Suggest replacing high-haircut assets with low-haircut ones
        let mut suggestions = Vec::new();
        for pos in current_positions {
            if pos.haircut > dec!(0.15) {
                // Suggest reducing this position
                suggestions.push((pos.token_id, -pos.quantity / dec!(2)));
            }
        }

        suggestions
    }

    /// Calculate minimum collateral required given exposure and haircuts
    pub fn calculate_minimum_required(
        exposure: Decimal,
        available_assets: &[(Decimal, Decimal)], // (value, haircut)
    ) -> Decimal {
        if available_assets.is_empty() {
            return exposure;
        }

        // Find asset with lowest haircut
        let default_haircut = dec!(0);
        let min_haircut = available_assets
            .iter()
            .map(|(_, haircut)| haircut)
            .min()
            .unwrap_or(&default_haircut);

        // Required = Exposure / (1 - haircut)
        exposure / (dec!(1) - min_haircut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credit_rating_default_probability() {
        assert_eq!(CreditRating::AAA.default_probability(), dec!(0.0001));
        assert_eq!(CreditRating::BBB.default_probability(), dec!(0.005));
        assert_eq!(CreditRating::D.default_probability(), dec!(1.00));
    }

    #[test]
    fn test_credit_rating_recovery_rate() {
        assert_eq!(CreditRating::AAA.recovery_rate(), dec!(0.80));
        assert_eq!(CreditRating::BBB.recovery_rate(), dec!(0.60));
        assert_eq!(CreditRating::D.recovery_rate(), dec!(0.05));
    }

    #[test]
    fn test_exposure_net_exposure() {
        let exposure = Exposure {
            counterparty_id: 1,
            current_exposure: dec!(1000),
            potential_future_exposure: dec!(200),
            collateral: dec!(300),
            timestamp: Utc::now(),
        };

        assert_eq!(exposure.net_exposure(), dec!(900));
    }

    #[test]
    fn test_exposure_net_exposure_with_excess_collateral() {
        let exposure = Exposure {
            counterparty_id: 1,
            current_exposure: dec!(1000),
            potential_future_exposure: dec!(200),
            collateral: dec!(1500),
            timestamp: Utc::now(),
        };

        // Net exposure should not be negative
        assert_eq!(exposure.net_exposure(), dec!(0));
    }

    #[test]
    fn test_expected_loss_calculation() {
        let el = ExpectedLoss::calculate(
            dec!(10000), // EAD
            dec!(0.05),  // PD (5%)
            dec!(0.60),  // Recovery rate (60%)
        );

        assert_eq!(el.exposure_at_default, dec!(10000));
        assert_eq!(el.probability_of_default, dec!(0.05));
        assert_eq!(el.loss_given_default, dec!(0.40));
        assert_eq!(el.expected_loss, dec!(200)); // 10000 * 0.05 * 0.40
    }

    #[test]
    fn test_collateral_adjusted_value() {
        let collateral = CollateralPosition {
            id: 1,
            counterparty_id: 1,
            token_id: 1,
            quantity: dec!(100),
            market_value: dec!(10000),
            haircut: dec!(0.20), // 20% haircut
            timestamp: Utc::now(),
        };

        assert_eq!(collateral.adjusted_value(), dec!(8000)); // 10000 * (1 - 0.20)
    }

    #[test]
    fn test_counterparty_risk_manager() {
        let mut manager = CounterpartyRiskManager::new();

        let counterparty = Counterparty {
            id: 1,
            name: "Test Counterparty".to_string(),
            credit_rating: CreditRating::BBB,
            credit_limit: dec!(10000),
        };

        manager.add_counterparty(counterparty);

        let exposure = Exposure {
            counterparty_id: 1,
            current_exposure: dec!(5000),
            potential_future_exposure: dec!(1000),
            collateral: dec!(500),
            timestamp: Utc::now(),
        };

        manager.update_exposure(exposure);

        let el = manager.calculate_expected_loss(1).unwrap();
        // EAD = 5000 + 1000 - 500 = 5500
        // PD = 0.005 (BBB rating)
        // LGD = 1 - 0.60 = 0.40
        // EL = 5500 * 0.005 * 0.40 = 11
        assert_eq!(el.expected_loss, dec!(11));
    }

    #[test]
    fn test_credit_limit_check() {
        let mut manager = CounterpartyRiskManager::new();

        let counterparty = Counterparty {
            id: 1,
            name: "Test Counterparty".to_string(),
            credit_rating: CreditRating::BBB,
            credit_limit: dec!(10000),
        };

        manager.add_counterparty(counterparty);

        let exposure = Exposure {
            counterparty_id: 1,
            current_exposure: dec!(15000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        };

        manager.update_exposure(exposure);

        assert!(manager.check_credit_limit(1).unwrap());
    }

    #[test]
    fn test_aggregate_by_rating() {
        let mut manager = CounterpartyRiskManager::new();

        // Add two BBB counterparties
        manager.add_counterparty(Counterparty {
            id: 1,
            name: "CP1".to_string(),
            credit_rating: CreditRating::BBB,
            credit_limit: dec!(10000),
        });

        manager.add_counterparty(Counterparty {
            id: 2,
            name: "CP2".to_string(),
            credit_rating: CreditRating::BBB,
            credit_limit: dec!(10000),
        });

        // Add one AA counterparty
        manager.add_counterparty(Counterparty {
            id: 3,
            name: "CP3".to_string(),
            credit_rating: CreditRating::AA,
            credit_limit: dec!(20000),
        });

        manager.update_exposure(Exposure {
            counterparty_id: 1,
            current_exposure: dec!(5000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        });

        manager.update_exposure(Exposure {
            counterparty_id: 2,
            current_exposure: dec!(7000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        });

        manager.update_exposure(Exposure {
            counterparty_id: 3,
            current_exposure: dec!(10000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        });

        let aggregated = manager.aggregate_by_rating();
        assert_eq!(aggregated.get(&CreditRating::BBB), Some(&dec!(12000)));
        assert_eq!(aggregated.get(&CreditRating::AA), Some(&dec!(10000)));
    }

    #[test]
    fn test_concentration_risk() {
        let mut manager = CounterpartyRiskManager::new();

        for i in 1..=10 {
            manager.add_counterparty(Counterparty {
                id: i,
                name: format!("CP{}", i),
                credit_rating: CreditRating::BBB,
                credit_limit: dec!(10000),
            });

            manager.update_exposure(Exposure {
                counterparty_id: i,
                current_exposure: Decimal::from(i * 1000),
                potential_future_exposure: dec!(0),
                collateral: dec!(0),
                timestamp: Utc::now(),
            });
        }

        // Top 3 exposures: 10000, 9000, 8000 = 27000
        // Total exposure: 55000
        // Concentration: 27000 / 55000 * 100 ≈ 49.09%
        let concentration = manager.calculate_concentration(3);
        assert!(concentration > dec!(49) && concentration < dec!(50));
    }

    #[test]
    fn test_cva_calculation() {
        let cva = CVACalculator::calculate(
            dec!(10000), // exposure
            dec!(0.05),  // 5% default probability
            dec!(0.60),  // 60% recovery rate
        );

        // CVA = 10000 * 0.05 * 0.40 = 200
        assert_eq!(cva, dec!(200));
    }

    #[test]
    fn test_total_collateral() {
        let mut manager = CounterpartyRiskManager::new();

        manager.add_collateral(CollateralPosition {
            id: 1,
            counterparty_id: 1,
            token_id: 1,
            quantity: dec!(100),
            market_value: dec!(10000),
            haircut: dec!(0.20),
            timestamp: Utc::now(),
        });

        manager.add_collateral(CollateralPosition {
            id: 2,
            counterparty_id: 1,
            token_id: 2,
            quantity: dec!(50),
            market_value: dec!(5000),
            haircut: dec!(0.10),
            timestamp: Utc::now(),
        });

        let total = manager.get_total_collateral(1);
        // 10000 * 0.8 + 5000 * 0.9 = 8000 + 4500 = 12500
        assert_eq!(total, dec!(12500));
    }

    #[test]
    fn test_cds_contract_creation() {
        let cds = CDSContract::new(dec!(100000), CreditRating::BBB, dec!(5));

        assert_eq!(cds.notional, dec!(100000));
        assert_eq!(cds.credit_rating, CreditRating::BBB);
        assert_eq!(cds.maturity_years, dec!(5));
        assert_eq!(cds.recovery_rate, CreditRating::BBB.recovery_rate());
    }

    #[test]
    fn test_cds_fair_spread() {
        let cds = CDSContract::new(dec!(100000), CreditRating::BBB, dec!(5));
        let risk_free_rate = dec!(0.03); // 3%

        let fair_spread = cds.fair_spread(risk_free_rate);

        // Fair spread should be positive and reasonable
        assert!(fair_spread > dec!(0));
        assert!(fair_spread < dec!(0.1)); // Should be less than 10%
    }

    #[test]
    fn test_cds_premium_leg() {
        let cds = CDSContract::new(dec!(100000), CreditRating::A, dec!(3));
        let risk_free_rate = dec!(0.02);

        let premium_pv = cds.premium_leg_pv(risk_free_rate);

        // Premium PV should be positive
        assert!(premium_pv > dec!(0));
    }

    #[test]
    fn test_cds_default_leg() {
        let cds = CDSContract::new(dec!(100000), CreditRating::BB, dec!(5));
        let risk_free_rate = dec!(0.03);

        let default_pv = cds.default_leg_pv(risk_free_rate);

        // Default leg PV should be positive
        assert!(default_pv > dec!(0));
    }

    #[test]
    fn test_netting_agreement() {
        let agreement = NettingAgreement::new(1, 101, 102, NettingType::CloseOut);

        assert_eq!(agreement.id, 1);
        assert_eq!(agreement.counterparty_a, 101);
        assert_eq!(agreement.counterparty_b, 102);
        assert_eq!(agreement.netting_type, NettingType::CloseOut);
        assert!(agreement.active);
    }

    #[test]
    fn test_netting_exposure_calculation() {
        let agreement = NettingAgreement::new(1, 101, 102, NettingType::Payment);

        let exposures_a = vec![
            Exposure {
                counterparty_id: 102,
                current_exposure: dec!(5000),
                potential_future_exposure: dec!(0),
                collateral: dec!(0),
                timestamp: Utc::now(),
            },
            Exposure {
                counterparty_id: 102,
                current_exposure: dec!(3000),
                potential_future_exposure: dec!(0),
                collateral: dec!(0),
                timestamp: Utc::now(),
            },
        ];

        let exposures_b = vec![Exposure {
            counterparty_id: 101,
            current_exposure: dec!(6000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        }];

        // A owes 8000, B owes 6000, net = |8000 - 6000| = 2000
        let net_exposure = agreement.calculate_netted_exposure(&exposures_a, &exposures_b);
        assert_eq!(net_exposure, dec!(2000));
    }

    #[test]
    fn test_netting_benefit() {
        let agreement = NettingAgreement::new(1, 101, 102, NettingType::CrossProduct);

        let exposures_a = vec![Exposure {
            counterparty_id: 102,
            current_exposure: dec!(10000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        }];

        let exposures_b = vec![Exposure {
            counterparty_id: 101,
            current_exposure: dec!(7000),
            potential_future_exposure: dec!(0),
            collateral: dec!(0),
            timestamp: Utc::now(),
        }];

        // Gross = 10000 + 7000 = 17000
        // Net = |10000 - 7000| = 3000
        // Benefit = 17000 - 3000 = 14000
        let benefit = agreement.calculate_netting_benefit(&exposures_a, &exposures_b);
        assert_eq!(benefit, dec!(14000));
    }

    #[test]
    fn test_collateral_optimization() {
        // Available: Token 1 (100 units, 10000 value, 20% haircut)
        //            Token 2 (50 units, 5000 value, 10% haircut)
        //            Token 3 (200 units, 20000 value, 30% haircut)
        let available = vec![
            (1i64, dec!(100), dec!(10000), dec!(0.20)),
            (2i64, dec!(50), dec!(5000), dec!(0.10)),
            (3i64, dec!(200), dec!(20000), dec!(0.30)),
        ];

        let required = dec!(10000);
        let allocation = CollateralOptimizer::optimize_allocation(required, &available);

        // Should prefer Token 2 (lowest haircut)
        assert!(!allocation.is_empty());
        assert_eq!(allocation[0].0, 2); // Token 2 should be first
    }

    #[test]
    fn test_collateral_efficiency() {
        let posted = dec!(10000);
        let value_after_haircut = dec!(8000);

        let efficiency = CollateralOptimizer::calculate_efficiency(posted, value_after_haircut);
        assert_eq!(efficiency, dec!(0.8)); // 80% efficiency
    }

    #[test]
    fn test_collateral_minimum_required() {
        let exposure = dec!(10000);
        let assets = vec![(dec!(5000), dec!(0.20)), (dec!(8000), dec!(0.10))];

        let min_required = CollateralOptimizer::calculate_minimum_required(exposure, &assets);

        // With 10% haircut, need 10000 / 0.9 = 11111.11...
        assert!(min_required > dec!(11000));
        assert!(min_required < dec!(11200));
    }

    #[test]
    fn test_collateral_rebalancing_suggestion() {
        let positions = vec![
            CollateralPosition {
                id: 1,
                counterparty_id: 1,
                token_id: 1,
                quantity: dec!(100),
                market_value: dec!(10000),
                haircut: dec!(0.25), // High haircut
                timestamp: Utc::now(),
            },
            CollateralPosition {
                id: 2,
                counterparty_id: 1,
                token_id: 2,
                quantity: dec!(50),
                market_value: dec!(5000),
                haircut: dec!(0.05), // Low haircut
                timestamp: Utc::now(),
            },
        ];

        let suggestions = CollateralOptimizer::suggest_rebalancing(&positions, dec!(0.90));

        // Should suggest reducing high-haircut position
        assert!(!suggestions.is_empty());
    }
}
