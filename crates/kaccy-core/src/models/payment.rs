//! Bitcoin payment tracking models for order fulfillment

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

use super::user::ValidationError;

/// Bitcoin payment order tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaymentOrder {
    pub payment_order_id: Uuid,
    /// Associated order ID
    pub order_id: Uuid,
    /// User making the payment
    pub user_id: Uuid,
    /// Token being purchased
    pub token_id: Uuid,
    /// Generated Bitcoin address for this payment
    pub btc_address: String,
    /// Expected payment amount in BTC
    pub expected_amount_btc: Decimal,
    /// Actual received amount in BTC
    pub received_amount_btc: Option<Decimal>,
    /// Payment status
    pub status: PaymentStatus,
    /// Bitcoin transaction ID (txid) when detected
    pub btc_txid: Option<String>,
    /// Number of confirmations
    pub confirmations: i32,
    /// Required confirmations for fulfillment
    pub required_confirmations: i32,
    /// When payment was first detected
    pub detected_at: Option<DateTime<Utc>>,
    /// When payment reached required confirmations
    pub confirmed_at: Option<DateTime<Utc>>,
    /// Expiration time for payment (typically 24 hours)
    pub expires_at: DateTime<Utc>,
    /// When order was fulfilled/executed
    pub fulfilled_at: Option<DateTime<Utc>>,
    /// HD wallet derivation path for address generation
    pub derivation_path: Option<String>,
    /// Notes or error messages
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payment order status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum PaymentStatus {
    /// Waiting for payment
    #[default]
    Pending,
    /// Payment detected but not yet confirmed
    Detected,
    /// Payment confirmed and processing
    Confirming,
    /// Payment confirmed and order fulfilled
    Completed,
    /// Payment amount insufficient
    Underpaid,
    /// Payment amount exceeded
    Overpaid,
    /// Payment expired without completion
    Expired,
    /// Payment failed or error occurred
    Failed,
    /// Refund initiated
    Refunding,
    /// Refund completed
    Refunded,
}

impl fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentStatus::Pending => write!(f, "pending"),
            PaymentStatus::Detected => write!(f, "detected"),
            PaymentStatus::Confirming => write!(f, "confirming"),
            PaymentStatus::Completed => write!(f, "completed"),
            PaymentStatus::Underpaid => write!(f, "underpaid"),
            PaymentStatus::Overpaid => write!(f, "overpaid"),
            PaymentStatus::Expired => write!(f, "expired"),
            PaymentStatus::Failed => write!(f, "failed"),
            PaymentStatus::Refunding => write!(f, "refunding"),
            PaymentStatus::Refunded => write!(f, "refunded"),
        }
    }
}

impl fmt::Display for PaymentOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PaymentOrder({}, addr={}, amount={} BTC, status={})",
            self.payment_order_id, self.btc_address, self.expected_amount_btc, self.status
        )
    }
}

impl PaymentOrder {
    /// Check if payment order has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at && self.status == PaymentStatus::Pending
    }

    /// Check if payment is complete
    pub fn is_completed(&self) -> bool {
        self.status == PaymentStatus::Completed
    }

    /// Check if payment has enough confirmations
    pub fn has_enough_confirmations(&self) -> bool {
        self.confirmations >= self.required_confirmations
    }

    /// Calculate payment variance percentage
    pub fn payment_variance_percent(&self) -> Option<Decimal> {
        self.received_amount_btc.map(|received| {
            if self.expected_amount_btc == dec!(0) {
                return dec!(0);
            }
            ((received - self.expected_amount_btc) / self.expected_amount_btc) * dec!(100)
        })
    }

    /// Check if payment amount is acceptable (within tolerance)
    pub fn is_amount_acceptable(&self, tolerance_percent: Decimal) -> bool {
        if let Some(variance) = self.payment_variance_percent() {
            // Allow slight overpayment, but underpayment must be within tolerance
            if variance >= dec!(0) {
                return true; // Overpayment is acceptable
            }
            variance.abs() <= tolerance_percent
        } else {
            false
        }
    }

    /// Get time remaining until expiration
    pub fn time_until_expiration(&self) -> chrono::Duration {
        self.expires_at - Utc::now()
    }

    /// Get time since payment was created
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Calculate refund amount (if applicable)
    pub fn calculate_refund_amount(&self) -> Option<Decimal> {
        match self.status {
            PaymentStatus::Overpaid => self
                .received_amount_btc
                .map(|received| (received - self.expected_amount_btc).max(dec!(0))),
            PaymentStatus::Failed | PaymentStatus::Expired => self.received_amount_btc,
            _ => None,
        }
    }
}

