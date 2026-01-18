//! Budget management and cost alerting for LLM usage
//!
//! This module provides tools to track, limit, and alert on LLM API spending.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{AiError, Result};

/// Alert callback type
type AlertCallback = Box<dyn Fn(BudgetAlert) + Send + Sync>;

/// Budget period for tracking costs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BudgetPeriod {
    /// Hourly budget
    Hourly,
    /// Daily budget
    Daily,
    /// Weekly budget
    Weekly,
    /// Monthly budget
    Monthly,
    /// Total/lifetime budget
    Total,
}

/// Budget alert level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    /// Informational (50% of budget)
    Info,
    /// Warning (75% of budget)
    Warning,
    /// Critical (90% of budget)
    Critical,
    /// Budget exceeded
    Exceeded,
}

/// Budget alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAlert {
    /// Alert level
    pub level: AlertLevel,
    /// Budget period
    pub period: BudgetPeriod,
    /// Current spending
    pub current_spend: f64,
    /// Budget limit
    pub limit: f64,
    /// Percentage used
    pub percentage_used: f64,
    /// Alert message
    pub message: String,
}

/// Budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Budget limits per period
    pub limits: HashMap<BudgetPeriod, f64>,
    /// Enable automatic blocking when budget exceeded
    pub auto_block: bool,
    /// Alert thresholds (percentage of budget)
    pub alert_thresholds: Vec<f64>,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            limits: HashMap::new(),
            auto_block: false,
            alert_thresholds: vec![0.5, 0.75, 0.9],
        }
    }
}

impl BudgetConfig {
    /// Create a new budget config with daily limit
    #[must_use]
    pub fn with_daily_limit(limit: f64) -> Self {
        let mut limits = HashMap::new();
        limits.insert(BudgetPeriod::Daily, limit);
        Self {
            limits,
            auto_block: false,
            alert_thresholds: vec![0.5, 0.75, 0.9],
        }
    }

    /// Create a new budget config with monthly limit
    #[must_use]
    pub fn with_monthly_limit(limit: f64) -> Self {
        let mut limits = HashMap::new();
        limits.insert(BudgetPeriod::Monthly, limit);
        Self {
            limits,
            auto_block: false,
            alert_thresholds: vec![0.5, 0.75, 0.9],
        }
    }

    /// Set a budget limit for a period
    pub fn set_limit(&mut self, period: BudgetPeriod, limit: f64) {
        self.limits.insert(period, limit);
    }

    /// Enable automatic blocking when budget exceeded
    #[must_use]
    pub fn with_auto_block(mut self) -> Self {
        self.auto_block = true;
        self
    }

    /// Set custom alert thresholds
    #[must_use]
    pub fn with_alert_thresholds(mut self, thresholds: Vec<f64>) -> Self {
        self.alert_thresholds = thresholds;
        self
    }
}

/// Period usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeriodUsage {
    /// Total spending in this period
    pub total_cost: f64,
    /// Number of requests
    pub request_count: u64,
    /// Period start timestamp (Unix epoch)
    pub period_start: u64,
    /// Last reset timestamp (Unix epoch)
    pub last_reset: u64,
}

/// Budget manager for tracking and limiting costs
pub struct BudgetManager {
    config: BudgetConfig,
    usage: Arc<RwLock<HashMap<BudgetPeriod, PeriodUsage>>>,
    alert_callbacks: Arc<RwLock<Vec<AlertCallback>>>,
}

impl BudgetManager {
    /// Create a new budget manager
    #[must_use]
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            usage: Arc::new(RwLock::new(HashMap::new())),
            alert_callbacks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a request cost
    pub async fn record_cost(&self, cost: f64) -> Result<()> {
        let mut usage = self.usage.write().await;
        let now = chrono::Utc::now().timestamp() as u64;

        for period in [
            BudgetPeriod::Hourly,
            BudgetPeriod::Daily,
            BudgetPeriod::Weekly,
            BudgetPeriod::Monthly,
            BudgetPeriod::Total,
        ] {
            let entry = usage.entry(period).or_insert_with(|| PeriodUsage {
                total_cost: 0.0,
                request_count: 0,
                period_start: now,
                last_reset: now,
            });

            // Check if period has elapsed and reset if needed
            if self.should_reset_period(period, entry.last_reset, now) {
                entry.total_cost = 0.0;
                entry.request_count = 0;
                entry.period_start = now;
                entry.last_reset = now;
            }

            entry.total_cost += cost;
            entry.request_count += 1;

            // Check for budget alerts
            if let Some(limit) = self.config.limits.get(&period) {
                self.check_alerts(period, entry.total_cost, *limit).await;
            }
        }

        Ok(())
    }

