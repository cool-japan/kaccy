//! Coin control for manual UTXO selection and management

use bitcoin::Txid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, trace};

use crate::utxo::{SelectionStrategy, Utxo};

/// UTXO label and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoLabel {
    /// Transaction ID of the labeled UTXO
    pub txid: Txid,
    /// Output index within the transaction
    pub vout: u32,
    /// Human-readable label for this UTXO
    pub label: String,
    /// Arbitrary tags for categorization
    pub tags: Vec<String>,
    /// Optional free-form notes
    pub notes: Option<String>,
    /// When this label was created
    pub created_at: DateTime<Utc>,
    /// Privacy score (0-100, higher is better)
    pub privacy_score: Option<u8>,
}

impl UtxoLabel {
    /// Create a new UTXO label
    pub fn new(txid: Txid, vout: u32, label: String) -> Self {
        Self {
            txid,
            vout,
            label,
            tags: Vec::new(),
            notes: None,
            created_at: Utc::now(),
            privacy_score: None,
        }
    }

    /// Add a tag to the UTXO
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag from the UTXO
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }

    /// Set privacy score (0-100, higher is better)
    pub fn set_privacy_score(&mut self, score: u8) {
        self.privacy_score = Some(score.min(100));
    }
}

/// Coin selection preferences for privacy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPreferences {
    /// Avoid address reuse
    pub avoid_address_reuse: bool,
    /// Avoid mixing UTXOs from different sources
    pub avoid_mixing_sources: bool,
    /// Prefer UTXOs with higher privacy scores
    pub prefer_high_privacy_score: bool,
    /// Minimum privacy score to use (0-100)
    pub min_privacy_score: u8,
    /// Avoid UTXOs with specific tags
    pub avoid_tags: HashSet<String>,
}

impl Default for PrivacyPreferences {
    fn default() -> Self {
        Self {
            avoid_address_reuse: true,
            avoid_mixing_sources: true,
            prefer_high_privacy_score: true,
            min_privacy_score: 0,
            avoid_tags: HashSet::new(),
        }
    }
}

/// Coin control manager
pub struct CoinControl {
    /// UTXO labels and metadata
    labels: HashMap<(Txid, u32), UtxoLabel>,
    /// Frozen (locked) UTXOs that should not be spent
    frozen_utxos: HashSet<(Txid, u32)>,
    /// Privacy preferences
    privacy_prefs: PrivacyPreferences,
}