/// Request to create a payment order
#[derive(Debug, Deserialize)]
pub struct CreatePaymentOrderRequest {
    pub order_id: Uuid,
    pub expected_amount_btc: Decimal,
    pub required_confirmations: Option<i32>,
}

impl CreatePaymentOrderRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.expected_amount_btc <= dec!(0) {
            return Err(ValidationError(
                "Payment amount must be positive".to_string(),
            ));
        }
        if self.expected_amount_btc < dec!(0.00001) {
            return Err(ValidationError(
                "Payment amount too small (minimum 0.00001 BTC)".to_string(),
            ));
        }
        if let Some(confs) = self.required_confirmations {
            if !(1..=6).contains(&confs) {
                return Err(ValidationError(
                    "Required confirmations must be between 1 and 6".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Bitcoin transaction confirmation event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaymentConfirmation {
    pub confirmation_id: Uuid,
    pub payment_order_id: Uuid,
    /// Transaction ID
    pub txid: String,
    /// Number of confirmations at time of event
    pub confirmations: i32,
    /// Amount in BTC
    pub amount_btc: Decimal,
    /// Block hash containing the transaction
    pub block_hash: Option<String>,
    /// Block height
    pub block_height: Option<i64>,
    /// Timestamp of block
    pub block_time: Option<DateTime<Utc>>,
    /// Transaction fee paid
    pub fee_btc: Option<Decimal>,
    pub detected_at: DateTime<Utc>,
}

impl fmt::Display for PaymentConfirmation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PaymentConfirmation(txid={}, confs={}, amount={} BTC)",
            self.txid, self.confirmations, self.amount_btc
        )
    }
}

/// Bitcoin address pool for pre-generated addresses
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BitcoinAddress {
    pub address_id: Uuid,
    /// Bitcoin address
    pub address: String,
    /// HD wallet derivation path
    pub derivation_path: String,
    /// Whether address is currently assigned to an order
    pub is_assigned: bool,
    /// Payment order ID if assigned
    pub assigned_to_payment_order_id: Option<Uuid>,
    /// When address was assigned
    pub assigned_at: Option<DateTime<Utc>>,
    /// Address type (p2pkh, p2wpkh, p2sh, etc.)
    pub address_type: BitcoinAddressType,
    pub created_at: DateTime<Utc>,
}

/// Bitcoin address type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[derive(Default)]
pub enum BitcoinAddressType {
    /// Legacy P2PKH (starts with 1)
    P2pkh,
    /// Native SegWit P2WPKH (starts with bc1)
    #[default]
    P2wpkh,
    /// Nested SegWit P2SH (starts with 3)
    P2sh,
    /// Taproot P2TR (starts with bc1p)
    P2tr,
}

impl fmt::Display for BitcoinAddressType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitcoinAddressType::P2pkh => write!(f, "P2PKH"),
            BitcoinAddressType::P2wpkh => write!(f, "P2WPKH"),
            BitcoinAddressType::P2sh => write!(f, "P2SH"),
            BitcoinAddressType::P2tr => write!(f, "P2TR"),
        }
    }
}

/// Payment monitoring configuration
pub struct PaymentMonitorConfig;

impl PaymentMonitorConfig {
    /// Default payment expiration time (24 hours)
    pub const DEFAULT_EXPIRATION_HOURS: i64 = 24;

    /// Default required confirmations for standard orders
    pub const DEFAULT_REQUIRED_CONFIRMATIONS: i32 = 2;

