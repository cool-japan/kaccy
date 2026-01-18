//! Security features for trading operations

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Emergency pause system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyPause {
    /// Whether system is paused
    pub is_paused: bool,
    /// Pause reason
    pub pause_reason: Option<String>,
    /// Who initiated the pause
    pub paused_by: Option<Uuid>,
    /// When the pause was initiated
    pub paused_at: Option<DateTime<Utc>>,
    /// Authorized pausers (admin user IDs)
    pub authorized_pausers: Vec<Uuid>,
}

impl EmergencyPause {
    /// Create a new emergency pause system
    pub fn new(authorized_pausers: Vec<Uuid>) -> Self {
        Self {
            is_paused: false,
            pause_reason: None,
            paused_by: None,
            paused_at: None,
            authorized_pausers,
        }
    }

    /// Pause the system
    pub fn pause(&mut self, admin_id: Uuid, reason: String) -> Result<()> {
        if !self.authorized_pausers.contains(&admin_id) {
            return Err(CoreError::Validation(
                "Unauthorized to pause system".to_string(),
            ));
        }

        self.is_paused = true;
        self.pause_reason = Some(reason);
        self.paused_by = Some(admin_id);
        self.paused_at = Some(Utc::now());

        Ok(())
    }

    /// Resume the system
    pub fn resume(&mut self, admin_id: Uuid) -> Result<()> {
        if !self.authorized_pausers.contains(&admin_id) {
            return Err(CoreError::Validation(
                "Unauthorized to resume system".to_string(),
            ));
        }

        self.is_paused = false;
        self.pause_reason = None;
        self.paused_by = None;
        self.paused_at = None;

        Ok(())
    }

    /// Check if operations are allowed
    pub fn check_allowed(&self) -> Result<()> {
        if self.is_paused {
            Err(CoreError::Validation(format!(
                "System is paused: {}",
                self.pause_reason
                    .as_ref()
                    .unwrap_or(&"Unknown reason".to_string())
            )))
        } else {
            Ok(())
        }
    }
}

/// Multi-signature operation
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MultiSigOperation {
    pub operation_id: Uuid,
    /// Operation type (withdraw, transfer, pause, etc.)
    pub operation_type: OperationType,
    /// Operation parameters (JSON encoded)
    pub parameters: String,
    /// Required number of signatures
    pub required_signatures: i32,
    /// Current signatures
    pub current_signatures: i32,
    /// Who signed (user IDs)
    pub signers: Vec<Uuid>,
    /// Authorized signers for this operation
    pub authorized_signers: Vec<Uuid>,
    /// Status
    pub status: MultiSigStatus,
    /// Expiration time
    pub expires_at: DateTime<Utc>,
    /// Initiated by
    pub initiated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum OperationType {
    Withdraw,
    #[default]
    Transfer,
    Pause,
    ConfigChange,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationType::Withdraw => write!(f, "withdraw"),
            OperationType::Transfer => write!(f, "transfer"),
            OperationType::Pause => write!(f, "pause"),
            OperationType::ConfigChange => write!(f, "config_change"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum MultiSigStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
    Expired,
    Executed,
}

impl fmt::Display for MultiSigStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MultiSigStatus::Pending => write!(f, "pending"),
            MultiSigStatus::Approved => write!(f, "approved"),
            MultiSigStatus::Rejected => write!(f, "rejected"),
            MultiSigStatus::Expired => write!(f, "expired"),
            MultiSigStatus::Executed => write!(f, "executed"),
        }
    }
}

impl MultiSigOperation {
    /// Create a new multi-sig operation
    pub fn new(
        operation_type: OperationType,
        parameters: String,
        required_signatures: i32,
        authorized_signers: Vec<Uuid>,
        initiator_id: Uuid,
        expiration_hours: i64,
    ) -> Result<Self> {
        if required_signatures <= 0 {
            return Err(CoreError::Validation(
                "Required signatures must be positive".to_string(),
            ));
        }

        if required_signatures > authorized_signers.len() as i32 {
            return Err(CoreError::Validation(
                "Required signatures exceeds authorized signers".to_string(),
            ));
        }

        let now = Utc::now();

        Ok(Self {
            operation_id: Uuid::new_v4(),
            operation_type,
            parameters,
            required_signatures,
            current_signatures: 0,
            signers: Vec::new(),
            authorized_signers,
            status: MultiSigStatus::Pending,
            expires_at: now + Duration::hours(expiration_hours),
            initiated_by: initiator_id,
            created_at: now,
            updated_at: now,
        })
    }

    /// Add a signature
    pub fn sign(&mut self, signer_id: Uuid) -> Result<()> {
        // Check if expired
        if Utc::now() > self.expires_at {
            self.status = MultiSigStatus::Expired;
            return Err(CoreError::Validation("Operation expired".to_string()));
        }

        // Check status
        if self.status != MultiSigStatus::Pending {
            return Err(CoreError::Validation(format!(
                "Operation is not pending: {}",
                self.status
            )));
        }

        // Check if authorized
        if !self.authorized_signers.contains(&signer_id) {
            return Err(CoreError::Validation("Signer not authorized".to_string()));
        }

        // Check if already signed
        if self.signers.contains(&signer_id) {
            return Err(CoreError::Validation("Already signed".to_string()));
        }

        // Add signature
        self.signers.push(signer_id);
        self.current_signatures += 1;
        self.updated_at = Utc::now();

        // Check if approved
        if self.current_signatures >= self.required_signatures {
            self.status = MultiSigStatus::Approved;
        }

        Ok(())
    }