impl CoinControl {
    /// Create a new coin control manager
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
            frozen_utxos: HashSet::new(),
            privacy_prefs: PrivacyPreferences::default(),
        }
    }

    /// Set privacy preferences
    pub fn set_privacy_preferences(&mut self, prefs: PrivacyPreferences) {
        self.privacy_prefs = prefs;
    }

    /// Get privacy preferences
    pub fn privacy_preferences(&self) -> &PrivacyPreferences {
        &self.privacy_prefs
    }

    /// Label a UTXO
    pub fn label_utxo(&mut self, label: UtxoLabel) {
        let key = (label.txid, label.vout);
        self.labels.insert(key, label);
        debug!(txid = ?key.0, vout = key.1, "UTXO labeled");
    }

    /// Get label for a UTXO
    pub fn get_label(&self, txid: &Txid, vout: u32) -> Option<&UtxoLabel> {
        self.labels.get(&(*txid, vout))
    }

    /// Get all labels
    pub fn get_all_labels(&self) -> Vec<&UtxoLabel> {
        self.labels.values().collect()
    }

    /// Remove label from a UTXO
    pub fn remove_label(&mut self, txid: &Txid, vout: u32) {
        self.labels.remove(&(*txid, vout));
        debug!(txid = ?txid, vout = vout, "UTXO label removed");
    }

    /// Freeze a UTXO (prevent it from being spent)
    pub fn freeze_utxo(&mut self, txid: Txid, vout: u32) {
        self.frozen_utxos.insert((txid, vout));
        debug!(txid = ?txid, vout = vout, "UTXO frozen");
    }

    /// Unfreeze a UTXO
    pub fn unfreeze_utxo(&mut self, txid: &Txid, vout: u32) {
        self.frozen_utxos.remove(&(*txid, vout));
        debug!(txid = ?txid, vout = vout, "UTXO unfrozen");
    }

    /// Check if a UTXO is frozen
    pub fn is_frozen(&self, txid: &Txid, vout: u32) -> bool {
        self.frozen_utxos.contains(&(*txid, vout))
    }

    /// Get all frozen UTXOs
    pub fn get_frozen_utxos(&self) -> Vec<(Txid, u32)> {
        self.frozen_utxos.iter().copied().collect()
    }

    /// Filter UTXOs based on coin control preferences
    pub fn filter_utxos(&self, utxos: Vec<Utxo>) -> Vec<Utxo> {
        utxos
            .into_iter()
            .filter(|utxo| {
                // Skip frozen UTXOs
                if self.is_frozen(&utxo.txid, utxo.vout) {
                    trace!(txid = ?utxo.txid, vout = utxo.vout, "Skipping frozen UTXO");
                    return false;
                }

                // Check privacy preferences
                if let Some(label) = self.get_label(&utxo.txid, utxo.vout) {
                    // Check privacy score
                    if let Some(score) = label.privacy_score {
                        if score < self.privacy_prefs.min_privacy_score {
                            trace!(
                                txid = ?utxo.txid,
                                vout = utxo.vout,
                                score = score,
                                "Skipping UTXO with low privacy score"
                            );
                            return false;
                        }
                    }

                    // Check tags to avoid
                    for tag in &label.tags {
                        if self.privacy_prefs.avoid_tags.contains(tag) {
                            trace!(
                                txid = ?utxo.txid,
                                vout = utxo.vout,
                                tag = tag,
                                "Skipping UTXO with avoided tag"
                            );
                            return false;
                        }
                    }
                }

                true
            })
            .collect()
    }

    /// Manually select specific UTXOs for a transaction
    pub fn select_utxos(
        &self,
        available_utxos: Vec<Utxo>,
        selected_txids: &[(Txid, u32)],
    ) -> ManualSelection {
        let mut selected = Vec::new();
        let mut not_found = Vec::new();
        let mut frozen = Vec::new();

        for (txid, vout) in selected_txids {
            // Check if frozen
            if self.is_frozen(txid, *vout) {
                frozen.push((*txid, *vout));
                continue;
            }

            // Find the UTXO
            if let Some(utxo) = available_utxos
                .iter()
                .find(|u| u.txid == *txid && u.vout == *vout)
            {
                selected.push(utxo.clone());
            } else {
                not_found.push((*txid, *vout));
            }
        }

        let total_amount = selected.iter().map(|u| u.amount_sats).sum();

        ManualSelection {
            selected,
            total_amount,
            not_found,
            frozen,
        }
    }

    /// Select UTXOs with privacy considerations
    pub fn select_privacy_preserving(
        &self,
        mut utxos: Vec<Utxo>,
        target_amount: u64,
        strategy: SelectionStrategy,
    ) -> PrivacySelection {
        // Filter based on privacy preferences
        utxos = self.filter_utxos(utxos);

        // Group by address if avoiding address reuse
        let mut address_groups: HashMap<String, Vec<Utxo>> = HashMap::new();
        if self.privacy_prefs.avoid_address_reuse {
            for utxo in utxos {
                address_groups
                    .entry(utxo.address.clone())
                    .or_default()
                    .push(utxo);
            }
        }

        // Sort based on strategy and privacy preferences
        let mut sorted_utxos = if self.privacy_prefs.avoid_address_reuse {
            // Prefer addresses with single UTXO
            let mut single_utxo_addresses: Vec<Utxo> = address_groups
                .iter()
                .filter(|(_, utxos)| utxos.len() == 1)
                .flat_map(|(_, utxos)| utxos.clone())
                .collect();

            match strategy {
                SelectionStrategy::LargestFirst => {
                    single_utxo_addresses.sort_by(|a, b| b.amount_sats.cmp(&a.amount_sats))
                }
                SelectionStrategy::SmallestFirst => {
                    single_utxo_addresses.sort_by(|a, b| a.amount_sats.cmp(&b.amount_sats))
                }
                _ => {}
            }

            single_utxo_addresses
        } else {
            let mut all_utxos: Vec<Utxo> = address_groups.into_values().flatten().collect();

            match strategy {
                SelectionStrategy::LargestFirst => {
                    all_utxos.sort_by(|a, b| b.amount_sats.cmp(&a.amount_sats))
                }
                SelectionStrategy::SmallestFirst => {
                    all_utxos.sort_by(|a, b| a.amount_sats.cmp(&b.amount_sats))
                }
                _ => {}
            }

            all_utxos
        };

        // Sort by privacy score if preferred
        if self.privacy_prefs.prefer_high_privacy_score {
            sorted_utxos.sort_by(|a, b| {
                let score_a = self
                    .get_label(&a.txid, a.vout)
                    .and_then(|l| l.privacy_score)
                    .unwrap_or(50);
                let score_b = self
                    .get_label(&b.txid, b.vout)
                    .and_then(|l| l.privacy_score)
                    .unwrap_or(50);

                score_b.cmp(&score_a)
            });
        }

        // Select UTXOs to meet target amount
        let mut selected = Vec::new();
        let mut total_amount = 0u64;

        for utxo in sorted_utxos {
            selected.push(utxo.clone());
            total_amount += utxo.amount_sats;

            if total_amount >= target_amount {
                break;
            }
        }

        let addresses_used: HashSet<String> = selected.iter().map(|u| u.address.clone()).collect();
        let avg_privacy_score = self.calculate_avg_privacy_score(&selected);

        PrivacySelection {
            selected,
            total_amount,
            target_amount,
            addresses_used: addresses_used.len(),
            avg_privacy_score,
        }
    }

    /// Calculate average privacy score for selected UTXOs
    fn calculate_avg_privacy_score(&self, utxos: &[Utxo]) -> Option<u8> {
        let scores: Vec<u8> = utxos
            .iter()
            .filter_map(|u| self.get_label(&u.txid, u.vout))
            .filter_map(|l| l.privacy_score)
            .collect();

        if scores.is_empty() {
            None
        } else {
            Some((scores.iter().map(|&s| s as u32).sum::<u32>() / scores.len() as u32) as u8)
        }
    }

    /// Get UTXOs by tag
    pub fn get_utxos_by_tag(&self, tag: &str) -> Vec<&UtxoLabel> {
        self.labels
            .values()
            .filter(|label| label.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Get UTXOs by label substring
    pub fn search_labels(&self, query: &str) -> Vec<&UtxoLabel> {
        self.labels
            .values()
            .filter(|label| {
                label.label.to_lowercase().contains(&query.to_lowercase())
                    || label
                        .notes
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&query.to_lowercase()))
            })
            .collect()
    }

    /// Get dust threshold (in satoshis)
    pub fn dust_threshold(&self, fee_rate: f64) -> u64 {
        // Dust threshold formula: input_size * fee_rate * 3
        // Typical input size is ~148 bytes for P2WPKH
        (148.0 * fee_rate * 3.0) as u64
    }

    /// Check if a UTXO is dust
    pub fn is_dust(&self, amount_sats: u64, fee_rate: f64) -> bool {
        amount_sats < self.dust_threshold(fee_rate)
    }

    /// Filter out dust UTXOs
    pub fn filter_dust(&self, utxos: Vec<Utxo>, fee_rate: f64) -> (Vec<Utxo>, Vec<Utxo>) {
        let threshold = self.dust_threshold(fee_rate);
        let (dust, non_dust): (Vec<_>, Vec<_>) = utxos
            .into_iter()
            .partition(|utxo| utxo.amount_sats < threshold);

        (non_dust, dust)
    }
}

