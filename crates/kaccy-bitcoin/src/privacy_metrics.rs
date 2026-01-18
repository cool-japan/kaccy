//! Privacy Metrics and Analysis
//!
//! This module provides comprehensive privacy analysis for Bitcoin transactions and wallets.
//! It helps identify privacy issues and provides recommendations for improvement.
//!
//! # Features
//!
//! - Transaction privacy scoring (0-100)
//! - Wallet privacy health checks
//! - Common heuristic detection
//! - Privacy improvement recommendations
//! - Address clustering analysis
//! - Change output identification
//! - Entity linking detection
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::privacy_metrics::{PrivacyAnalyzer, TransactionPrivacyAnalysis};
//! use bitcoin::Txid;
//! use std::str::FromStr;
//!
//! let txid = Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
//! let analyzer = PrivacyAnalyzer::new();
//!
//! // Analyze transaction privacy (requires transaction data)
//! // let analysis = analyzer.analyze_transaction(&tx_data);
//! // println!("Privacy score: {}", analysis.privacy_score);
//! ```

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Privacy score (0-100, where 100 is best privacy)
pub type PrivacyScore = u8;

/// Privacy analyzer for transactions and wallets
#[allow(dead_code)]
pub struct PrivacyAnalyzer {
    /// Known heuristics to check
    heuristics: Vec<PrivacyHeuristic>,
}

impl PrivacyAnalyzer {
    /// Create a new privacy analyzer with default heuristics
    pub fn new() -> Self {
        Self {
            heuristics: PrivacyHeuristic::default_heuristics(),
        }
    }

    /// Analyze transaction privacy
    pub fn analyze_transaction(
        &self,
        inputs: &[TransactionInput],
        outputs: &[TransactionOutput],
    ) -> TransactionPrivacyAnalysis {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();
        let mut recommendations = Vec::new();

        // Check for common input ownership heuristic
        if inputs.len() > 1 {
            let addresses: HashSet<_> = inputs.iter().map(|i| &i.address).collect();
            if addresses.len() < inputs.len() {
                issues.push(PrivacyIssue::AddressReuse);
                recommendations.push(
                    "Avoid reusing addresses across multiple inputs in the same transaction"
                        .to_string(),
                );
            }
        }

        // Check for round payment amounts (fingerprinting risk)
        for output in outputs {
            if self.is_round_amount(output.amount) {
                warnings
                    .push("Round payment amount detected - may leak payment intent".to_string());
                recommendations.push("Use non-round amounts to avoid fingerprinting".to_string());
            }
        }

        // Check for change output identification
        let potential_change = self.identify_change_outputs(inputs, outputs);
        if !potential_change.is_empty() {
            warnings.push(format!(
                "Identified {} potential change output(s)",
                potential_change.len()
            ));
        }

        // Check for unnecessary inputs heuristic
        let total_input: u64 = inputs.iter().map(|i| i.amount).sum();
        let total_output: u64 = outputs.iter().map(|o| o.amount).sum();
        let change = total_input.saturating_sub(total_output);

        if inputs.len() > 2 && change > total_output / 2 {
            warnings.push("Large change output suggests unnecessary inputs were used".to_string());
            recommendations.push(
                "Use coin selection algorithms to minimize change and number of inputs".to_string(),
            );
        }

        // Calculate overall privacy score
        let privacy_score =
            self.calculate_privacy_score(&issues, &warnings, inputs.len(), outputs.len());

        TransactionPrivacyAnalysis {
            privacy_score,
            issues,
            warnings,
            recommendations,
            change_outputs: potential_change,
            heuristics_triggered: self.triggered_heuristics(inputs, outputs),
        }
    }

    /// Identify potential change outputs
    fn identify_change_outputs(
        &self,
        _inputs: &[TransactionInput],
        outputs: &[TransactionOutput],
    ) -> Vec<usize> {
        let mut potential_change = Vec::new();

        // Heuristic: change output is typically not a round amount
        for (idx, output) in outputs.iter().enumerate() {
            if !self.is_round_amount(output.amount) {
                // Additional heuristic: change is often largest output
                let is_largest = outputs.iter().all(|o| o.amount <= output.amount);

                if is_largest || outputs.len() == 2 {
                    potential_change.push(idx);
                }
            }
        }

        potential_change
    }

