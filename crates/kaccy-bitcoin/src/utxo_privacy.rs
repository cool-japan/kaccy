//! UTXO management with privacy features
//!
//! Implements privacy-focused UTXO management including:
//! - Consolidation privacy analysis
//! - Toxic change detection
//! - Privacy score calculation for UTXOs

use crate::error::BitcoinError;
use crate::utxo::Utxo;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Privacy analysis result for a UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoPrivacyAnalysis {
    /// UTXO identifier
    pub utxo_id: String,
    /// Privacy score (0-100)
    pub privacy_score: u32,
    /// Is this UTXO potentially toxic (deanonymized)
    pub is_toxic: bool,
    /// Privacy issues detected
    pub issues: Vec<PrivacyIssue>,
    /// Recommended action
    pub recommendation: PrivacyRecommendation,
}

/// Privacy issue with a UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyIssue {
    /// Address has been reused
    AddressReuse,
    /// Round amount (fingerprintable)
    RoundAmount,
    /// Change output from identified transaction
    KnownChange,
    /// Part of cluster with deanonymized UTXOs
    ClusterContamination,
    /// Small amount (dust-like)
    SmallAmount,
    /// Single large UTXO (unusual pattern)
    LargeAmount,
}

/// Privacy recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyRecommendation {
    /// UTXO is safe to use
    Safe,
    /// Use with caution
    UseWithCaution,
    /// Avoid using this UTXO
    Avoid,
    /// Consolidate with other UTXOs
    Consolidate,
    /// Mix with other coins first
    MixFirst,
}

/// UTXO cluster analysis
#[derive(Debug, Clone)]
pub struct UtxoCluster {
    /// Cluster ID
    pub id: String,
    /// UTXOs in this cluster
    pub utxos: Vec<Utxo>,
    /// Addresses in cluster
    pub addresses: HashSet<String>,
    /// Cluster privacy score
    pub privacy_score: u32,
}

/// Consolidation privacy analyzer
#[derive(Debug)]
pub struct ConsolidationPrivacyAnalyzer {
    /// Minimum privacy score threshold
    min_privacy_score: u32,
    /// Track address usage
    address_usage: HashMap<String, usize>,
}

impl ConsolidationPrivacyAnalyzer {
    /// Create a new consolidation privacy analyzer
    pub fn new(min_privacy_score: u32) -> Self {
        Self {
            min_privacy_score,
            address_usage: HashMap::new(),
        }
    }

    /// Analyze consolidation privacy impact
    pub fn analyze_consolidation(
        &mut self,
        utxos: &[Utxo],
    ) -> Result<ConsolidationPrivacyReport, BitcoinError> {
        // Update address usage tracking
        for utxo in utxos {
            *self.address_usage.entry(utxo.address.clone()).or_insert(0) += 1;
        }

        // Check for address reuse
        let has_address_reuse = self.detect_address_reuse(utxos);

        // Check for round amounts
        let has_round_amounts = self.detect_round_amounts(utxos);

        // Calculate privacy score
        let privacy_score =
            self.calculate_consolidation_score(utxos, has_address_reuse, has_round_amounts);

        // Determine if consolidation is privacy-safe
        let is_safe = privacy_score >= self.min_privacy_score;

        Ok(ConsolidationPrivacyReport {
            privacy_score,
            is_safe,
            has_address_reuse,
            has_round_amounts,
            unique_addresses: self.count_unique_addresses(utxos),
            total_utxos: utxos.len(),
            recommendation: if is_safe {
                "Consolidation appears privacy-safe".to_string()
            } else {
                "Consolidation may reveal ownership linkage".to_string()
            },
        })
    }

    /// Detect address reuse
    fn detect_address_reuse(&self, utxos: &[Utxo]) -> bool {
        let addresses: HashSet<_> = utxos.iter().map(|u| &u.address).collect();
        addresses.len() < utxos.len()
    }

    /// Detect round amounts (potential fingerprinting)
    fn detect_round_amounts(&self, utxos: &[Utxo]) -> bool {
        for utxo in utxos {
            // Check if amount is suspiciously round (e.g., exactly 1 BTC, 0.1 BTC)
            if utxo.amount_sats % 100_000_000 == 0 || utxo.amount_sats % 10_000_000 == 0 {
                return true;
            }
        }
        false
    }

    /// Calculate consolidation privacy score
    fn calculate_consolidation_score(
        &self,
        utxos: &[Utxo],
        has_reuse: bool,
        has_round: bool,
    ) -> u32 {
        let mut score = 100u32;

        // Penalty for address reuse
        if has_reuse {
            score = score.saturating_sub(30);
        }

        // Penalty for round amounts
        if has_round {
            score = score.saturating_sub(15);
        }

        // Penalty for too many inputs (creates large transaction)
        if utxos.len() > 20 {
            score = score.saturating_sub(10);
        }

        // Penalty for very few inputs (not much consolidation benefit)
        if utxos.len() < 5 {
            score = score.saturating_sub(5);
        }

        score
    }

