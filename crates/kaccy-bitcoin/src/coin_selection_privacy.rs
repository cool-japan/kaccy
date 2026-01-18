//! Privacy-focused coin selection strategies
//!
//! Implements algorithms to resist address clustering, amount fingerprinting,
//! and other blockchain analysis techniques.

use crate::error::BitcoinError;
use crate::utxo::Utxo;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Privacy score for a coin selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PrivacyScore(pub u32);

impl PrivacyScore {
    /// Perfect privacy score (100)
    pub const PERFECT: Self = Self(100);
    /// Good privacy score (75-99)
    pub const GOOD: Self = Self(75);
    /// Fair privacy score (50-74)
    pub const FAIR: Self = Self(50);
    /// Poor privacy score (25-49)
    pub const POOR: Self = Self(25);
    /// Very poor privacy score (0-24)
    pub const VERY_POOR: Self = Self(0);

    /// Create a new privacy score
    pub fn new(score: u32) -> Self {
        Self(score.min(100))
    }

    /// Get the score value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Check if privacy is good
    pub fn is_good(&self) -> bool {
        self.0 >= 75
    }
}

/// Privacy concerns in coin selection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyConcerns {
    /// Address reuse detected
    pub address_reuse: bool,
    /// Round number amounts (fingerprinting risk)
    pub round_amounts: bool,
    /// Common input ownership revealed
    pub common_ownership: bool,
    /// Change output creates unnecessary linkage
    pub change_linkage: bool,
    /// Exact payment (no change) opportunity missed
    pub missed_exact_payment: bool,
}

/// Privacy-enhanced coin selection result
#[derive(Debug, Clone)]
pub struct PrivateSelection {
    /// Selected UTXOs
    pub utxos: Vec<Utxo>,
    /// Change amount in satoshis (if any)
    pub change_sats: Option<u64>,
    /// Privacy score
    pub privacy_score: PrivacyScore,
    /// Privacy concerns
    pub concerns: PrivacyConcerns,
    /// Total fee in satoshis
    pub fee_sats: u64,
}

/// Privacy-focused coin selector
#[derive(Debug)]
pub struct PrivacyCoinSelector {
    /// Minimum privacy score to accept
    min_privacy_score: PrivacyScore,
    /// Prefer exact payments (no change)
    prefer_exact: bool,
    /// Avoid address reuse
    #[allow(dead_code)]
    avoid_address_reuse: bool,
    /// Randomize selection to resist fingerprinting
    randomize: bool,
}

impl PrivacyCoinSelector {
    /// Create a new privacy coin selector
    pub fn new() -> Self {
        Self {
            min_privacy_score: PrivacyScore::FAIR,
            prefer_exact: true,
            avoid_address_reuse: true,
            randomize: true,
        }
    }

    /// Set minimum acceptable privacy score
    pub fn with_min_score(mut self, score: PrivacyScore) -> Self {
        self.min_privacy_score = score;
        self
    }

    /// Enable/disable exact payment preference
    pub fn with_exact_preference(mut self, prefer: bool) -> Self {
        self.prefer_exact = prefer;
        self
    }

    /// Select coins with privacy optimization
    pub fn select_coins(
        &self,
        available_utxos: &[Utxo],
        target_sats: u64,
        fee_rate: u64,
    ) -> Result<PrivateSelection, BitcoinError> {
        // First, try to find exact match (no change)
        if self.prefer_exact {
            if let Ok(exact) = self.find_exact_match(available_utxos, target_sats, fee_rate) {
                return Ok(exact);
            }
        }

        // Try anti-clustering selection
        let clustered = self.anti_clustering_selection(available_utxos, target_sats, fee_rate)?;

        // Try random selection
        let random = if self.randomize {
            self.randomized_selection(available_utxos, target_sats, fee_rate)
                .ok()
        } else {
            None
        };

        // Choose best privacy score
        let best = match random {
            Some(r) if r.privacy_score > clustered.privacy_score => r,
            _ => clustered,
        };

        // Check minimum privacy score
        if best.privacy_score < self.min_privacy_score {
            return Err(BitcoinError::InvalidAddress(format!(
                "Privacy score {} below minimum {}",
                best.privacy_score.0, self.min_privacy_score.0
            )));
        }

        Ok(best)
    }