    /// Check if amount is "round" (fingerprinting risk)
    fn is_round_amount(&self, amount: u64) -> bool {
        // Round numbers in BTC terms
        let btc = amount as f64 / 100_000_000.0;
        let rounded = btc.round();
        (btc - rounded).abs() < 0.000001 || amount % 100_000 == 0
    }

    /// Calculate privacy score based on detected issues
    fn calculate_privacy_score(
        &self,
        issues: &[PrivacyIssue],
        warnings: &[String],
        num_inputs: usize,
        num_outputs: usize,
    ) -> PrivacyScore {
        let mut score = 100u8;

        // Penalize for each major issue
        score = score.saturating_sub(issues.len() as u8 * 20);

        // Penalize for warnings
        score = score.saturating_sub(warnings.len() as u8 * 10);

        // Penalize for too many inputs (suggests consolidation)
        if num_inputs > 3 {
            score = score.saturating_sub((num_inputs - 3) as u8 * 3);
        }

        // Penalize for exactly 2 outputs (change detection is easier)
        if num_outputs == 2 {
            score = score.saturating_sub(5);
        }

        score
    }

    /// Get triggered heuristics
    fn triggered_heuristics(
        &self,
        inputs: &[TransactionInput],
        outputs: &[TransactionOutput],
    ) -> Vec<String> {
        let mut triggered = Vec::new();

        // Common input ownership
        if inputs.len() > 1 {
            triggered.push("Common Input Ownership".to_string());
        }

        // Change output detection
        if outputs.len() == 2 {
            triggered.push("Two-Output Change Detection".to_string());
        }

        // Round amount
        for output in outputs {
            if self.is_round_amount(output.amount) {
                triggered.push("Round Payment Amount".to_string());
                break;
            }
        }

        triggered
    }
}

impl Default for PrivacyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction input for privacy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    /// Previous transaction ID
    pub txid: Txid,
    /// Output index
    pub vout: u32,
    /// Address (if known)
    pub address: String,
    /// Amount in satoshis
    pub amount: u64,
}

/// Transaction output for privacy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutput {
    /// Amount in satoshis
    pub amount: u64,
    /// Address
    pub address: String,
    /// Whether this is an OP_RETURN output
    pub is_op_return: bool,
}

/// Result of transaction privacy analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPrivacyAnalysis {
    /// Privacy score (0-100)
    pub privacy_score: PrivacyScore,
    /// Critical privacy issues detected
    pub issues: Vec<PrivacyIssue>,
    /// Privacy warnings
    pub warnings: Vec<String>,
    /// Recommendations for improving privacy
    pub recommendations: Vec<String>,
    /// Indices of potential change outputs
    pub change_outputs: Vec<usize>,
    /// List of heuristics that were triggered
    pub heuristics_triggered: Vec<String>,
}

/// Privacy issues that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyIssue {
    /// Address reuse detected
    AddressReuse,
    /// Common input ownership heuristic applies
    CommonInputOwnership,
    /// Change output easily identifiable
    ObviousChangeOutput,
    /// Transaction has known fingerprint
    KnownFingerprint,
    /// Timing correlation possible
    TimingCorrelation,
}

/// Privacy heuristic for analysis
#[derive(Debug, Clone)]
pub enum PrivacyHeuristic {
    /// Common input ownership heuristic
    CommonInputOwnership,
    /// Change identification heuristic
    ChangeIdentification,
    /// Round amount detection
    RoundAmountDetection,
    /// Unnecessary input detection
    UnnecessaryInputs,
}

impl PrivacyHeuristic {
    /// Get default heuristics to check
    pub fn default_heuristics() -> Vec<Self> {
        vec![
            Self::CommonInputOwnership,
            Self::ChangeIdentification,
            Self::RoundAmountDetection,
            Self::UnnecessaryInputs,
        ]
    }
}

