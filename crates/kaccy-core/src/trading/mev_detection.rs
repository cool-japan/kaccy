//! MEV (Maximal Extractable Value) Detection System
//!
//! This module provides sophisticated detection mechanisms for various MEV attack patterns,
//! with a focus on sandwich attacks which are one of the most common forms of MEV extraction.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// Types of MEV attacks that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MevAttackType {
    /// Sandwich attack (front-run + back-run)
    Sandwich,
    /// Front-running only
    FrontRun,
    /// Back-running only
    BackRun,
    /// Just-in-time (JIT) liquidity provision
    JitLiquidity,
    /// Liquidation sniping
    LiquidationSnipe,
}

/// Detection result for a potential MEV attack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevDetection {
    /// Type of attack detected
    pub attack_type: MevAttackType,
    /// Suspected attacker user ID
    pub attacker_user_id: String,
    /// Victim transaction/user ID (if applicable)
    pub victim_user_id: Option<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: Decimal,
    /// Estimated profit extracted (in BTC)
    pub estimated_profit: Decimal,
    /// Timestamp of detection
    pub detected_at: SystemTime,
    /// Transaction IDs involved
    pub transaction_ids: Vec<String>,
    /// Additional evidence/metadata
    pub evidence: HashMap<String, String>,
}

/// Transaction pattern for MEV detection
#[derive(Debug, Clone)]
pub struct TransactionPattern {
    /// Unique identifier of the transaction.
    pub tx_id: String,
    /// User who submitted the transaction.
    pub user_id: String,
    /// Token being traded in this transaction.
    pub token_id: String,
    /// Whether the transaction is a buy (`true`) or sell (`false`).
    pub is_buy: bool,
    /// Trade amount.
    pub amount: Decimal,
    /// Execution price.
    pub price: Decimal,
    /// Timestamp of the transaction.
    pub timestamp: SystemTime,
    /// Gas price used (relevant on EVM chains).
    pub gas_price: Option<Decimal>,
}

/// Sandwich attack detector
///
/// Detects sandwich attacks by analyzing transaction patterns:
/// 1. Front-run: Attacker buys before victim
/// 2. Victim: User executes their intended trade
/// 3. Back-run: Attacker sells after victim
#[derive(Debug)]
pub struct SandwichDetector {
    /// Recent transactions window for pattern matching
    transaction_window: VecDeque<TransactionPattern>,
    /// Maximum time window to consider for sandwich detection
    detection_window: Duration,
    /// Minimum price impact to consider (in percentage)
    min_price_impact: Decimal,
    /// Minimum profit threshold for flagging
    min_profit_threshold: Decimal,
}

impl SandwichDetector {
    /// Creates a new sandwich attack detector
    pub fn new(
        detection_window: Duration,
        min_price_impact: Decimal,
        min_profit_threshold: Decimal,
    ) -> Self {
        Self {
            transaction_window: VecDeque::new(),
            detection_window,
            min_price_impact,
            min_profit_threshold,
        }
    }

    /// Creates a detector with default parameters
    pub fn with_defaults() -> Self {
        Self::new(
            Duration::from_secs(60), // 1 minute window
            Decimal::new(50, 4),     // 0.5% minimum price impact
            Decimal::new(1, 3),      // 0.001 BTC minimum profit
        )
    }

    /// Adds a transaction to the detection window
    pub fn add_transaction(&mut self, tx: TransactionPattern) {
        // Remove old transactions outside the detection window
        let cutoff_time = tx.timestamp - self.detection_window;
        while let Some(front) = self.transaction_window.front() {
            if front.timestamp < cutoff_time {
                self.transaction_window.pop_front();
            } else {
                break;
            }
        }

        // Add new transaction
        self.transaction_window.push_back(tx);
    }

    /// Analyzes recent transactions for sandwich attack patterns
    pub fn detect_sandwich_attacks(&self) -> Vec<MevDetection> {
        let mut detections = Vec::new();

        // Group transactions by token
        let mut token_txs: HashMap<String, Vec<&TransactionPattern>> = HashMap::new();
        for tx in &self.transaction_window {
            token_txs.entry(tx.token_id.clone()).or_default().push(tx);
        }

        // Analyze each token's transactions
        for (token_id, txs) in token_txs {
            if txs.len() < 3 {
                continue; // Need at least 3 transactions for sandwich pattern
            }

            // Look for sandwich patterns: Buy -> Victim Trade -> Sell
            for i in 0..txs.len().saturating_sub(2) {
                if let Some(detection) = self.check_sandwich_pattern(&txs[i..=i + 2], &token_id) {
                    detections.push(detection);
                }
            }
        }

        detections
    }

