//! UTXO Set Optimization for Long-term Wallet Health
//!
//! This module provides tools for optimizing UTXO sets over time,
//! including health scoring, proactive consolidation triggers,
//! and fee market prediction integration.

use crate::btc_utils::{calculate_fee_from_rate, estimate_simple_transaction_vsize, is_dust};
use crate::utxo::Utxo;
use serde::{Deserialize, Serialize};

/// UTXO health score (0-100, higher is better)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthScore(
    /// Score value between 0 (worst) and 100 (best)
    pub u8,
);

impl HealthScore {
    /// Create a new health score, clamping to 0-100
    pub fn new(score: u8) -> Self {
        Self(score.min(100))
    }

    /// Get the raw score value
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Check if health is critical (needs immediate attention)
    pub fn is_critical(&self) -> bool {
        self.0 < 30
    }

    /// Check if health is poor (needs attention soon)
    pub fn is_poor(&self) -> bool {
        self.0 < 50
    }

    /// Check if health is good
    pub fn is_good(&self) -> bool {
        self.0 >= 70
    }

    /// Check if health is excellent
    pub fn is_excellent(&self) -> bool {
        self.0 >= 90
    }
}

/// UTXO set health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSetHealth {
    /// Overall health score
    pub overall_score: HealthScore,
    /// Number of total UTXOs
    pub total_utxos: usize,
    /// Number of dust UTXOs
    pub dust_utxos: usize,
    /// Number of economical UTXOs (can be spent profitably)
    pub economical_utxos: usize,
    /// Number of small UTXOs (< 10,000 sats)
    pub small_utxos: usize,
    /// Number of medium UTXOs (10,000 - 100,000 sats)
    pub medium_utxos: usize,
    /// Number of large UTXOs (> 100,000 sats)
    pub large_utxos: usize,
    /// Average UTXO value in satoshis
    pub average_value: u64,
    /// Total value in satoshis
    pub total_value: u64,
    /// Estimated consolidation cost at current fees
    pub consolidation_cost_sats: u64,
    /// Recommended action
    pub recommendation: HealthRecommendation,
}

/// Health-based recommendations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthRecommendation {
    /// UTXO set is healthy, no action needed
    Healthy,
    /// Consider consolidation during low fee periods
    ConsiderConsolidation,
    /// Consolidation recommended soon
    ConsolidationRecommended,
    /// Urgent consolidation needed
    UrgentConsolidation,
    /// Too many dust UTXOs, consider abandoning or consolidating
    ExcessiveDust,
}

/// UTXO optimizer for long-term health management
#[allow(dead_code)]
pub struct UtxoOptimizer {
    /// Fee rate thresholds (sat/vB)
    low_fee_threshold: u64,
    medium_fee_threshold: u64,
    high_fee_threshold: u64,

    /// UTXO count thresholds
    max_healthy_utxos: usize,
    max_acceptable_utxos: usize,

    /// Value thresholds (satoshis)
    dust_threshold: u64,
    small_utxo_threshold: u64,
    medium_utxo_threshold: u64,
}

impl UtxoOptimizer {
    /// Create a new UTXO optimizer with default thresholds
    pub fn new() -> Self {
        Self {
            low_fee_threshold: 5,     // < 5 sat/vB
            medium_fee_threshold: 20, // < 20 sat/vB
            high_fee_threshold: 50,   // < 50 sat/vB
            max_healthy_utxos: 20,    // Ideal maximum
            max_acceptable_utxos: 50, // Warning threshold
            dust_threshold: 546,      // Standard dust limit
            small_utxo_threshold: 10_000,
            medium_utxo_threshold: 100_000,
        }
    }