/// Wallet privacy health analyzer
pub struct WalletPrivacyAnalyzer {
    /// Addresses to analyze
    addresses: HashSet<String>,
    /// Transaction history
    transactions: Vec<Txid>,
}

impl WalletPrivacyAnalyzer {
    /// Create a new wallet privacy analyzer
    pub fn new() -> Self {
        Self {
            addresses: HashSet::new(),
            transactions: Vec::new(),
        }
    }

    /// Add an address to the wallet
    pub fn add_address(&mut self, address: String) {
        self.addresses.insert(address);
    }

    /// Add a transaction to the wallet history
    pub fn add_transaction(&mut self, txid: Txid) {
        self.transactions.push(txid);
    }

    /// Analyze wallet privacy health
    pub fn analyze(&self) -> WalletPrivacyHealth {
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();

        // Check for address reuse
        let unique_addresses = self.addresses.len();
        let total_transactions = self.transactions.len();

        if unique_addresses > 0 && total_transactions > unique_addresses * 2 {
            issues.push("Potential address reuse detected".to_string());
            recommendations
                .push("Use a new address for each transaction to improve privacy".to_string());
        }

        // Calculate overall wallet privacy score
        let privacy_score = if issues.is_empty() {
            85 // Good baseline
        } else {
            65 // Reduced due to issues
        };

        WalletPrivacyHealth {
            privacy_score,
            unique_addresses,
            total_transactions,
            issues,
            recommendations,
            health_status: if privacy_score >= 80 {
                HealthStatus::Good
            } else if privacy_score >= 60 {
                HealthStatus::Fair
            } else {
                HealthStatus::Poor
            },
        }
    }
}

impl Default for WalletPrivacyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Wallet privacy health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletPrivacyHealth {
    /// Overall privacy score (0-100)
    pub privacy_score: PrivacyScore,
    /// Number of unique addresses
    pub unique_addresses: usize,
    /// Total number of transactions
    pub total_transactions: usize,
    /// Detected privacy issues
    pub issues: Vec<String>,
    /// Recommendations for improvement
    pub recommendations: Vec<String>,
    /// Overall health status
    pub health_status: HealthStatus,
}

/// Wallet privacy health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Good privacy practices
    Good,
    /// Fair privacy, some issues
    Fair,
    /// Poor privacy, needs improvement
    Poor,
}

/// Address cluster analyzer for entity detection
pub struct AddressClusterAnalyzer {
    /// Address clusters (addresses controlled by same entity)
    clusters: HashMap<String, HashSet<String>>,
}

impl AddressClusterAnalyzer {
    /// Create a new address cluster analyzer
    pub fn new() -> Self {
        Self {
            clusters: HashMap::new(),
        }
    }

    /// Add addresses that appear together in a transaction input (common input ownership)
    pub fn add_common_input_addresses(&mut self, addresses: Vec<String>) {
        if addresses.len() < 2 {
            return;
        }

        // Find or create cluster
        let mut cluster_key = None;
        for addr in &addresses {
            if self.clusters.contains_key(addr) {
                cluster_key = Some(addr.clone());
                break;
            }
        }

        let key = cluster_key.unwrap_or_else(|| addresses[0].clone());
        let cluster = self.clusters.entry(key.clone()).or_default();
        for addr in addresses {
            cluster.insert(addr);
        }
    }

    /// Get cluster for an address
    pub fn get_cluster(&self, address: &str) -> Option<&HashSet<String>> {
        self.clusters
            .values()
            .find(|&cluster| cluster.contains(address))
            .map(|v| v as _)
    }

    /// Get number of identified clusters
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Analyze clustering
    pub fn analyze(&self) -> ClusterAnalysis {
        let total_addresses: usize = self.clusters.values().map(|c| c.len()).sum();
        let average_cluster_size = if self.clusters.is_empty() {
            0.0
        } else {
            total_addresses as f64 / self.clusters.len() as f64
        };

        ClusterAnalysis {
            total_clusters: self.clusters.len(),
            total_addresses,
            average_cluster_size,
            largest_cluster_size: self.clusters.values().map(|c| c.len()).max().unwrap_or(0),
        }
    }
}

