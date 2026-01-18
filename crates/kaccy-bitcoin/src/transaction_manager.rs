//! Transaction Manager Integration Module
//!
//! This module provides a high-level interface that integrates UTXO management,
//! transaction limits, and suspicious activity detection into a single cohesive API.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::activity_monitor::{
    ActivityMonitor, ActivityMonitorConfig, SuspiciousActivity, TransactionRecord,
};
use crate::client::BitcoinClient;
use crate::error::Result;
use crate::tx_limits::{LimitConfig, LimitEnforcer};
use crate::utxo::{
    ConsolidationConfig, ConsolidationPlan, SelectionStrategy, UtxoManager, UtxoSelection,
};

/// Transaction manager configuration
#[derive(Debug, Clone)]
pub struct TransactionManagerConfig {
    /// UTXO consolidation config
    pub consolidation: ConsolidationConfig,
    /// Transaction limits config
    pub limits: LimitConfig,
    /// Activity monitoring config
    pub activity: ActivityMonitorConfig,
    /// Enable automatic UTXO consolidation
    pub auto_consolidate: bool,
}

impl Default for TransactionManagerConfig {
    fn default() -> Self {
        Self {
            consolidation: ConsolidationConfig::default(),
            limits: LimitConfig::default(),
            activity: ActivityMonitorConfig::default(),
            auto_consolidate: true,
        }
    }
}

/// Integrated transaction manager
pub struct TransactionManager {
    utxo_manager: UtxoManager,
    limit_enforcer: Arc<RwLock<LimitEnforcer>>,
    activity_monitor: Arc<ActivityMonitor>,
    config: TransactionManagerConfig,
}

/// Transaction validation result
#[derive(Debug, Clone)]
pub struct TransactionValidation {
    /// Whether the transaction is allowed
    pub allowed: bool,
    /// Reason for rejection (if not allowed)
    pub rejection_reason: Option<String>,
    /// Suspicious activity detected (if any)
    pub suspicious_activity: Option<SuspiciousActivity>,
    /// UTXO selection (if requested)
    pub utxo_selection: Option<UtxoSelection>,
}

/// Consolidated transaction statistics
#[derive(Debug, Clone)]
pub struct TransactionStats {
    /// Total UTXOs available
    pub total_utxos: usize,
    /// Total balance in satoshis
    pub total_balance_sats: u64,
    /// Number of spendable UTXOs
    pub spendable_utxos: usize,
    /// Whether consolidation is recommended
    pub consolidation_recommended: bool,
    /// Consolidation plan (if available)
    pub consolidation_plan: Option<ConsolidationPlan>,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new(client: Arc<BitcoinClient>, config: TransactionManagerConfig) -> Self {
        let utxo_manager = UtxoManager::new(client, config.consolidation.clone());
        let limit_enforcer = Arc::new(RwLock::new(LimitEnforcer::new(config.limits.clone())));
        let activity_monitor = Arc::new(ActivityMonitor::new(config.activity.clone()));

        Self {
            utxo_manager,
            limit_enforcer,
            activity_monitor,
            config,
        }
    }

    /// Validate and prepare a transaction
    pub async fn validate_transaction(
        &self,
        user_id: &str,
        amount_sats: u64,
        fee_rate: f64,
        txid: Option<String>,
    ) -> Result<TransactionValidation> {
        // Check transaction limits
        let limit_check = {
            let enforcer = self.limit_enforcer.read().await;
            enforcer.check_transaction(user_id, amount_sats).await
        };

        if let Err(e) = limit_check {
            return Ok(TransactionValidation {
                allowed: false,
                rejection_reason: Some(format!("Limit exceeded: {}", e)),
                suspicious_activity: None,
                utxo_selection: None,
            });
        }

        // Check for suspicious activity
        let activity_record = TransactionRecord {
            txid: txid.clone().unwrap_or_else(|| "pending".to_string()),
            user_id: user_id.to_string(),
            amount_sats,
            timestamp: chrono::Utc::now(),
        };

        let suspicious = self
            .activity_monitor
            .record_transaction(activity_record)
            .await;

        // Select UTXOs if amount is specified
        let utxo_selection = if amount_sats > 0 {
            match self.utxo_manager.select_utxos(
                amount_sats,
                fee_rate,
                SelectionStrategy::BranchAndBound,
            ) {
                Ok(selection) => Some(selection),
                Err(e) => {
                    return Ok(TransactionValidation {
                        allowed: false,
                        rejection_reason: Some(format!("UTXO selection failed: {}", e)),
                        suspicious_activity: suspicious,
                        utxo_selection: None,
                    });
                }
            }
        } else {
            None
        };

        Ok(TransactionValidation {
            allowed: true,
            rejection_reason: None,
            suspicious_activity: suspicious,
            utxo_selection,
        })
    }