    /// Required confirmations for large orders (>0.1 BTC)
    pub const HIGH_VALUE_REQUIRED_CONFIRMATIONS: i32 = 3;

    /// Payment amount tolerance percentage (allow 1% underpayment due to fees)
    pub const PAYMENT_TOLERANCE_PERCENT: Decimal = dec!(1);

    /// Polling interval for monitoring payments (seconds)
    pub const MONITORING_POLL_INTERVAL_SECS: u64 = 30;

    /// Maximum retries for payment monitoring before marking as failed
    pub const MAX_MONITORING_RETRIES: i32 = 100;

    /// Threshold for high-value orders requiring extra confirmations (BTC)
    pub const HIGH_VALUE_THRESHOLD_BTC: Decimal = dec!(0.1);

    /// Get required confirmations based on amount
    pub fn get_required_confirmations(amount_btc: Decimal) -> i32 {
        if amount_btc >= Self::HIGH_VALUE_THRESHOLD_BTC {
            Self::HIGH_VALUE_REQUIRED_CONFIRMATIONS
        } else {
            Self::DEFAULT_REQUIRED_CONFIRMATIONS
        }
    }

    /// Calculate expiration time from now
    pub fn calculate_expiration() -> DateTime<Utc> {
        Utc::now() + chrono::Duration::hours(Self::DEFAULT_EXPIRATION_HOURS)
    }

    /// Check if payment should be marked as expired
    pub fn should_expire(payment: &PaymentOrder) -> bool {
        payment.is_expired()
            && matches!(
                payment.status,
                PaymentStatus::Pending | PaymentStatus::Detected
            )
    }

    /// Check if payment monitoring should continue
    pub fn should_continue_monitoring(payment: &PaymentOrder, retry_count: i32) -> bool {
        if retry_count >= Self::MAX_MONITORING_RETRIES {
            return false;
        }
        matches!(
            payment.status,
            PaymentStatus::Pending | PaymentStatus::Detected | PaymentStatus::Confirming
        )
    }
}

/// Payment statistics for admin dashboard
#[derive(Debug, Serialize)]
pub struct PaymentStats {
    pub total_orders: i64,
    pub pending_orders: i64,
    pub completed_orders: i64,
    pub expired_orders: i64,
    pub failed_orders: i64,
    pub total_volume_btc: Decimal,
    pub avg_confirmation_time_minutes: Option<f64>,
    pub success_rate_percent: Decimal,
}

/// Payment webhook event (for external integrations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentWebhookEvent {
    pub event_type: PaymentEventType,
    pub payment_order_id: Uuid,
    pub btc_address: String,
    pub amount_btc: Decimal,
    pub confirmations: i32,
    pub txid: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Type of payment event
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentEventType {
    /// Payment order created
    Created,
    /// Payment detected on blockchain
    Detected,
    /// Confirmation count updated
    ConfirmationUpdate,
    /// Payment fully confirmed
    Confirmed,
    /// Order fulfilled
    Completed,
    /// Payment expired
    Expired,
    /// Payment failed
    Failed,
}

