//! Transaction graph obfuscation for chain analysis resistance
//!
//! Provides strategies to resist blockchain analysis by obfuscating transaction
//! patterns and timing.

use crate::btc_utils::round_for_privacy;
use crate::error::BitcoinError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction structure randomization options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureRandomization {
    /// Add decoy outputs
    pub add_decoy_outputs: bool,
    /// Randomize output order
    pub randomize_output_order: bool,
    /// Randomize input order
    pub randomize_input_order: bool,
    /// Add random nSequence values
    pub randomize_nsequence: bool,
}

impl Default for StructureRandomization {
    fn default() -> Self {
        Self {
            add_decoy_outputs: true,
            randomize_output_order: true,
            randomize_input_order: true,
            randomize_nsequence: false,
        }
    }
}

/// Amount obfuscation strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AmountObfuscation {
    /// No obfuscation
    None,
    /// Round to nearest power of 10
    RoundPowerOfTen,
    /// Round to specific denomination
    RoundDenomination {
        /// Denomination in satoshis to round to
        sats: u64,
    },
    /// Add random dust to amounts
    AddRandomDust {
        /// Maximum dust to add in satoshis
        max_dust: u64,
    },
}

/// Timing analysis resistance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingObfuscation {
    /// Add random delay before broadcasting (seconds)
    pub random_delay_secs: Option<(u64, u64)>, // (min, max)
    /// Broadcast at specific time of day (UTC hour)
    pub broadcast_hour: Option<u8>,
    /// Batch with other transactions
    pub batch_broadcast: bool,
}

impl Default for TimingObfuscation {
    fn default() -> Self {
        Self {
            random_delay_secs: Some((0, 300)), // 0-5 minutes
            broadcast_hour: None,
            batch_broadcast: false,
        }
    }
}

/// Transaction privacy enhancer
pub struct TransactionPrivacyEnhancer {
    structure_randomization: StructureRandomization,
    amount_obfuscation: AmountObfuscation,
    timing_obfuscation: TimingObfuscation,
}

impl TransactionPrivacyEnhancer {
    /// Create a new privacy enhancer
    pub fn new(
        structure_randomization: StructureRandomization,
        amount_obfuscation: AmountObfuscation,
        timing_obfuscation: TimingObfuscation,
    ) -> Self {
        Self {
            structure_randomization,
            amount_obfuscation,
            timing_obfuscation,
        }
    }

    /// Apply amount obfuscation
    pub fn obfuscate_amount(&self, amount: u64) -> u64 {
        match &self.amount_obfuscation {
            AmountObfuscation::None => amount,
            AmountObfuscation::RoundPowerOfTen => round_for_privacy(amount, 10_000),
            AmountObfuscation::RoundDenomination { sats } => {
                let remainder = amount % sats;
                if remainder < sats / 2 {
                    amount - remainder
                } else {
                    amount + (sats - remainder)
                }
            }
            AmountObfuscation::AddRandomDust { max_dust } => {
                use rand::RngExt;
                let mut rng = rand::rng();
                let dust = rng.random_range(0..*max_dust);
                amount + dust
            }
        }
    }

    /// Generate decoy output amount
    pub fn generate_decoy_amount(&self, total_amount: u64) -> u64 {
        use rand::RngExt;
        let mut rng = rand::rng();
        // Decoy should be between 1% and 20% of total
        let min = total_amount / 100;
        let max = total_amount / 5;
        if min >= max {
            return min;
        }
        rng.random_range(min..=max)
    }

    /// Check if decoy output should be added
    pub fn should_add_decoy(&self) -> bool {
        use rand::RngExt;
        self.structure_randomization.add_decoy_outputs && {
            let mut rng = rand::rng();
            rng.random_bool(0.3) // 30% chance
        }
    }

    /// Calculate broadcast delay in seconds
    pub fn calculate_broadcast_delay(&self) -> u64 {
        use rand::RngExt;
        if let Some((min, max)) = self.timing_obfuscation.random_delay_secs {
            let mut rng = rand::rng();
            rng.random_range(min..=max)
        } else {
            0
        }
    }

    /// Check if transaction should be batched
    pub fn should_batch(&self) -> bool {
        self.timing_obfuscation.batch_broadcast
    }
}

impl Default for TransactionPrivacyEnhancer {
    fn default() -> Self {
        Self::new(
            StructureRandomization::default(),
            AmountObfuscation::RoundPowerOfTen,
            TimingObfuscation::default(),
        )
    }
}