    /// Checks if three consecutive transactions form a sandwich pattern
    fn check_sandwich_pattern(
        &self,
        txs: &[&TransactionPattern],
        _token_id: &str,
    ) -> Option<MevDetection> {
        if txs.len() != 3 {
            return None;
        }

        let (front_run, victim, back_run) = (txs[0], txs[1], txs[2]);

        // Pattern: front-run (buy) -> victim (buy) -> back-run (sell)
        // or: front-run (sell) -> victim (sell) -> back-run (buy)

        // Check for buy sandwich
        if front_run.is_buy && victim.is_buy && !back_run.is_buy {
            // Same attacker for front-run and back-run
            if front_run.user_id != back_run.user_id {
                return None;
            }

            // Different victim
            if victim.user_id == front_run.user_id {
                return None;
            }

            // Check timing - should be close together
            let time_between_front_victim =
                victim.timestamp.duration_since(front_run.timestamp).ok()?;
            let time_between_victim_back =
                back_run.timestamp.duration_since(victim.timestamp).ok()?;

            if time_between_front_victim > Duration::from_secs(30)
                || time_between_victim_back > Duration::from_secs(30)
            {
                return None;
            }

            // Calculate price impact and profit
            let price_increase =
                ((victim.price - front_run.price) / front_run.price) * Decimal::new(100, 0);
            if price_increase < self.min_price_impact {
                return None;
            }

            // Estimate attacker profit
            let buy_cost = front_run.amount * front_run.price;
            let sell_revenue = back_run.amount * back_run.price;
            let profit = sell_revenue - buy_cost;

            if profit < self.min_profit_threshold {
                return None;
            }

            // Build evidence
            let mut evidence = HashMap::new();
            evidence.insert(
                "price_impact".to_string(),
                format!("{:.2}%", price_increase),
            );
            evidence.insert("front_run_price".to_string(), front_run.price.to_string());
            evidence.insert("victim_price".to_string(), victim.price.to_string());
            evidence.insert("back_run_price".to_string(), back_run.price.to_string());
            evidence.insert(
                "time_span".to_string(),
                format!("{:?}", time_between_front_victim + time_between_victim_back),
            );

            // Calculate confidence based on various factors
            let mut confidence = Decimal::new(70, 2); // Base 0.70

            // Higher confidence if gas price indicates priority
            if let (Some(front_gas), Some(victim_gas), Some(back_gas)) =
                (front_run.gas_price, victim.gas_price, back_run.gas_price)
            {
                if front_gas > victim_gas && back_gas > victim_gas {
                    confidence += Decimal::new(15, 2); // +0.15
                    evidence.insert("gas_price_manipulation".to_string(), "true".to_string());
                }
            }

            // Higher confidence if profit is significant
            let profit_ratio = profit / buy_cost;
            if profit_ratio > Decimal::new(5, 2) {
                confidence += Decimal::new(10, 2); // +0.10
            }

            // Cap confidence at 1.0
            confidence = confidence.min(Decimal::ONE);

            return Some(MevDetection {
                attack_type: MevAttackType::Sandwich,
                attacker_user_id: front_run.user_id.clone(),
                victim_user_id: Some(victim.user_id.clone()),
                confidence,
                estimated_profit: profit,
                detected_at: SystemTime::now(),
                transaction_ids: vec![
                    front_run.tx_id.clone(),
                    victim.tx_id.clone(),
                    back_run.tx_id.clone(),
                ],
                evidence,
            });
        }

        None
    }