    /// Check if a request would exceed budget
    pub async fn check_budget(&self, estimated_cost: f64) -> Result<()> {
        let usage = self.usage.read().await;

        for (period, limit) in &self.config.limits {
            if let Some(current_usage) = usage.get(period) {
                if current_usage.total_cost + estimated_cost > *limit && self.config.auto_block {
                    return Err(AiError::QuotaExceeded(format!(
                        "Budget exceeded for {:?} period: ${:.4} + ${:.4} > ${:.4}",
                        period, current_usage.total_cost, estimated_cost, limit
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get current usage for a period
    pub async fn get_usage(&self, period: BudgetPeriod) -> Option<PeriodUsage> {
        let usage = self.usage.read().await;
        usage.get(&period).cloned()
    }

    /// Get all usage statistics
    pub async fn get_all_usage(&self) -> HashMap<BudgetPeriod, PeriodUsage> {
        self.usage.read().await.clone()
    }

    /// Reset usage for a period
    pub async fn reset_period(&self, period: BudgetPeriod) {
        let mut usage = self.usage.write().await;
        let now = chrono::Utc::now().timestamp() as u64;

        if let Some(entry) = usage.get_mut(&period) {
            entry.total_cost = 0.0;
            entry.request_count = 0;
            entry.period_start = now;
            entry.last_reset = now;
        }
    }

    /// Register an alert callback
    pub async fn on_alert<F>(&self, callback: F)
    where
        F: Fn(BudgetAlert) + Send + Sync + 'static,
    {
        let mut callbacks = self.alert_callbacks.write().await;
        callbacks.push(Box::new(callback));
    }

    /// Check if period should be reset
    fn should_reset_period(&self, period: BudgetPeriod, last_reset: u64, now: u64) -> bool {
        let elapsed = now.saturating_sub(last_reset);

        match period {
            BudgetPeriod::Hourly => elapsed >= 3600,
            BudgetPeriod::Daily => elapsed >= 86400,
            BudgetPeriod::Weekly => elapsed >= 604_800,
            BudgetPeriod::Monthly => elapsed >= 2_592_000, // Approximate 30 days
            BudgetPeriod::Total => false,
        }
    }

    /// Check for budget alerts
    async fn check_alerts(&self, period: BudgetPeriod, current_spend: f64, limit: f64) {
        let percentage_used = (current_spend / limit) * 100.0;

        let level = if current_spend >= limit {
            Some(AlertLevel::Exceeded)
        } else if percentage_used >= 90.0 {
            Some(AlertLevel::Critical)
        } else if percentage_used >= 75.0 {
            Some(AlertLevel::Warning)
        } else if percentage_used >= 50.0 {
            Some(AlertLevel::Info)
        } else {
            None
        };

        if let Some(level) = level {
            let alert = BudgetAlert {
                level,
                period,
                current_spend,
                limit,
                percentage_used,
                message: format!(
                    "{period:?} budget at {percentage_used:.1}%: ${current_spend:.4} / ${limit:.4}"
                ),
            };

            // Trigger callbacks
            let callbacks = self.alert_callbacks.read().await;
            for callback in callbacks.iter() {
                callback(alert.clone());
            }
        }
    }

    /// Get remaining budget for a period
    pub async fn get_remaining(&self, period: BudgetPeriod) -> Option<f64> {
        if let Some(limit) = self.config.limits.get(&period) {
            let usage = self.usage.read().await;
            if let Some(current) = usage.get(&period) {
                return Some((limit - current.total_cost).max(0.0));
            }
            return Some(*limit);
        }
        None
    }

    /// Get budget utilization percentage for a period
    pub async fn get_utilization(&self, period: BudgetPeriod) -> Option<f64> {
        if let Some(limit) = self.config.limits.get(&period) {
            let usage = self.usage.read().await;
            if let Some(current) = usage.get(&period) {
                return Some((current.total_cost / limit) * 100.0);
            }
            return Some(0.0);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_config_creation() {
        let config = BudgetConfig::with_daily_limit(10.0);
        assert_eq!(config.limits.get(&BudgetPeriod::Daily), Some(&10.0));
        assert!(!config.auto_block);
    }

    #[test]
    fn test_budget_config_with_auto_block() {
        let config = BudgetConfig::with_daily_limit(10.0).with_auto_block();
        assert!(config.auto_block);
    }

    #[tokio::test]
    async fn test_budget_manager_record_cost() {
        let config = BudgetConfig::with_daily_limit(10.0);
        let manager = BudgetManager::new(config);

        manager.record_cost(5.0).await.unwrap();

        let usage = manager.get_usage(BudgetPeriod::Daily).await;
        assert!(usage.is_some());
        assert!((usage.unwrap().total_cost - 5.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_budget_manager_check_budget() {
        let config = BudgetConfig::with_daily_limit(10.0).with_auto_block();
        let manager = BudgetManager::new(config);

        manager.record_cost(5.0).await.unwrap();

        // Should succeed
        assert!(manager.check_budget(4.0).await.is_ok());

        // Should fail (exceeds budget)
        assert!(manager.check_budget(6.0).await.is_err());
    }

    #[tokio::test]
    async fn test_budget_manager_get_remaining() {
        let config = BudgetConfig::with_daily_limit(10.0);
        let manager = BudgetManager::new(config);

        manager.record_cost(3.0).await.unwrap();

        let remaining = manager.get_remaining(BudgetPeriod::Daily).await;
        assert!(remaining.is_some());
        assert!((remaining.unwrap() - 7.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_budget_manager_utilization() {
        let config = BudgetConfig::with_daily_limit(10.0);
        let manager = BudgetManager::new(config);

        manager.record_cost(7.5).await.unwrap();

        let utilization = manager.get_utilization(BudgetPeriod::Daily).await;
        assert!(utilization.is_some());
        assert!((utilization.unwrap() - 75.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_budget_manager_reset() {
        let config = BudgetConfig::with_daily_limit(10.0);
        let manager = BudgetManager::new(config);

        manager.record_cost(5.0).await.unwrap();
        manager.reset_period(BudgetPeriod::Daily).await;

        let usage = manager.get_usage(BudgetPeriod::Daily).await;
        assert!(usage.is_some());
        assert!((usage.unwrap().total_cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_alert_level_ordering() {
        // Just verify the enum exists and can be compared
        let info = AlertLevel::Info;
        let exceeded = AlertLevel::Exceeded;
        assert_ne!(info, exceeded);
    }
}
