//! Suspicious Activity Detection Module
//!
//! This module monitors for suspicious transaction patterns including
//! large transactions and rapid succession of transactions.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Suspicious activity type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityType {
    /// Large single transaction
    LargeTransaction,
    /// Rapid succession of transactions
    RapidTransactions,
    /// Unusual pattern (e.g., round-number amounts)
    UnusualPattern,
    /// Velocity limit exceeded (too much volume too fast)
    VelocityExceeded,
}

/// Suspicious activity alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousActivity {
    /// User ID
    pub user_id: String,
    /// Activity type
    pub activity_type: ActivityType,
    /// Description
    pub description: String,
    /// Transaction IDs involved
    pub txids: Vec<String>,
    /// Total amount involved (in satoshis)
    pub amount_sats: u64,
    /// When detected
    pub detected_at: DateTime<Utc>,
    /// Risk score (0-100)
    pub risk_score: u8,
}

/// Activity monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMonitorConfig {
    /// Large transaction threshold (in satoshis)
    pub large_tx_threshold_sats: u64,
    /// Rapid transaction count threshold
    pub rapid_tx_count: u32,
    /// Rapid transaction time window (seconds)
    pub rapid_tx_window_secs: i64,
    /// Velocity limit (satoshis per hour)
    pub velocity_limit_sats_per_hour: u64,
    /// Minimum risk score to trigger alert
    pub min_risk_score: u8,
}

impl Default for ActivityMonitorConfig {
    fn default() -> Self {
        Self {
            large_tx_threshold_sats: 10_000_000, // 0.1 BTC
            rapid_tx_count: 5,
            rapid_tx_window_secs: 300,                // 5 minutes
            velocity_limit_sats_per_hour: 50_000_000, // 0.5 BTC per hour
            min_risk_score: 50,
        }
    }
}

/// Transaction record for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    /// Transaction ID
    pub txid: String,
    /// User ID
    pub user_id: String,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Activity monitor
pub struct ActivityMonitor {
    config: ActivityMonitorConfig,
    /// Recent transactions by user
    user_transactions: Arc<RwLock<HashMap<String, VecDeque<TransactionRecord>>>>,
    /// Alert broadcaster
    alert_tx: broadcast::Sender<SuspiciousActivity>,
}

impl ActivityMonitor {
    /// Create a new activity monitor
    pub fn new(config: ActivityMonitorConfig) -> Self {
        let (alert_tx, _) = broadcast::channel(100);
        Self {
            config,
            user_transactions: Arc::new(RwLock::new(HashMap::new())),
            alert_tx,
        }
    }

    /// Subscribe to alerts
    pub fn subscribe(&self) -> broadcast::Receiver<SuspiciousActivity> {
        self.alert_tx.subscribe()
    }

    /// Record a transaction and check for suspicious activity
    pub async fn record_transaction(
        &self,
        record: TransactionRecord,
    ) -> Option<SuspiciousActivity> {
        let user_id = record.user_id.clone();

        // Add to user transactions
        {
            let mut transactions = self.user_transactions.write().await;
            transactions
                .entry(user_id.clone())
                .or_insert_with(VecDeque::new)
                .push_back(record.clone());
        }

        // Cleanup old records
        self.cleanup_old_records(&user_id).await;

        // Check for suspicious activity
        let mut alerts = Vec::new();

        // Check for large transaction
        if let Some(alert) = self.check_large_transaction(&record).await {
            alerts.push(alert);
        }

        // Check for rapid transactions
        if let Some(alert) = self.check_rapid_transactions(&user_id).await {
            alerts.push(alert);
        }

        // Check velocity
        if let Some(alert) = self.check_velocity(&user_id).await {
            alerts.push(alert);
        }

        // Send alerts (highest risk score first)
        let mut highest_alert: Option<SuspiciousActivity> = None;
        for alert in alerts {
            if alert.risk_score >= self.config.min_risk_score {
                let _ = self.alert_tx.send(alert.clone());
                if highest_alert.is_none()
                    || alert.risk_score > highest_alert.as_ref().unwrap().risk_score
                {
                    highest_alert = Some(alert);
                }
            }
        }

        highest_alert
    }

    /// Check for large transaction
    async fn check_large_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Option<SuspiciousActivity> {
        if record.amount_sats >= self.config.large_tx_threshold_sats {
            let risk_score = self.calculate_large_tx_risk_score(record.amount_sats);
            Some(SuspiciousActivity {
                user_id: record.user_id.clone(),
                activity_type: ActivityType::LargeTransaction,
                description: format!("Large transaction detected: {} sats", record.amount_sats),
                txids: vec![record.txid.clone()],
                amount_sats: record.amount_sats,
                detected_at: Utc::now(),
                risk_score,
            })
        } else {
            None
        }
    }

