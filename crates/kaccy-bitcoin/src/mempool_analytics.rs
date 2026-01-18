//! Advanced Mempool Analytics
//!
//! This module provides sophisticated mempool analysis tools including:
//! - Mempool histogram analysis for fee distribution
//! - Transaction clustering detection
//! - Fee spike prediction using historical data
//! - Replacement cycle detection (RBF chains)

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Mempool histogram showing fee rate distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolHistogram {
    /// Fee rate buckets (sat/vB)
    pub buckets: Vec<FeeBucket>,
    /// Total transactions in mempool
    pub total_transactions: usize,
    /// Total size in vbytes
    pub total_vsize: u64,
    /// Timestamp of analysis
    pub timestamp: u64,
}

/// A fee rate bucket in the histogram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeBucket {
    /// Minimum fee rate for this bucket (sat/vB)
    pub min_fee_rate: u64,
    /// Maximum fee rate for this bucket (sat/vB)
    pub max_fee_rate: u64,
    /// Number of transactions in this bucket
    pub tx_count: usize,
    /// Total size in vbytes
    pub total_vsize: u64,
    /// Percentage of total mempool
    pub percentage: f64,
}

/// Mempool histogram analyzer
pub struct MempoolHistogramAnalyzer {
    bucket_ranges: Vec<(u64, u64)>,
}

impl MempoolHistogramAnalyzer {
    /// Create a new histogram analyzer with default buckets
    pub fn new() -> Self {
        Self {
            bucket_ranges: vec![
                (0, 1),
                (1, 2),
                (2, 5),
                (5, 10),
                (10, 20),
                (20, 50),
                (50, 100),
                (100, 200),
                (200, 500),
                (500, 1000),
                (1000, u64::MAX),
            ],
        }
    }

    /// Create analyzer with custom bucket ranges
    pub fn with_buckets(bucket_ranges: Vec<(u64, u64)>) -> Self {
        Self { bucket_ranges }
    }

    /// Analyze mempool transactions and create histogram
    pub fn analyze(&self, transactions: &[MempoolTransaction]) -> MempoolHistogram {
        let mut buckets: HashMap<usize, (usize, u64)> = HashMap::new();

        for tx in transactions {
            if let Some(bucket_idx) = self.find_bucket(tx.fee_rate) {
                let entry = buckets.entry(bucket_idx).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += tx.vsize;
            }
        }

        let total_vsize: u64 = transactions.iter().map(|tx| tx.vsize).sum();

        let bucket_vec: Vec<FeeBucket> = self
            .bucket_ranges
            .iter()
            .enumerate()
            .map(|(idx, (min, max))| {
                let (count, vsize) = buckets.get(&idx).copied().unwrap_or((0, 0));
                let percentage = if !transactions.is_empty() {
                    (count as f64 / transactions.len() as f64) * 100.0
                } else {
                    0.0
                };

                FeeBucket {
                    min_fee_rate: *min,
                    max_fee_rate: *max,
                    tx_count: count,
                    total_vsize: vsize,
                    percentage,
                }
            })
            .collect();

        MempoolHistogram {
            buckets: bucket_vec,
            total_transactions: transactions.len(),
            total_vsize,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    fn find_bucket(&self, fee_rate: u64) -> Option<usize> {
        self.bucket_ranges
            .iter()
            .position(|(min, max)| fee_rate >= *min && fee_rate < *max)
    }

    /// Get recommended fee rate for target confirmation
    pub fn recommend_fee_rate(&self, histogram: &MempoolHistogram, target_blocks: u32) -> u64 {
        // Simple heuristic: for 1-2 blocks, use 75th percentile
        // for 3-6 blocks, use median, for 7+ blocks, use 25th percentile
        let percentile = match target_blocks {
            1..=2 => 75.0,
            3..=6 => 50.0,
            _ => 25.0,
        };

        self.calculate_percentile_fee(histogram, percentile)
    }

    fn calculate_percentile_fee(&self, histogram: &MempoolHistogram, percentile: f64) -> u64 {
        let target_count = (histogram.total_transactions as f64 * percentile / 100.0) as usize;
        let mut cumulative = 0;

        for bucket in &histogram.buckets {
            cumulative += bucket.tx_count;
            if cumulative >= target_count {
                return (bucket.min_fee_rate + bucket.max_fee_rate) / 2;
            }
        }

        // Default to 10 sat/vB if calculation fails
        10
    }
}

impl Default for MempoolHistogramAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Mempool transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolTransaction {
    /// Transaction ID
    pub txid: Txid,
    /// Virtual size in vbytes
    pub vsize: u64,
    /// Fee in satoshis
    pub fee: u64,
    /// Fee rate in sat/vB
    pub fee_rate: u64,
    /// Time entered mempool (unix timestamp)
    pub time: u64,
    /// Input transaction IDs
    pub depends: Vec<Txid>,
    /// Whether this replaces another transaction (RBF)
    pub replaces: Option<Txid>,
}

/// Transaction cluster in mempool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCluster {
    /// Cluster ID
    pub id: String,
    /// Transactions in this cluster
    pub transactions: Vec<Txid>,
    /// Total size in vbytes
    pub total_vsize: u64,
    /// Total fees
    pub total_fees: u64,
    /// Cluster fee rate (total_fees / total_vsize)
    pub cluster_fee_rate: u64,
    /// Ancestor count
    pub ancestor_count: usize,
}

/// Transaction clustering detector
pub struct ClusteringDetector {
    max_cluster_size: usize,
}

impl ClusteringDetector {
    pub fn new() -> Self {
        Self {
            max_cluster_size: 100,
        }
    }