    /// Find exact match (no change output)
    fn find_exact_match(
        &self,
        available_utxos: &[Utxo],
        target_sats: u64,
        fee_rate: u64,
    ) -> Result<PrivateSelection, BitcoinError> {
        // Try single UTXO exact match
        for utxo in available_utxos {
            let estimated_fee = self.estimate_fee_sats(std::slice::from_ref(utxo), false, fee_rate);
            let required = target_sats + estimated_fee;

            if utxo.amount_sats == required {
                return Ok(PrivateSelection {
                    utxos: vec![utxo.clone()],
                    change_sats: None,
                    privacy_score: PrivacyScore::PERFECT,
                    concerns: PrivacyConcerns::default(),
                    fee_sats: estimated_fee,
                });
            }
        }

        // Try combinations for exact match (2-UTXO only for simplicity)
        for i in 0..available_utxos.len() {
            for j in (i + 1)..available_utxos.len() {
                let utxos = vec![available_utxos[i].clone(), available_utxos[j].clone()];
                let estimated_fee = self.estimate_fee_sats(&utxos, false, fee_rate);
                let total: u64 = utxos.iter().map(|u| u.amount_sats).sum();
                let required = target_sats + estimated_fee;

                if total == required {
                    return Ok(PrivateSelection {
                        utxos,
                        change_sats: None,
                        privacy_score: PrivacyScore::new(95),
                        concerns: PrivacyConcerns::default(),
                        fee_sats: estimated_fee,
                    });
                }
            }
        }

        Err(BitcoinError::InvalidAddress(
            "No exact match found".to_string(),
        ))
    }

    /// Anti-clustering selection strategy
    fn anti_clustering_selection(
        &self,
        available_utxos: &[Utxo],
        target_sats: u64,
        fee_rate: u64,
    ) -> Result<PrivateSelection, BitcoinError> {
        // Group UTXOs by address to avoid clustering
        let mut address_groups: HashMap<String, Vec<Utxo>> = HashMap::new();

        for utxo in available_utxos {
            address_groups
                .entry(utxo.address.clone())
                .or_default()
                .push(utxo.clone());
        }

        // Select from different addresses when possible
        let mut selected = Vec::new();
        let mut used_addresses = HashSet::new();
        let mut total = 0u64;

        // Sort addresses by UTXO count (prefer addresses with fewer UTXOs)
        let mut sorted_groups: Vec<_> = address_groups.iter().collect();
        sorted_groups.sort_by_key(|(_, utxos)| utxos.len());

        for (addr, utxos) in sorted_groups {
            if used_addresses.contains(addr) {
                continue;
            }

            // Take largest UTXO from this address
            if let Some(utxo) = utxos.iter().max_by_key(|u| u.amount_sats) {
                selected.push(utxo.clone());
                total += utxo.amount_sats;
                used_addresses.insert(addr.clone());

                let estimated_fee = self.estimate_fee_sats(&selected, true, fee_rate);
                if total >= target_sats + estimated_fee {
                    break;
                }
            }
        }

        let final_fee = self.estimate_fee_sats(&selected, true, fee_rate);
        let total_amount: u64 = selected.iter().map(|u| u.amount_sats).sum();

        if total_amount < target_sats + final_fee {
            return Err(BitcoinError::InvalidAddress(
                "Insufficient funds".to_string(),
            ));
        }

        let change = total_amount - target_sats - final_fee;
        let privacy_score = self.calculate_privacy_score(&selected, change);
        let concerns =
            self.analyze_concerns(&selected.iter().map(|u| &u.address).collect::<Vec<_>>());

        Ok(PrivateSelection {
            utxos: selected,
            change_sats: if change > 0 { Some(change) } else { None },
            privacy_score,
            concerns,
            fee_sats: final_fee,
        })
    }

    /// Randomized selection for fingerprinting resistance
    fn randomized_selection(
        &self,
        available_utxos: &[Utxo],
        target_sats: u64,
        fee_rate: u64,
    ) -> Result<PrivateSelection, BitcoinError> {
        use rand::seq::SliceRandom;

        let mut rng = rand::rng();
        let mut shuffled = available_utxos.to_vec();
        shuffled.shuffle(&mut rng);

        let mut selected = Vec::new();
        let mut total = 0u64;

        for utxo in shuffled {
            total += utxo.amount_sats;
            selected.push(utxo);

            let estimated_fee = self.estimate_fee_sats(&selected, true, fee_rate);
            if total >= target_sats + estimated_fee {
                break;
            }
        }

        let final_fee = self.estimate_fee_sats(&selected, true, fee_rate);
        let total_amount: u64 = selected.iter().map(|u| u.amount_sats).sum();

        if total_amount < target_sats + final_fee {
            return Err(BitcoinError::InvalidAddress(
                "Insufficient funds".to_string(),
            ));
        }

        let change = total_amount - target_sats - final_fee;
        let privacy_score = self.calculate_privacy_score(&selected, change);
        let concerns =
            self.analyze_concerns(&selected.iter().map(|u| &u.address).collect::<Vec<_>>());

        Ok(PrivateSelection {
            utxos: selected,
            change_sats: if change > 0 { Some(change) } else { None },
            privacy_score,
            concerns,
            fee_sats: final_fee,
        })
    }