    /// Count unique addresses
    fn count_unique_addresses(&self, utxos: &[Utxo]) -> usize {
        let addresses: HashSet<_> = utxos.iter().map(|u| &u.address).collect();
        addresses.len()
    }
}

/// Consolidation privacy report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationPrivacyReport {
    /// Privacy score (0-100)
    pub privacy_score: u32,
    /// Is consolidation safe from privacy perspective
    pub is_safe: bool,
    /// Address reuse detected
    pub has_address_reuse: bool,
    /// Round amounts detected
    pub has_round_amounts: bool,
    /// Number of unique addresses
    pub unique_addresses: usize,
    /// Total number of UTXOs
    pub total_utxos: usize,
    /// Privacy recommendation
    pub recommendation: String,
}

/// Toxic change detector
#[derive(Debug)]
pub struct ToxicChangeDetector {
    /// Known toxic addresses
    toxic_addresses: HashSet<String>,
    /// Cluster analysis
    #[allow(dead_code)]
    clusters: HashMap<String, UtxoCluster>,
}

impl ToxicChangeDetector {
    /// Create a new toxic change detector
    pub fn new() -> Self {
        Self {
            toxic_addresses: HashSet::new(),
            clusters: HashMap::new(),
        }
    }

    /// Mark an address as toxic
    pub fn mark_toxic(&mut self, address: String) {
        self.toxic_addresses.insert(address);
    }

    /// Check if UTXO is toxic
    pub fn is_toxic(&self, utxo: &Utxo) -> bool {
        self.toxic_addresses.contains(&utxo.address)
    }

    /// Analyze UTXO for toxicity
    pub fn analyze_utxo(&self, utxo: &Utxo) -> UtxoPrivacyAnalysis {
        let mut issues = Vec::new();
        let mut privacy_score = 100u32;
        let is_toxic = self.is_toxic(utxo);

        if is_toxic {
            issues.push(PrivacyIssue::ClusterContamination);
            privacy_score = privacy_score.saturating_sub(50);
        }

        // Check for round amount
        if Self::is_round_amount(utxo.amount_sats) {
            issues.push(PrivacyIssue::RoundAmount);
            privacy_score = privacy_score.saturating_sub(10);
        }

        // Check for small amount (dust-like)
        if utxo.amount_sats < 10_000 {
            issues.push(PrivacyIssue::SmallAmount);
            privacy_score = privacy_score.saturating_sub(5);
        }

        // Check for very large amount
        if utxo.amount_sats > 100_000_000 {
            // > 1 BTC
            issues.push(PrivacyIssue::LargeAmount);
            privacy_score = privacy_score.saturating_sub(5);
        }

        let recommendation = if privacy_score >= 80 {
            PrivacyRecommendation::Safe
        } else if privacy_score >= 60 {
            PrivacyRecommendation::UseWithCaution
        } else if privacy_score >= 40 {
            PrivacyRecommendation::Consolidate
        } else {
            PrivacyRecommendation::Avoid
        };

        UtxoPrivacyAnalysis {
            utxo_id: format!("{}:{}", utxo.txid, utxo.vout),
            privacy_score,
            is_toxic,
            issues,
            recommendation,
        }
    }

    /// Check if amount is round
    fn is_round_amount(sats: u64) -> bool {
        sats % 100_000_000 == 0 || sats % 10_000_000 == 0 || sats % 1_000_000 == 0
    }

    /// Get count of toxic addresses
    pub fn toxic_count(&self) -> usize {
        self.toxic_addresses.len()
    }

    /// Cluster UTXOs by common ownership heuristics
    pub fn cluster_utxos(&mut self, utxos: &[Utxo]) -> Vec<UtxoCluster> {
        // Simple clustering by address
        let mut address_clusters: HashMap<String, Vec<Utxo>> = HashMap::new();

        for utxo in utxos {
            address_clusters
                .entry(utxo.address.clone())
                .or_default()
                .push(utxo.clone());
        }

        let mut clusters = Vec::new();
        for (address, cluster_utxos) in address_clusters {
            let mut addresses = HashSet::new();
            addresses.insert(address.clone());

            let cluster = UtxoCluster {
                id: address.clone(),
                utxos: cluster_utxos,
                addresses,
                privacy_score: 100, // Simplified
            };

            clusters.push(cluster);
        }

        clusters
    }
}