    /// Detect transaction clusters based on dependencies
    pub fn detect_clusters(&self, transactions: &[MempoolTransaction]) -> Vec<TransactionCluster> {
        let mut tx_map: HashMap<Txid, &MempoolTransaction> = HashMap::new();
        for tx in transactions {
            tx_map.insert(tx.txid, tx);
        }

        let mut visited: HashSet<Txid> = HashSet::new();
        let mut clusters = Vec::new();

        for tx in transactions {
            if visited.contains(&tx.txid) {
                continue;
            }

            let mut cluster_txs = Vec::new();
            let mut local_visited: HashSet<Txid> = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(tx.txid);

            while let Some(txid) = queue.pop_front() {
                if local_visited.contains(&txid) || cluster_txs.len() >= self.max_cluster_size {
                    continue;
                }

                local_visited.insert(txid);
                cluster_txs.push(txid);

                if let Some(tx) = tx_map.get(&txid) {
                    for dep in &tx.depends {
                        if !local_visited.contains(dep) {
                            queue.push_back(*dep);
                        }
                    }
                }
            }

            if cluster_txs.len() > 1 {
                // Mark all transactions in this cluster as visited
                for txid in &cluster_txs {
                    visited.insert(*txid);
                }

                let total_vsize: u64 = cluster_txs
                    .iter()
                    .filter_map(|txid| tx_map.get(txid))
                    .map(|tx| tx.vsize)
                    .sum();

                let total_fees: u64 = cluster_txs
                    .iter()
                    .filter_map(|txid| tx_map.get(txid))
                    .map(|tx| tx.fee)
                    .sum();

                let cluster_fee_rate = if total_vsize > 0 {
                    total_fees / total_vsize
                } else {
                    0
                };

                clusters.push(TransactionCluster {
                    id: format!("cluster_{}", clusters.len()),
                    transactions: cluster_txs.clone(),
                    total_vsize,
                    total_fees,
                    cluster_fee_rate,
                    ancestor_count: cluster_txs.len(),
                });
            }
        }

        clusters
    }