    /// Calculate privacy score
    fn calculate_privacy_score(&self, selected: &[Utxo], change_sats: u64) -> PrivacyScore {
        let mut score = 100u32;

        // Penalty for address reuse
        let addresses: HashSet<_> = selected.iter().map(|u| &u.address).collect();
        if addresses.len() < selected.len() {
            score = score.saturating_sub(20);
        }

        // Penalty for round amounts
        if self.has_round_amount(selected) {
            score = score.saturating_sub(10);
        }

        // Penalty for creating change
        if change_sats > 0 {
            score = score.saturating_sub(5);
        }

        // Bonus for using multiple inputs from different addresses
        if addresses.len() > 1 {
            score = score.saturating_add(10);
        }

        PrivacyScore::new(score)
    }

    /// Check for round amounts
    fn has_round_amount(&self, utxos: &[Utxo]) -> bool {
        // Check for suspiciously round amounts that could be fingerprinting risks
        for utxo in utxos {
            // Check for exact BTC amounts (1 BTC, 0.1 BTC, 0.01 BTC, etc.)
            if utxo.amount_sats % 100_000_000 == 0 {
                return true; // Exact BTC
            }
            if utxo.amount_sats % 10_000_000 == 0 {
                return true; // 0.1 BTC multiples
            }
            if utxo.amount_sats % 1_000_000 == 0 {
                return true; // 0.01 BTC multiples
            }
            // Check for round mBTC amounts
            if utxo.amount_sats % 100_000 == 0 && utxo.amount_sats >= 1_000_000 {
                return true; // Round mBTC (>= 0.01 BTC)
            }
        }
        false
    }

    /// Analyze privacy concerns
    fn analyze_concerns(&self, addresses: &[&String]) -> PrivacyConcerns {
        let mut concerns = PrivacyConcerns::default();

        // Check for address reuse
        let unique_addresses: HashSet<_> = addresses.iter().collect();
        if unique_addresses.len() < addresses.len() {
            concerns.address_reuse = true;
        }

        // Check if common input ownership will be revealed
        // When multiple inputs are combined, it reveals common ownership
        if addresses.len() > 1 {
            concerns.common_ownership = true;
        }

        concerns
    }

    /// Estimate transaction fee in satoshis
    fn estimate_fee_sats(&self, utxos: &[Utxo], has_change: bool, fee_rate: u64) -> u64 {
        // Simple estimation: 10 + 148*inputs + 34*outputs vbytes
        let input_vbytes = utxos.len() as u64 * 148;
        let output_vbytes = if has_change { 68 } else { 34 }; // 2 outputs or 1
        let overhead_vbytes = 10;

        let total_vbytes = overhead_vbytes + input_vbytes + output_vbytes;
        total_vbytes * fee_rate
    }
}