impl Default for CoinControl {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of manual UTXO selection
#[derive(Debug, Clone)]
pub struct ManualSelection {
    /// UTXOs that were successfully selected
    pub selected: Vec<Utxo>,
    /// Total amount of selected UTXOs in satoshis
    pub total_amount: u64,
    /// UTXOs requested but not found in the wallet
    pub not_found: Vec<(Txid, u32)>,
    /// UTXOs requested but currently frozen
    pub frozen: Vec<(Txid, u32)>,
}

impl ManualSelection {
    /// Check if selection was successful
    pub fn is_complete(&self) -> bool {
        self.not_found.is_empty() && self.frozen.is_empty()
    }

    /// Get total amount in BTC
    pub fn total_btc(&self) -> f64 {
        self.total_amount as f64 / 100_000_000.0
    }
}

/// Result of privacy-preserving selection
#[derive(Debug, Clone)]
pub struct PrivacySelection {
    /// UTXOs selected to maximize privacy
    pub selected: Vec<Utxo>,
    /// Total amount of selected UTXOs in satoshis
    pub total_amount: u64,
    /// Target amount requested in satoshis
    pub target_amount: u64,
    /// Number of distinct addresses contributing to the selection
    pub addresses_used: usize,
    /// Average privacy score of selected UTXOs (0-100)
    pub avg_privacy_score: Option<u8>,
}

impl PrivacySelection {
    /// Check if selection meets target amount
    pub fn is_sufficient(&self) -> bool {
        self.total_amount >= self.target_amount
    }

    /// Get amount in BTC
    pub fn total_btc(&self) -> f64 {
        self.total_amount as f64 / 100_000_000.0
    }