    /// Find the largest cluster
    pub fn largest_cluster<'a>(
        &self,
        clusters: &'a [TransactionCluster],
    ) -> Option<&'a TransactionCluster> {
        clusters.iter().max_by_key(|c| c.transactions.len())
    }
}

impl Default for ClusteringDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee spike predictor using historical data
pub struct FeeSpikePredictor {
    history: VecDeque<FeeDataPoint>,
    max_history_hours: usize,
}

impl FeeSpikePredictor {
    pub fn new(max_history_hours: usize) -> Self {
        Self {
            history: VecDeque::new(),
            max_history_hours,
        }
    }

    /// Record a fee rate observation
    pub fn record(&mut self, fee_rate: u64, mempool_size_mb: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.history.push_back(FeeDataPoint {
            timestamp: now,
            fee_rate,
            mempool_size_mb,
        });

        // Keep only recent history
        let cutoff_time = now - (self.max_history_hours as u64 * 3600);
        while let Some(front) = self.history.front() {
            if front.timestamp < cutoff_time {
                self.history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Predict if a fee spike is likely in the next period
    pub fn predict_spike(&self, lookback_hours: usize) -> SpikePrediction {
        if self.history.len() < lookback_hours {
            return SpikePrediction {
                spike_likely: false,
                confidence: 0.0,
                expected_peak_fee: 0,
                reason: "Insufficient historical data".to_string(),
            };
        }

        let recent_count = lookback_hours.min(self.history.len());
        let recent: Vec<_> = self.history.iter().rev().take(recent_count).collect();

        // Calculate trends
        let recent_avg_fee: u64 =
            recent.iter().map(|d| d.fee_rate).sum::<u64>() / recent.len() as u64;
        let recent_avg_size: f64 =
            recent.iter().map(|d| d.mempool_size_mb).sum::<f64>() / recent.len() as f64;

        let historical_avg_fee: u64 =
            self.history.iter().map(|d| d.fee_rate).sum::<u64>() / self.history.len() as u64;
        let historical_avg_size: f64 =
            self.history.iter().map(|d| d.mempool_size_mb).sum::<f64>() / self.history.len() as f64;

        // Check for upward trends
        let fee_increasing = recent_avg_fee > historical_avg_fee * 12 / 10; // 20% increase
        let mempool_growing = recent_avg_size > historical_avg_size * 1.3; // 30% increase

        let spike_likely = fee_increasing && mempool_growing;
        let confidence = if spike_likely {
            let fee_factor = recent_avg_fee as f64 / historical_avg_fee.max(1) as f64;
            let size_factor = recent_avg_size / historical_avg_size.max(1.0);
            ((fee_factor + size_factor) / 2.0 * 50.0).min(95.0)
        } else {
            0.0
        };

        let expected_peak_fee = if spike_likely {
            (recent_avg_fee as f64 * 1.5) as u64
        } else {
            recent_avg_fee
        };

        let reason = if spike_likely {
            format!(
                "Fees up {}%, mempool up {}%",
                ((recent_avg_fee as f64 / historical_avg_fee.max(1) as f64 - 1.0) * 100.0) as i32,
                ((recent_avg_size / historical_avg_size.max(1.0) - 1.0) * 100.0) as i32
            )
        } else {
            "No significant upward trend detected".to_string()
        };

        SpikePrediction {
            spike_likely,
            confidence,
            expected_peak_fee,
            reason,
        }
    }

    /// Get current mempool statistics
    pub fn get_stats(&self) -> MempoolStats {
        if self.history.is_empty() {
            return MempoolStats {
                current_fee_rate: 0,
                avg_fee_rate_1h: 0,
                avg_fee_rate_24h: 0,
                current_mempool_size_mb: 0.0,
                avg_mempool_size_24h: 0.0,
            };
        }

        let latest = self.history.back().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let one_hour_ago = now - 3600;
        let one_hour_data: Vec<_> = self
            .history
            .iter()
            .filter(|d| d.timestamp >= one_hour_ago)
            .collect();

        let avg_fee_1h = if !one_hour_data.is_empty() {
            one_hour_data.iter().map(|d| d.fee_rate).sum::<u64>() / one_hour_data.len() as u64
        } else {
            latest.fee_rate
        };

        let avg_fee_24h =
            self.history.iter().map(|d| d.fee_rate).sum::<u64>() / self.history.len() as u64;
        let avg_size_24h =
            self.history.iter().map(|d| d.mempool_size_mb).sum::<f64>() / self.history.len() as f64;

        MempoolStats {
            current_fee_rate: latest.fee_rate,
            avg_fee_rate_1h: avg_fee_1h,
            avg_fee_rate_24h: avg_fee_24h,
            current_mempool_size_mb: latest.mempool_size_mb,
            avg_mempool_size_24h: avg_size_24h,
        }
    }
}

/// Fee data point for historical tracking
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct FeeDataPoint {
    timestamp: u64,
    fee_rate: u64,
    mempool_size_mb: f64,
}

/// Fee spike prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpikePrediction {
    /// Whether a spike is likely
    pub spike_likely: bool,
    /// Confidence level (0-100)
    pub confidence: f64,
    /// Expected peak fee rate
    pub expected_peak_fee: u64,
    /// Reason for prediction
    pub reason: String,
}

/// Mempool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolStats {
    /// Current fee rate
    pub current_fee_rate: u64,
    /// Average fee rate last 1 hour
    pub avg_fee_rate_1h: u64,
    /// Average fee rate last 24 hours
    pub avg_fee_rate_24h: u64,
    /// Current mempool size in MB
    pub current_mempool_size_mb: f64,
    /// Average mempool size last 24 hours
    pub avg_mempool_size_24h: f64,
}

/// RBF replacement cycle detector
pub struct ReplacementCycleDetector {
    replacement_chains: HashMap<Txid, Vec<Txid>>,
    max_chain_length: usize,
}

impl ReplacementCycleDetector {
    pub fn new() -> Self {
        Self {
            replacement_chains: HashMap::new(),
            max_chain_length: 50,
        }
    }