/// Change output strategy to resist fingerprinting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeStrategy {
    /// Standard change output
    Standard,
    /// Multiple change outputs to obfuscate
    Multiple {
        /// Number of change outputs to create
        count: usize,
    },
    /// Match payment output amount
    MatchPayment,
    /// Random amount split
    RandomSplit,
}

/// Change output generator
pub struct ChangeOutputGenerator {
    strategy: ChangeStrategy,
    min_change: u64,
}

impl ChangeOutputGenerator {
    /// Create a new change output generator
    pub fn new(strategy: ChangeStrategy, min_change: u64) -> Self {
        Self {
            strategy,
            min_change,
        }
    }

    /// Generate change outputs
    pub fn generate_change_outputs(
        &self,
        total_change: u64,
        payment_amount: Option<u64>,
    ) -> Result<Vec<u64>, BitcoinError> {
        if total_change < self.min_change {
            return Ok(Vec::new());
        }

        match &self.strategy {
            ChangeStrategy::Standard => Ok(vec![total_change]),
            ChangeStrategy::Multiple { count } => self.split_change_multiple(total_change, *count),
            ChangeStrategy::MatchPayment => {
                if let Some(payment) = payment_amount {
                    Ok(vec![payment, total_change.saturating_sub(payment)]).map(|outputs| {
                        if outputs[1] < self.min_change {
                            vec![total_change]
                        } else {
                            outputs
                        }
                    })
                } else {
                    Ok(vec![total_change])
                }
            }
            ChangeStrategy::RandomSplit => self.split_change_random(total_change),
        }
    }

    /// Split change into multiple outputs
    fn split_change_multiple(&self, total: u64, count: usize) -> Result<Vec<u64>, BitcoinError> {
        if count == 0 {
            return Err(BitcoinError::InvalidTransaction(
                "Count must be greater than 0".to_string(),
            ));
        }

        if count == 1 {
            return Ok(vec![total]);
        }

        let min_per_output = self.min_change;
        if total < min_per_output * count as u64 {
            return Ok(vec![total]);
        }

        use rand::RngExt;
        let mut outputs = Vec::new();
        let mut remaining = total;
        let mut rng = rand::rng();

        for i in 0..count {
            if i == count - 1 {
                // Last output gets remainder
                outputs.push(remaining);
            } else {
                let max_amount = remaining - (min_per_output * (count - i - 1) as u64);
                let amount = rng.random_range(min_per_output..=max_amount);
                outputs.push(amount);
                remaining -= amount;
            }
        }

        Ok(outputs)
    }

    /// Split change randomly
    fn split_change_random(&self, total: u64) -> Result<Vec<u64>, BitcoinError> {
        use rand::RngExt;
        let mut rng = rand::rng();
        let count = rng.random_range(1..=3);
        self.split_change_multiple(total, count)
    }
}

/// Transaction timing coordinator
pub struct TimingCoordinator {
    /// Pending transactions awaiting broadcast
    pending: HashMap<String, PendingBroadcast>,
}

/// Pending broadcast information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBroadcast {
    /// Transaction hex
    pub tx_hex: String,
    /// Scheduled broadcast time
    pub broadcast_at: chrono::DateTime<chrono::Utc>,
    /// Priority level
    pub priority: BroadcastPriority,
}

/// Broadcast priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BroadcastPriority {
    /// Low priority - can be delayed
    Low,
    /// Normal priority
    Normal,
    /// High priority - broadcast soon
    High,
}

impl TimingCoordinator {
    /// Create a new timing coordinator
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Schedule a transaction for broadcast
    pub fn schedule_broadcast(
        &mut self,
        tx_id: String,
        tx_hex: String,
        delay_secs: u64,
        priority: BroadcastPriority,
    ) {
        let broadcast_at = chrono::Utc::now() + chrono::Duration::seconds(delay_secs as i64);

        self.pending.insert(
            tx_id,
            PendingBroadcast {
                tx_hex,
                broadcast_at,
                priority,
            },
        );
    }

    /// Get transactions ready for broadcast
    pub fn get_ready_broadcasts(&mut self) -> Vec<(String, String)> {
        let now = chrono::Utc::now();
        let mut ready = Vec::new();

        let ready_ids: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.broadcast_at <= now)
            .map(|(id, _)| id.clone())
            .collect();

        for id in ready_ids {
            if let Some(pending) = self.pending.remove(&id) {
                ready.push((id, pending.tx_hex));
            }
        }