    /// Check if operation is approved
    pub fn is_approved(&self) -> bool {
        self.status == MultiSigStatus::Approved
    }

    /// Mark as executed
    pub fn mark_executed(&mut self) {
        self.status = MultiSigStatus::Executed;
        self.updated_at = Utc::now();
    }
}

/// Withdrawal rate limiter
#[derive(Debug, Clone)]
pub struct WithdrawalRateLimiter {
    /// Max withdrawal amount per user per hour
    pub max_per_hour: Decimal,
    /// Max withdrawal amount per user per day
    pub max_per_day: Decimal,
    /// Recent withdrawals (user_id -> withdrawal history)
    recent_withdrawals: HashMap<Uuid, VecDeque<WithdrawalRecord>>,
}

#[derive(Debug, Clone)]
struct WithdrawalRecord {
    amount: Decimal,
    timestamp: DateTime<Utc>,
}

impl WithdrawalRateLimiter {
    /// Create a new rate limiter
    pub fn new(max_per_hour: Decimal, max_per_day: Decimal) -> Self {
        Self {
            max_per_hour,
            max_per_day,
            recent_withdrawals: HashMap::new(),
        }
    }

    /// Check if withdrawal is allowed
    pub fn check_withdrawal(&mut self, user_id: Uuid, amount: Decimal) -> Result<()> {
        let now = Utc::now();
        let hour_ago = now - Duration::hours(1);
        let day_ago = now - Duration::days(1);

        // Clean old records and get recent withdrawals
        let user_withdrawals = self.recent_withdrawals.entry(user_id).or_default();

        // Remove old entries
        user_withdrawals.retain(|w| w.timestamp > day_ago);

        // Calculate totals
        let hour_total: Decimal = user_withdrawals
            .iter()
            .filter(|w| w.timestamp > hour_ago)
            .map(|w| w.amount)
            .sum();

        let day_total: Decimal = user_withdrawals.iter().map(|w| w.amount).sum();

        // Check limits
        if hour_total + amount > self.max_per_hour {
            return Err(CoreError::Validation(format!(
                "Hourly withdrawal limit exceeded. Current: {}, Limit: {}",
                hour_total + amount,
                self.max_per_hour
            )));
        }

        if day_total + amount > self.max_per_day {
            return Err(CoreError::Validation(format!(
                "Daily withdrawal limit exceeded. Current: {}, Limit: {}",
                day_total + amount,
                self.max_per_day
            )));
        }

        Ok(())
    }

    /// Record a withdrawal
    pub fn record_withdrawal(&mut self, user_id: Uuid, amount: Decimal) {
        let user_withdrawals = self.recent_withdrawals.entry(user_id).or_default();
        user_withdrawals.push_back(WithdrawalRecord {
            amount,
            timestamp: Utc::now(),
        });
    }
}

/// Anomaly detector for suspicious trading patterns
#[derive(Debug, Clone)]
pub struct AnomalyDetector {
    /// Threshold for large trade (as % of pool liquidity)
    pub large_trade_threshold: Decimal,
    /// Threshold for rapid trading (trades per minute)
    pub rapid_trade_threshold: i32,
    /// Recent trades per user
    recent_trades: HashMap<Uuid, VecDeque<DateTime<Utc>>>,
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    pub fn new(large_trade_threshold: Decimal, rapid_trade_threshold: i32) -> Self {
        Self {
            large_trade_threshold,
            rapid_trade_threshold,
            recent_trades: HashMap::new(),
        }
    }