    /// Record a replacement transaction
    pub fn record_replacement(&mut self, original: Txid, replacement: Txid) {
        // Find the chain this belongs to
        let updated_chain = if let Some(chain) = self.replacement_chains.get_mut(&original) {
            chain.push(replacement);
            if chain.len() > self.max_chain_length {
                chain.remove(0);
            }
            chain.clone()
        } else {
            // Start a new chain
            let chain = vec![original, replacement];
            self.replacement_chains.insert(original, chain.clone());
            chain
        };

        // Add a direct reference to the replacement (after the mutable borrow is done)
        self.replacement_chains.insert(replacement, updated_chain);
    }

    /// Get the replacement chain for a transaction
    pub fn get_chain(&self, txid: &Txid) -> Option<&Vec<Txid>> {
        self.replacement_chains.get(txid)
    }

    /// Detect excessive replacement cycles
    pub fn detect_excessive_cycles(&self) -> Vec<ReplacementCycle> {
        let mut cycles = Vec::new();

        for (txid, chain) in &self.replacement_chains {
            if chain.len() >= 5 {
                // Consider 5+ replacements as excessive
                let fee_increases = self.estimate_fee_increases(chain);

                cycles.push(ReplacementCycle {
                    original_txid: chain[0],
                    current_txid: *txid,
                    replacement_count: chain.len() - 1,
                    chain: chain.clone(),
                    total_fee_increase: fee_increases.iter().sum(),
                    is_suspicious: chain.len() >= 10,
                });
            }
        }

        cycles.sort_by_key(|c| std::cmp::Reverse(c.replacement_count));
        cycles
    }

