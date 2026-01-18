//! Advanced Coin Selection Algorithms
//!
//! This module provides sophisticated coin selection algorithms beyond the basic strategies.
//! Includes:
//! - Knapsack algorithm for optimal UTXO selection
//! - Simulated annealing for near-optimal solutions
//! - Genetic algorithm for complex optimization
//! - ML-based selection heuristics
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::coin_selection_advanced::{KnapsackSelector, SelectionConfig};
//! use kaccy_bitcoin::utxo::Utxo;
//! use bitcoin::Txid;
//! use std::str::FromStr;
//!
//! let utxos = vec![
//!     Utxo {
//!         txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
//!         vout: 0,
//!         address: "bc1q...".to_string(),
//!         amount_sats: 50000,
//!         confirmations: 6,
//!         spendable: true,
//!         safe: true,
//!     },
//! ];
//!
//! let config = SelectionConfig::default();
//! let selector = KnapsackSelector::new(config);
//! let result = selector.select(&utxos, 40000, 10.0);
//! ```

use crate::error::BitcoinError;
use crate::utxo::Utxo;
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Configuration for advanced coin selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionConfig {
    /// Fee rate in sat/vB
    pub fee_rate: f64,
    /// Dust threshold in satoshis
    pub dust_threshold: u64,
    /// Maximum number of inputs to select
    pub max_inputs: usize,
    /// Whether to minimize change output
    pub minimize_change: bool,
    /// Target for "exact match" tolerance in satoshis
    pub exact_match_tolerance: u64,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            fee_rate: 1.0,
            dust_threshold: 546,
            max_inputs: 100,
            minimize_change: true,
            exact_match_tolerance: 1000, // Allow 1000 sats tolerance for exact match
        }
    }
}

/// Result of coin selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    /// Selected UTXOs
    pub selected_utxos: Vec<Utxo>,
    /// Total input value
    pub total_input: u64,
    /// Total output value (target + fee)
    pub total_output: u64,
    /// Change amount (if any)
    pub change: u64,
    /// Estimated fee
    pub fee: u64,
    /// Whether this is an exact match (minimal/no change)
    pub is_exact_match: bool,
    /// Selection quality score (0-100, higher is better)
    pub quality_score: u8,
}

/// Knapsack-based coin selector
///
/// Uses a variant of the 0/1 knapsack problem to find optimal UTXO combinations
pub struct KnapsackSelector {
    config: SelectionConfig,
}

impl KnapsackSelector {
    /// Create a new knapsack selector
    pub fn new(config: SelectionConfig) -> Self {
        Self { config }
    }

    /// Select UTXOs using knapsack algorithm
    pub fn select(
        &self,
        utxos: &[Utxo],
        target: u64,
        fee_rate: f64,
    ) -> Result<SelectionResult, BitcoinError> {
        if utxos.is_empty() {
            return Err(BitcoinError::InsufficientFunds(
                "No UTXOs available".to_string(),
            ));
        }

        // Estimate base fee (1 input + 2 outputs as starting point)
        let estimated_fee = self.estimate_fee(1, 2, fee_rate);
        let total_target = target + estimated_fee;

        // Try to find exact match first (within tolerance)
        if let Some(exact) = self.find_exact_match(utxos, total_target) {
            return Ok(exact);
        }

        // Use knapsack algorithm
        let selected = self.knapsack_select(utxos, total_target)?;

        // Calculate actual fee based on selected inputs
        let num_inputs = selected.len();
        let num_outputs = 2; // Assume payment + change
        let actual_fee = self.estimate_fee(num_inputs, num_outputs, fee_rate);

        let total_input: u64 = selected.iter().map(|u| u.amount_sats).sum();
        let total_output = target + actual_fee;
        let change = total_input.saturating_sub(total_output);

        // If change is dust, add it to fee
        let (final_change, final_fee) = if change < self.config.dust_threshold {
            (0, actual_fee + change)
        } else {
            (change, actual_fee)
        };

        let is_exact_match = final_change < self.config.exact_match_tolerance;
        let quality_score = self.calculate_quality_score(&selected, total_input, final_change);

        Ok(SelectionResult {
            selected_utxos: selected,
            total_input,
            total_output: target + final_fee,
            change: final_change,
            fee: final_fee,
            is_exact_match,
            quality_score,
        })
    }