    /// Check for rapid transactions
    async fn check_rapid_transactions(&self, user_id: &str) -> Option<SuspiciousActivity> {
        let transactions = self.user_transactions.read().await;
        let user_txs = transactions.get(user_id)?;

        let now = Utc::now();
        let window_start = now - Duration::seconds(self.config.rapid_tx_window_secs);

        let recent: Vec<_> = user_txs
            .iter()
            .filter(|tx| tx.timestamp >= window_start)
            .collect();

        if recent.len() as u32 >= self.config.rapid_tx_count {
            let total_amount: u64 = recent.iter().map(|tx| tx.amount_sats).sum();
            let txids: Vec<_> = recent.iter().map(|tx| tx.txid.clone()).collect();

            let risk_score = self.calculate_rapid_tx_risk_score(recent.len());

            Some(SuspiciousActivity {
                user_id: user_id.to_string(),
                activity_type: ActivityType::RapidTransactions,
                description: format!(
                    "{} transactions in {} seconds",
                    recent.len(),
                    self.config.rapid_tx_window_secs
                ),
                txids,
                amount_sats: total_amount,
                detected_at: Utc::now(),
                risk_score,
            })
        } else {
            None
        }
    }

    /// Check velocity (amount per hour)
    async fn check_velocity(&self, user_id: &str) -> Option<SuspiciousActivity> {
        let transactions = self.user_transactions.read().await;
        let user_txs = transactions.get(user_id)?;

        let now = Utc::now();
        let hour_ago = now - Duration::hours(1);

        let recent: Vec<_> = user_txs
            .iter()
            .filter(|tx| tx.timestamp >= hour_ago)
            .collect();

        let total_amount: u64 = recent.iter().map(|tx| tx.amount_sats).sum();

        if total_amount >= self.config.velocity_limit_sats_per_hour {
            let txids: Vec<_> = recent.iter().map(|tx| tx.txid.clone()).collect();

            let risk_score = self.calculate_velocity_risk_score(total_amount);

            Some(SuspiciousActivity {
                user_id: user_id.to_string(),
                activity_type: ActivityType::VelocityExceeded,
                description: format!("High velocity: {} sats in 1 hour", total_amount),
                txids,
                amount_sats: total_amount,
                detected_at: Utc::now(),
                risk_score,
            })
        } else {
            None
        }
    }

    /// Calculate risk score for large transaction (0-100)
    fn calculate_large_tx_risk_score(&self, amount_sats: u64) -> u8 {
        let threshold = self.config.large_tx_threshold_sats;
        let ratio = amount_sats as f64 / threshold as f64;

        // Risk increases logarithmically with amount
        let score = 50.0 + 25.0 * ratio.ln();
        score.clamp(0.0, 100.0) as u8
    }

    /// Calculate risk score for rapid transactions (0-100)
    fn calculate_rapid_tx_risk_score(&self, count: usize) -> u8 {
        let threshold = self.config.rapid_tx_count as f64;
        let ratio = count as f64 / threshold;

        let score = 50.0 + 30.0 * (ratio - 1.0);
        score.clamp(0.0, 100.0) as u8
    }

    /// Calculate risk score for velocity (0-100)
    fn calculate_velocity_risk_score(&self, amount_sats: u64) -> u8 {
        let threshold = self.config.velocity_limit_sats_per_hour;
        let ratio = amount_sats as f64 / threshold as f64;

        let score = 50.0 + 30.0 * (ratio - 1.0);
        score.clamp(0.0, 100.0) as u8
    }

    /// Cleanup old transaction records
    async fn cleanup_old_records(&self, user_id: &str) {
        let mut transactions = self.user_transactions.write().await;
        if let Some(user_txs) = transactions.get_mut(user_id) {
            let now = Utc::now();
            let cutoff = now - Duration::hours(24); // Keep 24 hours of history

            while let Some(tx) = user_txs.front() {
                if tx.timestamp < cutoff {
                    user_txs.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Get user transaction history
    pub async fn get_user_history(&self, user_id: &str) -> Vec<TransactionRecord> {
        let transactions = self.user_transactions.read().await;
        transactions
            .get(user_id)
            .map(|txs| txs.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_monitor_config_defaults() {
        let config = ActivityMonitorConfig::default();
        assert_eq!(config.large_tx_threshold_sats, 10_000_000);
        assert_eq!(config.rapid_tx_count, 5);
        assert_eq!(config.rapid_tx_window_secs, 300);
        assert_eq!(config.velocity_limit_sats_per_hour, 50_000_000);
    }

    #[tokio::test]
    async fn test_large_transaction_detection() {
        let monitor = ActivityMonitor::new(ActivityMonitorConfig::default());

        let record = TransactionRecord {
            txid: "tx1".to_string(),
            user_id: "user1".to_string(),
            amount_sats: 20_000_000, // Above threshold
            timestamp: Utc::now(),
        };

        let alert = monitor.record_transaction(record).await;
        assert!(alert.is_some());

        let alert = alert.unwrap();
        assert_eq!(alert.activity_type, ActivityType::LargeTransaction);
        assert!(alert.risk_score >= 50);
    }

    #[tokio::test]
    async fn test_rapid_transactions_detection() {
        let monitor = ActivityMonitor::new(ActivityMonitorConfig::default());

        // Record 5 transactions in quick succession
        for i in 0..5 {
            let record = TransactionRecord {
                txid: format!("tx{}", i),
                user_id: "user1".to_string(),
                amount_sats: 1_000_000,
                timestamp: Utc::now(),
            };
            monitor.record_transaction(record).await;
        }

        // The 5th transaction should trigger an alert
        let history = monitor.get_user_history("user1").await;
        assert_eq!(history.len(), 5);
    }
}