    /// Analyze UTXO set health
    pub fn analyze_health(&self, utxos: &[Utxo], current_fee_rate: u64) -> UtxoSetHealth {
        let total_utxos = utxos.len();
        let mut dust_count = 0;
        let mut economical_count = 0;
        let mut small_count = 0;
        let mut medium_count = 0;
        let mut large_count = 0;
        let mut total_value = 0u64;

        for utxo in utxos {
            total_value += utxo.amount_sats;

            // Check if dust
            if is_dust(utxo.amount_sats, current_fee_rate) {
                dust_count += 1;
                continue;
            }

            // Check if economical to spend
            let input_size = 68; // Approximate P2WPKH input size
            let cost_to_spend = calculate_fee_from_rate(input_size, current_fee_rate);

            if utxo.amount_sats > cost_to_spend * 2 {
                // Profitable with 2x margin
                economical_count += 1;
            }

            // Categorize by size
            if utxo.amount_sats < self.small_utxo_threshold {
                small_count += 1;
            } else if utxo.amount_sats < self.medium_utxo_threshold {
                medium_count += 1;
            } else {
                large_count += 1;
            }
        }

        let average_value = if total_utxos > 0 {
            total_value / total_utxos as u64
        } else {
            0
        };

        // Estimate consolidation cost (consolidate all into 1 output)
        // Using P2WPKH: input = 68 vbytes, output = 31 vbytes
        let consolidation_vsize = estimate_simple_transaction_vsize(
            total_utxos as u64,
            1,
            68, // P2WPKH input size
            31, // P2WPKH output size
        );
        let consolidation_cost = calculate_fee_from_rate(consolidation_vsize, current_fee_rate);

        // Calculate health score
        let overall_score = self.calculate_health_score(
            total_utxos,
            dust_count,
            economical_count,
            total_value,
            consolidation_cost,
        );

        // Generate recommendation
        let recommendation =
            self.generate_recommendation(&overall_score, total_utxos, dust_count, current_fee_rate);

        UtxoSetHealth {
            overall_score,
            total_utxos,
            dust_utxos: dust_count,
            economical_utxos: economical_count,
            small_utxos: small_count,
            medium_utxos: medium_count,
            large_utxos: large_count,
            average_value,
            total_value,
            consolidation_cost_sats: consolidation_cost,
            recommendation,
        }
    }

    /// Calculate overall health score
    fn calculate_health_score(
        &self,
        total_utxos: usize,
        dust_count: usize,
        economical_count: usize,
        total_value: u64,
        consolidation_cost: u64,
    ) -> HealthScore {
        let mut score = 100u8;

        // Penalize for too many UTXOs
        if total_utxos > self.max_acceptable_utxos {
            score = score.saturating_sub(30);
        } else if total_utxos > self.max_healthy_utxos {
            let excess = total_utxos - self.max_healthy_utxos;
            let penalty = (excess * 2).min(20) as u8;
            score = score.saturating_sub(penalty);
        }

        // Penalize for dust
        if dust_count > 0 {
            let dust_ratio = (dust_count * 100) / total_utxos.max(1);
            let penalty = (dust_ratio / 2).min(25) as u8;
            score = score.saturating_sub(penalty);
        }

        // Penalize for uneconomical UTXOs
        if let Some(economical_ratio) = (economical_count * 100).checked_div(total_utxos) {
            if economical_ratio < 50 {
                score = score.saturating_sub(15);
            } else if economical_ratio < 75 {
                score = score.saturating_sub(5);
            }
        }

        // Penalize if consolidation cost is high relative to total value
        if let Some(cost_ratio) = (consolidation_cost * 100).checked_div(total_value) {
            if cost_ratio > 10 {
                score = score.saturating_sub(20);
            } else if cost_ratio > 5 {
                score = score.saturating_sub(10);
            }
        }

        HealthScore::new(score)
    }

    /// Generate health-based recommendation
    fn generate_recommendation(
        &self,
        score: &HealthScore,
        total_utxos: usize,
        dust_count: usize,
        current_fee_rate: u64,
    ) -> HealthRecommendation {
        // Check for excessive dust first
        if dust_count > 10 || (total_utxos > 0 && (dust_count * 100) / total_utxos > 20) {
            return HealthRecommendation::ExcessiveDust;
        }

        // Check urgency based on score and UTXO count
        if score.is_critical() || total_utxos > self.max_acceptable_utxos * 2 {
            return HealthRecommendation::UrgentConsolidation;
        }

        if score.is_poor() || total_utxos > self.max_acceptable_utxos {
            return HealthRecommendation::ConsolidationRecommended;
        }

        if total_utxos > self.max_healthy_utxos && current_fee_rate < self.low_fee_threshold {
            return HealthRecommendation::ConsiderConsolidation;
        }

        HealthRecommendation::Healthy
    }

    /// Check if now is a good time to consolidate
    pub fn should_consolidate(&self, health: &UtxoSetHealth, current_fee_rate: u64) -> bool {
        match health.recommendation {
            HealthRecommendation::UrgentConsolidation => true,
            HealthRecommendation::ConsolidationRecommended => {
                current_fee_rate < self.medium_fee_threshold
            }
            HealthRecommendation::ConsiderConsolidation | HealthRecommendation::ExcessiveDust => {
                current_fee_rate < self.low_fee_threshold
            }
            HealthRecommendation::Healthy => false,
        }
    }

    /// Get optimal consolidation target count
    pub fn optimal_target_count(&self, current_count: usize) -> usize {
        if current_count <= self.max_healthy_utxos {
            current_count
        } else if current_count <= self.max_acceptable_utxos {
            self.max_healthy_utxos
        } else {
            // Aggressive consolidation for very fragmented sets
            (self.max_healthy_utxos / 2).max(5)
        }
    }