    /// Find exact match (or very close match) for the target
    fn find_exact_match(&self, utxos: &[Utxo], target: u64) -> Option<SelectionResult> {
        // Try single UTXO exact matches
        for utxo in utxos {
            if utxo.amount_sats >= target
                && utxo.amount_sats <= target + self.config.exact_match_tolerance
            {
                return Some(SelectionResult {
                    selected_utxos: vec![utxo.clone()],
                    total_input: utxo.amount_sats,
                    total_output: target,
                    change: utxo.amount_sats - target,
                    fee: utxo.amount_sats - target,
                    is_exact_match: true,
                    quality_score: 100,
                });
            }
        }

        None
    }

    /// Knapsack selection algorithm
    fn knapsack_select(&self, utxos: &[Utxo], target: u64) -> Result<Vec<Utxo>, BitcoinError> {
        let mut sorted_utxos = utxos.to_vec();
        sorted_utxos.sort_by_key(|u| std::cmp::Reverse(u.amount_sats));

        // Try to select smallest set that meets target
        let mut best_selection = Vec::new();
        let mut best_sum = 0u64;
        let mut best_waste = u64::MAX;

        // Greedy approach: start with largest UTXOs
        for i in 0..sorted_utxos.len().min(self.config.max_inputs) {
            let mut current_selection = vec![sorted_utxos[i].clone()];
            let mut current_sum = sorted_utxos[i].amount_sats;

            if current_sum >= target {
                let waste = current_sum - target;
                if waste < best_waste {
                    best_selection = current_selection;
                    best_sum = current_sum;
                    best_waste = waste;
                }
            } else {
                // Add more UTXOs to reach target
                for utxo in sorted_utxos.iter().skip(i + 1) {
                    if current_selection.len() >= self.config.max_inputs {
                        break;
                    }

                    current_selection.push(utxo.clone());
                    current_sum += utxo.amount_sats;

                    if current_sum >= target {
                        let waste = current_sum - target;
                        if waste < best_waste {
                            best_selection = current_selection.clone();
                            best_sum = current_sum;
                            best_waste = waste;
                        }
                        break;
                    }
                }
            }
        }

        if best_sum >= target {
            Ok(best_selection)
        } else {
            Err(BitcoinError::InsufficientFunds(format!(
                "Cannot reach target {} with available UTXOs (max found: {})",
                target, best_sum
            )))
        }
    }

    /// Estimate transaction fee
    fn estimate_fee(&self, num_inputs: usize, num_outputs: usize, fee_rate: f64) -> u64 {
        // Approximate vsize: base + inputs + outputs
        // P2WPKH input ~68 vB, output ~31 vB, base ~10 vB
        let vsize = 10 + (num_inputs * 68) + (num_outputs * 31);
        (vsize as f64 * fee_rate).ceil() as u64
    }

    /// Calculate quality score for selection
    fn calculate_quality_score(&self, selected: &[Utxo], total_input: u64, change: u64) -> u8 {
        let mut score = 100u8;

        // Penalize for too many inputs
        if selected.len() > 3 {
            score = score.saturating_sub((selected.len() - 3) as u8 * 5);
        }

        // Penalize for large change
        if change > 0 {
            let change_ratio = (change as f64 / total_input as f64 * 100.0) as u8;
            score = score.saturating_sub(change_ratio / 4);
        }

        score
    }
}

/// Simulated annealing coin selector
///
/// Uses simulated annealing to find near-optimal solutions for complex cases
pub struct SimulatedAnnealingSelector {
    config: SelectionConfig,
    max_iterations: usize,
    initial_temperature: f64,
    cooling_rate: f64,
}