impl fmt::Display for PaymentEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentEventType::Created => write!(f, "created"),
            PaymentEventType::Detected => write!(f, "detected"),
            PaymentEventType::ConfirmationUpdate => write!(f, "confirmation_update"),
            PaymentEventType::Confirmed => write!(f, "confirmed"),
            PaymentEventType::Completed => write!(f, "completed"),
            PaymentEventType::Expired => write!(f, "expired"),
            PaymentEventType::Failed => write!(f, "failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_expiration() {
        let payment = PaymentOrder {
            payment_order_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            btc_address: "bc1qtest".to_string(),
            expected_amount_btc: dec!(0.001),
            received_amount_btc: None,
            status: PaymentStatus::Pending,
            btc_txid: None,
            confirmations: 0,
            required_confirmations: 2,
            detected_at: None,
            confirmed_at: None,
            expires_at: Utc::now() - chrono::Duration::hours(1),
            fulfilled_at: None,
            derivation_path: Some("m/84'/0'/0'/0/0".to_string()),
            notes: None,
            created_at: Utc::now() - chrono::Duration::hours(25),
            updated_at: Utc::now(),
        };

        assert!(payment.is_expired());
    }

    #[test]
    fn test_payment_variance() {
        let mut payment = PaymentOrder {
            payment_order_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            btc_address: "bc1qtest".to_string(),
            expected_amount_btc: dec!(0.001),
            received_amount_btc: Some(dec!(0.00095)), // 5% underpayment
            status: PaymentStatus::Detected,
            btc_txid: Some("test_txid".to_string()),
            confirmations: 1,
            required_confirmations: 2,
            detected_at: Some(Utc::now()),
            confirmed_at: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            fulfilled_at: None,
            derivation_path: Some("m/84'/0'/0'/0/0".to_string()),
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let variance = payment.payment_variance_percent().unwrap();
        assert_eq!(variance, dec!(-5));

        // Should not be acceptable with 1% tolerance
        assert!(!payment.is_amount_acceptable(dec!(1)));

        // Should be acceptable with 5% tolerance
        assert!(payment.is_amount_acceptable(dec!(5)));

        // Overpayment should always be acceptable
        payment.received_amount_btc = Some(dec!(0.0011));
        assert!(payment.is_amount_acceptable(dec!(1)));
    }

    #[test]
    fn test_required_confirmations() {
        // Small amount: 2 confirmations
        assert_eq!(
            PaymentMonitorConfig::get_required_confirmations(dec!(0.01)),
            2
        );

        // Large amount: 3 confirmations
        assert_eq!(
            PaymentMonitorConfig::get_required_confirmations(dec!(0.15)),
            3
        );
    }

    #[test]
    fn test_has_enough_confirmations() {
        let payment = PaymentOrder {
            payment_order_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            btc_address: "bc1qtest".to_string(),
            expected_amount_btc: dec!(0.001),
            received_amount_btc: Some(dec!(0.001)),
            status: PaymentStatus::Confirming,
            btc_txid: Some("test_txid".to_string()),
            confirmations: 2,
            required_confirmations: 2,
            detected_at: Some(Utc::now()),
            confirmed_at: None,
            expires_at: Utc::now() + chrono::Duration::hours(1),
            fulfilled_at: None,
            derivation_path: Some("m/84'/0'/0'/0/0".to_string()),
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(payment.has_enough_confirmations());
    }

    #[test]
    fn test_calculate_refund_amount() {
        // Overpaid scenario
        let payment = PaymentOrder {
            payment_order_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            btc_address: "bc1qtest".to_string(),
            expected_amount_btc: dec!(0.001),
            received_amount_btc: Some(dec!(0.0015)),
            status: PaymentStatus::Overpaid,
            btc_txid: Some("test_txid".to_string()),
            confirmations: 2,
            required_confirmations: 2,
            detected_at: Some(Utc::now()),
            confirmed_at: Some(Utc::now()),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            fulfilled_at: None,
            derivation_path: Some("m/84'/0'/0'/0/0".to_string()),
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let refund = payment.calculate_refund_amount();
        assert_eq!(refund, Some(dec!(0.0005)));
    }

    #[test]
    fn test_create_payment_order_validation() {
        let valid_request = CreatePaymentOrderRequest {
            order_id: Uuid::new_v4(),
            expected_amount_btc: dec!(0.001),
            required_confirmations: Some(2),
        };
        assert!(valid_request.validate().is_ok());

        // Too small
        let invalid_request = CreatePaymentOrderRequest {
            order_id: Uuid::new_v4(),
            expected_amount_btc: dec!(0.000001),
            required_confirmations: Some(2),
        };
        assert!(invalid_request.validate().is_err());

        // Invalid confirmations
        let invalid_request = CreatePaymentOrderRequest {
            order_id: Uuid::new_v4(),
            expected_amount_btc: dec!(0.001),
            required_confirmations: Some(10),
        };
        assert!(invalid_request.validate().is_err());
    }
}