    /// Track UTXO health over time
    pub fn create_health_trend(&self, history: &[UtxoSetHealth]) -> HealthTrend {
        if history.is_empty() {
            return HealthTrend {
                trend: TrendDirection::Stable,
                score_change: 0,
                utxo_count_change: 0,
            };
        }

        let latest = &history[history.len() - 1];
        let earliest = &history[0];

        let score_change =
            latest.overall_score.value() as i16 - earliest.overall_score.value() as i16;
        let utxo_count_change = latest.total_utxos as i64 - earliest.total_utxos as i64;

        let trend = if score_change > 10 {
            TrendDirection::Improving
        } else if score_change < -10 {
            TrendDirection::Degrading
        } else {
            TrendDirection::Stable
        };

        HealthTrend {
            trend,
            score_change,
            utxo_count_change,
        }
    }
}

impl Default for UtxoOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Health trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthTrend {
    /// Trend direction
    pub trend: TrendDirection,
    /// Change in health score
    pub score_change: i16,
    /// Change in UTXO count
    pub utxo_count_change: i64,
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Health is improving
    Improving,
    /// Health is stable
    Stable,
    /// Health is degrading
    Degrading,
}

/// Proactive consolidation scheduler
pub struct ConsolidationScheduler {
    optimizer: UtxoOptimizer,
    last_check: Option<u64>,
    check_interval_secs: u64,
}

impl ConsolidationScheduler {
    /// Create a new consolidation scheduler with hourly check interval
    pub fn new() -> Self {
        Self {
            optimizer: UtxoOptimizer::new(),
            last_check: None,
            check_interval_secs: 3600, // Check hourly
        }
    }

    /// Check if it's time for a scheduled check
    pub fn should_check(&mut self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        if let Some(last) = self.last_check {
            if now - last >= self.check_interval_secs {
                self.last_check = Some(now);
                true
            } else {
                false
            }
        } else {
            self.last_check = Some(now);
            true
        }
    }

    /// Evaluate if consolidation should be triggered
    pub fn evaluate(&self, utxos: &[Utxo], current_fee_rate: u64) -> ConsolidationDecision {
        let health = self.optimizer.analyze_health(utxos, current_fee_rate);
        let should_consolidate = self.optimizer.should_consolidate(&health, current_fee_rate);
        let reason = self.get_decision_reason(&health, current_fee_rate);

        ConsolidationDecision {
            should_consolidate,
            health,
            reason,
        }
    }

    fn get_decision_reason(&self, health: &UtxoSetHealth, current_fee_rate: u64) -> String {
        match &health.recommendation {
            HealthRecommendation::Healthy => "UTXO set is healthy".to_string(),
            HealthRecommendation::ConsiderConsolidation => {
                format!(
                    "Low fees ({} sat/vB), good time to consolidate {} UTXOs",
                    current_fee_rate, health.total_utxos
                )
            }
            HealthRecommendation::ConsolidationRecommended => {
                format!(
                    "UTXO count ({}) exceeds healthy threshold, consolidation recommended",
                    health.total_utxos
                )
            }
            HealthRecommendation::UrgentConsolidation => {
                format!(
                    "Critical: {} UTXOs with health score {}, urgent consolidation needed",
                    health.total_utxos,
                    health.overall_score.value()
                )
            }
            HealthRecommendation::ExcessiveDust => {
                format!(
                    "Excessive dust: {} dust UTXOs out of {}",
                    health.dust_utxos, health.total_utxos
                )
            }
        }
    }
}

impl Default for ConsolidationScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Consolidation decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationDecision {
    /// Whether consolidation is recommended at this time
    pub should_consolidate: bool,
    /// Current UTXO set health metrics
    pub health: UtxoSetHealth,
    /// Human-readable explanation of the decision
    pub reason: String,
}

/// Fee market predictor for consolidation timing
pub struct FeeMarketPredictor {
    fee_history: Vec<FeeDataPoint>,
    max_history: usize,
}

impl FeeMarketPredictor {
    /// Create a new fee market predictor with 1 week of history capacity
    pub fn new() -> Self {
        Self {
            fee_history: Vec::new(),
            max_history: 168, // 1 week of hourly data
        }
    }

    /// Record a fee rate observation
    pub fn record_fee_rate(&mut self, fee_rate: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        self.fee_history.push(FeeDataPoint {
            timestamp: now,
            fee_rate,
        });

        // Keep only recent history
        if self.fee_history.len() > self.max_history {
            self.fee_history.remove(0);
        }
    }

    /// Predict if fees are likely to spike soon
    pub fn predict_fee_spike(&self) -> bool {
        if self.fee_history.len() < 24 {
            // Not enough data
            return false;
        }

        // Check if fees are trending upward
        let recent_avg = self.average_recent_fees(6);
        let earlier_avg = self.average_earlier_fees(6, 18);

        // If recent fees are 50% higher than earlier fees, spike likely
        recent_avg > earlier_avg * 15 / 10
    }