        ready
    }

    /// Cancel a scheduled broadcast
    pub fn cancel_broadcast(&mut self, tx_id: &str) -> bool {
        self.pending.remove(tx_id).is_some()
    }

    /// Get number of pending broadcasts
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for TimingCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Fingerprinting resistance analyzer
pub struct FingerprintingAnalyzer;

impl FingerprintingAnalyzer {
    /// Analyze transaction for fingerprinting issues
    #[allow(dead_code)]
    pub fn analyze_fingerprints(
        &self,
        input_count: usize,
        output_count: usize,
        output_amounts: &[u64],
    ) -> Vec<FingerprintingIssue> {
        let mut issues = Vec::new();

        // Check for exact round numbers
        for amount in output_amounts {
            if self.is_exact_round_number(*amount) {
                issues.push(FingerprintingIssue::RoundNumber { amount: *amount });
            }
        }

        // Check for common patterns
        if output_count == 2 && input_count == 1 {
            issues.push(FingerprintingIssue::SimplePayment);
        }

        // Check for duplicate amounts
        let mut amount_counts: HashMap<u64, usize> = HashMap::new();
        for amount in output_amounts {
            *amount_counts.entry(*amount).or_insert(0) += 1;
        }

        for (amount, count) in amount_counts {
            if count > 1 {
                issues.push(FingerprintingIssue::DuplicateAmount { amount, count });
            }
        }

        issues
    }

    /// Check if amount is an exact round number
    fn is_exact_round_number(&self, amount: u64) -> bool {
        if amount == 0 {
            return false;
        }

        // Check if divisible by 1 BTC, 0.1 BTC, 0.01 BTC, etc.
        let btc = 100_000_000u64;
        for divisor in [btc, btc / 10, btc / 100, btc / 1000] {
            if amount % divisor == 0 {
                return true;
            }
        }

        false
    }
}

/// Fingerprinting issues detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FingerprintingIssue {
    /// Amount is a round number
    RoundNumber {
        /// The round amount in satoshis
        amount: u64,
    },
    /// Simple 1-input, 2-output pattern
    SimplePayment,
    /// Duplicate output amounts
    DuplicateAmount {
        /// The duplicated amount in satoshis
        amount: u64,
        /// Number of outputs with this amount
        count: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_obfuscation_none() {
        let enhancer = TransactionPrivacyEnhancer::new(
            StructureRandomization::default(),
            AmountObfuscation::None,
            TimingObfuscation::default(),
        );

        assert_eq!(enhancer.obfuscate_amount(12345), 12345);
    }

    #[test]
    fn test_amount_obfuscation_round() {
        let enhancer = TransactionPrivacyEnhancer::new(
            StructureRandomization::default(),
            AmountObfuscation::RoundPowerOfTen,
            TimingObfuscation::default(),
        );

        let result = enhancer.obfuscate_amount(12345);
        assert!(result % 10000 == 0);
    }

    #[test]
    fn test_change_output_standard() {
        let generator = ChangeOutputGenerator::new(ChangeStrategy::Standard, 546);
        let outputs = generator.generate_change_outputs(100_000, None).unwrap();

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], 100_000);
    }

    #[test]
    fn test_change_output_multiple() {
        let generator = ChangeOutputGenerator::new(ChangeStrategy::Multiple { count: 2 }, 546);
        let outputs = generator.generate_change_outputs(100_000, None).unwrap();

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs.iter().sum::<u64>(), 100_000);
    }

    #[test]
    fn test_timing_coordinator() {
        let mut coordinator = TimingCoordinator::new();

        coordinator.schedule_broadcast(
            "tx1".to_string(),
            "hex1".to_string(),
            0,
            BroadcastPriority::Normal,
        );

        assert_eq!(coordinator.pending_count(), 1);

        let ready = coordinator.get_ready_broadcasts();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].0, "tx1");
    }

    #[test]
    fn test_fingerprinting_analyzer() {
        let analyzer = FingerprintingAnalyzer;
        let issues = analyzer.analyze_fingerprints(
            1,
            2,
            &[100_000_000, 50_000_000], // 1 BTC and 0.5 BTC
        );

        assert!(!issues.is_empty());
    }

    #[test]
    fn test_broadcast_priority() {
        let low = BroadcastPriority::Low;
        let high = BroadcastPriority::High;

        assert_ne!(low, high);
    }

    #[test]
    fn test_structure_randomization_default() {
        let config = StructureRandomization::default();
        assert!(config.add_decoy_outputs);
        assert!(config.randomize_output_order);
        assert!(config.randomize_input_order);
    }
}