impl Default for ToxicChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;
    use bitcoin::hashes::Hash;

    fn create_test_utxo(sats: u64, address: &str) -> Utxo {
        Utxo {
            txid: Txid::all_zeros(),
            vout: 0,
            amount_sats: sats,
            address: address.to_string(),
            confirmations: 6,
            spendable: true,
            safe: true,
        }
    }

    #[test]
    fn test_consolidation_analyzer_creation() {
        let analyzer = ConsolidationPrivacyAnalyzer::new(50);
        assert_eq!(analyzer.min_privacy_score, 50);
    }

    #[test]
    fn test_consolidation_privacy_analysis() {
        let mut analyzer = ConsolidationPrivacyAnalyzer::new(50);
        let utxos = vec![
            create_test_utxo(100000, "addr1"),
            create_test_utxo(200000, "addr2"),
            create_test_utxo(300000, "addr3"),
        ];

        let report = analyzer.analyze_consolidation(&utxos).unwrap();
        assert!(report.privacy_score > 0);
        assert_eq!(report.total_utxos, 3);
        assert_eq!(report.unique_addresses, 3);
    }

    #[test]
    fn test_address_reuse_detection() {
        let mut analyzer = ConsolidationPrivacyAnalyzer::new(50);
        let utxos = vec![
            create_test_utxo(100000, "addr1"),
            create_test_utxo(200000, "addr1"), // Reused address
        ];

        let report = analyzer.analyze_consolidation(&utxos).unwrap();
        assert!(report.has_address_reuse);
        assert!(report.privacy_score < 100);
    }

    #[test]
    fn test_round_amount_detection() {
        let mut analyzer = ConsolidationPrivacyAnalyzer::new(50);
        let utxos = vec![create_test_utxo(100_000_000, "addr1")]; // Exactly 1 BTC

        let report = analyzer.analyze_consolidation(&utxos).unwrap();
        assert!(report.has_round_amounts);
    }

    #[test]
    fn test_toxic_detector_creation() {
        let detector = ToxicChangeDetector::new();
        assert_eq!(detector.toxic_count(), 0);
    }

    #[test]
    fn test_toxic_address_marking() {
        let mut detector = ToxicChangeDetector::new();
        detector.mark_toxic("toxic_addr".to_string());

        assert_eq!(detector.toxic_count(), 1);
    }

    #[test]
    fn test_toxic_utxo_detection() {
        let mut detector = ToxicChangeDetector::new();
        detector.mark_toxic("toxic_addr".to_string());

        let toxic_utxo = create_test_utxo(100000, "toxic_addr");
        let clean_utxo = create_test_utxo(100000, "clean_addr");

        assert!(detector.is_toxic(&toxic_utxo));
        assert!(!detector.is_toxic(&clean_utxo));
    }

    #[test]
    fn test_utxo_privacy_analysis() {
        let detector = ToxicChangeDetector::new();
        let utxo = create_test_utxo(100000, "addr1");

        let analysis = detector.analyze_utxo(&utxo);
        assert!(analysis.privacy_score > 0);
        assert!(!analysis.is_toxic);
    }

    #[test]
    fn test_toxic_utxo_analysis() {
        let mut detector = ToxicChangeDetector::new();
        detector.mark_toxic("toxic".to_string());

        let utxo = create_test_utxo(100000, "toxic");
        let analysis = detector.analyze_utxo(&utxo);

        assert!(analysis.is_toxic);
        assert!(analysis.privacy_score < 100);
        assert!(!analysis.issues.is_empty());
    }

    #[test]
    fn test_round_amount_issue() {
        let detector = ToxicChangeDetector::new();
        let utxo = create_test_utxo(100_000_000, "addr1"); // Exactly 1 BTC

        let analysis = detector.analyze_utxo(&utxo);
        assert!(
            analysis
                .issues
                .iter()
                .any(|i| matches!(i, PrivacyIssue::RoundAmount))
        );
    }

    #[test]
    fn test_small_amount_issue() {
        let detector = ToxicChangeDetector::new();
        let utxo = create_test_utxo(5000, "addr1"); // Small amount

        let analysis = detector.analyze_utxo(&utxo);
        assert!(
            analysis
                .issues
                .iter()
                .any(|i| matches!(i, PrivacyIssue::SmallAmount))
        );
    }

    #[test]
    fn test_utxo_clustering() {
        let mut detector = ToxicChangeDetector::new();
        let utxos = vec![
            create_test_utxo(100000, "addr1"),
            create_test_utxo(200000, "addr1"),
            create_test_utxo(300000, "addr2"),
        ];

        let clusters = detector.cluster_utxos(&utxos);
        assert_eq!(clusters.len(), 2); // Two unique addresses
    }
}