    /// Get privacy assessment
    pub fn privacy_assessment(&self) -> &str {
        match self.avg_privacy_score {
            Some(score) if score >= 80 => "Excellent",
            Some(score) if score >= 60 => "Good",
            Some(score) if score >= 40 => "Fair",
            Some(_) => "Poor",
            None => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_utxo_label_creation() {
        let txid = Txid::all_zeros();
        let label = UtxoLabel::new(txid, 0, "Test Label".to_string());

        assert_eq!(label.label, "Test Label");
        assert_eq!(label.txid, txid);
        assert_eq!(label.vout, 0);
        assert!(label.tags.is_empty());
    }

    #[test]
    fn test_utxo_label_tags() {
        let mut label = UtxoLabel::new(Txid::all_zeros(), 0, "Test".to_string());

        label.add_tag("exchange".to_string());
        label.add_tag("deposit".to_string());
        assert_eq!(label.tags.len(), 2);

        // Adding duplicate tag should not increase count
        label.add_tag("exchange".to_string());
        assert_eq!(label.tags.len(), 2);

        label.remove_tag("exchange");
        assert_eq!(label.tags.len(), 1);
    }

    #[test]
    fn test_coin_control_freeze() {
        let mut cc = CoinControl::new();
        let txid = Txid::all_zeros();

        assert!(!cc.is_frozen(&txid, 0));

        cc.freeze_utxo(txid, 0);
        assert!(cc.is_frozen(&txid, 0));

        cc.unfreeze_utxo(&txid, 0);
        assert!(!cc.is_frozen(&txid, 0));
    }

    #[test]
    fn test_coin_control_labeling() {
        let mut cc = CoinControl::new();
        let txid = Txid::all_zeros();
        let label = UtxoLabel::new(txid, 0, "My UTXO".to_string());

        cc.label_utxo(label);

        let retrieved = cc.get_label(&txid, 0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().label, "My UTXO");

        cc.remove_label(&txid, 0);
        assert!(cc.get_label(&txid, 0).is_none());
    }

    #[test]
    fn test_dust_threshold() {
        let cc = CoinControl::new();
        let fee_rate = 10.0; // 10 sat/vB

        let threshold = cc.dust_threshold(fee_rate);
        assert!(threshold > 0);

        assert!(cc.is_dust(100, fee_rate));
        assert!(!cc.is_dust(10_000, fee_rate));
    }

    #[test]
    fn test_privacy_preferences_defaults() {
        let prefs = PrivacyPreferences::default();
        assert!(prefs.avoid_address_reuse);
        assert!(prefs.avoid_mixing_sources);
        assert_eq!(prefs.min_privacy_score, 0);
    }

    #[test]
    fn test_manual_selection() {
        let cc = CoinControl::new();
        let utxos = vec![];
        let selected_txids = vec![];

        let selection = cc.select_utxos(utxos, &selected_txids);
        assert!(selection.is_complete());
        assert_eq!(selection.total_amount, 0);
    }

    #[test]
    fn test_privacy_selection() {
        let cc = CoinControl::new();
        let utxos = vec![];
        let target_amount = 100_000;

        let selection =
            cc.select_privacy_preserving(utxos, target_amount, SelectionStrategy::LargestFirst);

        assert!(!selection.is_sufficient());
        assert_eq!(selection.privacy_assessment(), "Unknown");
    }

    #[test]
    fn test_get_utxos_by_tag() {
        let mut cc = CoinControl::new();
        let txid1 = Txid::all_zeros();
        let txid2 = Txid::from_byte_array([1; 32]);

        let mut label1 = UtxoLabel::new(txid1, 0, "UTXO 1".to_string());
        label1.add_tag("exchange".to_string());
        cc.label_utxo(label1);

        let mut label2 = UtxoLabel::new(txid2, 0, "UTXO 2".to_string());
        label2.add_tag("mining".to_string());
        cc.label_utxo(label2);

        let exchange_utxos = cc.get_utxos_by_tag("exchange");
        assert_eq!(exchange_utxos.len(), 1);
        assert_eq!(exchange_utxos[0].label, "UTXO 1");
    }

    #[test]
    fn test_search_labels() {
        let mut cc = CoinControl::new();
        let txid = Txid::all_zeros();

        let label = UtxoLabel::new(txid, 0, "Payment from Alice".to_string());
        cc.label_utxo(label);

        let results = cc.search_labels("alice");
        assert_eq!(results.len(), 1);

        let results = cc.search_labels("bob");
        assert_eq!(results.len(), 0);
    }
}
