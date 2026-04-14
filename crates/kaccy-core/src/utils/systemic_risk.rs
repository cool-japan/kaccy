//! Systemic Risk Analysis
//!
//! This module provides tools for analyzing systemic risk, including contagion modeling,
//! interconnectedness metrics, stress propagation, and system-wide risk indicators.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Entity in the financial network (e.g., institution, user, protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntity {
    /// Entity ID
    pub id: i64,
    /// Entity name
    pub name: String,
    /// Total assets
    pub total_assets: Decimal,
    /// Total liabilities
    pub total_liabilities: Decimal,
    /// Capital buffer
    pub capital: Decimal,
    /// Systemically important flag
    pub systemically_important: bool,
}

impl NetworkEntity {
    /// Create a new network entity
    pub fn new(
        id: i64,
        name: String,
        total_assets: Decimal,
        total_liabilities: Decimal,
        capital: Decimal,
    ) -> Self {
        Self {
            id,
            name,
            total_assets,
            total_liabilities,
            capital,
            systemically_important: false,
        }
    }

    /// Calculate leverage ratio
    pub fn leverage_ratio(&self) -> Decimal {
        if self.capital > dec!(0) {
            self.total_assets / self.capital
        } else {
            dec!(0)
        }
    }

    /// Check if entity is solvent
    pub fn is_solvent(&self) -> bool {
        self.total_assets >= self.total_liabilities
    }

    /// Calculate capital adequacy ratio
    pub fn capital_adequacy_ratio(&self) -> Decimal {
        if self.total_assets > dec!(0) {
            self.capital / self.total_assets
        } else {
            dec!(0)
        }
    }

    /// Check if entity would default given a loss
    pub fn would_default(&self, loss: Decimal) -> bool {
        self.capital < loss
    }
}

/// Exposure between two entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interconnection {
    /// Creditor entity ID
    pub from_id: i64,
    /// Debtor entity ID
    pub to_id: i64,
    /// Exposure amount
    pub exposure: Decimal,
    /// Exposure type
    pub exposure_type: ExposureType,
}

/// Category of financial exposure between counterparties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExposureType {
    /// Direct lending
    Lending,
    /// Derivative contracts
    Derivatives,
    /// Payment obligations
    Payment,
    /// Other exposure
    Other,
}

/// Contagion simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContagionResult {
    /// Initial defaulter IDs
    pub initial_defaulters: Vec<i64>,
    /// Total number of defaults
    pub total_defaults: usize,
    /// Entities that defaulted in cascade
    pub cascade_defaults: Vec<i64>,
    /// Total loss to system
    pub total_system_loss: Decimal,
    /// Contagion rounds
    pub contagion_rounds: usize,
}

/// Systemic risk analyzer
pub struct SystemicRiskAnalyzer {
    /// Network entities
    entities: HashMap<i64, NetworkEntity>,
    /// Interconnections (exposures)
    interconnections: Vec<Interconnection>,
}