    /// Detects front-running patterns (without back-run)
    pub fn detect_front_running(&self) -> Vec<MevDetection> {
        let mut detections = Vec::new();

        // Group transactions by token
        let mut token_txs: HashMap<String, Vec<&TransactionPattern>> = HashMap::new();
        for tx in &self.transaction_window {
            token_txs.entry(tx.token_id.clone()).or_default().push(tx);
        }

        // Analyze pairs of transactions
        for (_token_id, txs) in token_txs {
            if txs.len() < 2 {
                continue;
            }

            for i in 0..txs.len() - 1 {
                let (front, victim) = (txs[i], txs[i + 1]);

                // Check timing
                let time_diff = victim.timestamp.duration_since(front.timestamp).ok();
                if time_diff.is_none()
                    || time_diff.expect("time_diff is Some per is_none check")
                        > Duration::from_secs(10)
                {
                    continue;
                }

                // Same direction trade
                if front.is_buy != victim.is_buy {
                    continue;
                }

                // Different users
                if front.user_id == victim.user_id {
                    continue;
                }

                // Front-runner likely used higher gas price
                let gas_priority = if let (Some(front_gas), Some(victim_gas)) =
                    (front.gas_price, victim.gas_price)
                {
                    front_gas > victim_gas
                } else {
                    false
                };

                // Calculate price impact
                let price_change =
                    ((victim.price - front.price).abs() / front.price) * Decimal::new(100, 0);
                if price_change < self.min_price_impact {
                    continue;
                }

                // Estimate benefit to front-runner
                let benefit = if front.is_buy {
                    // Buy before price increase
                    front.amount * (victim.price - front.price)
                } else {
                    // Sell before price decrease
                    front.amount * (front.price - victim.price)
                };

                if benefit <= Decimal::ZERO {
                    continue;
                }

                let mut evidence = HashMap::new();
                evidence.insert("price_impact".to_string(), format!("{:.2}%", price_change));
                evidence.insert("front_run_price".to_string(), front.price.to_string());
                evidence.insert("victim_price".to_string(), victim.price.to_string());
                evidence.insert("gas_priority".to_string(), gas_priority.to_string());

                let confidence = if gas_priority {
                    Decimal::new(75, 2) // 0.75
                } else {
                    Decimal::new(60, 2) // 0.60
                };

                detections.push(MevDetection {
                    attack_type: MevAttackType::FrontRun,
                    attacker_user_id: front.user_id.clone(),
                    victim_user_id: Some(victim.user_id.clone()),
                    confidence,
                    estimated_profit: benefit,
                    detected_at: SystemTime::now(),
                    transaction_ids: vec![front.tx_id.clone(), victim.tx_id.clone()],
                    evidence,
                });
            }
        }

        detections
    }

    /// Gets statistics about the current transaction window
    pub fn get_stats(&self) -> DetectorStats {
        DetectorStats {
            transaction_count: self.transaction_window.len(),
            unique_users: self
                .transaction_window
                .iter()
                .map(|tx| tx.user_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            unique_tokens: self
                .transaction_window
                .iter()
                .map(|tx| tx.token_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
        }
    }
}

/// Statistics about the MEV detector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStats {
    /// Total number of transactions analysed.
    pub transaction_count: usize,
    /// Number of distinct users seen.
    pub unique_users: usize,
    /// Number of distinct tokens seen.
    pub unique_tokens: usize,
}

/// MEV victim transaction tracker
///
/// Tracks transactions that were victims of MEV attacks for analysis and compensation
#[derive(Debug)]
pub struct VictimTracker {
    victims: HashMap<String, Vec<MevDetection>>,
}

impl VictimTracker {
    /// Creates a new empty `VictimTracker`.
    pub fn new() -> Self {
        Self {
            victims: HashMap::new(),
        }
    }

    /// Records a MEV detection with victim information
    pub fn record_attack(&mut self, detection: MevDetection) {
        if let Some(victim_id) = &detection.victim_user_id {
            self.victims
                .entry(victim_id.clone())
                .or_default()
                .push(detection);
        }
    }