    /// Get average fee rate for recent hours
    fn average_recent_fees(&self, hours: usize) -> u64 {
        let count = hours.min(self.fee_history.len());
        if count == 0 {
            return 0;
        }

        let recent: Vec<_> = self.fee_history.iter().rev().take(count).collect();
        let sum: u64 = recent.iter().map(|d| d.fee_rate).sum();
        sum / count as u64
    }

    /// Get average fee rate for earlier period
    fn average_earlier_fees(&self, hours: usize, offset: usize) -> u64 {
        if self.fee_history.len() < offset + hours {
            return 0;
        }

        let start = self.fee_history.len() - offset - hours;
        let end = self.fee_history.len() - offset;
        let period: Vec<_> = self.fee_history[start..end].to_vec();

        if period.is_empty() {
            return 0;
        }

        let sum: u64 = period.iter().map(|d| d.fee_rate).sum();
        sum / period.len() as u64
    }

    /// Get current fee percentile (0-100)
    pub fn current_fee_percentile(&self) -> u8 {
        if self.fee_history.is_empty() {
            return 50;
        }

        let latest = self
            .fee_history
            .last()
            .expect("fee_history is non-empty per is_empty check above")
            .fee_rate;
        let mut sorted: Vec<_> = self.fee_history.iter().map(|d| d.fee_rate).collect();
        sorted.sort_unstable();

        let position = sorted
            .iter()
            .position(|&r| r >= latest)
            .unwrap_or(sorted.len() - 1);
        ((position * 100) / sorted.len().max(1)).min(100) as u8
    }
}

impl Default for FeeMarketPredictor {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee rate data point
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct FeeDataPoint {
    timestamp: u64,
    fee_rate: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_score() {
        let score = HealthScore::new(75);
        assert_eq!(score.value(), 75);
        assert!(!score.is_critical());
        assert!(!score.is_poor());
        assert!(score.is_good());
        assert!(!score.is_excellent());

        let critical = HealthScore::new(25);
        assert!(critical.is_critical());
        assert!(critical.is_poor());
    }

    #[test]
    fn test_health_score_clamping() {
        let score = HealthScore::new(150);
        assert_eq!(score.value(), 100);
    }

    #[test]
    fn test_optimizer_creation() {
        let optimizer = UtxoOptimizer::new();
        assert_eq!(optimizer.low_fee_threshold, 5);
        assert_eq!(optimizer.max_healthy_utxos, 20);
    }

    #[test]
    fn test_health_analysis() {
        use bitcoin::Txid;
        use std::str::FromStr;

        let optimizer = UtxoOptimizer::new();
        let utxos = vec![
            Utxo {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000001",
                )
                .unwrap(),
                vout: 0,
                address: "bc1qtest".to_string(),
                amount_sats: 100_000,
                confirmations: 6,
                spendable: true,
                safe: true,
            },
            Utxo {
                txid: Txid::from_str(
                    "0000000000000000000000000000000000000000000000000000000000000002",
                )
                .unwrap(),
                vout: 0,
                address: "bc1qtest2".to_string(),
                amount_sats: 50_000,
                confirmations: 6,
                spendable: true,
                safe: true,
            },
        ];

        let health = optimizer.analyze_health(&utxos, 10);
        assert_eq!(health.total_utxos, 2);
        assert_eq!(health.total_value, 150_000);
        assert!(health.overall_score.is_excellent());
    }

    #[test]
    fn test_optimal_target_count() {
        let optimizer = UtxoOptimizer::new();

        assert_eq!(optimizer.optimal_target_count(15), 15);
        assert_eq!(optimizer.optimal_target_count(25), 20);
        assert_eq!(optimizer.optimal_target_count(100), 10);
    }

    #[test]
    fn test_consolidation_scheduler() {
        let mut scheduler = ConsolidationScheduler::new();
        assert!(scheduler.should_check()); // First check should return true
    }

    #[test]
    fn test_fee_market_predictor() {
        let mut predictor = FeeMarketPredictor::new();

        // Add some fee data
        for rate in [5, 6, 7, 10, 15, 20] {
            predictor.record_fee_rate(rate);
        }

        assert_eq!(predictor.fee_history.len(), 6);
    }

    #[test]
    fn test_fee_percentile() {
        let mut predictor = FeeMarketPredictor::new();

        for rate in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
            predictor.record_fee_rate(rate);
        }

        // Latest is 10, should be at high percentile
        let percentile = predictor.current_fee_percentile();
        assert!(percentile > 80);
    }

    #[test]
    fn test_health_trend() {
        let optimizer = UtxoOptimizer::new();
        let history = vec![];

        let trend = optimizer.create_health_trend(&history);
        assert_eq!(trend.trend, TrendDirection::Stable);
    }
}