impl SystemicRiskAnalyzer {
    /// Create a new systemic risk analyzer
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            interconnections: Vec::new(),
        }
    }

    /// Add an entity to the network
    pub fn add_entity(&mut self, entity: NetworkEntity) {
        self.entities.insert(entity.id, entity);
    }

    /// Add an interconnection (exposure)
    pub fn add_interconnection(&mut self, interconnection: Interconnection) {
        self.interconnections.push(interconnection);
    }

    /// Calculate network density (actual connections / possible connections)
    pub fn network_density(&self) -> Decimal {
        let n = self.entities.len() as i64;
        if n <= 1 {
            return dec!(0);
        }

        let possible_connections = n * (n - 1);
        let actual_connections = self.interconnections.len() as i64;

        Decimal::from(actual_connections) / Decimal::from(possible_connections)
    }

    /// Calculate interconnectedness score for an entity
    pub fn interconnectedness_score(&self, entity_id: i64) -> Decimal {
        let incoming: Decimal = self
            .interconnections
            .iter()
            .filter(|ic| ic.to_id == entity_id)
            .map(|ic| ic.exposure)
            .sum();

        let outgoing: Decimal = self
            .interconnections
            .iter()
            .filter(|ic| ic.from_id == entity_id)
            .map(|ic| ic.exposure)
            .sum();

        incoming + outgoing
    }

    /// Identify systemically important entities (SIFIs)
    pub fn identify_systemically_important(&mut self, threshold_percentile: Decimal) {
        let mut scores: Vec<(i64, Decimal)> = self
            .entities
            .keys()
            .map(|&id| (id, self.interconnectedness_score(id)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let threshold_index = ((scores.len() as f64)
            * (dec!(1) - threshold_percentile).to_f64().unwrap_or(0.1))
            as usize;

        for (i, &(id, _)) in scores.iter().enumerate() {
            if let Some(entity) = self.entities.get_mut(&id) {
                entity.systemically_important = i <= threshold_index;
            }
        }
    }

    /// Simulate contagion from initial defaulters
    pub fn simulate_contagion(
        &self,
        initial_defaulters: Vec<i64>,
        recovery_rate: Decimal,
    ) -> ContagionResult {
        let mut defaulted = HashSet::new();
        let mut to_check = initial_defaulters.clone();
        let mut total_loss = dec!(0);
        let mut round = 0;

        // Add initial defaulters
        for &id in &initial_defaulters {
            defaulted.insert(id);
            if let Some(entity) = self.entities.get(&id) {
                total_loss += entity.capital;
            }
        }

        // Propagate defaults
        while !to_check.is_empty() && round < 100 {
            round += 1;
            let mut next_round = Vec::new();

            for &defaulter_id in &to_check {
                // Find all creditors of the defaulter
                for ic in &self.interconnections {
                    if ic.to_id == defaulter_id && !defaulted.contains(&ic.from_id) {
                        // Calculate loss to creditor
                        let loss = ic.exposure * (dec!(1) - recovery_rate);

                        if let Some(creditor) = self.entities.get(&ic.from_id) {
                            if creditor.would_default(loss) && !defaulted.contains(&ic.from_id) {
                                defaulted.insert(ic.from_id);
                                next_round.push(ic.from_id);
                                total_loss += creditor.capital;
                            }
                        }
                    }
                }
            }

            to_check = next_round;
        }

        let cascade_defaults: Vec<i64> = defaulted
            .iter()
            .filter(|id| !initial_defaulters.contains(id))
            .copied()
            .collect();

        ContagionResult {
            initial_defaulters,
            total_defaults: defaulted.len(),
            cascade_defaults,
            total_system_loss: total_loss,
            contagion_rounds: round,
        }
    }

    /// Calculate system-wide capital adequacy
    pub fn system_capital_adequacy(&self) -> Decimal {
        let total_assets: Decimal = self.entities.values().map(|e| e.total_assets).sum();
        let total_capital: Decimal = self.entities.values().map(|e| e.capital).sum();

        if total_assets > dec!(0) {
            total_capital / total_assets
        } else {
            dec!(0)
        }
    }

    /// Calculate concentration risk (percentage of total exposure to top N entities)
    pub fn concentration_risk(&self, top_n: usize) -> Decimal {
        let mut scores: Vec<(i64, Decimal)> = self
            .entities
            .keys()
            .map(|&id| (id, self.interconnectedness_score(id)))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let total_interconnectedness: Decimal = scores.iter().map(|(_, score)| score).sum();

        if total_interconnectedness == dec!(0) {
            return dec!(0);
        }

        let top_exposure: Decimal = scores.iter().take(top_n).map(|(_, score)| score).sum();

        (top_exposure / total_interconnectedness) * dec!(100)
    }

    /// Calculate probability of cascade default
    pub fn cascade_probability(&self, initial_default_prob: Decimal) -> Decimal {
        let n_entities = self.entities.len() as i64;
        if n_entities == 0 {
            return dec!(0);
        }

        // Simplified cascade probability using network density
        let density = self.network_density();
        let avg_leverage: Decimal = self
            .entities
            .values()
            .map(|e| e.leverage_ratio())
            .sum::<Decimal>()
            / Decimal::from(n_entities);

        // P(cascade) ≈ P(initial) × density × (leverage / 10)
        initial_default_prob * density * (avg_leverage / dec!(10))
    }

    /// Stress test: apply shock to all entities and measure impact
    pub fn stress_test(&self, asset_shock_pct: Decimal) -> StressTestResult {
        let mut stressed_entities = 0;
        let mut defaults = Vec::new();
        let mut total_capital_loss = dec!(0);

        for entity in self.entities.values() {
            let asset_loss = entity.total_assets * asset_shock_pct;
            let new_capital = entity.capital - asset_loss;

            if new_capital < dec!(0) {
                defaults.push(entity.id);
                total_capital_loss += entity.capital;
            } else if new_capital < entity.capital * dec!(0.5) {
                stressed_entities += 1;
            }
        }

        StressTestResult {
            shock_percentage: asset_shock_pct,
            entities_defaulted: defaults.len(),
            defaulted_entity_ids: defaults,
            entities_stressed: stressed_entities,
            total_capital_loss,
            system_capital_before: self.entities.values().map(|e| e.capital).sum(),
            system_capital_after: self
                .entities
                .values()
                .map(|e| (e.capital - e.total_assets * asset_shock_pct).max(dec!(0)))
                .sum(),
        }
    }

    /// Calculate DebtRank metric for systemic importance
    pub fn debt_rank(&self, entity_id: i64) -> Decimal {
        // DebtRank measures the potential impact of an entity's distress on the system
        let total_system_assets: Decimal = self.entities.values().map(|e| e.total_assets).sum();

        if total_system_assets == dec!(0) {
            return dec!(0);
        }

        // Calculate entity's share of system assets
        let entity_assets = self
            .entities
            .get(&entity_id)
            .map(|e| e.total_assets)
            .unwrap_or(dec!(0));

        // Calculate total exposure of others to this entity
        let exposure_to_entity: Decimal = self
            .interconnections
            .iter()
            .filter(|ic| ic.to_id == entity_id)
            .map(|ic| ic.exposure)
            .sum();

        // DebtRank ≈ (entity assets + exposure to entity) / system assets
        ((entity_assets + exposure_to_entity) / total_system_assets) * dec!(100)
    }

    /// Get the most connected entities (network hubs)
    pub fn get_network_hubs(&self, n: usize) -> Vec<(i64, usize, Decimal)> {
        let mut connections: HashMap<i64, usize> = HashMap::new();

        for ic in &self.interconnections {
            *connections.entry(ic.from_id).or_insert(0) += 1;
            *connections.entry(ic.to_id).or_insert(0) += 1;
        }

        let mut hubs: Vec<(i64, usize, Decimal)> = connections
            .into_iter()
            .map(|(id, count)| (id, count, self.interconnectedness_score(id)))
            .collect();

        hubs.sort_by(|a, b| b.1.cmp(&a.1));
        hubs.into_iter().take(n).collect()
    }
}

impl Default for SystemicRiskAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Stress test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    /// Shock percentage applied
    pub shock_percentage: Decimal,
    /// Number of entities that defaulted
    pub entities_defaulted: usize,
    /// IDs of defaulted entities
    pub defaulted_entity_ids: Vec<i64>,
    /// Number of entities under stress (capital < 50% of original)
    pub entities_stressed: usize,
    /// Total capital lost
    pub total_capital_loss: Decimal,
    /// System capital before shock
    pub system_capital_before: Decimal,
    /// System capital after shock
    pub system_capital_after: Decimal,
}