    fn estimate_fee_increases(&self, _chain: &[Txid]) -> Vec<u64> {
        // Placeholder - in real implementation, would track actual fee increases
        vec![0; _chain.len().saturating_sub(1)]
    }

    /// Clean up old chains
    pub fn cleanup(&mut self, keep_count: usize) {
        if self.replacement_chains.len() > keep_count {
            // Keep only the most recent chains (simplified)
            let excess = self.replacement_chains.len() - keep_count;
            let keys_to_remove: Vec<_> = self
                .replacement_chains
                .keys()
                .take(excess)
                .copied()
                .collect();
            for key in keys_to_remove {
                self.replacement_chains.remove(&key);
            }
        }
    }
}

impl Default for ReplacementCycleDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detected replacement cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementCycle {
    /// Original transaction ID
    pub original_txid: Txid,
    /// Current (latest) transaction ID
    pub current_txid: Txid,
    /// Number of replacements
    pub replacement_count: usize,
    /// Full chain of transactions
    pub chain: Vec<Txid>,
    /// Total fee increase across all replacements
    pub total_fee_increase: u64,
    /// Whether this appears suspicious (too many replacements)
    pub is_suspicious: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_histogram_analyzer() {
        let analyzer = MempoolHistogramAnalyzer::new();
        let txs = vec![
            MempoolTransaction {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .unwrap(),
                vsize: 200,
                fee: 1000,
                fee_rate: 5,
                time: 0,
                depends: vec![],
                replaces: None,
            },
            MempoolTransaction {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000002",
                )
                .unwrap(),
                vsize: 300,
                fee: 3000,
                fee_rate: 10,
                time: 0,
                depends: vec![],
                replaces: None,
            },
        ];

        let histogram = analyzer.analyze(&txs);
        assert_eq!(histogram.total_transactions, 2);
        assert_eq!(histogram.total_vsize, 500);
    }

    #[test]
    fn test_clustering_detector() {
        let detector = ClusteringDetector::new();
        let tx1_id =
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let tx2_id =
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000002")
                .unwrap();

        let txs = vec![
            MempoolTransaction {
                txid: tx1_id,
                vsize: 200,
                fee: 1000,
                fee_rate: 5,
                time: 0,
                depends: vec![],
                replaces: None,
            },
            MempoolTransaction {
                txid: tx2_id,
                vsize: 300,
                fee: 3000,
                fee_rate: 10,
                time: 0,
                depends: vec![tx1_id],
                replaces: None,
            },
        ];

        let clusters = detector.detect_clusters(&txs);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].transactions.len(), 2);
    }

    #[test]
    fn test_fee_spike_predictor() {
        let mut predictor = FeeSpikePredictor::new(24);

        // Record some stable fees
        for _ in 0..10 {
            predictor.record(10, 50.0);
        }

        // Record increasing fees
        for i in 1..=5 {
            predictor.record(10 + i * 5, 50.0 + i as f64 * 10.0);
        }

        let prediction = predictor.predict_spike(5);
        assert!(prediction.spike_likely);
        assert!(prediction.confidence > 0.0);
    }

    #[test]
    fn test_replacement_cycle_detector() {
        let mut detector = ReplacementCycleDetector::new();
        let tx1 =
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let tx2 =
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000002")
                .unwrap();
        let tx3 =
            Txid::from_str("0000000000000000000000000000000000000000000000000000000000000003")
                .unwrap();

        detector.record_replacement(tx1, tx2);
        detector.record_replacement(tx2, tx3);

        let chain = detector.get_chain(&tx3).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], tx1);
        assert_eq!(chain[2], tx3);
    }

    #[test]
    fn test_mempool_stats() {
        let mut predictor = FeeSpikePredictor::new(24);
        predictor.record(10, 50.0);
        predictor.record(15, 60.0);

        let stats = predictor.get_stats();
        assert_eq!(stats.current_fee_rate, 15);
        assert!(stats.avg_fee_rate_24h > 0);
    }
}