    /// Check for large trade anomaly
    pub fn check_large_trade(
        &self,
        trade_amount: Decimal,
        pool_liquidity: Decimal,
    ) -> Option<AnomalyAlert> {
        if pool_liquidity == dec!(0) {
            return None;
        }

        let trade_percentage = trade_amount / pool_liquidity;

        if trade_percentage > self.large_trade_threshold {
            Some(AnomalyAlert {
                alert_type: AnomalyType::LargeTrade,
                severity: AlertSeverity::High,
                description: format!(
                    "Large trade detected: {} ({:.2}% of pool liquidity)",
                    trade_amount,
                    trade_percentage * dec!(100)
                ),
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Check for rapid trading anomaly
    pub fn check_rapid_trading(&mut self, user_id: Uuid) -> Option<AnomalyAlert> {
        let now = Utc::now();
        let minute_ago = now - Duration::minutes(1);

        // Get or create user's trade history
        let user_trades = self.recent_trades.entry(user_id).or_default();

        // Remove old trades
        user_trades.retain(|t| *t > minute_ago);

        // Record current trade
        user_trades.push_back(now);

        // Check threshold
        if user_trades.len() as i32 > self.rapid_trade_threshold {
            Some(AnomalyAlert {
                alert_type: AnomalyType::RapidTrading,
                severity: AlertSeverity::Medium,
                description: format!(
                    "Rapid trading detected: {} trades in 1 minute (threshold: {})",
                    user_trades.len(),
                    self.rapid_trade_threshold
                ),
                timestamp: now,
            })
        } else {
            None
        }
    }

    /// Check for price manipulation
    pub fn check_price_manipulation(
        &self,
        price_change: Decimal,
        max_allowed_change: Decimal,
    ) -> Option<AnomalyAlert> {
        if price_change.abs() > max_allowed_change {
            Some(AnomalyAlert {
                alert_type: AnomalyType::PriceManipulation,
                severity: AlertSeverity::Critical,
                description: format!(
                    "Suspicious price movement: {:.2}% (max allowed: {:.2}%)",
                    price_change * dec!(100),
                    max_allowed_change * dec!(100)
                ),
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }
}

/// Anomaly alert
#[derive(Debug, Clone, Serialize)]
pub struct AnomalyAlert {
    pub alert_type: AnomalyType,
    pub severity: AlertSeverity,
    pub description: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum AnomalyType {
    LargeTrade,
    RapidTrading,
    PriceManipulation,
    SuspiciousPattern,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_pause() {
        let admin1 = Uuid::new_v4();
        let admin2 = Uuid::new_v4();
        let unauthorized = Uuid::new_v4();

        let mut pause_system = EmergencyPause::new(vec![admin1, admin2]);

        // Check initially not paused
        assert!(!pause_system.is_paused);
        assert!(pause_system.check_allowed().is_ok());

        // Pause system
        assert!(
            pause_system
                .pause(admin1, "Security issue".to_string())
                .is_ok()
        );
        assert!(pause_system.is_paused);
        assert!(pause_system.check_allowed().is_err());

        // Try to pause with unauthorized user
        assert!(pause_system.resume(unauthorized).is_err());

        // Resume with authorized admin
        assert!(pause_system.resume(admin2).is_ok());
        assert!(!pause_system.is_paused);
        assert!(pause_system.check_allowed().is_ok());
    }

    #[test]
    fn test_multisig_operation() {
        let signer1 = Uuid::new_v4();
        let signer2 = Uuid::new_v4();
        let signer3 = Uuid::new_v4();
        let unauthorized = Uuid::new_v4();

        let mut operation = MultiSigOperation::new(
            OperationType::Withdraw,
            "{\"amount\": 1000}".to_string(),
            2, // Require 2 signatures
            vec![signer1, signer2, signer3],
            signer1,
            24, // 24 hour expiration
        )
        .unwrap();

        assert_eq!(operation.status, MultiSigStatus::Pending);
        assert_eq!(operation.current_signatures, 0);

        // First signature
        assert!(operation.sign(signer1).is_ok());
        assert_eq!(operation.current_signatures, 1);
        assert!(!operation.is_approved());

        // Try unauthorized signer
        assert!(operation.sign(unauthorized).is_err());

        // Second signature - should approve
        assert!(operation.sign(signer2).is_ok());
        assert_eq!(operation.current_signatures, 2);
        assert!(operation.is_approved());

        // Try to sign again
        assert!(operation.sign(signer2).is_err());
    }

    #[test]
    fn test_withdrawal_rate_limiter() {
        let mut limiter = WithdrawalRateLimiter::new(
            dec!(1000), // 1000 per hour
            dec!(5000), // 5000 per day
        );

        let user = Uuid::new_v4();

        // First withdrawal should be ok
        assert!(limiter.check_withdrawal(user, dec!(500)).is_ok());
        limiter.record_withdrawal(user, dec!(500));

        // Second withdrawal within hour limit
        assert!(limiter.check_withdrawal(user, dec!(400)).is_ok());
        limiter.record_withdrawal(user, dec!(400));

        // This would exceed hourly limit
        assert!(limiter.check_withdrawal(user, dec!(200)).is_err());

        // Smaller amount should still work for daily limit
        assert!(limiter.check_withdrawal(user, dec!(100)).is_ok());
    }

    #[test]
    fn test_anomaly_detector() {
        let mut detector = AnomalyDetector::new(
            dec!(0.1), // 10% of pool for large trade
            5,         // 5 trades per minute for rapid trading
        );

        // Test large trade detection
        let alert = detector.check_large_trade(dec!(150), dec!(1000));
        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.alert_type, AnomalyType::LargeTrade);
        assert_eq!(alert.severity, AlertSeverity::High);

        // Test normal trade
        let alert = detector.check_large_trade(dec!(50), dec!(1000));
        assert!(alert.is_none());

        // Test rapid trading
        let user = Uuid::new_v4();
        for _ in 0..6 {
            let alert = detector.check_rapid_trading(user);
            if let Some(a) = alert {
                assert_eq!(a.alert_type, AnomalyType::RapidTrading);
                break;
            }
        }

        // Test price manipulation
        let alert = detector.check_price_manipulation(dec!(0.15), dec!(0.1));
        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.alert_type, AnomalyType::PriceManipulation);
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }
}