impl StressTestResult {
    /// Calculate capital preservation ratio
    pub fn capital_preservation_ratio(&self) -> Decimal {
        if self.system_capital_before > dec!(0) {
            self.system_capital_after / self.system_capital_before
        } else {
            dec!(0)
        }
    }

    /// Calculate system resilience score (0-100)
    pub fn resilience_score(&self) -> Decimal {
        let preservation = self.capital_preservation_ratio();
        let default_penalty = Decimal::from(self.entities_defaulted as i64) * dec!(5);

        (preservation * dec!(100) - default_penalty).max(dec!(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_entity_creation() {
        let entity = NetworkEntity::new(
            1,
            "Bank A".to_string(),
            dec!(1000000),
            dec!(900000),
            dec!(100000),
        );

        assert_eq!(entity.id, 1);
        assert_eq!(entity.total_assets, dec!(1000000));
        assert_eq!(entity.total_liabilities, dec!(900000));
        assert_eq!(entity.capital, dec!(100000));
        assert!(!entity.systemically_important);
    }

    #[test]
    fn test_leverage_ratio() {
        let entity = NetworkEntity::new(
            1,
            "Bank A".to_string(),
            dec!(1000000),
            dec!(900000),
            dec!(100000),
        );

        // Leverage = Assets / Capital = 1000000 / 100000 = 10
        assert_eq!(entity.leverage_ratio(), dec!(10));
    }

    #[test]
    fn test_solvency() {
        let solvent = NetworkEntity::new(
            1,
            "Bank A".to_string(),
            dec!(1000000),
            dec!(900000),
            dec!(100000),
        );

        let insolvent = NetworkEntity::new(
            2,
            "Bank B".to_string(),
            dec!(800000),
            dec!(900000),
            dec!(-100000),
        );

        assert!(solvent.is_solvent());
        assert!(!insolvent.is_solvent());
    }

    #[test]
    fn test_would_default() {
        let entity = NetworkEntity::new(
            1,
            "Bank A".to_string(),
            dec!(1000000),
            dec!(900000),
            dec!(100000),
        );

        assert!(!entity.would_default(dec!(50000))); // Loss < Capital
        assert!(entity.would_default(dec!(150000))); // Loss > Capital
    }

    #[test]
    fn test_network_density() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        analyzer.add_entity(NetworkEntity::new(
            1,
            "A".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));
        analyzer.add_entity(NetworkEntity::new(
            2,
            "B".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));
        analyzer.add_entity(NetworkEntity::new(
            3,
            "C".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));

        // Add 2 connections
        analyzer.add_interconnection(Interconnection {
            from_id: 1,
            to_id: 2,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 2,
            to_id: 3,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });

        // Density = 2 / (3 * 2) = 2 / 6 = 0.333...
        let density = analyzer.network_density();
        assert!(density > dec!(0.3) && density < dec!(0.4));
    }

    #[test]
    fn test_interconnectedness_score() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        analyzer.add_entity(NetworkEntity::new(
            1,
            "A".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));
        analyzer.add_entity(NetworkEntity::new(
            2,
            "B".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));

        analyzer.add_interconnection(Interconnection {
            from_id: 1,
            to_id: 2,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 2,
            to_id: 1,
            exposure: dec!(5),
            exposure_type: ExposureType::Payment,
        });

        // Entity 1: incoming 5, outgoing 10 = 15
        // Entity 2: incoming 10, outgoing 5 = 15
        assert_eq!(analyzer.interconnectedness_score(1), dec!(15));
        assert_eq!(analyzer.interconnectedness_score(2), dec!(15));
    }

    #[test]
    fn test_contagion_simulation() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        // Create a simple network: A -> B -> C
        analyzer.add_entity(NetworkEntity::new(
            1,
            "A".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));
        analyzer.add_entity(NetworkEntity::new(
            2,
            "B".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));
        analyzer.add_entity(NetworkEntity::new(
            3,
            "C".to_string(),
            dec!(100),
            dec!(80),
            dec!(20),
        ));

        // A lends 25 to B, B lends 25 to C
        analyzer.add_interconnection(Interconnection {
            from_id: 1,
            to_id: 2,
            exposure: dec!(25),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 2,
            to_id: 3,
            exposure: dec!(25),
            exposure_type: ExposureType::Lending,
        });

        // Simulate B defaulting (recovery rate 40%)
        let result = analyzer.simulate_contagion(vec![2], dec!(0.4));

        assert_eq!(result.total_defaults, 1); // Only B defaults (A and C have enough capital)
        assert!(result.total_system_loss > dec!(0));
    }