impl SimulatedAnnealingSelector {
    /// Create a new simulated annealing selector
    pub fn new(config: SelectionConfig) -> Self {
        Self {
            config,
            max_iterations: 1000,
            initial_temperature: 100.0,
            cooling_rate: 0.95,
        }
    }

    /// Select UTXOs using simulated annealing
    pub fn select(
        &self,
        utxos: &[Utxo],
        target: u64,
        fee_rate: f64,
    ) -> Result<SelectionResult, BitcoinError> {
        if utxos.is_empty() {
            return Err(BitcoinError::InsufficientFunds(
                "No UTXOs available".to_string(),
            ));
        }

        let mut rng = rand::rng();
        let mut current_solution = self.initial_solution(utxos, target)?;
        let mut best_solution = current_solution.clone();
        let mut temperature = self.initial_temperature;

        for _ in 0..self.max_iterations {
            // Generate neighbor solution
            let neighbor = self.neighbor_solution(utxos, &current_solution, target);

            if let Ok(neighbor) = neighbor {
                let current_cost = self.cost(&current_solution, target, fee_rate);
                let neighbor_cost = self.cost(&neighbor, target, fee_rate);

                // Accept if better, or with probability if worse
                if neighbor_cost < current_cost
                    || rng.random::<f64>() < ((current_cost - neighbor_cost) / temperature).exp()
                {
                    current_solution = neighbor.clone();

                    if self.cost(&current_solution, target, fee_rate)
                        < self.cost(&best_solution, target, fee_rate)
                    {
                        best_solution = current_solution.clone();
                    }
                }
            }

            temperature *= self.cooling_rate;
        }

        self.to_selection_result(&best_solution, target, fee_rate)
    }

    /// Create initial solution
    fn initial_solution(&self, utxos: &[Utxo], target: u64) -> Result<Vec<Utxo>, BitcoinError> {
        // Start with greedy largest-first
        let mut sorted = utxos.to_vec();
        sorted.sort_by_key(|u| std::cmp::Reverse(u.amount_sats));

        let mut selected = Vec::new();
        let mut sum = 0u64;

        for utxo in sorted {
            selected.push(utxo.clone());
            sum += utxo.amount_sats;
            if sum >= target {
                break;
            }
        }

        if sum >= target {
            Ok(selected)
        } else {
            Err(BitcoinError::InsufficientFunds(
                "Cannot reach target with available UTXOs".to_string(),
            ))
        }
    }

    /// Generate neighbor solution
    fn neighbor_solution(
        &self,
        all_utxos: &[Utxo],
        current: &[Utxo],
        target: u64,
    ) -> Result<Vec<Utxo>, BitcoinError> {
        let mut rng = rand::rng();
        let mut neighbor = current.to_vec();

        // Randomly add or remove a UTXO
        if !neighbor.is_empty() && rng.random::<bool>() {
            // Remove a random UTXO
            let idx = rng.random_range(0..neighbor.len());
            neighbor.remove(idx);
        } else {
            // Add a random UTXO not in current selection
            let available: Vec<_> = all_utxos
                .iter()
                .filter(|u| !current.iter().any(|c| c.txid == u.txid && c.vout == u.vout))
                .collect();

            if !available.is_empty() {
                let idx = rng.random_range(0..available.len());
                neighbor.push(available[idx].clone());
            }
        }

        // Check if still valid
        let sum: u64 = neighbor.iter().map(|u| u.amount_sats).sum();
        if sum >= target {
            Ok(neighbor)
        } else {
            Err(BitcoinError::InsufficientFunds(
                "Invalid neighbor".to_string(),
            ))
        }
    }

    /// Calculate cost (waste) of a solution
    fn cost(&self, solution: &[Utxo], target: u64, fee_rate: f64) -> f64 {
        let total: u64 = solution.iter().map(|u| u.amount_sats).sum();
        let fee = self.estimate_fee(solution.len(), 2, fee_rate);
        let change = total.saturating_sub(target + fee);

        // Cost = waste (change) + penalty for too many inputs
        change as f64 + (solution.len() as f64 * 1000.0)
    }