impl Default for PrivacyCoinSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[allow(dead_code)]
    fn create_test_utxo(sats: u64, addr_suffix: &str) -> Utxo {
        Utxo {
            txid: bitcoin::Txid::all_zeros(),
            vout: 0,
            amount_sats: sats,
            address: format!("tb1q{}xy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh", addr_suffix),
            confirmations: 6,
            spendable: true,
            safe: true,
        }
    }

    #[test]
    fn test_privacy_score_creation() {
        let score = PrivacyScore::new(75);
        assert_eq!(score.value(), 75);
        assert!(score.is_good());
    }

    #[test]
    fn test_privacy_score_capping() {
        let score = PrivacyScore::new(150);
        assert_eq!(score.value(), 100);
    }

    #[test]
    fn test_selector_creation() {
        let selector = PrivacyCoinSelector::new();
        assert!(selector.prefer_exact);
        assert!(selector.avoid_address_reuse);
    }

    #[test]
    fn test_selector_configuration() {
        let selector = PrivacyCoinSelector::new()
            .with_min_score(PrivacyScore::GOOD)
            .with_exact_preference(false);

        assert!(!selector.prefer_exact);
        assert_eq!(selector.min_privacy_score.value(), 75);
    }

    #[test]
    fn test_exact_match_single_utxo() {
        let selector = PrivacyCoinSelector::new();
        let target = 10000;
        let fee_rate = 10;

        // Estimated fee for 1 input, 1 output: (10 + 148 + 34) * 10 = 1920
        let exact_amount = 10000 + 1920;
        let utxos = vec![create_test_utxo(exact_amount, "1")];

        let result = selector.find_exact_match(&utxos, target, fee_rate);
        assert!(result.is_ok());

        let selection = result.unwrap();
        assert_eq!(selection.utxos.len(), 1);
        assert!(selection.change_sats.is_none());
        assert_eq!(selection.privacy_score, PrivacyScore::PERFECT);
    }

    #[test]
    fn test_privacy_concerns_default() {
        let concerns = PrivacyConcerns::default();
        assert!(!concerns.address_reuse);
        assert!(!concerns.round_amounts);
    }

    #[test]
    fn test_anti_clustering_selection() {
        let selector = PrivacyCoinSelector::new();
        let utxos = vec![
            create_test_utxo(50000, "1"),
            create_test_utxo(30000, "2"),
            create_test_utxo(20000, "3"),
        ];

        let result = selector.anti_clustering_selection(&utxos, 60000, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_randomized_selection() {
        let selector = PrivacyCoinSelector::new();
        let utxos = vec![
            create_test_utxo(50000, "1"),
            create_test_utxo(30000, "2"),
            create_test_utxo(20000, "3"),
        ];

        let result = selector.randomized_selection(&utxos, 60000, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_insufficient_funds() {
        let selector = PrivacyCoinSelector::new();
        let utxos = vec![create_test_utxo(1000, "1")];

        let result = selector.select_coins(&utxos, 100000, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_round_amount_detection() {
        let selector = PrivacyCoinSelector::new();

        // Test exact BTC amounts
        let utxos_exact_btc = vec![create_test_utxo(100_000_000, "1")]; // 1 BTC
        assert!(selector.has_round_amount(&utxos_exact_btc));

        // Test 0.1 BTC multiples
        let utxos_tenth_btc = vec![create_test_utxo(10_000_000, "1")]; // 0.1 BTC
        assert!(selector.has_round_amount(&utxos_tenth_btc));

        // Test 0.01 BTC multiples
        let utxos_hundredth_btc = vec![create_test_utxo(1_000_000, "1")]; // 0.01 BTC
        assert!(selector.has_round_amount(&utxos_hundredth_btc));

        // Test non-round amounts
        let utxos_non_round = vec![create_test_utxo(12_345_678, "1")];
        assert!(!selector.has_round_amount(&utxos_non_round));
    }

    #[test]
    fn test_privacy_concerns_analysis() {
        let selector = PrivacyCoinSelector::new();

        // Test address reuse detection
        let addr1 = "addr1".to_string();
        let addr1_dup = "addr1".to_string();
        let addr2 = "addr2".to_string();
        let addresses_with_reuse = vec![&addr1, &addr1_dup, &addr2];
        let concerns_reuse = selector.analyze_concerns(&addresses_with_reuse);
        assert!(concerns_reuse.address_reuse);
        assert!(concerns_reuse.common_ownership); // Multiple inputs

        // Test common ownership (no address reuse)
        let addr_a = "addr1".to_string();
        let addr_b = "addr2".to_string();
        let addr_c = "addr3".to_string();
        let addresses_no_reuse = vec![&addr_a, &addr_b, &addr_c];
        let concerns_ownership = selector.analyze_concerns(&addresses_no_reuse);
        assert!(!concerns_ownership.address_reuse);
        assert!(concerns_ownership.common_ownership); // Multiple inputs reveal common ownership

        // Test single address (no common ownership revelation)
        let single_addr = "addr1".to_string();
        let addresses_single = vec![&single_addr];
        let concerns_single = selector.analyze_concerns(&addresses_single);
        assert!(!concerns_single.address_reuse);
        assert!(!concerns_single.common_ownership); // Single input doesn't reveal linkage
    }

    #[test]
    fn test_round_amount_penalties() {
        let selector = PrivacyCoinSelector::new();

        // UTXOs with round amounts should get lower privacy scores
        let round_utxos = vec![create_test_utxo(100_000_000, "1")]; // 1 BTC exactly
        let round_score = selector.calculate_privacy_score(&round_utxos, 0);

        let non_round_utxos = vec![create_test_utxo(99_876_543, "2")]; // Non-round
        let non_round_score = selector.calculate_privacy_score(&non_round_utxos, 0);

        // Non-round should have better or equal privacy score
        assert!(non_round_score.value() >= round_score.value());
    }
}