    /// Record a completed transaction
    pub async fn record_completed_transaction(
        &self,
        user_id: &str,
        amount_sats: u64,
        txid: String,
    ) {
        let enforcer = self.limit_enforcer.write().await;
        enforcer
            .record_transaction(user_id, amount_sats, Some(txid))
            .await;
    }

    /// Get transaction statistics
    pub fn get_statistics(&self) -> Result<TransactionStats> {
        let utxos = self.utxo_manager.list_utxos()?;

        let total_balance_sats: u64 = utxos.iter().map(|u| u.amount_sats).sum();
        let spendable_utxos = utxos.iter().filter(|u| u.spendable && u.safe).count();

        let consolidation_recommended = self.utxo_manager.should_consolidate()?;
        let consolidation_plan = if consolidation_recommended {
            self.utxo_manager.get_consolidation_plan()?
        } else {
            None
        };

        Ok(TransactionStats {
            total_utxos: utxos.len(),
            total_balance_sats,
            spendable_utxos,
            consolidation_recommended,
            consolidation_plan,
        })
    }

    /// Get user usage statistics for a period
    pub async fn get_user_usage(
        &self,
        user_id: &str,
        period: crate::tx_limits::LimitPeriod,
    ) -> (u64, u32) {
        let enforcer = self.limit_enforcer.read().await;
        enforcer.get_user_usage(user_id, period).await
    }

    /// Get platform usage statistics for a period
    pub async fn get_platform_usage(&self, period: crate::tx_limits::LimitPeriod) -> (u64, u32) {
        let enforcer = self.limit_enforcer.read().await;
        enforcer.get_platform_usage(period).await
    }

    /// Subscribe to suspicious activity alerts
    pub fn subscribe_to_alerts(&self) -> tokio::sync::broadcast::Receiver<SuspiciousActivity> {
        self.activity_monitor.subscribe()
    }

    /// Check if consolidation should be performed
    pub async fn should_consolidate_now(&self) -> Result<bool> {
        if !self.config.auto_consolidate {
            return Ok(false);
        }

        let stats = self.get_statistics()?;
        Ok(stats.consolidation_recommended)
    }

    /// Get consolidation plan
    pub fn get_consolidation_plan(&self) -> Result<Option<ConsolidationPlan>> {
        self.utxo_manager.get_consolidation_plan()
    }

    /// Get activity monitor reference
    pub fn activity_monitor(&self) -> &ActivityMonitor {
        &self.activity_monitor
    }

    /// Get UTXO manager reference
    pub fn utxo_manager(&self) -> &UtxoManager {
        &self.utxo_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_manager_config_defaults() {
        let config = TransactionManagerConfig::default();
        assert!(config.auto_consolidate);
        assert_eq!(config.consolidation.min_utxo_count, 50);
        assert_eq!(config.limits.single_tx_max_sats, 50_000_000);
        assert_eq!(config.activity.large_tx_threshold_sats, 10_000_000);
    }

    #[test]
    fn test_transaction_validation_result() {
        let validation = TransactionValidation {
            allowed: true,
            rejection_reason: None,
            suspicious_activity: None,
            utxo_selection: None,
        };

        assert!(validation.allowed);
        assert!(validation.rejection_reason.is_none());
    }
}