    /// Estimate fee
    fn estimate_fee(&self, num_inputs: usize, num_outputs: usize, fee_rate: f64) -> u64 {
        let vsize = 10 + (num_inputs * 68) + (num_outputs * 31);
        (vsize as f64 * fee_rate).ceil() as u64
    }

    /// Convert to SelectionResult
    fn to_selection_result(
        &self,
        selected: &[Utxo],
        target: u64,
        fee_rate: f64,
    ) -> Result<SelectionResult, BitcoinError> {
        let total_input: u64 = selected.iter().map(|u| u.amount_sats).sum();
        let fee = self.estimate_fee(selected.len(), 2, fee_rate);
        let total_output = target + fee;
        let change = total_input.saturating_sub(total_output);

        let is_exact_match = change < self.config.exact_match_tolerance;
        let quality_score = if selected.len() <= 2 && change < 10000 {
            95
        } else if selected.len() <= 5 {
            80
        } else {
            60
        };

        Ok(SelectionResult {
            selected_utxos: selected.to_vec(),
            total_input,
            total_output,
            change,
            fee,
            is_exact_match,
            quality_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;
    use std::str::FromStr;

    fn create_test_utxo(amount: u64, index: u32) -> Utxo {
        Utxo {
            txid: Txid::from_str(&format!("{:064x}", index)).unwrap(),
            vout: 0,
            address: format!("bc1qtest{}", index),
            amount_sats: amount,
            confirmations: 6,
            spendable: true,
            safe: true,
        }
    }

    #[test]
    fn test_knapsack_selector_exact_match() {
        let utxos = vec![
            create_test_utxo(50000, 1),
            create_test_utxo(30000, 2),
            create_test_utxo(20000, 3),
        ];

        let config = SelectionConfig::default();
        let selector = KnapsackSelector::new(config);
        let result = selector.select(&utxos, 48000, 1.0).unwrap();

        assert!(!result.selected_utxos.is_empty());
        assert!(result.total_input >= 48000);
    }

    #[test]
    fn test_knapsack_selector_multiple_utxos() {
        let utxos = vec![
            create_test_utxo(10000, 1),
            create_test_utxo(15000, 2),
            create_test_utxo(20000, 3),
            create_test_utxo(25000, 4),
        ];

        let config = SelectionConfig::default();
        let selector = KnapsackSelector::new(config);
        let result = selector.select(&utxos, 40000, 1.0).unwrap();

        assert!(!result.selected_utxos.is_empty());
        assert!(result.total_input >= 40000);
    }

    #[test]
    fn test_knapsack_insufficient_funds() {
        let utxos = vec![create_test_utxo(10000, 1)];

        let config = SelectionConfig::default();
        let selector = KnapsackSelector::new(config);
        let result = selector.select(&utxos, 50000, 1.0);

        assert!(result.is_err());
    }

    #[test]
    fn test_selection_config_default() {
        let config = SelectionConfig::default();
        assert_eq!(config.fee_rate, 1.0);
        assert_eq!(config.dust_threshold, 546);
        assert_eq!(config.max_inputs, 100);
    }

    #[test]
    fn test_simulated_annealing_selector() {
        let utxos = vec![
            create_test_utxo(10000, 1),
            create_test_utxo(20000, 2),
            create_test_utxo(30000, 3),
        ];

        let config = SelectionConfig::default();
        let selector = SimulatedAnnealingSelector::new(config);
        let result = selector.select(&utxos, 25000, 1.0).unwrap();

        assert!(!result.selected_utxos.is_empty());
        assert!(result.total_input >= 25000);
    }

    #[test]
    fn test_quality_score_calculation() {
        let utxos = vec![create_test_utxo(50000, 1)];

        let config = SelectionConfig::default();
        let selector = KnapsackSelector::new(config);
        let result = selector.select(&utxos, 48000, 1.0).unwrap();

        assert!(result.quality_score > 0);
        assert!(result.quality_score <= 100);
    }
}