    #[test]
    fn test_stress_test() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        analyzer.add_entity(NetworkEntity::new(
            1,
            "A".to_string(),
            dec!(1000),
            dec!(900),
            dec!(100),
        ));
        analyzer.add_entity(NetworkEntity::new(
            2,
            "B".to_string(),
            dec!(1000),
            dec!(900),
            dec!(100),
        ));

        // Apply 5% asset shock
        let result = analyzer.stress_test(dec!(0.05));

        assert_eq!(result.shock_percentage, dec!(0.05));
        assert_eq!(result.entities_defaulted, 0); // 5% shock on 1000 = 50 loss, capital = 100
        // Each entity loses 50 from 100 capital, so preservation = 50% (100 / 200)
        assert!(result.capital_preservation_ratio() > dec!(0.4));
        assert!(result.capital_preservation_ratio() < dec!(0.6));
    }

    #[test]
    fn test_debt_rank() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        analyzer.add_entity(NetworkEntity::new(
            1,
            "A".to_string(),
            dec!(1000),
            dec!(800),
            dec!(200),
        ));
        analyzer.add_entity(NetworkEntity::new(
            2,
            "B".to_string(),
            dec!(500),
            dec!(400),
            dec!(100),
        ));

        analyzer.add_interconnection(Interconnection {
            from_id: 2,
            to_id: 1,
            exposure: dec!(100),
            exposure_type: ExposureType::Lending,
        });

        // A has 1000 assets + 100 exposure from B = 1100
        // Total system = 1500
        // DebtRank ≈ 1100 / 1500 * 100 ≈ 73.3%
        let rank = analyzer.debt_rank(1);
        assert!(rank > dec!(70) && rank < dec!(75));
    }

    #[test]
    fn test_concentration_risk() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        for i in 1..=10 {
            analyzer.add_entity(NetworkEntity::new(
                i,
                format!("Entity {}", i),
                dec!(100),
                dec!(80),
                dec!(20),
            ));

            if i < 10 {
                analyzer.add_interconnection(Interconnection {
                    from_id: i,
                    to_id: i + 1,
                    exposure: Decimal::from(i * 10),
                    exposure_type: ExposureType::Lending,
                });
            }
        }

        let concentration = analyzer.concentration_risk(3);
        // Top 3 should have significant portion
        assert!(concentration > dec!(30));
    }

    #[test]
    fn test_network_hubs() {
        let mut analyzer = SystemicRiskAnalyzer::new();

        for i in 1..=5 {
            analyzer.add_entity(NetworkEntity::new(
                i,
                format!("Entity {}", i),
                dec!(100),
                dec!(80),
                dec!(20),
            ));
        }

        // Entity 3 is a hub
        analyzer.add_interconnection(Interconnection {
            from_id: 1,
            to_id: 3,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 2,
            to_id: 3,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 3,
            to_id: 4,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });
        analyzer.add_interconnection(Interconnection {
            from_id: 3,
            to_id: 5,
            exposure: dec!(10),
            exposure_type: ExposureType::Lending,
        });

        let hubs = analyzer.get_network_hubs(1);
        assert_eq!(hubs.len(), 1);
        assert_eq!(hubs[0].0, 3); // Entity 3 should be top hub
        assert_eq!(hubs[0].1, 4); // 4 connections
    }
}