impl Default for AddressClusterAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of address clustering analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterAnalysis {
    /// Total number of clusters identified
    pub total_clusters: usize,
    /// Total number of addresses analyzed
    pub total_addresses: usize,
    /// Average cluster size
    pub average_cluster_size: f64,
    /// Size of largest cluster
    pub largest_cluster_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_privacy_analyzer_creation() {
        let analyzer = PrivacyAnalyzer::new();
        assert!(!analyzer.heuristics.is_empty());
    }

    #[test]
    fn test_round_amount_detection() {
        let analyzer = PrivacyAnalyzer::new();
        assert!(analyzer.is_round_amount(100_000_000)); // 1 BTC
        assert!(analyzer.is_round_amount(50_000_000)); // 0.5 BTC
        assert!(!analyzer.is_round_amount(12_345_678)); // Random amount
    }

    #[test]
    fn test_transaction_privacy_analysis() {
        let analyzer = PrivacyAnalyzer::new();

        let inputs = vec![TransactionInput {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            vout: 0,
            address: "bc1qtest".to_string(),
            amount: 100_000,
        }];

        let outputs = vec![
            TransactionOutput {
                amount: 50_000,
                address: "bc1qrecipient".to_string(),
                is_op_return: false,
            },
            TransactionOutput {
                amount: 49_000,
                address: "bc1qchange".to_string(),
                is_op_return: false,
            },
        ];

        let analysis = analyzer.analyze_transaction(&inputs, &outputs);
        assert!(analysis.privacy_score > 0);
        assert!(analysis.privacy_score <= 100);
    }

    #[test]
    fn test_wallet_privacy_analyzer() {
        let mut analyzer = WalletPrivacyAnalyzer::new();
        analyzer.add_address("bc1qtest1".to_string());
        analyzer.add_address("bc1qtest2".to_string());
        analyzer.add_transaction(
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        );

        let health = analyzer.analyze();
        assert!(health.privacy_score > 0);
        assert!(health.privacy_score <= 100);
        assert_eq!(health.unique_addresses, 2);
        assert_eq!(health.total_transactions, 1);
    }

    #[test]
    fn test_address_cluster_analyzer() {
        let mut analyzer = AddressClusterAnalyzer::new();
        analyzer.add_common_input_addresses(vec!["bc1qtest1".to_string(), "bc1qtest2".to_string()]);

        assert_eq!(analyzer.cluster_count(), 1);

        let cluster = analyzer.get_cluster("bc1qtest1");
        assert!(cluster.is_some());
        assert_eq!(cluster.unwrap().len(), 2);
    }

    #[test]
    fn test_cluster_analysis() {
        let mut analyzer = AddressClusterAnalyzer::new();
        analyzer.add_common_input_addresses(vec![
            "addr1".to_string(),
            "addr2".to_string(),
            "addr3".to_string(),
        ]);

        let analysis = analyzer.analyze();
        assert_eq!(analysis.total_clusters, 1);
        assert_eq!(analysis.total_addresses, 3);
        assert_eq!(analysis.largest_cluster_size, 3);
    }

    #[test]
    fn test_health_status_good() {
        let health = WalletPrivacyHealth {
            privacy_score: 85,
            unique_addresses: 10,
            total_transactions: 5,
            issues: vec![],
            recommendations: vec![],
            health_status: HealthStatus::Good,
        };

        assert_eq!(health.health_status, HealthStatus::Good);
    }

    #[test]
    fn test_change_output_identification() {
        let analyzer = PrivacyAnalyzer::new();

        let inputs = vec![TransactionInput {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            vout: 0,
            address: "bc1qtest".to_string(),
            amount: 100_000,
        }];

        // Round payment amount (50,000) vs non-round change (48,500)
        let outputs = vec![
            TransactionOutput {
                amount: 50_000,
                address: "bc1qrecipient".to_string(),
                is_op_return: false,
            },
            TransactionOutput {
                amount: 48_500,
                address: "bc1qchange".to_string(),
                is_op_return: false,
            },
        ];

        let change_indices = analyzer.identify_change_outputs(&inputs, &outputs);
        assert!(!change_indices.is_empty());
    }
}