    /// Gets all attacks where the user was a victim
    pub fn get_victim_attacks(&self, user_id: &str) -> Vec<&MevDetection> {
        self.victims
            .get(user_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Calculates total estimated loss for a victim
    pub fn calculate_total_loss(&self, user_id: &str) -> Decimal {
        self.victims
            .get(user_id)
            .map(|attacks| attacks.iter().map(|a| a.estimated_profit).sum())
            .unwrap_or(Decimal::ZERO)
    }

    /// Gets the most frequent attackers against a specific victim
    pub fn get_frequent_attackers(&self, user_id: &str, limit: usize) -> Vec<(String, usize)> {
        let attacks = match self.victims.get(user_id) {
            Some(a) => a,
            None => return Vec::new(),
        };

        let mut attacker_counts: HashMap<String, usize> = HashMap::new();
        for attack in attacks {
            *attacker_counts
                .entry(attack.attacker_user_id.clone())
                .or_insert(0) += 1;
        }

        let mut sorted: Vec<_> = attacker_counts.into_iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
        sorted.truncate(limit);
        sorted
    }
}

impl Default for VictimTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tx(
        tx_id: &str,
        user_id: &str,
        token_id: &str,
        is_buy: bool,
        amount: i64,
        price: i64,
        timestamp_offset: u64,
    ) -> TransactionPattern {
        TransactionPattern {
            tx_id: tx_id.to_string(),
            user_id: user_id.to_string(),
            token_id: token_id.to_string(),
            is_buy,
            amount: Decimal::new(amount, 2),
            price: Decimal::new(price, 4),
            timestamp: SystemTime::now() - Duration::from_secs(timestamp_offset),
            gas_price: Some(Decimal::new(50, 0)),
        }
    }

    #[test]
    fn test_sandwich_detection() {
        let mut detector = SandwichDetector::with_defaults();

        // Create a sandwich attack pattern
        // Front-run: Attacker buys at 1.0
        detector.add_transaction(create_test_tx(
            "tx1", "attacker", "token1", true, 100, 10000, 30,
        ));

        // Victim: User buys at 1.1 (price increased)
        detector.add_transaction(create_test_tx(
            "tx2", "victim", "token1", true, 50, 11000, 20,
        ));

        // Back-run: Attacker sells at 1.1
        detector.add_transaction(create_test_tx(
            "tx3", "attacker", "token1", false, 100, 11000, 10,
        ));

        let detections = detector.detect_sandwich_attacks();
        assert_eq!(detections.len(), 1);

        let detection = &detections[0];
        assert_eq!(detection.attack_type, MevAttackType::Sandwich);
        assert_eq!(detection.attacker_user_id, "attacker");
        assert_eq!(detection.victim_user_id.as_deref(), Some("victim"));
        assert!(detection.confidence > Decimal::ZERO);
        assert!(detection.estimated_profit > Decimal::ZERO);
    }

    #[test]
    fn test_front_running_detection() {
        let mut detector = SandwichDetector::with_defaults();

        // Front-run: Attacker buys at 1.0 with high gas
        let mut front_tx = create_test_tx("tx1", "attacker", "token1", true, 100, 10000, 10);
        front_tx.gas_price = Some(Decimal::new(100, 0));

        // Victim: User buys at 1.1 with normal gas
        let mut victim_tx = create_test_tx("tx2", "victim", "token1", true, 50, 11000, 5);
        victim_tx.gas_price = Some(Decimal::new(50, 0));

        detector.add_transaction(front_tx);
        detector.add_transaction(victim_tx);

        let detections = detector.detect_front_running();
        assert!(!detections.is_empty());

        let detection = &detections[0];
        assert_eq!(detection.attack_type, MevAttackType::FrontRun);
    }

    #[test]
    fn test_victim_tracker() {
        let mut tracker = VictimTracker::new();

        let detection = MevDetection {
            attack_type: MevAttackType::Sandwich,
            attacker_user_id: "attacker".to_string(),
            victim_user_id: Some("victim".to_string()),
            confidence: Decimal::new(85, 2),
            estimated_profit: Decimal::new(100, 2),
            detected_at: SystemTime::now(),
            transaction_ids: vec!["tx1".to_string(), "tx2".to_string(), "tx3".to_string()],
            evidence: HashMap::new(),
        };

        tracker.record_attack(detection);

        let victim_attacks = tracker.get_victim_attacks("victim");
        assert_eq!(victim_attacks.len(), 1);

        let total_loss = tracker.calculate_total_loss("victim");
        assert_eq!(total_loss, Decimal::new(100, 2));
    }

    #[test]
    fn test_detector_stats() {
        let mut detector = SandwichDetector::with_defaults();

        detector.add_transaction(create_test_tx(
            "tx1", "user1", "token1", true, 100, 10000, 10,
        ));
        detector.add_transaction(create_test_tx(
            "tx2", "user2", "token1", false, 50, 11000, 5,
        ));
        detector.add_transaction(create_test_tx("tx3", "user1", "token2", true, 75, 12000, 2));

        let stats = detector.get_stats();
        assert_eq!(stats.transaction_count, 3);
        assert_eq!(stats.unique_users, 2);
        assert_eq!(stats.unique_tokens, 2);
    }

    #[test]
    fn test_no_false_positive_same_user() {
        let mut detector = SandwichDetector::with_defaults();

        // Same user making multiple trades - should not be flagged
        detector.add_transaction(create_test_tx(
            "tx1", "user1", "token1", true, 100, 10000, 30,
        ));
        detector.add_transaction(create_test_tx(
            "tx2", "user1", "token1", true, 50, 11000, 20,
        ));
        detector.add_transaction(create_test_tx(
            "tx3", "user1", "token1", false, 100, 11000, 10,
        ));

        let detections = detector.detect_sandwich_attacks();
        assert_eq!(detections.len(), 0);
    }
}
